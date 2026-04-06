//! Minimal type checking: numeric series vs simple, builtins, and scope rules.
//!
//! Intentionally small surface: enough to catch obvious mistakes before HIR / codegen.

use std::collections::HashMap;

use crate::frontend::ast::{
    AssignOp, BinOp, ElseBody, ExportDecl, Expr, ExprKind, FnBody, FnDecl, FnParam, IfStmt, Item,
    NodeId, Script, ScriptDeclaration, Span, Stmt, StmtKind, Type as AstType, UnaryOp, VarDecl,
    VarQualifier,
};
use crate::frontend::ast::PrimitiveType;
use crate::hir::HirType;
use crate::semantic::builtin_registry;
use crate::semantic::{AnalyzeError, SemanticDiagnostic};
use crate::session::CompilerSession;

/// Run type checking on a script (after earlier semantic passes).
pub fn typecheck_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut session = CompilerSession::new();
    session.prepare_analysis(script);
    typecheck_script_in_session(&mut session, script)
}

/// Infer types and store them on `session.expr_types` indexed by expression [`NodeId`].
pub fn typecheck_script_in_session(
    session: &mut CompilerSession,
    script: &Script,
) -> Result<(), AnalyzeError> {
    let mut c = Checker::new(script, session);
    c.check_script(script)
}

/// Top-level user function signature for arity / argument checks.
#[derive(Debug, Clone)]
struct FnSig {
    params: Vec<HirType>,
    ret: HirType,
}

struct Checker<'a> {
    /// Import aliases — names exist but have unknown types until library typing exists.
    import_aliases: HashMap<String, HirType>,
    /// `f name(...)` / `name(...) =>` declarations (name → params + return type).
    fn_sigs: HashMap<String, FnSig>,
    scopes: Vec<HashMap<String, HirType>>,
    issues: Vec<SemanticDiagnostic>,
    session: &'a mut CompilerSession,
}

impl<'a> Checker<'a> {
    fn new(script: &Script, session: &'a mut CompilerSession) -> Self {
        let mut import_aliases = HashMap::new();
        let mut fn_sigs = HashMap::new();
        for item in &script.items {
            if let Item::Import(i) = item {
                import_aliases.insert(i.alias.clone(), HirType::Simple(AstType::Primitive(PrimitiveType::String)));
            }
            if let Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) = item {
                fn_sigs.insert(f.name.clone(), fn_sig_from_decl(f));
            }
        }
        Self {
            import_aliases,
            fn_sigs,
            scopes: vec![HashMap::new()],
            issues: Vec::new(),
            session,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.session.push_symbol_scope();
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.session.pop_symbol_scope();
    }

