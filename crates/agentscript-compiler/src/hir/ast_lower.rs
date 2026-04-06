//! AST → HIR lowering for a **small** supported subset (indicator body: inputs, `ta.sma` /
//! `ta.ema`, `request.security`, `plot`).
//!
//! Run [`check_script`](crate::semantic::check_script) before lowering so resolution / early rules
//! have already run.

use std::collections::{HashMap, HashSet};

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::bindings::NameBinding;
use crate::frontend::ast::{
    AssignOp, BinOp, ElseBody, ExportDecl, Expr, ExprKind, FnBody, FnDecl, IfStmt, Item, NodeId,
    PrimitiveType, Script, ScriptDeclaration, ScriptKind, Span, Stmt, StmtKind, Type, VarQualifier,
};
use crate::session::CompilerSession;

use super::builtin::BuiltinKind;
use super::expr::HirExpr;
use super::ids::HirId;
use super::literal::HirLiteral;
use super::lowering::LowerToHir;
use super::script::{HirDeclaration, HirInputDecl, HirScript, HirUserFunction};
use super::security::{GapMode, Lookahead, SecurityCall};
use super::stmt::HirStmt;
use super::symbols::SymbolTable;
use super::ty::HirType;

fn script_declaration_span(decl: &ScriptDeclaration) -> Span {
    decl.args
        .first()
        .map(|(_, ex)| ex.span)
        .unwrap_or(Span::DUMMY)
}

/// Lowering failed: construct not in the supported subset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("HIR lowering: {message}")]
pub struct HirLowerError {
    pub message: String,
    pub span: Span,
}

impl HirLowerError {
    fn at(span: Span, msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            span,
        }
    }
}

/// Stateless [`LowerToHir`] implementation (tiny-subset driver).
#[derive(Debug, Default, Clone, Copy)]
pub struct AstHirLowerer;

impl LowerToHir for AstHirLowerer {
    type Err = HirLowerError;

    fn lower(&mut self, script: &Script) -> Result<HirScript, Self::Err> {
        lower_script_to_hir(script)
    }
}

/// Parse-time script → HIR for the supported subset (uses a fresh [`Bump`] for the expr arena).
pub fn lower_script_to_hir(script: &Script) -> Result<HirScript, HirLowerError> {
    let arena = Bump::new();
    lower_script_to_hir_in_bump(&arena, script)
}

/// Lower using the given arena (e.g. [`crate::session::CompilerSession::arena`] for session-scoped allocation).
pub fn lower_script_to_hir_in_bump(
    bump: &Bump,
    script: &Script,
) -> Result<HirScript, HirLowerError> {
    lower_script_to_hir_in_bump_with_session(bump, script, None)
}

/// Lower with optional [`CompilerSession`] from the semantic pipeline (lexical + typecheck).
/// When `session` is `Some`, lowering aligns [`crate::bindings::SemanticSymbolId`] with HIR
/// [`super::ids::SymbolId`] via [`CompilerSession::def_semantic_ids`] and uses `expr_types` /
/// `name_bindings` for variable metadata.
pub fn lower_script_to_hir_in_bump_with_session(
    bump: &Bump,
    script: &Script,
    session: Option<&CompilerSession>,
) -> Result<HirScript, HirLowerError> {
    let mut lower = LowerCtx::new(bump, session);
    lower.lower_script(script)
}

struct LowerCtx<'a, 'sess> {
    bump: &'a Bump,
    session: Option<&'sess CompilerSession>,
    /// Index into [`CompilerSession::def_semantic_ids`] (hoisted defs consumed in pass 1, then walk order).
    def_idx: usize,
    sid_to_hir: HashMap<crate::bindings::SemanticSymbolId, super::ids::SymbolId>,
    /// Locals / block-scoped names when lowering with a session (Pine-style shadowing).
    scope_stack: Vec<HashMap<String, super::ids::SymbolId>>,
    exprs: BumpVec<'a, HirExpr>,
    expr_spans: BumpVec<'a, Span>,
    symbols: SymbolTable,
    names: HashMap<String, super::ids::SymbolId>,
    /// Names introduced by `input.int` / `input int` (typed as simple int in HIR).
    input_int_names: HashSet<String>,
    /// User `f(...) =>` / Pine expr-body functions: name → arity (parameters).
    user_fn_arity: HashMap<String, usize>,
}

