//! Minimal type checking: numeric series vs simple, builtins, and scope rules.
//!
//! Intentionally small surface: enough to catch obvious mistakes before HIR / codegen.

use std::collections::HashMap;

use crate::frontend::ast::{
    BinOp, ElseBody, Expr, ExprKind, FnBody, FnDecl, FnParam, IfStmt, Item, Script, ScriptDeclaration,
    Stmt, StmtKind, Type as AstType, UnaryOp, VarDecl, VarQualifier,
};
use crate::frontend::ast::PrimitiveType;
use crate::hir::HirType;
use crate::semantic::AnalyzeError;

/// Run type checking on a script (after earlier semantic passes).
pub fn typecheck_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut c = Checker::new(script);
    c.check_script(script)
}

struct Checker {
    /// Import aliases — names exist but have unknown types until library typing exists.
    import_aliases: HashMap<String, HirType>,
    scopes: Vec<HashMap<String, HirType>>,
    issues: Vec<String>,
}

impl Checker {
    fn new(script: &Script) -> Self {
        let mut import_aliases = HashMap::new();
        for item in &script.items {
            if let Item::Import(i) = item {
                import_aliases.insert(i.alias.clone(), HirType::Simple(AstType::Primitive(PrimitiveType::String)));
            }
        }
        Self {
            import_aliases,
            scopes: vec![HashMap::new()],
            issues: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: impl Into<String>, ty: HirType) {
        let name = name.into();
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn resolve_local(&self, name: &str) -> Option<HirType> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn err(&mut self, msg: impl Into<String>) {
        self.issues.push(msg.into());
    }

    fn check_script(&mut self, script: &Script) -> Result<(), AnalyzeError> {
        self.collect_top_level_functions(script);
        for item in &script.items {
            self.check_item(item);
        }
        self.finish_result()
    }

    fn collect_top_level_functions(&mut self, script: &Script) {
        for item in &script.items {
            match item {
                Item::FnDecl(f) | Item::Export(crate::frontend::ast::ExportDecl::Fn(f)) => {
                    let ty = fn_decl_type(f);
                    self.define(&f.name, ty);
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
            Item::FnDecl(f) | Item::Export(crate::frontend::ast::ExportDecl::Fn(f)) => {
                self.check_fn_decl(f);
            }
            Item::ScriptDecl(ScriptDeclaration { args, .. }) => {
                for (_, e) in args {
                    let _ = self.type_expr(e);
                }
            }
            Item::Enum(e) | Item::Export(crate::frontend::ast::ExportDecl::Enum(e)) => {
                for v in &e.variants {
                    let _ = self.type_expr(&v.value);
                }
            }
            Item::TypeDef(t) | Item::Export(crate::frontend::ast::ExportDecl::TypeDef(t)) => {
                for field in &t.fields {
                    let _ = self.type_expr(&field.default);
                }
            }
            Item::Export(crate::frontend::ast::ExportDecl::Var(v)) => {
                self.check_var_decl(v);
            }
            Item::Import(_) => {}
        }
    }

    fn check_fn_decl(&mut self, f: &FnDecl) {
        self.push_scope();
        for p in &f.params {
            let ty = param_hir_type(p);
            self.define(&p.name, ty.clone());
            if let Some(d) = &p.default {
                let dt = match self.type_expr(d) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if !assignable(&dt, &ty) {
                    self.err(format!(
                        "default for parameter `{}` does not match parameter type",
                        p.name
                    ));
                }
            }
        }
        match &f.body {
            FnBody::Expr(e) => {
                let _ = self.type_expr(e);
            }
            FnBody::Block(stmts) => {
                for s in stmts {
                    self.check_stmt(s);
                }
            }
        }
        self.pop_scope();
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::VarDecl(v) => self.check_var_decl(v),
            StmtKind::Assign { name, value, .. } => {
                let rhs = match self.type_expr(value) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                match self.resolve_local(name) {
                    Some(lhs) => {
                        if !assignable(&rhs, &lhs) {
                            self.err(format!(
                                "assignment to `{name}`: value type does not match binding"
                            ));
                        }
                    }
                    None => {
                        // Pine-style first assignment declares the name in this scope.
                        self.define(name, rhs);
                    }
                }
            }
            StmtKind::TupleAssign { names, value, .. } => {
                let rhs = match self.type_expr(value) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                for n in names {
                    match self.resolve_local(n) {
                        Some(lhs) => {
                            if !assignable(&rhs, &lhs) {
                                self.err(format!(
                                    "tuple assignment: value type does not match `{n}`"
                                ));
                            }
                        }
                        None => self.define(n, rhs.clone()),
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
                self.define(var, HirType::Simple(AstType::Primitive(PrimitiveType::Int)));
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
                        self.define(n, HirType::Series(AstType::Primitive(PrimitiveType::Float)));
                    }
                    crate::frontend::ast::ForInPattern::Pair(i, v) => {
                        self.define(i, HirType::Simple(AstType::Primitive(PrimitiveType::Int)));
                        self.define(v, HirType::Series(AstType::Primitive(PrimitiveType::Float)));
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
                if let Some(sc) = scrutinee {
                    let _ = self.type_expr(sc);
                }
                for (e, st) in cases {
                    let _ = self.type_expr(e);
                    self.check_stmt(st);
                }
                if let Some(d) = default {
                    self.check_stmt(d.as_ref());
                }
            }
            StmtKind::While { cond, body } => {
                let _ = self.type_expr(cond);
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
        let _ = self.type_expr(&i.cond);
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

    fn check_var_decl(&mut self, v: &VarDecl) {
        let binding = var_decl_binding_type(v);
        let rhs = match self.type_expr(&v.value) {
            Ok(t) => t,
            Err(_) => {
                self.define(&v.name, binding);
                return;
            }
        };
        if !assignable(&rhs, &binding) {
            self.err(format!(
                "variable `{}`: initializer type does not match binding",
                v.name
            ));
        }
        self.define(&v.name, binding);
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
            ExprKind::IdentPath(path) => self.type_ident_path(path)?,
            ExprKind::Member { base, field: _ } => {
                let _base = self.type_expr(base)?;
                HirType::Series(AstType::Primitive(PrimitiveType::Float))
            }
            ExprKind::Call {
                callee,
                type_args: _,
                args,
            } => self.type_call(callee.as_ref(), args)?,
            ExprKind::Index { base, index } => {
                let base_ty = self.type_expr(base)?;
                let idx_ty = self.type_expr(index)?;
                if !is_integral(&idx_ty) {
                    self.err("series index must be integral");
                }
                index_result_type(&base_ty)?
            }
            ExprKind::Array(elts) => {
                if elts.is_empty() {
                    HirType::Simple(AstType::Primitive(PrimitiveType::Float))
                } else {
                    let mut first = self.type_expr(&elts[0])?;
                    for x in &elts[1..] {
                        let u = self.type_expr(x)?;
                        first = binary_meet(&first, &u).unwrap_or(first);
                    }
                    first
                }
            }
            ExprKind::Unary { op, expr } => {
                let inner = self.type_expr(expr)?;
                match op {
                    UnaryOp::Pos | UnaryOp::Neg => {
                        if !is_numeric(&inner) {
                            self.err("unary +/- expects a numeric operand");
                        }
                        inner
                    }
                    UnaryOp::Not => {
                        if !is_bool_like(&inner) {
                            self.err("unary not expects a boolean operand");
                        }
                        match inner {
                            HirType::Series(_) => HirType::Series(AstType::Primitive(PrimitiveType::Bool)),
                            HirType::Simple(_) => HirType::Simple(AstType::Primitive(PrimitiveType::Bool)),
                        }
                    }
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l = self.type_expr(left)?;
                let r = self.type_expr(right)?;
                self.type_binary(*op, l, r)?
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
                let _c = self.type_expr(cond)?;
                let t = self.type_expr(then_b)?;
                let u = self.type_expr(else_b)?;
                match binary_meet(&t, &u) {
                    Some(ty) => ty,
                    None => {
                        self.err("branches of conditional have incompatible types");
                        return Err(());
                    }
                }
            }
        };
        Ok(t)
    }

    fn type_ident_path(&mut self, path: &[String]) -> Result<HirType, ()> {
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
            self.err(format!("unknown identifier `{name}`"));
            return Err(());
        }
        if let Some(t) = builtin_global(path) {
            return Ok(t);
        }
        Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
    }

    fn type_call(
        &mut self,
        callee: &Expr,
        args: &[(Option<String>, Expr)],
    ) -> Result<HirType, ()> {
        let name = dotted_name(callee).unwrap_or_default();
        let mut arg_tys = Vec::with_capacity(args.len());
        for (_, e) in args {
            arg_tys.push(self.type_expr(e)?);
        }

        match name.as_str() {
            "ta.sma" | "ta.ema" | "ta.wma" | "ta.rma" => {
                if arg_tys.len() < 2 {
                    self.err(format!("`{name}` expects at least two arguments"));
                    return Err(());
                }
                let src = promote_numeric_series(coerce_simple_to_series(arg_tys[0].clone()));
                if !is_numeric(&src) {
                    self.err(format!("`{name}`: first argument must be numeric"));
                    return Err(());
                }
                if !is_integral(&arg_tys[1]) {
                    self.err(format!("`{name}`: length must be integral"));
                    return Err(());
                }
                Ok(HirType::Series(AstType::Primitive(PrimitiveType::Float)))
            }
            "math.abs" | "math.sqrt" | "math.log" | "math.exp" => {
                if arg_tys.len() != 1 {
                    self.err(format!("`{name}` expects one argument"));
                    return Err(());
                }
                let a = arg_tys[0].clone();
                if !is_numeric(&a) {
                    self.err(format!("`{name}` expects a numeric argument"));
                    return Err(());
                }
                Ok(a)
            }
            "math.max" | "math.min" => {
                if arg_tys.len() != 2 {
                    self.err(format!("`{name}` expects two arguments"));
                    return Err(());
                }
                match binary_numeric_result(&arg_tys[0], &arg_tys[1]) {
                    Ok(t) => Ok(t),
                    Err(m) => {
                        self.err(m);
                        Err(())
                    }
                }
            }
            "input.int" => {
                if arg_tys.len() != 1 {
                    self.err("`input.int` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Int)))
            }
            "input.float" => {
                if arg_tys.len() != 1 {
                    self.err("`input.float` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Float)))
            }
            "input.bool" => {
                if arg_tys.len() != 1 {
                    self.err("`input.bool` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Bool)))
            }
            "input.string" => {
                if arg_tys.len() != 1 {
                    self.err("`input.string` expects one default argument");
                    return Err(());
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::String)))
            }
            "nz" => {
                if arg_tys.is_empty() {
                    self.err("`nz` expects at least one argument");
                    return Err(());
                }
                Ok(arg_tys[0].clone())
            }
            s if s.starts_with("plot.") || s == "plot" => {
                for (i, t) in arg_tys.iter().enumerate().take(3) {
                    if i == 0 && !is_numeric(t) {
                        self.err("`plot`: first argument should be numeric");
                    }
                }
                Ok(HirType::Simple(AstType::Primitive(PrimitiveType::Float)))
            }
            "request.security" => {
                for a in &arg_tys {
                    let _ = a;
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
                self.err("invalid call callee");
                Err(())
            }
        }
    }

    fn type_binary(&mut self, op: BinOp, l: HirType, r: HirType) -> Result<HirType, ()> {
        use BinOp::*;
        match op {
            Add | Sub | Mul | Div | Mod => match binary_numeric_result(&l, &r) {
                Ok(t) => Ok(t),
                Err(m) => {
                    self.err(m);
                    Err(())
                }
            },
            Eq | Ne => {
                if !type_compatible_eq(&l, &r) {
                    self.err("equality operands have incompatible types");
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
                    self.err("comparison expects numeric operands");
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
                    self.err("logical operator expects boolean operands");
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
            Err(AnalyzeError {
                message: std::mem::take(&mut self.issues).join("\n"),
            })
        }
    }
}

fn fn_decl_type(_f: &FnDecl) -> HirType {
    HirType::Simple(AstType::Primitive(PrimitiveType::Float))
}

fn param_hir_type(p: &FnParam) -> HirType {
    match &p.ty {
        Some(AstType::Primitive(pr)) => HirType::Series(AstType::Primitive(*pr)),
        Some(_) => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        None => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
    }
}

fn var_decl_binding_type(v: &VarDecl) -> HirType {
    let prim = v
        .ty
        .as_ref()
        .and_then(|t| match t {
            AstType::Primitive(p) => Some(*p),
            _ => None,
        })
        .unwrap_or(PrimitiveType::Float);
    let ast = AstType::Primitive(prim);
    match v.qualifier {
        Some(VarQualifier::Simple) | Some(VarQualifier::Const) | Some(VarQualifier::Input) => {
            HirType::Simple(ast)
        }
        Some(VarQualifier::Series) | Some(VarQualifier::Var) | Some(VarQualifier::Varip) | None => {
            HirType::Series(ast)
        }
    }
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
    binary_numeric_result(a, b).ok()
}

fn index_result_type(base: &HirType) -> Result<HirType, ()> {
    match base {
        HirType::Series(a) => Ok(HirType::Series(a.clone())),
        HirType::Simple(AstType::Primitive(p)) => Ok(HirType::Simple(AstType::Primitive(*p))),
        _ => Err(()),
    }
}

fn type_compatible_eq(a: &HirType, b: &HirType) -> bool {
    assignable(a, b) || assignable(b, a)
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
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::typecheck_script;
    use crate::parse_script;

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
        assert!(e.message.contains("initializer"), "{}", e.message);
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
}