    fn define_at(&mut self, span: Span, name: impl Into<String>, ty: HirType) {
        let name = name.into();
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.clone(), ty.clone());
        }
        self.session.record_symbol_def(span, &name, ty);
    }

    fn resolve_local(&self, name: &str) -> Option<HirType> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.issues.push(SemanticDiagnostic {
            message: msg.into(),
            span,
        });
    }

    fn check_script(&mut self, script: &Script) -> Result<(), AnalyzeError> {
        self.collect_top_level_definitions(script);
        for item in &script.items {
            self.check_item(item);
        }
        self.finish_result()
    }

    fn collect_top_level_definitions(&mut self, script: &Script) {
        for item in &script.items {
            match item {
                Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => {
                    let ty = fn_decl_type(f);
                    self.define_at(f.span, &f.name, ty);
                }
                Item::Enum(e) | Item::Export(ExportDecl::Enum(e)) => {
                    self.define_at(
                        e.span,
                        &e.name,
                        HirType::Simple(AstType::Named(e.name.clone())),
                    );
                }
                Item::TypeDef(t) | Item::Export(ExportDecl::TypeDef(t)) => {
                    self.define_at(
                        t.span,
                        &t.name,
                        HirType::Simple(AstType::Named(t.name.clone())),
                    );
                }
                _ => {}
            }
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Stmt(s) => {
                self.check_stmt(s);
            }
            Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => {
                self.check_fn_decl(f);
            }
            Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
                for (_, e) in args {
                    let _ = self.type_expr(e);
                }
            }
            Item::Enum(e) | Item::Export(ExportDecl::Enum(e)) => {
                for v in &e.variants {
                    let _ = self.type_expr(&v.value);
                }
            }
            Item::TypeDef(t) | Item::Export(ExportDecl::TypeDef(t)) => {
                for field in &t.fields {
                    let _ = self.type_expr(&field.default);
                }
            }
            Item::Export(ExportDecl::Var(v)) => {
                self.check_var_decl(v, v.span);
            }
            Item::Import(_) => {}
        }
    }

    fn check_fn_decl(&mut self, f: &FnDecl) {
        self.push_scope();
        for p in &f.params {
            let ty = param_hir_type(p);
            self.define_at(f.span, &p.name, ty.clone());
            if let Some(d) = &p.default {
                let dt = match self.type_expr(d) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if !assignable(&dt, &ty) {
                    self.err(
                        d.span,
                        format!(
                            "default for parameter `{}` does not match parameter type",
                            p.name
                        ),
                    );
                }
            }
        }
        let inferred_ret = match &f.body {
            FnBody::Expr(e) => match self.type_expr(e) {
                Ok(t) => t,
                Err(()) => default_fn_return_hir(),
            },
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.check_stmt(s);
                }
                infer_return_from_block(self, stmts)
            }
        };
        if let Some(sig) = self.fn_sigs.get_mut(&f.name) {
            sig.ret = inferred_ret;
        }
        self.pop_scope();
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::VarDecl(v) => self.check_var_decl(v, s.span),
            StmtKind::Assign {
                name,
                op,
                value,
            } => match op {
                AssignOp::Eq => {
                    let rhs = match self.type_expr(value) {
                        Ok(t) => t,
                        Err(_) => return,
                    };
                    match self.resolve_local(name) {
                        Some(lhs) => {
                            if !assignable(&rhs, &lhs) {
                                self.err(
                                    s.span,
                                    format!(
                                        "assignment to `{name}`: value type does not match binding"
                                    ),
                                );
                            }
                        }
                        None => {
                            self.define_at(s.span, name, rhs);
                        }
                    }
                }
                AssignOp::ColonEq => {
                    let rhs = match self.type_expr(value) {
                        Ok(t) => t,
                        Err(_) => return,
                    };
                    match self.resolve_local(name) {
                        Some(lhs) => {
                            if !assignable(&rhs, &lhs) {
                                self.err(
                                    s.span,
                                    format!(
                                        "`:=` reassignment to `{name}`: value type does not match binding"
                                    ),
                                );
                            }
                        }
                        None => {
                            self.err(
                                s.span,
                                format!("unknown variable `{name}` for `:=` reassignment"),
                            );
                        }
                    }
                }
                AssignOp::PlusEq
                | AssignOp::MinusEq
                | AssignOp::StarEq
                | AssignOp::SlashEq
                | AssignOp::PercentEq => {
                    let rhs = match self.type_expr(value) {
                        Ok(t) => t,
                        Err(_) => return,
                    };
                    let Some(lhs) = self.resolve_local(name) else {
                        self.err(
                            s.span,
                            format!("unknown variable `{name}` for compound assignment"),
                        );
                        return;
                    };
                    if !is_numeric(&lhs) || !is_numeric(&rhs) {
                        self.err(
                            s.span,
                            "compound assignment requires numeric variable and value",
                        );
                        return;
                    }
                    match binary_numeric_result(&lhs, &rhs) {
                        Ok(out) => {
                            if !assignable(&out, &lhs) {
                                self.err(
                                    s.span,
                                    format!(
                                        "compound assignment to `{name}`: result type is not assignable"
                                    ),
                                );
                            }
                        }
                        Err(m) => self.err(s.span, m),
                    }
                }
            },
            StmtKind::TupleAssign { names, op, value } => {
                let rhs = match self.type_expr(value) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                match op {
                    AssignOp::Eq => {
                        for n in names {
                            match self.resolve_local(n) {
                                Some(lhs) => {
                                    if !assignable(&rhs, &lhs) {
                                        self.err(
                                            s.span,
                                            format!(
                                                "tuple assignment: value type does not match `{n}`"
                                            ),
                                        );
                                    }
                                }
                                None => self.define_at(s.span, n, rhs.clone()),
                            }
                        }
                    }
                    AssignOp::ColonEq => {
                        for n in names {
                            match self.resolve_local(n) {
                                Some(lhs) => {
                                    if !assignable(&rhs, &lhs) {
                                        self.err(
                                            s.span,
                                            format!(
                                                "tuple `:=`: value type does not match `{n}`"
                                            ),
                                        );
                                    }
                                }
                                None => {
                                    self.err(
                                        s.span,
                                        format!("unknown variable `{n}` in tuple `:=`"),
                                    );
                                }
                            }
                        }
                    }
                    AssignOp::PlusEq
                    | AssignOp::MinusEq
                    | AssignOp::StarEq
                    | AssignOp::SlashEq
                    | AssignOp::PercentEq => {
                        self.err(
                            s.span,
                            "compound operators are not supported on tuple assignment",
                        );
                    }
                }
            }
            StmtKind::Expr(e) => {
                let _ = self.type_expr(e);
            }
            StmtKind::Block(stmts) => {
                self.push_scope();
                for x in stmts {
                    self.check_stmt(x);
                }
                self.pop_scope();
            }
            StmtKind::If(i) => self.check_if_stmt(i),
            StmtKind::For {
                var,
                from,
                to,
                by,
                body,
            } => {
                let _ = self.type_expr(from);
                let _ = self.type_expr(to);
                if let Some(b) = by {
                    let _ = self.type_expr(b);
                }
                self.push_scope();
                self.define_at(
                    s.span,
                    var,
                    HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
                );
                for x in body {
                    self.check_stmt(x);
                }
                self.pop_scope();
            }
            StmtKind::ForIn {
                pattern,
                iterable,
                body,
            } => {
                let _ = self.type_expr(iterable);
                self.push_scope();
                match pattern {
                    crate::frontend::ast::ForInPattern::Name(n) => {
                        self.define_at(
                            s.span,
                            n,
                            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
                        );
                    }
                    crate::frontend::ast::ForInPattern::Pair(i, v) => {
                        self.define_at(s.span, i, HirType::Simple(AstType::Primitive(PrimitiveType::Int)));
                        self.define_at(
                            s.span,
                            v,
                            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
                        );
                    }
                }
                for x in body {
                    self.check_stmt(x);
                }
                self.pop_scope();
            }
            StmtKind::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let scrut_ty = scrutinee
                    .as_ref()
                    .and_then(|sc| self.type_expr(sc).ok());
                for (e, st) in cases {
                    if let Ok(arm_ty) = self.type_expr(e) {
                        if let Some(sty) = &scrut_ty {
                            if !type_compatible_eq(&arm_ty, sty) {
                                self.err(
                                    e.span,
                                    "switch arm expression type does not match scrutinee",
                                );
                            }
                        } else if !is_bool_like(&arm_ty) {
                            self.err(
                                e.span,
                                "switch without scrutinee: each arm condition must be boolean or series bool",
                            );
                        }
                    }
                    self.push_scope();
                    self.check_stmt(st);
                    self.pop_scope();
                }
                if let Some(d) = default {
                    self.push_scope();
                    self.check_stmt(d.as_ref());
                    self.pop_scope();
                }
            }
            StmtKind::While { cond, body } => {
                if let Ok(c_ty) = self.type_expr(cond) {
                    if !is_bool_like(&c_ty) {
                        self.err(
                            cond.span,
                            "`while` condition must be boolean or series bool",
                        );
                    }
                }
                self.push_scope();
                for x in body {
                    self.check_stmt(x);
                }
                self.pop_scope();
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn check_if_stmt(&mut self, i: &IfStmt) {
        if let Ok(c_ty) = self.type_expr(&i.cond) {
            if !is_bool_like(&c_ty) {
                self.err(
                    i.cond.span,
                    "`if` condition must be boolean or series bool",
                );
            }
        }
        self.push_scope();
        for s in &i.then_body {
            self.check_stmt(s);
        }
        self.pop_scope();
        if let Some(e) = &i.else_body {
            match e {
                ElseBody::If(nested) => self.check_if_stmt(nested),
                ElseBody::Block(stmts) => {
                    self.push_scope();
                    for s in stmts {
                        self.check_stmt(s);
                    }
                    self.pop_scope();
                }
            }
        }
    }

    fn check_var_decl(&mut self, v: &VarDecl, decl_span: Span) {
        let binding = var_decl_binding_type(v);
        let rhs = match self.type_expr(&v.value) {
            Ok(t) => t,
            Err(_) => {
                self.define_at(decl_span, &v.name, binding);
                return;
            }
        };
        if !assignable(&rhs, &binding) {
            let msg =
                if matches!(binding, HirType::Simple(_)) && is_series_shape(&rhs) {
                    format!(
                        "variable `{}`: simple/const/input binding cannot hold a series value",
                        v.name
                    )
                } else {
                    format!(
                        "variable `{}`: initializer type does not match binding",
                        v.name
                    )
                };
            self.err(decl_span, msg);
        }
        self.define_at(decl_span, &v.name, binding);
    }

    fn type_expr(&mut self, e: &Expr) -> Result<HirType, ()> {
        let t = match &e.kind {
            ExprKind::Int(_) => HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            ExprKind::Float(_) => HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
            ExprKind::String(_) => HirType::Simple(AstType::Primitive(PrimitiveType::String)),
            ExprKind::Bool(_) => HirType::Simple(AstType::Primitive(PrimitiveType::Bool)),
            ExprKind::Na => HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
            ExprKind::Color(_) | ExprKind::HexColor(_) => {
                HirType::Simple(AstType::Primitive(PrimitiveType::Color))
            }
            ExprKind::IdentPath(path) => self.type_ident_path(path, e.span)?,
            ExprKind::Member { base, field: _ } => {
                let _base = self.type_expr(base)?;
                HirType::Series(AstType::Primitive(PrimitiveType::Float))
            }
            ExprKind::Call {
                callee,
                type_args: _,
                args,
            } => self.type_call(callee.as_ref(), args, e.span)?,
            ExprKind::Index { base, index } => {
                let base_res = self.type_expr(base);
                let idx_res = self.type_expr(index);
                let base_failed = base_res.is_err();
                let idx_failed = idx_res.is_err();
                let base_ty = base_res.unwrap_or_else(|_| {
                    HirType::Series(AstType::Primitive(PrimitiveType::Float))
                });
                let idx_ty = idx_res.unwrap_or_else(|_| {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Int))
                });
                if base_failed || idx_failed {
                    return Err(());
                }
                if !is_integral(&idx_ty) {
                    self.err(e.span, "series index must be integral");
                }
                match index_result_type(&base_ty) {
                    Ok(t) => t,
                    Err(()) => {
                        self.err(e.span, "cannot subscript this type");
                        return Err(());
                    }
                }
            }
            ExprKind::Array(elts) => {
                if elts.is_empty() {
                    HirType::Array(Box::new(HirType::Simple(AstType::Primitive(
                        PrimitiveType::Float,
                    ))))
                } else {
                    let mut tys: Vec<HirType> = Vec::with_capacity(elts.len());
                    let mut failed = false;
                    for x in elts {
                        match self.type_expr(x) {
                            Ok(t) => tys.push(t),
                            Err(()) => {
                                failed = true;
                                tys.push(HirType::Series(AstType::Primitive(PrimitiveType::Float)));
                            }
                        }
                    }
                    if failed {
                        return Err(());
                    }
                    let mut first = tys[0].clone();
                    for u in tys.into_iter().skip(1) {
                        first = match binary_meet(&first, &u) {
                            Some(t) => t,
                            None => {
                                self.err(e.span, "array literal elements have incompatible types");
                                return Err(());
                            }
                        };
                    }
                    HirType::Array(Box::new(first))
                }
            }
            ExprKind::Unary { op, expr } => {
                let inner = self.type_expr(expr)?;
                match op {
                    UnaryOp::Pos | UnaryOp::Neg => {
                        if !is_numeric(&inner) {
                            self.err(e.span, "unary +/- expects a numeric operand");
                        }
                        inner
                    }
                    UnaryOp::Not => {
                        if !is_bool_like(&inner) {
                            self.err(e.span, "unary not expects a boolean operand");
                        }
                        if is_series_shape(&inner) {
                            HirType::Series(AstType::Primitive(PrimitiveType::Bool))
                        } else {
                            HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
                        }
                    }
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l_res = self.type_expr(left);
                let r_res = self.type_expr(right);
                let l_failed = l_res.is_err();
                let r_failed = r_res.is_err();
                let l = l_res.unwrap_or_else(|_| {
                    HirType::Series(AstType::Primitive(PrimitiveType::Float))
                });
                let r = r_res.unwrap_or_else(|_| {
                    HirType::Series(AstType::Primitive(PrimitiveType::Float))
                });
                if l_failed || r_failed {
                    return Err(());
                }
                self.type_binary(*op, l, r, e.span)?
            }
            ExprKind::Ternary {
                cond,
                then_b,
                else_b,
            }
            | ExprKind::IfExpr {
                cond,
                then_b,
                else_b,
            } => {
                if let Ok(c_ty) = self.type_expr(cond) {
                    if !is_bool_like(&c_ty) {
                        self.err(
                            cond.span,
                            "conditional expression: condition must be boolean or series bool",
                        );
                    }
                }
                let t_res = self.type_expr(then_b);
                let u_res = self.type_expr(else_b);
                let t_failed = t_res.is_err();
                let u_failed = u_res.is_err();
                let t = t_res.unwrap_or_else(|_| default_fn_return_hir());
                let u = u_res.unwrap_or_else(|_| default_fn_return_hir());
                if t_failed || u_failed {
                    return Err(());
                }
                match binary_meet(&t, &u) {
                    Some(ty) => ty,
                    None => {
                        self.err(e.span, "branches of conditional have incompatible types");
                        return Err(());
                    }
                }
            }
        };
        if e.id != NodeId::UNASSIGNED {
            self.session.set_expr_type(e.id, t.clone());
        }
        Ok(t)
    }

    fn type_ident_path(&mut self, path: &[String], span: Span) -> Result<HirType, ()> {
        if path.len() == 1 {
            let name = &path[0];
            if let Some(t) = self.resolve_local(name) {
                return Ok(t);
            }
            if self.import_aliases.contains_key(name) {
                return Ok(HirType::Simple(AstType::Primitive(PrimitiveType::String)));
            }
            if let Some(t) = builtin_ident(name) {
                return Ok(t);
            }
            self.err(span, format!("unknown identifier `{name}`"));
            return Err(());
        }
        if let Some(t) = builtin_global(path) {
            return Ok(t);
        }
        Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
    }

    /// Type every call argument (so later args still get `expr_types` / errors if an earlier arg fails).
    fn collect_call_arg_types(
        &mut self,
        args: &[(Option<String>, Expr)],
    ) -> Result<Vec<HirType>, ()> {
        let mut v = Vec::with_capacity(args.len());
        let mut failed = false;
        for (_, e) in args {
            match self.type_expr(e) {
                Ok(t) => v.push(t),
                Err(()) => {
                    failed = true;
                    v.push(HirType::Series(AstType::Primitive(PrimitiveType::Float)));
                }
            }
        }
        if failed {
            Err(())
        } else {
            Ok(v)
        }
    }

    fn type_call(
        &mut self,
        callee: &Expr,
        args: &[(Option<String>, Expr)],
        call_span: Span,
    ) -> Result<HirType, ()> {
        let name = dotted_name(callee).unwrap_or_default();
        let cspan = callee.span;
        let arg_tys = self.collect_call_arg_types(args)?;

        if let Some(entry) = builtin_registry::lookup_dotted(name.as_str()) {
            if arg_tys.len() < entry.min_args {
                self.err(
                    cspan,
                    format!(
                        "`{}` expects at least {} arguments",
                        entry.dotted_name,
                        entry.min_args
                    ),
                );
                return Err(());
            }
            if entry.moving_average {
                let src = promote_numeric_series(coerce_simple_to_series(arg_tys[0].clone()));
                if !is_numeric(&src) {
                    self.err(
                        cspan,
                        format!("`{}`: first argument must be numeric", name),
                    );
                    return Err(());
                }
                if arg_tys.len() < 2 || !is_integral(&arg_tys[1]) {
                    self.err(
                        cspan,
                        format!("`{}`: length must be integral", name),
                    );
                    return Err(());
                }
                return Ok(entry.result.to_hir());
            }
            if entry.binary_numeric {
                if arg_tys.len() != 2 {
                    self.err(
                        cspan,
                        format!("`{}` expects exactly two arguments", entry.dotted_name),
                    );
                    return Err(());
                }
                return match binary_numeric_result(&arg_tys[0], &arg_tys[1]) {
                    Ok(t) => Ok(t),
                    Err(m) => {
                        self.err(cspan, m);
                        Err(())
                    }
                };
            }
            if entry.unary_numeric {
                if arg_tys.len() != 1 {
                    self.err(
                        cspan,
                        format!("`{}` expects exactly one argument", entry.dotted_name),
                    );
                    return Err(());
                }
                if !is_numeric(&arg_tys[0]) {
                    self.err(
                        cspan,
                        format!("`{}`: argument must be numeric", entry.dotted_name),
                    );
                    return Err(());
                }
                return Ok(builtin_result_with_series_promotion(entry, &arg_tys));
            }
            if entry.unary_string_to_float {
                if arg_tys.len() != 1 {
                    self.err(
                        cspan,
                        format!("`{}` expects exactly one argument", entry.dotted_name),
                    );
                    return Err(());
                }
                if !is_stringish(&arg_tys[0]) {
                    self.err(
                        cspan,
                        format!(
                            "`{}`: argument must be string or series string",
                            entry.dotted_name
                        ),
                    );
                    return Err(());
                }
                return Ok(entry.result.to_hir());
            }
            if entry.bool_binary {
                if arg_tys.len() < 2 {
                    self.err(
                        cspan,
                        format!("`{}` expects at least two arguments", entry.dotted_name),
                    );
                    return Err(());
                }
                if !is_bool_like(&arg_tys[0]) || !is_bool_like(&arg_tys[1]) {
                    self.err(
                        cspan,
                        format!(
                            "`{}`: first two arguments must be boolean or series bool",
                            entry.dotted_name
                        ),
                    );
                    return Err(());
                }
                let any_s = is_series_shape(&arg_tys[0]) || is_series_shape(&arg_tys[1]);
                return Ok(if any_s {
                    HirType::Series(AstType::Primitive(PrimitiveType::Bool))
                } else {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
                });
            }
            if entry.dotted_name == "ta.macd" {
                for (i, t) in arg_tys.iter().enumerate().take(4) {
                    if !is_numeric(t) {
                        self.err(
                            cspan,
                            format!("`ta.macd`: argument {} must be numeric", i + 1),
                        );
                        return Err(());
                    }
                }
                return Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)));
            }
            for t in &arg_tys {
                if !is_numeric(t) {
                    self.err(
                        cspan,
                        format!(
                            "`{}`: all arguments must be numeric",
                            entry.dotted_name
                        ),
                    );
                    return Err(());
                }
            }
            return Ok(builtin_result_with_series_promotion(entry, &arg_tys));
        }

        if !name.contains('.') && !name.is_empty() {
            if let Some(sig) = self.fn_sigs.get(name.as_str()).cloned() {
                if args.len() != sig.params.len() {
                    self.err(
                        call_span,
                        format!(
                            "`{name}` expects {} arguments, got {}",
                            sig.params.len(),
                            args.len()
                        ),
                    );
                    return Err(());
                }
                for (i, ((_, e), pt)) in args.iter().zip(sig.params.iter()).enumerate() {
                    let t = self.type_expr(e)?;
                    if !assignable(&t, pt) {
                        self.err(
                            e.span,
                            format!("argument {} to `{name}` has incompatible type", i + 1),
                        );
                        return Err(());
                    }
                }
                return Ok(sig.ret);
            }
        }

        match name.as_str() {
            "math.abs" | "math.sqrt" | "math.log" | "math.exp" => {
                if arg_tys.len() != 1 {
                    self.err(cspan, format!("`{name}` expects one argument"));
                    return Err(());
                }
                let a = arg_tys[0].clone();
                if !is_numeric(&a) {
                    self.err(cspan, format!("`{name}` expects a numeric argument"));
                    return Err(());
                }
                Ok(a)
            }
            "math.max" | "math.min" => {
                if arg_tys.len() != 2 {
                    self.err(cspan, format!("`{name}` expects two arguments"));
                    return Err(());
                }
                match binary_numeric_result(&arg_tys[0], &arg_tys[1]) {
                    Ok(t) => Ok(t),
                    Err(m) => {
                        self.err(cspan, m);
                        Err(())
                    }
                }
            }
            "input.int" => {
                if arg_tys.len() != 1 {
                    self.err(cspan, "`input.int` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Int)))
            }
            "input.float" => {
                if arg_tys.len() != 1 {
                    self.err(cspan, "`input.float` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Float)))
            }
            "input.bool" => {
                if arg_tys.len() != 1 {
                    self.err(cspan, "`input.bool` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Bool)))
            }
            "input.string" => {
                if arg_tys.len() != 1 {
                    self.err(cspan, "`input.string` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::String)))
            }
            "nz" => {
                if arg_tys.is_empty() {
                    self.err(cspan, "`nz` expects at least one argument");
                    return Err(());
                }
                Ok(arg_tys[0].clone())
            }
            s if s.starts_with("plot.") || s == "plot" => {
                for (i, t) in arg_tys.iter().enumerate().take(3) {
                    if i == 0 && !is_numeric(t) {
                        self.err(call_span, "`plot`: first argument should be numeric");
                    }
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Float)))
            }
            "request.security" => {
                if args.len() < 3 {
                    self.err(
                        call_span,
                        "`request.security` expects at least three arguments (symbol, timeframe, expression)",
                    );
                    return Err(());
                }
                if !is_stringish(&arg_tys[0]) {
                    self.err(
                        args[0].1.span,
                        "`request.security`: symbol must be `string` or `series string`",
                    );
                }
                if !is_stringish(&arg_tys[1]) {
                    self.err(
                        args[1].1.span,
                        "`request.security`: timeframe must be `string` or `series string`",
                    );
                }
                for (i, (nm, ex)) in args.iter().enumerate().skip(3) {
                    let arg_ty = arg_tys.get(i);
                    match nm.as_deref() {
                        Some("gaps") | Some("lookahead") => {
                            if !is_valid_security_gaps_lookahead_arg(ex) {
                                self.err(
                                    ex.span,
                                    "`request.security`: `gaps` / `lookahead` must be `barmerge.*` or a boolean literal",
                                );
                            }
                        }
                        Some("ignore_invalid_symbol") => {
                            if let Some(t) = arg_ty {
                                if !is_bool_like(t) {
                                    self.err(
                                        ex.span,
                                        "`request.security`: `ignore_invalid_symbol` must be bool or series bool",
                                    );
                                }
                            }
                        }
                        None => {
                            if !is_valid_security_gaps_lookahead_arg(ex) {
                                self.err(
                                    ex.span,
                                    "`request.security`: positional merge args must be `barmerge.*` or a boolean literal",
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Ok(request_security_result_type(&arg_tys[2]))
            }
            "request.financial" => {
                if args.len() < 3 {
                    self.err(
                        call_span,
                        "`request.financial` expects at least three arguments (symbol, financial id, period)",
                    );
                    return Err(());
                }
                if !is_stringish(&arg_tys[0]) {
                    self.err(
                        args[0].1.span,
                        "`request.financial`: symbol must be `string` or `series string`",
                    );
                }
                if !is_stringish(&arg_tys[1]) {
                    self.err(
                        args[1].1.span,
                        "`request.financial`: financial id must be `string` or `series string`",
                    );
                }
                if !is_stringish(&arg_tys[2]) {
                    self.err(
                        args[2].1.span,
                        "`request.financial`: period must be `string` or `series string`",
                    );
                }
                for (i, (nm, ex)) in args.iter().enumerate().skip(3) {
                    if nm.as_deref() == Some("ignore_invalid_symbol") {
                        if let Some(t) = arg_tys.get(i) {
                            if !is_bool_like(t) {
                                self.err(
                                    ex.span,
                                    "`request.financial`: `ignore_invalid_symbol` must be bool or series bool",
                                );
                            }
                        }
                    }
                }
                Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
            }
            _ if name.starts_with("strategy.") => Ok(HirType::Simple(AstType::Primitive(
                PrimitiveType::Float,
            ))),
            _ if !name.is_empty() => {
                for (_, a) in args {
                    let _ = self.type_expr(a);
                }
                Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
            }
            _ => {
                self.err(call_span, "invalid call callee");
                Err(())
            }
        }
    }

    fn type_binary(
        &mut self,
        op: BinOp,
        l: HirType,
        r: HirType,
        span: Span,
    ) -> Result<HirType, ()> {
        use BinOp::*;
        match op {
            Add | Sub | Mul | Div | Mod => match binary_numeric_result(&l, &r) {
                Ok(t) => Ok(t),
                Err(m) => {
                    self.err(span, m);
                    Err(())
                }
            },
            Eq | Ne => {
                if !type_compatible_eq(&l, &r) {
                    self.err(span, "equality operands have incompatible types");
                    return Err(());
                }
                Ok(if is_series_shape(&l) || is_series_shape(&r) {
                    HirType::Series(AstType::Primitive(PrimitiveType::Bool))
                } else {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
                })
            }
            Lt | Le | Gt | Ge => {
                if !is_numeric(&l) || !is_numeric(&r) {
                    self.err(span, "comparison expects numeric operands");
                    return Err(());
                }
                Ok(if is_series_shape(&l) || is_series_shape(&r) {
                    HirType::Series(AstType::Primitive(PrimitiveType::Bool))
                } else {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
                })
            }
            And | Or => {
                if !is_bool_like(&l) || !is_bool_like(&r) {
                    self.err(span, "logical operator expects boolean operands");
                    return Err(());
                }
                Ok(if is_series_shape(&l) || is_series_shape(&r) {
                    HirType::Series(AstType::Primitive(PrimitiveType::Bool))
                } else {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
                })
            }
        }
    }

    fn finish_result(&mut self) -> Result<(), AnalyzeError> {
        if self.issues.is_empty() {
            Ok(())
        } else {
            Err(AnalyzeError::new(std::mem::take(&mut self.issues)))
        }
    }
}

fn default_fn_return_hir() -> HirType {
    HirType::Series(AstType::Primitive(PrimitiveType::Float))
}

fn infer_return_from_block(checker: &mut Checker, stmts: &[Stmt]) -> HirType {
    for s in stmts.iter().rev() {
        if let StmtKind::Expr(e) = &s.kind {
            let i = e.id.0 as usize;
            if let Some(t) = checker.session.expr_types.get(i).and_then(|x| x.as_ref()) {
                return t.clone();
            }
            return checker
                .type_expr(e)
                .unwrap_or_else(|_| default_fn_return_hir());
        }
    }
    default_fn_return_hir()
}

fn fn_decl_type(_f: &FnDecl) -> HirType {
    HirType::Simple(AstType::Primitive(PrimitiveType::Float))
}

fn fn_sig_from_decl(f: &FnDecl) -> FnSig {
    FnSig {
        params: f.params.iter().map(param_hir_type).collect(),
        ret: default_fn_return_hir(),
    }
}

fn builtin_result_with_series_promotion(
    entry: &builtin_registry::BuiltinEntry,
    arg_tys: &[HirType],
) -> HirType {
    let mut t = entry.result.to_hir();
    if entry.series_from_args && arg_tys.iter().any(is_series_shape) {
        if let HirType::Simple(AstType::Primitive(p)) = t {
            t = HirType::Series(AstType::Primitive(p));
        }
    }
    t
}

fn is_stringish(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::String))
            | HirType::Series(AstType::Primitive(PrimitiveType::String))
    )
}

/// Pine `request.security`: result is a series whose element type follows the expression argument.
fn request_security_result_type(expr_ty: &HirType) -> HirType {
    match expr_ty {
        HirType::Simple(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(*p)),
        HirType::Series(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(*p)),
        HirType::Array(_) | HirType::Matrix(_) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        _ => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
    }
}

fn param_hir_type(p: &FnParam) -> HirType {
    match &p.ty {
        Some(AstType::Primitive(pr)) => HirType::Series(AstType::Primitive(*pr)),
        Some(_) => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        None => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
    }
}

fn var_decl_binding_type(v: &VarDecl) -> HirType {
    fn elem_hir_for_container(inner: &AstType, q: Option<VarQualifier>) -> HirType {
        let ast = match inner {
            AstType::Primitive(p) => AstType::Primitive(*p),
            _ => AstType::Primitive(PrimitiveType::Float),
        };
        match q {
            Some(VarQualifier::Simple) | Some(VarQualifier::Const) | Some(VarQualifier::Input) => {
                HirType::Simple(ast)
            }
            Some(VarQualifier::Series) | Some(VarQualifier::Var) | Some(VarQualifier::Varip) | None => {
                HirType::Series(ast)
            }
        }
    }

    match &v.ty {
        Some(AstType::Array(elem)) => {
            HirType::Array(Box::new(elem_hir_for_container(elem.as_ref(), v.qualifier)))
        }
        Some(AstType::Matrix(elem)) => {
            HirType::Matrix(Box::new(elem_hir_for_container(elem.as_ref(), v.qualifier)))
        }
        Some(AstType::Primitive(prim)) => {
            let ast = AstType::Primitive(*prim);
            match v.qualifier {
                Some(VarQualifier::Simple) | Some(VarQualifier::Const) | Some(VarQualifier::Input) => {
                    HirType::Simple(ast)
                }
                Some(VarQualifier::Series)
                | Some(VarQualifier::Var)
                | Some(VarQualifier::Varip)
                | None => HirType::Series(ast),
            }
        }
        _ => {
            let ast = AstType::Primitive(PrimitiveType::Float);
            match v.qualifier {
                Some(VarQualifier::Simple) | Some(VarQualifier::Const) | Some(VarQualifier::Input) => {
                    HirType::Simple(ast)
                }
                Some(VarQualifier::Series)
                | Some(VarQualifier::Var)
                | Some(VarQualifier::Varip)
                | None => HirType::Series(ast),
            }
        }
    }
}

fn dotted_member_path(ex: &Expr) -> Option<Vec<String>> {
    match &ex.kind {
        ExprKind::IdentPath(p) => Some(p.clone()),
        ExprKind::Member { base, field } => {
            let mut p = dotted_member_path(base.as_ref())?;
            p.push(field.clone());
            Some(p)
        }
        _ => None,
    }
}

/// `request.security` optional `gaps` / `lookahead` (and legacy positional merge args): Pine-style
/// `barmerge.*` or boolean literal in the minimal typechecker.
fn is_valid_security_gaps_lookahead_arg(ex: &Expr) -> bool {
    if let Some(p) = dotted_member_path(ex) {
        if p.len() == 2 && p[0] == "barmerge" {
            return matches!(
                p[1].as_str(),
                "gaps_on" | "gaps_off" | "lookahead_on" | "lookahead_off"
            );
        }
    }
    matches!(ex.kind, ExprKind::Bool(_))
}

fn builtin_ident(name: &str) -> Option<HirType> {
    match name {
        "close" | "open" | "high" | "low" | "hl2" | "hlc3" | "ohlc4" | "hlcc4" => {
            Some(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
        }
        "volume" => Some(HirType::Series(AstType::Primitive(PrimitiveType::Float))),
        "bar_index" | "time" | "timenow" => {
            Some(HirType::Series(AstType::Primitive(PrimitiveType::Int)))
        }
        "true" | "false" => Some(HirType::Simple(AstType::Primitive(PrimitiveType::Bool))),
        _ => None,
    }
}

fn builtin_global(path: &[String]) -> Option<HirType> {
    match path {
        [a, b] if a == "syminfo" && (b == "ticker" || b == "prefix") => {
            Some(HirType::Series(AstType::Primitive(PrimitiveType::String)))
        }
        _ => None,
    }
}

fn dotted_name(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::IdentPath(p) => Some(p.join(".")),
        ExprKind::Member { base, field } => {
            dotted_name(base).map(|s| format!("{s}.{field}"))
        }
        _ => None,
    }
}

fn is_numeric(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float))
            | HirType::Series(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float))
    )
}

fn is_integral(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Int))
            | HirType::Series(AstType::Primitive(PrimitiveType::Int))
    )
}