impl<'a, 'sess> LowerCtx<'a, 'sess> {
    fn new(bump: &'a Bump, session: Option<&'sess CompilerSession>) -> Self {
        let scope_stack = if session.is_some() {
            vec![HashMap::new()]
        } else {
            Vec::new()
        };
        Self {
            bump,
            session,
            def_idx: 0,
            sid_to_hir: HashMap::new(),
            scope_stack,
            exprs: BumpVec::new_in(bump),
            expr_spans: BumpVec::new_in(bump),
            symbols: SymbolTable::new(),
            names: HashMap::new(),
            input_int_names: HashSet::new(),
            user_fn_arity: HashMap::new(),
        }
    }

    fn next_def_sid(&mut self, span: Span) -> Result<Option<crate::bindings::SemanticSymbolId>, HirLowerError> {
        let Some(sess) = self.session else {
            return Ok(None);
        };
        let sid = sess
            .def_semantic_ids
            .get(self.def_idx)
            .copied()
            .ok_or_else(|| {
                HirLowerError::at(
                    span,
                    "internal: def semantic id queue underflow (lexical vs HIR mismatch)",
                )
            })?;
        self.def_idx += 1;
        Ok(Some(sid))
    }

    fn resolve_var_symbol(&self, name: &str) -> Option<super::ids::SymbolId> {
        if self.session.is_some() {
            for sc in self.scope_stack.iter().rev() {
                if let Some(s) = sc.get(name) {
                    return Some(*s);
                }
            }
        }
        self.names.get(name).copied()
    }

    fn declare_local(&mut self, name: &str, span: Span) -> Result<super::ids::SymbolId, HirLowerError> {
        let sid = self
            .next_def_sid(span)?
            .ok_or_else(|| HirLowerError::at(span, "internal: session missing for local declaration"))?;
        let sym = self.alloc_fresh_symbol(name);
        self.sid_to_hir.insert(sid, sym);
        if self.session.is_some() {
            self.scope_stack
                .last_mut()
                .expect("scope stack")
                .insert(name.to_string(), sym);
        } else {
            self.names.insert(name.to_string(), sym);
        }
        Ok(sym)
    }

    fn push_scope(&mut self) {
        if self.session.is_some() {
            self.scope_stack.push(HashMap::new());
        }
    }

    fn pop_scope(&mut self) {
        if self.session.is_some() {
            let _ = self.scope_stack.pop();
        }
    }

    fn variable_hir_type(&self, name: &str, expr: &Expr) -> HirType {
        if let Some(sess) = self.session {
            if expr.id != NodeId::UNASSIGNED {
                if let Some(t) = sess.expr_types.get(expr.id.0 as usize).and_then(|x| x.as_ref()) {
                    return t.clone();
                }
            }
        }
        if name == "close" {
            return HirType::Series(Type::Primitive(PrimitiveType::Float));
        }
        if self.input_int_names.contains(name) {
            return HirType::Simple(Type::Primitive(PrimitiveType::Int));
        }
        HirType::Series(Type::Primitive(PrimitiveType::Float))
    }

    fn register_user_fn(&mut self, f: &FnDecl) -> Result<(), HirLowerError> {
        if f.is_method {
            return Err(HirLowerError::at(
                f.span,
                "method functions are not supported in this HIR lowering pass",
            ));
        }
        if self.user_fn_arity.insert(f.name.clone(), f.params.len()).is_some() {
            return Err(HirLowerError::at(
                f.span,
                format!("duplicate function `{}` in HIR lowering", f.name),
            ));
        }
        let sym = self.intern_name(&f.name);
        if let Some(sid) = self.next_def_sid(f.span)? {
            self.sid_to_hir.insert(sid, sym);
        }
        Ok(())
    }

    fn register_input_int(&mut self, name: &str) {
        self.input_int_names.insert(name.to_string());
    }

    fn intern_name(&mut self, name: &str) -> super::ids::SymbolId {
        if let Some(id) = self.names.get(name) {
            return *id;
        }
        let id = self.symbols.push(name);
        self.names.insert(name.to_string(), id);
        id
    }