fn is_bool_like(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
            | HirType::Series(AstType::Primitive(PrimitiveType::Bool))
    )
}

fn is_series_shape(t: &HirType) -> bool {
    matches!(t, HirType::Series(_))
}

fn coerce_simple_to_series(t: HirType) -> HirType {
    match t {
        HirType::Simple(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(p)),
        o => o,
    }
}

fn promote_numeric_series(t: HirType) -> HirType {
    match t {
        HirType::Simple(AstType::Primitive(PrimitiveType::Int)) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        HirType::Series(AstType::Primitive(PrimitiveType::Int)) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        o => o,
    }
}

fn numeric_prim(t: &HirType) -> Option<PrimitiveType> {
    match t {
        HirType::Simple(AstType::Primitive(p)) | HirType::Series(AstType::Primitive(p)) => {
            Some(*p)
        }
        _ => None,
    }
}

fn binary_numeric_result(l: &HirType, r: &HirType) -> Result<HirType, String> {
    if !is_numeric(l) || !is_numeric(r) {
        return Err("numeric operator expects numeric operands".into());
    }
    let series = is_series_shape(l) || is_series_shape(r);
    let pl = numeric_prim(l).unwrap();
    let pr = numeric_prim(r).unwrap();
    let prim = match (pl, pr) {
        (PrimitiveType::Float, _) | (_, PrimitiveType::Float) => PrimitiveType::Float,
        _ => PrimitiveType::Int,
    };
    Ok(if series {
        HirType::Series(AstType::Primitive(prim))
    } else {
        HirType::Simple(AstType::Primitive(prim))
    })
}

fn binary_meet(a: &HirType, b: &HirType) -> Option<HirType> {
    if assignable(a, b) {
        return Some(b.clone());
    }
    if assignable(b, a) {
        return Some(a.clone());
    }
    if let (HirType::Array(ea), HirType::Array(eb)) = (a, b) {
        return binary_meet(ea, eb).map(|e| HirType::Array(Box::new(e)));
    }
    if let (HirType::Matrix(ea), HirType::Matrix(eb)) = (a, b) {
        return binary_meet(ea, eb).map(|e| HirType::Matrix(Box::new(e)));
    }
    binary_numeric_result(a, b).ok()
}

fn index_result_type(base: &HirType) -> Result<HirType, ()> {
    match base {
        HirType::Series(a) => Ok(HirType::Series(a.clone())),
        HirType::Simple(AstType::Primitive(p)) => Ok(HirType::Simple(AstType::Primitive(*p))),
        HirType::Array(elem) | HirType::Matrix(elem) => Ok((**elem).clone()),
        _ => Err(()),
    }
}

// Equality operand compatibility; see spec `spec/hir.md` ("Typing notes: equality and `na`").
fn type_compatible_eq(a: &HirType, b: &HirType) -> bool {
    assignable(a, b)
        || assignable(b, a)
        || (is_numeric(a) && is_numeric(b))
}