    /// Per-scope symbol (e.g. function parameter) without deduplicating names across functions.
    fn alloc_fresh_symbol(&mut self, name: &str) -> super::ids::SymbolId {
        self.symbols.push(name)
    }

    fn alloc_expr(&mut self, e: HirExpr, span: Span) -> HirId {
        let id = HirId(self.exprs.len() as u32);
        self.exprs.push(e);
        self.expr_spans.push(span);
        id
    }

    fn expr_hir_type(&self, id: HirId) -> HirType {
        match &self.exprs[id.0 as usize] {
            HirExpr::Literal(_, ty) => ty.clone(),
            HirExpr::Variable(_, ty) => ty.clone(),
            HirExpr::Binary { ty, .. } => ty.clone(),
            HirExpr::BuiltinCall { ty, .. } => ty.clone(),
            HirExpr::UserCall { ty, .. } => ty.clone(),
            HirExpr::SeriesAccess { ty, .. } => ty.clone(),
            HirExpr::Security(sec) => sec.ty.clone(),
            HirExpr::Plot { .. } => HirType::Series(Type::Primitive(PrimitiveType::Float)),
        }
    }

    fn lower_script(&mut self, script: &Script) -> Result<HirScript, HirLowerError> {
        let version = script.version.unwrap_or(5);

        let mut declaration = HirDeclaration::FromAst(ScriptKind::Indicator);
        let mut inputs: Vec<HirInputDecl> = Vec::new();
        let mut body: Vec<HirStmt> = Vec::new();

        // Builtin series names used by the tiny subset (Pine `close`, …).
        self.intern_name("close");

        for item in &script.items {
            if let Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) = item {
                self.register_user_fn(f)?;
            }
        }

        let mut user_functions: Vec<HirUserFunction> = Vec::new();