fn assignable(from: &HirType, to: &HirType) -> bool {
    if from == to {
        return true;
    }
    match (from, to) {
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (
            HirType::Series(AstType::Primitive(PrimitiveType::Int)),
            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            HirType::Series(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float)),
        ) => true,
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (HirType::Array(f), HirType::Array(t)) => assignable(f, t),
        (HirType::Matrix(f), HirType::Matrix(t)) => assignable(f, t),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{typecheck_script, typecheck_script_in_session};
    use crate::frontend::ast::{
        ExprKind, Item, NodeId, PrimitiveType, Script, StmtKind, Type as AstType,
    };
    use crate::hir::HirType;
    use crate::parse_script;
    use crate::session::CompilerSession;
    use crate::Compiler;

    fn float_literal_id_in_first_binary_assign_rhs(script: &Script) -> NodeId {
        for item in &script.items {
            if let Item::Stmt(st) = item {
                if let StmtKind::Assign { value, .. } = &st.kind {
                    if let ExprKind::Binary { right, .. } = &value.kind {
                        if matches!(right.kind, ExprKind::Float(_)) {
                            return right.id;
                        }
                    }
                }
            }
        }
        panic!("expected script shape: x = <bad> + <float literal>");
    }

    #[test]
    fn typecheck_ok_indicator_arithmetic() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\nfloat y = close + 1.0\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn typecheck_rejects_bad_initializer() {
        let s = parse_script("t", "indicator(\"x\")\nfloat y = \"no\"\n").unwrap();
        let e = typecheck_script(&s).unwrap_err();
        assert!(e.message().contains("initializer"), "{}", e.message());
    }

    #[test]
    fn first_assignment_declares_name() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\na = 1\nb = a + 1\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn request_security_requires_three_args() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\ny = request.security(\"SYM\", \"D\", close)\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
        let bad = parse_script(
            "t",
            "indicator(\"x\")\ny = request.security(\"SYM\", \"D\")\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(e.message().contains("request.security"), "{}", e.message());
        assert!(e.message().contains("three"), "{}", e.message());
    }

    #[test]
    fn request_financial_requires_three_args() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\ny = request.financial(\"SYM\", \"TOTAL_REVENUE\", \"FY\")\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
        let bad = parse_script(
            "t",
            "indicator(\"x\")\ny = request.financial(\"SYM\", \"TOTAL_REVENUE\")\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(e.message().contains("request.financial"), "{}", e.message());
    }

    #[test]
    fn array_literal_promotes_series_and_simple() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\na = [close, 1.0]\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn array_subscript_yields_element_type() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\na = [1.0, 2.0]\nb = a[0]\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn math_pow_promotes_series() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\ny = math.pow(close, 2.0)\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn request_security_series_int_from_bar_index() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\ny = request.security(\"X\", \"D\", bar_index)\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn user_function_call_checked_for_arity() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\nf(double x) => x * 2.0\nz = f(1.0)\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
        let bad = parse_script(
            "t",
            "indicator(\"x\")\nf(double x) => x * 2.0\nz = f()\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(e.message().contains('f'), "{}", e.message());
    }

    #[test]
    fn compound_assignment_numeric_ok() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\na = 1.0\na += 2.0\n",
        )
        .unwrap();
        typecheck_script(&s).unwrap();
    }

    #[test]
    fn conditional_expr_requires_bool_condition() {
        let ok = parse_script(
            "t",
            "indicator(\"x\")\nx = true ? 1.0 : 2.0\n",
        )
        .unwrap();
        typecheck_script(&ok).unwrap();
        let bad = parse_script(
            "t",
            "indicator(\"x\")\nx = 1.0 ? 2.0 : 3.0\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(
            e.message().contains("boolean") || e.message().contains("bool"),
            "{}",
            e.message()
        );
    }

    #[test]
    fn if_statement_requires_bool_condition() {
        let bad = parse_script(
            "t",
            "indicator(\"x\")\nif 1 {\n  x = 2\n}\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(
            e.message().contains("if`") || e.message().contains("if"),
            "{}",
            e.message()
        );
    }

    #[test]
    fn simple_binding_rejects_series_initializer() {
        let bad = parse_script(
            "t",
            "indicator(\"x\")\nsimple float x = close\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(
            e.message().contains("simple") || e.message().contains("series"),
            "{}",
            e.message()
        );
    }

    #[test]
    fn request_security_rejects_invalid_gaps_arg() {
        let bad = parse_script(
            "t",
            "indicator(\"x\")\ny = request.security(\"A\", \"D\", close, 123)\n",
        )
        .unwrap();
        let e = typecheck_script(&bad).unwrap_err();
        assert!(
            e.message().contains("gaps") || e.message().contains("barmerge"),
            "{}",
            e.message()
        );
    }

    #[test]
    fn compiler_records_expr_types_for_nodes() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\nfloat y = close + 1.0\n",
        )
        .unwrap();
        let mut c = Compiler::new();
        c.run_semantic_passes(&s).unwrap();
        let float_series = HirType::Series(AstType::Primitive(PrimitiveType::Float));
        assert!(
            c.session
                .expr_types
                .iter()
                .any(|t| t.as_ref() == Some(&float_series)),
            "expected at least one Series(float) in {:?}",
            c.session.expr_types
        );
    }

    /// When the left operand of `+` fails, the right-hand literal is still typed (IDE / partial maps).
    #[test]
    fn binary_rhs_expr_type_recorded_when_lhs_fails() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\nx = no_such_identifier + 2.5\n",
        )
        .unwrap();
        let rhs_id = float_literal_id_in_first_binary_assign_rhs(&s);
        let mut session = CompilerSession::new();
        session.prepare_analysis(&s);
        assert!(typecheck_script_in_session(&mut session, &s).is_err());
        let i = rhs_id.0 as usize;
        assert_eq!(
            session.expr_types.get(i).and_then(|t| t.as_ref()),
            Some(&HirType::Simple(AstType::Primitive(PrimitiveType::Float))),
        );
    }
}