        for item in &script.items {
            match item {
                Item::ScriptDecl(decl) => {
                    declaration = self.script_declaration(decl)?;
                }
                Item::Stmt(stmt) => {
                    self.lower_top_stmt(stmt, &mut inputs, &mut body)?;
                }
                Item::Import(i) => {
                    return Err(HirLowerError::at(
                        i.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
                Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => {
                    user_functions.push(self.lower_user_function(f)?);
                }
                Item::Enum(e) => {
                    return Err(HirLowerError::at(
                        e.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
                Item::TypeDef(t) => {
                    return Err(HirLowerError::at(
                        t.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
                Item::Export(ExportDecl::Var(v)) => {
                    return Err(HirLowerError::at(
                        v.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
                Item::Export(ExportDecl::Enum(e)) => {
                    return Err(HirLowerError::at(
                        e.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
                Item::Export(ExportDecl::TypeDef(t)) => {
                    return Err(HirLowerError::at(
                        t.span,
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
            }
        }

        Ok(HirScript {
            version,
            declaration,
            inputs,
            exprs: std::mem::replace(&mut self.exprs, BumpVec::new_in(self.bump))
                .into_iter()
                .collect(),
            expr_spans: std::mem::replace(&mut self.expr_spans, BumpVec::new_in(self.bump))
                .into_iter()
                .collect(),
            body,
            user_functions,
            symbols: std::mem::take(&mut self.symbols),
        })
    }

    fn lower_user_function(&mut self, f: &FnDecl) -> Result<HirUserFunction, HirLowerError> {
        let sym = *self.names.get(f.name.as_str()).ok_or_else(|| {
            HirLowerError::at(
                f.span,
                format!("internal: function `{}` missing from HIR symbol map", f.name),
            )
        })?;
        if self.session.is_some() {
            let saved_stack = std::mem::take(&mut self.scope_stack);
            self.scope_stack = vec![HashMap::new()];
            let mut params = Vec::with_capacity(f.params.len());
            for p in &f.params {
                let sid = self
                    .next_def_sid(f.span)?
                    .ok_or_else(|| HirLowerError::at(f.span, "internal: missing param semantic id"))?;
                let pid = self.alloc_fresh_symbol(&p.name);
                self.sid_to_hir.insert(sid, pid);
                self.scope_stack[0].insert(p.name.clone(), pid);
                params.push(pid);
            }
            let (body_stmts, result) = match &f.body {
                FnBody::Expr(e) => (Vec::new(), self.lower_expr(e)?),
                FnBody::Block(stmts) => self.lower_fn_block_stmts(f.span, stmts)?,
            };
            self.scope_stack = saved_stack;
            return Ok(HirUserFunction {
                symbol: sym,
                params,
                body_stmts,
                result,
            });
        }
        let saved_names = self.names.clone();
        let mut params = Vec::with_capacity(f.params.len());
        for p in &f.params {
            let pid = self.alloc_fresh_symbol(&p.name);
            self.names.insert(p.name.clone(), pid);
            params.push(pid);
        }
        let (body_stmts, result) = match &f.body {
            FnBody::Expr(e) => (Vec::new(), self.lower_expr(e)?),
            FnBody::Block(stmts) => self.lower_fn_block_stmts(f.span, stmts)?,
        };
        self.names = saved_names;
        Ok(HirUserFunction {
            symbol: sym,
            params,
            body_stmts,
            result,
        })
    }

    fn lower_fn_block_stmts(
        &mut self,
        span: Span,
        stmts: &[Stmt],
    ) -> Result<(Vec<HirStmt>, HirId), HirLowerError> {
        if stmts.is_empty() {
            return Err(HirLowerError::at(
                span,
                "user function block body must not be empty",
            ));
        }
        let mut body = Vec::new();
        let mut inputs_stub: Vec<HirInputDecl> = Vec::new();
        let last = stmts.len() - 1;
        for (i, s) in stmts.iter().enumerate() {
            if i == last {
                if let StmtKind::Expr(e) = &s.kind {
                    let hir = self.lower_expr(e)?;
                    return Ok((body, hir));
                }
            }
            self.lower_stmt_into(s, &mut inputs_stub, &mut body)?;
        }
        let z = self.alloc_expr(
            HirExpr::Literal(
                HirLiteral::Float(0.0),
                HirType::Series(Type::Primitive(PrimitiveType::Float)),
            ),
            span,
        );
        Ok((body, z))
    }

    fn script_declaration(&self, decl: &ScriptDeclaration) -> Result<HirDeclaration, HirLowerError> {
        match decl.kind {
            ScriptKind::Indicator => {
                let title = first_string_arg(&decl.args);
                Ok(HirDeclaration::Indicator { title })
            }
            ScriptKind::Strategy => Ok(HirDeclaration::Strategy {
                title: first_string_arg(&decl.args),
            }),
            ScriptKind::Library => Err(HirLowerError::at(
                script_declaration_span(decl),
                "library() scripts are not supported by this HIR lowering pass",
            )),
        }
    }

    fn lower_top_stmt(
        &mut self,
        stmt: &Stmt,
        inputs: &mut Vec<HirInputDecl>,
        body: &mut Vec<HirStmt>,
    ) -> Result<(), HirLowerError> {
        self.lower_stmt_into(stmt, inputs, body)
    }

    fn lower_stmt_into(
        &mut self,
        stmt: &Stmt,
        inputs: &mut Vec<HirInputDecl>,
        body: &mut Vec<HirStmt>,
    ) -> Result<(), HirLowerError> {
        match &stmt.kind {
            StmtKind::VarDecl(v) => {
                if v.qualifier == Some(VarQualifier::Input) {
                    let def = int_default_from_expr(&v.value)?;
                    inputs.push(HirInputDecl {
                        name: v.name.clone(),
                        default_int: def,
                    });
                    self.register_input_int(&v.name);
                    let sym = self.intern_name(&v.name);
                    if let Some(sid) = self.next_def_sid(v.span)? {
                        self.sid_to_hir.insert(sid, sym);
                    }
                    return Ok(());
                }
                if v.qualifier.is_some() || v.ty.is_some() {
                    return Err(HirLowerError::at(
                        v.span,
                        "only plain `name = expr` or `input …` declarations are supported",
                    ));
                }
                let sym = if self.session.is_some() {
                    self.declare_local(&v.name, v.span)?
                } else {
                    self.intern_name(&v.name)
                };
                let value = self.lower_expr(&v.value)?;
                body.push(HirStmt::Let {
                    symbol: sym,
                    value,
                });
                Ok(())
            }
            StmtKind::Assign {
                name,
                op: AssignOp::Eq,
                value,
            } => {
                if let Some(n) = try_input_int_default(value) {
                    inputs.push(HirInputDecl {
                        name: name.clone(),
                        default_int: n,
                    });
                    self.register_input_int(name);
                    let sym = self.intern_name(name);
                    if let Some(sid) = self.next_def_sid(stmt.span)? {
                        self.sid_to_hir.insert(sid, sym);
                    }
                    return Ok(());
                }
                let sym = if self.session.is_some() {
                    if let Some(s) = self.resolve_var_symbol(name) {
                        s
                    } else {
                        self.declare_local(name, stmt.span)?
                    }
                } else {
                    self.intern_name(name)
                };
                let hir = self.lower_expr(value)?;
                body.push(HirStmt::Let { symbol: sym, value: hir });
                Ok(())
            }
            StmtKind::Expr(e) => {
                if let Some(plot) = try_plot_stmt(self, e)? {
                    body.push(plot);
                    return Ok(());
                }
                Err(HirLowerError::at(
                    stmt.span,
                    "only `plot(...)` expression statements are supported in this pass",
                ))
            }
            StmtKind::If(i) => self.lower_if_stmt(i, inputs, body),
            StmtKind::Block(stmts) => {
                self.push_scope();
                let mut inner = Vec::new();
                for s in stmts {
                    self.lower_stmt_into(s, inputs, &mut inner)?;
                }
                self.pop_scope();
                body.push(HirStmt::Block(inner));
                Ok(())
            }
            _ => Err(HirLowerError::at(
                stmt.span,
                "statement kind not supported by this HIR lowering pass",
            )),
        }
    }

    fn lower_if_stmt(
        &mut self,
        i: &IfStmt,
        inputs: &mut Vec<HirInputDecl>,
        body: &mut Vec<HirStmt>,
    ) -> Result<(), HirLowerError> {
        let cond = self.lower_expr(&i.cond)?;
        self.push_scope();
        let mut then_branch = Vec::new();
        for s in &i.then_body {
            self.lower_stmt_into(s, inputs, &mut then_branch)?;
        }
        self.pop_scope();
        let else_branch = match &i.else_body {
            None => None,
            Some(ElseBody::Block(stmts)) => {
                self.push_scope();
                let mut v = Vec::new();
                for s in stmts {
                    self.lower_stmt_into(s, inputs, &mut v)?;
                }
                self.pop_scope();
                Some(v)
            }
            Some(ElseBody::If(inner)) => {
                let mut v = Vec::new();
                self.lower_if_stmt(inner, inputs, &mut v)?;
                Some(v)
            }
        };
        body.push(HirStmt::If {
            cond,
            then_branch,
            else_branch,
        });
        Ok(())
    }

    fn lower_expr(&mut self, e: &Expr) -> Result<HirId, HirLowerError> {
        match &e.kind {
            ExprKind::Int(i) => Ok(self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::Int(*i),
                    HirType::Simple(Type::Primitive(PrimitiveType::Int)),
                ),
                e.span,
            )),
            ExprKind::Float(f) => Ok(self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::Float(*f),
                    HirType::Simple(Type::Primitive(PrimitiveType::Float)),
                ),
                e.span,
            )),
            ExprKind::Bool(b) => Ok(self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::Bool(*b),
                    HirType::Simple(Type::Primitive(PrimitiveType::Bool)),
                ),
                e.span,
            )),
            ExprKind::String(s) => Ok(self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::String(s.clone()),
                    HirType::Simple(Type::Primitive(PrimitiveType::String)),
                ),
                e.span,
            )),
            ExprKind::Index { base, index } => {
                let base_id = self.lower_expr(base.as_ref())?;
                let idx = index.as_ref();
                let offset = match &idx.kind {
                    ExprKind::Int(i) if *i >= 0 && *i <= i64::from(i32::MAX) => *i as i32,
                    _ => {
                        return Err(HirLowerError::at(
                            idx.span,
                            "series history index must be a non-negative integer literal in this HIR pass",
                        ));
                    }
                };
                let ty = self.expr_hir_type(base_id);
                Ok(self.alloc_expr(
                    HirExpr::SeriesAccess {
                        base: base_id,
                        offset,
                        ty,
                    },
                    e.span,
                ))
            }
            ExprKind::IdentPath(path) => self.lower_ident_path(path, e.span, e),
            ExprKind::Binary { op, left, right } => {
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {}
                    _ => {
                        return Err(HirLowerError::at(
                            e.span,
                            "binary operator not supported by this HIR lowering pass",
                        ));
                    }
                }
                let lhs = self.lower_expr(left.as_ref())?;
                let rhs = self.lower_expr(right.as_ref())?;
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: *op,
                        lhs,
                        rhs,
                        ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                    },
                    e.span,
                ))
            }
            ExprKind::Call {
                callee,
                type_args,
                args,
            } => {
                if type_args.is_some() {
                    return Err(HirLowerError::at(
                        callee.span,
                        "generic calls are not supported in this HIR lowering pass",
                    ));
                }
                self.lower_call(callee.as_ref(), args, e.span)
            }
            _ => Err(HirLowerError::at(
                e.span,
                "expression kind not supported by this HIR lowering pass",
            )),
        }
    }

    fn lower_ident_path(
        &mut self,
        path: &[String],
        span: Span,
        expr: &Expr,
    ) -> Result<HirId, HirLowerError> {
        if path.len() == 1 {
            let name = &path[0];
            if let Some(sess) = self.session {
                if expr.id != NodeId::UNASSIGNED {
                    if let Some(b) = sess
                        .name_bindings
                        .get(expr.id.0 as usize)
                        .and_then(|x| x.as_ref())
                    {
                        match b {
                            NameBinding::Local(sid) => {
                                if let Some(id) = self.sid_to_hir.get(sid).copied() {
                                    let ty = self.variable_hir_type(name, expr);
                                    return Ok(self.alloc_expr(HirExpr::Variable(id, ty), span));
                                }
                            }
                            NameBinding::UnqualifiedBuiltin(bn) => {
                                if let Some(id) = self.resolve_var_symbol(bn.as_str()) {
                                    let ty = self.variable_hir_type(bn.as_str(), expr);
                                    return Ok(self.alloc_expr(HirExpr::Variable(id, ty), span));
                                }
                            }
                            NameBinding::QualifiedPath(_) => {
                                return Err(HirLowerError::at(
                                    span,
                                    format!(
                                        "qualified identifier `{}` not supported as HIR value",
                                        path.join(".")
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
            let id = self.resolve_var_symbol(name.as_str()).ok_or_else(|| {
                HirLowerError::at(span, format!("unknown identifier `{name}`"))
            })?;
            let ty = self.variable_hir_type(name, expr);
            return Ok(self.alloc_expr(HirExpr::Variable(id, ty), span));
        }
        Err(HirLowerError::at(
            span,
            format!(
                "qualified identifier `{}` not supported",
                path.join(".")
            ),
        ))
    }

    fn lower_call(
        &mut self,
        callee: &Expr,
        args: &[(Option<String>, Expr)],
        expr_span: Span,
    ) -> Result<HirId, HirLowerError> {
        let path = match &callee.kind {
            ExprKind::IdentPath(p) => p.as_slice(),
            _ => {
                return Err(HirLowerError::at(
                    callee.span,
                    "only simple path callees are supported",
                ));
            }
        };

        if path == ["input", "int"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(
                    expr_span,
                    "input.int expects one argument",
                ));
            }
            let n = match &args[0].1.kind {
                ExprKind::Int(i) => *i,
                _ => {
                    return Err(HirLowerError::at(
                        args[0].1.span,
                        "input.int default must be an integer literal in this pass",
                    ));
                }
            };
            let lit = self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::Int(n),
                    HirType::Simple(Type::Primitive(PrimitiveType::Int)),
                ),
                args[0].1.span,
            );
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::InputInt,
                    args: vec![lit],
                    ty: HirType::Simple(Type::Primitive(PrimitiveType::Int)),
                },
                expr_span,
            ));
        }

        if path == ["ta", "sma"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "ta.sma expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaSma,
                    args: vec![a0, a1],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                },
                expr_span,
            ));
        }

        if path == ["ta", "ema"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "ta.ema expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaEma,
                    args: vec![a0, a1],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                },
                expr_span,
            ));
        }

        if path == ["request", "security"] {
            if args.len() < 3 {
                return Err(HirLowerError::at(
                    expr_span,
                    "request.security expects at least three arguments (symbol, timeframe, expression)",
                ));
            }
            let sym = self.lower_expr(&args[0].1)?;
            let tf = self.lower_expr(&args[1].1)?;
            let inner = self.lower_expr(&args[2].1)?;
            let mut gaps = GapMode::NoGaps;
            let mut lookahead = Lookahead::Off;
            let mut gaps_set = false;
            let mut lookahead_set = false;
            for (nm, ex) in args.iter().skip(3) {
                match nm.as_deref() {
                    Some("gaps") => {
                        gaps = gap_mode_from_expr(ex);
                        gaps_set = true;
                    }
                    Some("lookahead") => {
                        lookahead = lookahead_from_expr(ex);
                        lookahead_set = true;
                    }
                    _ => {}
                }
            }
            let positional: Vec<&Expr> = args
                .iter()
                .skip(3)
                .filter(|(n, _)| n.is_none())
                .map(|(_, ex)| ex)
                .collect();
            if !gaps_set {
                if let Some(ex) = positional.first() {
                    gaps = gap_mode_from_expr(ex);
                }
            }
            if !lookahead_set {
                if let Some(ex) = positional.get(1) {
                    lookahead = lookahead_from_expr(ex);
                }
            }
            let inner_ty = self.expr_hir_type(inner);
            let sec_ty = match inner_ty {
                HirType::Simple(Type::Primitive(p)) => HirType::Series(Type::Primitive(p)),
                HirType::Series(Type::Primitive(p)) => HirType::Series(Type::Primitive(p)),
                _ => HirType::Series(Type::Primitive(PrimitiveType::Float)),
            };
            let sec = SecurityCall {
                symbol: sym,
                timeframe: tf,
                expression: inner,
                gaps,
                lookahead,
                ty: sec_ty,
            };
            return Ok(self.alloc_expr(HirExpr::Security(Box::new(sec)), expr_span));
        }

        if path.len() == 1 {
            let name = &path[0];
            if let Some(&arity) = self.user_fn_arity.get(name.as_str()) {
                if args.len() != arity {
                    return Err(HirLowerError::at(
                        expr_span,
                        format!(
                            "`{name}` expects {arity} arguments, got {}",
                            args.len()
                        ),
                    ));
                }
                let sym = *self.names.get(name.as_str()).ok_or_else(|| {
                    HirLowerError::at(expr_span, format!("unknown function `{name}`"))
                })?;
                let mut call_args = Vec::new();
                for (_, e) in args {
                    call_args.push(self.lower_expr(e)?);
                }
                return Ok(self.alloc_expr(
                    HirExpr::UserCall {
                        callee: sym,
                        args: call_args,
                        ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                    },
                    expr_span,
                ));
            }
        }

        if path == ["plot"] {
            return Err(HirLowerError::at(
                expr_span,
                "use `plot(expr)` as a statement, not as a nested expression",
            ));
        }

        Err(HirLowerError::at(
            expr_span,
            format!("call `{}` not supported", path.join(".")),
        ))
    }
}

fn path_tail_from_expr(e: &Expr) -> Option<Vec<String>> {
    match &e.kind {
        ExprKind::IdentPath(p) => Some(p.clone()),
        ExprKind::Member { base, field } => {
            let mut p = path_tail_from_expr(base.as_ref())?;
            p.push(field.clone());
            Some(p)
        }
        _ => None,
    }
}

fn gap_mode_from_expr(e: &Expr) -> GapMode {
    let Some(p) = path_tail_from_expr(e) else {
        return GapMode::NoGaps;
    };
    if p.len() == 2 && p[0] == "barmerge" && p[1] == "gaps_on" {
        GapMode::WithGaps
    } else {
        GapMode::NoGaps
    }
}

fn lookahead_from_expr(e: &Expr) -> Lookahead {
    let Some(p) = path_tail_from_expr(e) else {
        return Lookahead::Off;
    };
    if p.len() == 2 && p[0] == "barmerge" && p[1] == "lookahead_on" {
        Lookahead::On
    } else {
        Lookahead::Off
    }
}

fn try_plot_stmt(lower: &mut LowerCtx, e: &Expr) -> Result<Option<HirStmt>, HirLowerError> {
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &e.kind
    else {
        return Ok(None);
    };
    if type_args.is_some() {
        return Ok(None);
    }
    let path = match &callee.kind {
        ExprKind::IdentPath(p) => p.as_slice(),
        _ => return Ok(None),
    };
    if path != ["plot"] {
        return Ok(None);
    }
    if args.is_empty() {
        return Err(HirLowerError::at(
            e.span,
            "plot needs at least one argument",
        ));
    }
    let expr = lower.lower_expr(&args[0].1)?;
    let title = args.get(1).and_then(|(_, ex)| match &ex.kind {
        ExprKind::String(s) => Some(s.clone()),
        _ => None,
    });
    Ok(Some(HirStmt::Plot { expr, title }))
}

fn first_string_arg(args: &[(Option<String>, Expr)]) -> Option<String> {
    for (_, e) in args {
        if let ExprKind::String(s) = &e.kind {
            return Some(s.clone());
        }
    }
    None
}

fn int_default_from_expr(e: &Expr) -> Result<i64, HirLowerError> {
    match &e.kind {
        ExprKind::Int(i) => Ok(*i),
        _ => {
            if let Some(n) = try_input_int_default(e) {
                return Ok(n);
            }
            Err(HirLowerError::at(
                e.span,
                "input declaration default must be an int literal or input.int(int)",
            ))
        }
    }
}

fn try_input_int_default(e: &Expr) -> Option<i64> {
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &e.kind
    else {
        return None;
    };
    if type_args.is_some() || args.len() != 1 {
        return None;
    }
    let path = match &callee.kind {
        ExprKind::IdentPath(p) => p.as_slice(),
        _ => return None,
    };
    if path != ["input", "int"] {
        return None;
    }
    match &args[0].1.kind {
        ExprKind::Int(i) => Some(*i),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_script;
    use crate::semantic::check_script;
    use insta::assert_debug_snapshot;

    const SAMPLE: &str = r#"//@version=6
indicator("Test Agent")

len = input.int(14)
sma = ta.sma(close, len)
htf = request.security("AAPL", "D", sma)
plot(htf)
"#;

    #[test]
    fn golden_tiny_indicator_pipeline() {
        let script = parse_script("test", SAMPLE).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    #[test]
    fn hir_pipeline_sets_session_hir() {
        let script = parse_script("test", SAMPLE).expect("parse");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert!(
            c.session.hir.is_some(),
            "tiny indicator should lower into HIR when HirLowerPass runs"
        );
    }

    const SAMPLE_SERIES_SECURITY: &str = r#"//@version=6
indicator("x")
len = input.int(14)
sma = ta.sma(close, len)
htf = request.security("AAPL", "D", sma, barmerge.gaps_on, barmerge.lookahead_on)
prev = close[1]
plot(htf + prev)
"#;

    #[test]
    fn golden_series_access_and_security_options() {
        let script = parse_script("test", SAMPLE_SERIES_SECURITY).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    const SAMPLE_EMA: &str = r#"//@version=6
indicator("EMA pipeline")
len = input.int(14)
ema = ta.ema(close, len)
plot(ema)
"#;

    #[test]
    fn golden_ta_ema_indicator() {
        let script = parse_script("test", SAMPLE_EMA).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    const SAMPLE_USER_FN_IF: &str = r#"//@version=6
indicator("UF")
f(float x) => x * 2.0
y = f(close)
if true {
  plot(y)
}
"#;

    const SAMPLE_BLOCK_USER_FN: &str = r#"//@version=6
indicator("block fn")
g(float a) {
  t = a * 3.0
  t
}
z = g(close)
plot(z)
"#;

    #[test]
    fn golden_user_fn_and_conditional_plot() {
        let script = parse_script("test", SAMPLE_USER_FN_IF).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    #[test]
    fn golden_block_user_function_body() {
        let script = parse_script("test", SAMPLE_BLOCK_USER_FN).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }
}
