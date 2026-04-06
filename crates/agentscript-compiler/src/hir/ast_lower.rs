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
    PrimitiveType, Script, ScriptDeclaration, ScriptKind, Span, Stmt, StmtKind, Type, UnaryOp,
    VarQualifier,
};
use crate::session::CompilerSession;

use super::builtin::BuiltinKind;
use super::expr::HirExpr;
use super::ids::{HirId, SymbolId};
use super::literal::HirLiteral;
use super::lowering::LowerToHir;
use super::script::{HirDeclaration, HirInputDecl, HirInputKind, HirScript, HirUserFunction};
use super::financial::FinancialCall;
use super::security::{GapMode, Lookahead, SecurityCall};
use super::stmt::HirStmt;
use super::symbols::SymbolTable;
use super::ty::HirType;

fn script_declaration_span(decl: &ScriptDeclaration) -> Span {
    decl.args
        .first()
        .map(|(_, ex)| ex.span)
        .unwrap_or(decl.span)
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
    /// Names introduced by `input.float` / `input float`.
    input_float_names: HashSet<String>,
    /// User `f(...) =>` / Pine expr-body functions: name → arity (parameters).
    user_fn_arity: HashMap<String, usize>,
    persist_symbols: HashSet<SymbolId>,
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
            input_float_names: HashSet::new(),
            user_fn_arity: HashMap::new(),
            persist_symbols: HashSet::new(),
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

    /// Pine built-ins `hl2` / `hlc3` / `ohlc4` / `hlcc4` as arithmetic on OHLC(V) series.
    fn lower_derived_price_alias(&mut self, name: &str, span: Span) -> Result<HirId, HirLowerError> {
        let sf = HirType::Series(Type::Primitive(PrimitiveType::Float));
        let simple_f = HirType::Simple(Type::Primitive(PrimitiveType::Float));
        match name {
            "hl2" => {
                let h_sym = self.resolve_var_symbol("high").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `high` missing for hl2")
                })?;
                let l_sym = self.resolve_var_symbol("low").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `low` missing for hl2")
                })?;
                let h = self.alloc_expr(HirExpr::Variable(h_sym, sf.clone()), span);
                let l = self.alloc_expr(HirExpr::Variable(l_sym, sf.clone()), span);
                let sum = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: h,
                        rhs: l,
                        ty: sf.clone(),
                    },
                    span,
                );
                let two = self.alloc_expr(
                    HirExpr::Literal(HirLiteral::Float(2.0), simple_f),
                    span,
                );
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Div,
                        lhs: sum,
                        rhs: two,
                        ty: sf,
                    },
                    span,
                ))
            }
            "hlc3" => {
                let h_sym = self.resolve_var_symbol("high").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `high` missing for hlc3")
                })?;
                let l_sym = self.resolve_var_symbol("low").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `low` missing for hlc3")
                })?;
                let c_sym = self.resolve_var_symbol("close").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `close` missing for hlc3")
                })?;
                let h = self.alloc_expr(HirExpr::Variable(h_sym, sf.clone()), span);
                let l = self.alloc_expr(HirExpr::Variable(l_sym, sf.clone()), span);
                let c = self.alloc_expr(HirExpr::Variable(c_sym, sf.clone()), span);
                let s1 = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: h,
                        rhs: l,
                        ty: sf.clone(),
                    },
                    span,
                );
                let sum = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: s1,
                        rhs: c,
                        ty: sf.clone(),
                    },
                    span,
                );
                let three = self.alloc_expr(
                    HirExpr::Literal(HirLiteral::Float(3.0), simple_f),
                    span,
                );
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Div,
                        lhs: sum,
                        rhs: three,
                        ty: sf,
                    },
                    span,
                ))
            }
            "ohlc4" => {
                let o_sym = self.resolve_var_symbol("open").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `open` missing for ohlc4")
                })?;
                let h_sym = self.resolve_var_symbol("high").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `high` missing for ohlc4")
                })?;
                let l_sym = self.resolve_var_symbol("low").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `low` missing for ohlc4")
                })?;
                let c_sym = self.resolve_var_symbol("close").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `close` missing for ohlc4")
                })?;
                let o = self.alloc_expr(HirExpr::Variable(o_sym, sf.clone()), span);
                let h = self.alloc_expr(HirExpr::Variable(h_sym, sf.clone()), span);
                let l = self.alloc_expr(HirExpr::Variable(l_sym, sf.clone()), span);
                let c = self.alloc_expr(HirExpr::Variable(c_sym, sf.clone()), span);
                let s1 = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: o,
                        rhs: h,
                        ty: sf.clone(),
                    },
                    span,
                );
                let s2 = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: l,
                        rhs: c,
                        ty: sf.clone(),
                    },
                    span,
                );
                let sum = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: s1,
                        rhs: s2,
                        ty: sf.clone(),
                    },
                    span,
                );
                let four = self.alloc_expr(
                    HirExpr::Literal(HirLiteral::Float(4.0), simple_f),
                    span,
                );
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Div,
                        lhs: sum,
                        rhs: four,
                        ty: sf,
                    },
                    span,
                ))
            }
            "hlcc4" => {
                let h_sym = self.resolve_var_symbol("high").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `high` missing for hlcc4")
                })?;
                let l_sym = self.resolve_var_symbol("low").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `low` missing for hlcc4")
                })?;
                let c_sym = self.resolve_var_symbol("close").ok_or_else(|| {
                    HirLowerError::at(span, "internal: builtin `close` missing for hlcc4")
                })?;
                let h = self.alloc_expr(HirExpr::Variable(h_sym, sf.clone()), span);
                let l = self.alloc_expr(HirExpr::Variable(l_sym, sf.clone()), span);
                let c1 = self.alloc_expr(HirExpr::Variable(c_sym, sf.clone()), span);
                let c2 = self.alloc_expr(HirExpr::Variable(c_sym, sf.clone()), span);
                let s1 = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: h,
                        rhs: l,
                        ty: sf.clone(),
                    },
                    span,
                );
                let s2 = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: c1,
                        rhs: c2,
                        ty: sf.clone(),
                    },
                    span,
                );
                let sum = self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Add,
                        lhs: s1,
                        rhs: s2,
                        ty: sf.clone(),
                    },
                    span,
                );
                let four = self.alloc_expr(
                    HirExpr::Literal(HirLiteral::Float(4.0), simple_f),
                    span,
                );
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: BinOp::Div,
                        lhs: sum,
                        rhs: four,
                        ty: sf,
                    },
                    span,
                ))
            }
            _ => Err(HirLowerError::at(
                span,
                format!("unknown derived price alias `{name}`"),
            )),
        }
    }

    fn expr_ty_from_session_or_float_series(&self, e: &Expr) -> HirType {
        self.session
            .and_then(|s| {
                if e.id != NodeId::UNASSIGNED {
                    s.expr_types
                        .get(e.id.0 as usize)
                        .and_then(|x| x.as_ref())
                        .cloned()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| HirType::Series(Type::Primitive(PrimitiveType::Float)))
    }

    fn variable_hir_type(&self, name: &str, expr: &Expr) -> HirType {
        if let Some(sess) = self.session {
            if expr.id != NodeId::UNASSIGNED {
                if let Some(t) = sess.expr_types.get(expr.id.0 as usize).and_then(|x| x.as_ref()) {
                    return t.clone();
                }
            }
        }
        if matches!(
            name,
            "close" | "open" | "high" | "low" | "volume" | "time" | "hl2" | "hlc3" | "ohlc4" | "hlcc4"
        ) {
            return HirType::Series(Type::Primitive(PrimitiveType::Float));
        }
        if self.input_int_names.contains(name) {
            return HirType::Simple(Type::Primitive(PrimitiveType::Int));
        }
        if self.input_float_names.contains(name) {
            return HirType::Simple(Type::Primitive(PrimitiveType::Float));
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

    fn register_input_float(&mut self, name: &str) {
        self.input_float_names.insert(name.to_string());
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
            HirExpr::Select { ty, .. } => ty.clone(),
            HirExpr::Not { ty, .. } => ty.clone(),
            HirExpr::Security(sec) => sec.ty.clone(),
            HirExpr::Financial(f) => f.ty.clone(),
            HirExpr::Plot { .. } => HirType::Series(Type::Primitive(PrimitiveType::Float)),
            HirExpr::Array { ty, .. } => ty.clone(),
        }
    }

    fn lower_script(&mut self, script: &Script) -> Result<HirScript, HirLowerError> {
        let version = script.version.unwrap_or(5);

        let mut declaration = HirDeclaration::FromAst(ScriptKind::Indicator);
        let mut source_span = Span::DUMMY;
        let mut inputs: Vec<HirInputDecl> = Vec::new();
        let mut body: Vec<HirStmt> = Vec::new();

        // Builtin series names (Pine OHLCV / time).
        for n in ["close", "open", "high", "low", "volume", "time"] {
            self.intern_name(n);
        }

        for item in &script.items {
            if let Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) = item {
                self.register_user_fn(f)?;
            }
        }

        let mut user_functions: Vec<HirUserFunction> = Vec::new();

        for item in &script.items {
            match item {
                Item::ScriptDecl(decl) => {
                    source_span = decl.span;
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
            source_span,
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
            persist_symbols: std::mem::take(&mut self.persist_symbols),
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
                let timeframe = string_kw_first_string(&decl.args, "timeframe");
                Ok(HirDeclaration::Indicator { title, timeframe })
            }
            ScriptKind::Strategy => Ok(HirDeclaration::Strategy {
                title: first_string_arg(&decl.args),
                timeframe: string_kw_first_string(&decl.args, "timeframe"),
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
                if matches!(
                    v.qualifier,
                    Some(VarQualifier::Var) | Some(VarQualifier::Varip)
                ) {
                    let sym = if self.session.is_some() {
                        self.declare_local(&v.name, v.span)?
                    } else {
                        self.intern_name(&v.name)
                    };
                    let value = self.lower_expr(&v.value)?;
                    self.persist_symbols.insert(sym);
                    body.push(HirStmt::VarInit { symbol: sym, value });
                    return Ok(());
                }
                if v.qualifier == Some(VarQualifier::Input) {
                    let is_float = matches!(
                        v.ty,
                        Some(Type::Primitive(PrimitiveType::Float))
                    );
                    if is_float {
                        let def = float_default_from_expr(&v.value)?;
                        inputs.push(HirInputDecl {
                            name: v.name.clone(),
                            kind: HirInputKind::Float(def),
                        });
                        self.register_input_float(&v.name);
                    } else {
                        let def = int_default_from_expr(&v.value)?;
                        inputs.push(HirInputDecl {
                            name: v.name.clone(),
                            kind: HirInputKind::Int(def),
                        });
                        self.register_input_int(&v.name);
                    }
                    let sym = self.intern_name(&v.name);
                    if let Some(sid) = self.next_def_sid(v.span)? {
                        self.sid_to_hir.insert(sid, sym);
                    }
                    return Ok(());
                }
                if v.qualifier.is_some() || v.ty.is_some() {
                    return Err(HirLowerError::at(
                        v.span,
                        "only plain `name = expr`, `input …`, or `var` / `varip` declarations are supported",
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
                op: AssignOp::ColonEq,
                value,
            } => {
                let sym = self.resolve_var_symbol(name).ok_or_else(|| {
                    HirLowerError::at(
                        stmt.span,
                        format!("unknown variable `{name}` for `:=` reassignment"),
                    )
                })?;
                let hir = self.lower_expr(value)?;
                body.push(HirStmt::Let { symbol: sym, value: hir });
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
                        kind: HirInputKind::Int(n),
                    });
                    self.register_input_int(name);
                    let sym = self.intern_name(name);
                    if let Some(sid) = self.next_def_sid(stmt.span)? {
                        self.sid_to_hir.insert(sid, sym);
                    }
                    return Ok(());
                }
                if let Some(kind) = try_input_plain_defval_for_decl(value) {
                    match &kind {
                        HirInputKind::Int(_) => self.register_input_int(name),
                        HirInputKind::Float(_) => self.register_input_float(name),
                    }
                    inputs.push(HirInputDecl {
                        name: name.clone(),
                        kind,
                    });
                    let sym = self.intern_name(name);
                    if let Some(sid) = self.next_def_sid(stmt.span)? {
                        self.sid_to_hir.insert(sid, sym);
                    }
                    return Ok(());
                }
                if let Some(x) = try_input_float_default(value) {
                    inputs.push(HirInputDecl {
                        name: name.clone(),
                        kind: HirInputKind::Float(x),
                    });
                    self.register_input_float(name);
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
            StmtKind::Assign {
                name,
                op,
                value,
            } => {
                let bin_op = match op {
                    AssignOp::PlusEq => BinOp::Add,
                    AssignOp::MinusEq => BinOp::Sub,
                    AssignOp::StarEq => BinOp::Mul,
                    AssignOp::SlashEq => BinOp::Div,
                    AssignOp::PercentEq => BinOp::Mod,
                    AssignOp::Eq | AssignOp::ColonEq => unreachable!("handled above"),
                };
                let sym = self.resolve_var_symbol(name).ok_or_else(|| {
                    HirLowerError::at(
                        stmt.span,
                        format!("unknown variable `{name}` for compound assignment"),
                    )
                })?;
                let v_ty = HirType::Series(Type::Primitive(PrimitiveType::Float));
                let lhs = self.alloc_expr(
                    HirExpr::Variable(sym, v_ty.clone()),
                    stmt.span,
                );
                let rhs = self.lower_expr(value)?;
                let merged = self.alloc_expr(
                    HirExpr::Binary {
                        op: bin_op,
                        lhs,
                        rhs,
                        ty: v_ty,
                    },
                    stmt.span,
                );
                body.push(HirStmt::Let {
                    symbol: sym,
                    value: merged,
                });
                Ok(())
            }
            StmtKind::Expr(e) => {
                if let Some(plot) = try_plot_stmt(self, e)? {
                    body.push(plot);
                    return Ok(());
                }
                if is_unlowered_viz_stmt(e) {
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
            ExprKind::Unary { op, expr } => {
                let inner = self.lower_expr(expr.as_ref())?;
                match op {
                    UnaryOp::Pos => Ok(inner),
                    UnaryOp::Neg => {
                        let z = self.alloc_expr(
                            HirExpr::Literal(
                                HirLiteral::Float(0.0),
                                HirType::Simple(Type::Primitive(PrimitiveType::Float)),
                            ),
                            e.span,
                        );
                        let ty = self.expr_ty_from_session_or_float_series(e);
                        Ok(self.alloc_expr(
                            HirExpr::Binary {
                                op: BinOp::Sub,
                                lhs: z,
                                rhs: inner,
                                ty,
                            },
                            e.span,
                        ))
                    }
                    UnaryOp::Not => {
                        let ty = self.expr_ty_from_session_or_float_series(e);
                        Ok(self.alloc_expr(
                            HirExpr::Not {
                                inner,
                                ty,
                            },
                            e.span,
                        ))
                    }
                }
            }
            ExprKind::Binary { op, left, right } => {
                let lhs = self.lower_expr(left.as_ref())?;
                let rhs = self.lower_expr(right.as_ref())?;
                let ty = self.expr_ty_from_session_or_float_series(e);
                Ok(self.alloc_expr(
                    HirExpr::Binary {
                        op: *op,
                        lhs,
                        rhs,
                        ty,
                    },
                    e.span,
                ))
            }
            ExprKind::Array(elts) => {
                let mut ids = Vec::with_capacity(elts.len());
                for x in elts {
                    ids.push(self.lower_expr(x)?);
                }
                let ty = self
                    .session
                    .and_then(|s| {
                        if e.id != NodeId::UNASSIGNED {
                            s.expr_types
                                .get(e.id.0 as usize)
                                .and_then(|t| t.as_ref())
                                .cloned()
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        if ids.is_empty() {
                            HirType::Array(Box::new(HirType::Simple(Type::Primitive(
                                PrimitiveType::Float,
                            ))))
                        } else {
                            HirType::Array(Box::new(self.expr_hir_type(ids[0])))
                        }
                    });
                Ok(self.alloc_expr(
                    HirExpr::Array {
                        elements: ids,
                        ty,
                    },
                    e.span,
                ))
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
                let c = self.lower_expr(cond.as_ref())?;
                let t = self.lower_expr(then_b.as_ref())?;
                let u = self.lower_expr(else_b.as_ref())?;
                let ty = self.expr_ty_from_session_or_float_series(e);
                Ok(self.alloc_expr(
                    HirExpr::Select {
                        cond: c,
                        then_b: t,
                        else_b: u,
                        ty,
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
                self.lower_call(callee.as_ref(), args, e.span, e)
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
        if path == ["ta", "tr"] {
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaTr,
                    args: vec![],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                },
                span,
            ));
        }
        if path.len() == 1 {
            let name = &path[0];
            if matches!(name.as_str(), "hl2" | "hlc3" | "ohlc4" | "hlcc4") {
                return self.lower_derived_price_alias(name.as_str(), span);
            }
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
        call_expr: &Expr,
    ) -> Result<HirId, HirLowerError> {
        let (path, method_receiver) = callee_path_with_receiver(callee)?;

        if path == ["input"] {
            if let Some((_, ex)) = args.iter().find(|(nm, _)| nm.as_deref() == Some("defval")) {
                return match &ex.kind {
                    ExprKind::Int(n) => {
                        let lit = self.alloc_expr(
                            HirExpr::Literal(
                                HirLiteral::Int(*n),
                                HirType::Simple(Type::Primitive(PrimitiveType::Int)),
                            ),
                            ex.span,
                        );
                        Ok(self.alloc_expr(
                            HirExpr::BuiltinCall {
                                kind: BuiltinKind::InputInt,
                                args: vec![lit],
                                ty: HirType::Simple(Type::Primitive(PrimitiveType::Int)),
                            },
                            expr_span,
                        ))
                    }
                    ExprKind::Float(f) => {
                        let lit = self.alloc_expr(
                            HirExpr::Literal(
                                HirLiteral::Float(*f),
                                HirType::Simple(Type::Primitive(PrimitiveType::Float)),
                            ),
                            ex.span,
                        );
                        Ok(self.alloc_expr(
                            HirExpr::BuiltinCall {
                                kind: BuiltinKind::InputFloat,
                                args: vec![lit],
                                ty: HirType::Simple(Type::Primitive(PrimitiveType::Float)),
                            },
                            expr_span,
                        ))
                    }
                    ExprKind::Bool(b) => Ok(self.alloc_expr(
                        HirExpr::Literal(
                            HirLiteral::Bool(*b),
                            HirType::Simple(Type::Primitive(PrimitiveType::Bool)),
                        ),
                        ex.span,
                    )),
                    _ => Err(HirLowerError::at(
                        ex.span,
                        "unsupported defval in `input()` for this lowering pass",
                    )),
                };
            }
            let (_, ex) = args.first().ok_or_else(|| {
                HirLowerError::at(expr_span, "`input()` expects a source series or `defval=`")
            })?;
            return self.lower_expr(ex);
        }

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

        if path == ["input", "float"] {
            let x = if args.len() == 1 {
                match &args[0].1.kind {
                    ExprKind::Float(f) => *f,
                    _ => {
                        return Err(HirLowerError::at(
                            args[0].1.span,
                            "input.float default must be a float literal in this pass",
                        ));
                    }
                }
            } else {
                let (_, ex) = args
                    .iter()
                    .find(|(nm, _)| nm.as_deref() == Some("defval"))
                    .ok_or_else(|| {
                        HirLowerError::at(
                            expr_span,
                            "input.float with keyword args needs defval=",
                        )
                    })?;
                match &ex.kind {
                    ExprKind::Float(f) => *f,
                    _ => {
                        return Err(HirLowerError::at(
                            ex.span,
                            "input.float defval must be a float literal in this pass",
                        ));
                    }
                }
            };
            let lit = self.alloc_expr(
                HirExpr::Literal(
                    HirLiteral::Float(x),
                    HirType::Simple(Type::Primitive(PrimitiveType::Float)),
                ),
                args[0].1.span,
            );
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::InputFloat,
                    args: vec![lit],
                    ty: HirType::Simple(Type::Primitive(PrimitiveType::Float)),
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

        // `close.sma(len)` → `ta.sma(close, len)` (Pine method form).
        if path == ["close", "sma"] {
            let recv = method_receiver.ok_or_else(|| {
                HirLowerError::at(callee.span, "internal: close.sma missing receiver")
            })?;
            if args.len() != 1 {
                return Err(HirLowerError::at(
                    expr_span,
                    "close.sma expects one argument (length)",
                ));
            }
            let a0 = self.lower_expr(recv)?;
            let a1 = self.lower_expr(&args[0].1)?;
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

        if path == ["close", "ema"] {
            let recv = method_receiver.ok_or_else(|| {
                HirLowerError::at(callee.span, "internal: close.ema missing receiver")
            })?;
            if args.len() != 1 {
                return Err(HirLowerError::at(
                    expr_span,
                    "close.ema expects one argument (length)",
                ));
            }
            let a0 = self.lower_expr(recv)?;
            let a1 = self.lower_expr(&args[0].1)?;
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

        if path == ["request", "financial"] {
            if args.len() < 3 {
                return Err(HirLowerError::at(
                    expr_span,
                    "request.financial expects at least three arguments (symbol, financial id, period)",
                ));
            }
            let symbol = self.lower_expr(&args[0].1)?;
            let financial_id = self.lower_expr(&args[1].1)?;
            let period = self.lower_expr(&args[2].1)?;
            let mut gaps = GapMode::NoGaps;
            let mut gaps_set = false;
            let mut ignore_invalid_symbol: Option<HirId> = None;
            let mut currency: Option<HirId> = None;
            for (nm, ex) in args.iter().skip(3) {
                match nm.as_deref() {
                    Some("gaps") => {
                        gaps = gap_mode_from_expr(ex);
                        gaps_set = true;
                    }
                    Some("ignore_invalid_symbol") => {
                        ignore_invalid_symbol = Some(self.lower_expr(ex)?);
                    }
                    Some("currency") => {
                        currency = Some(self.lower_expr(ex)?);
                    }
                    _ => {}
                }
            }
            let positionals: Vec<&Expr> = args
                .iter()
                .skip(3)
                .filter(|(n, _)| n.is_none())
                .map(|(_, ex)| ex)
                .collect();
            let mut pi = 0usize;
            if !gaps_set {
                if let Some(ex) = positionals.get(pi) {
                    if is_barmerge_financial_gaps_expr(ex) {
                        gaps = gap_mode_from_expr(ex);
                        pi += 1;
                    }
                }
            }
            if ignore_invalid_symbol.is_none() {
                if let Some(ex) = positionals.get(pi) {
                    if matches!(ex.kind, ExprKind::Bool(_)) {
                        ignore_invalid_symbol = Some(self.lower_expr(ex)?);
                        pi += 1;
                    }
                }
            }
            if currency.is_none() {
                if let Some(ex) = positionals.get(pi) {
                    currency = Some(self.lower_expr(ex)?);
                }
            }
            let ty = self.expr_ty_from_session_or_float_series(call_expr);
            let fin = FinancialCall {
                symbol,
                financial_id,
                period,
                gaps,
                ignore_invalid_symbol,
                currency,
                ty,
            };
            return Ok(self.alloc_expr(HirExpr::Financial(Box::new(fin)), expr_span));
        }

        if path == ["ta", "crossover"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(
                    expr_span,
                    "ta.crossover expects two arguments",
                ));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaCrossover,
                    args: vec![a0, a1],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Bool)),
                },
                expr_span,
            ));
        }

        if path == ["ta", "atr"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(
                    expr_span,
                    "ta.atr expects one argument (length)",
                ));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaAtr,
                    args: vec![a0],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                },
                expr_span,
            ));
        }

        if path == ["nz"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "nz expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::Nz,
                    args: vec![a0, a1],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["ta", "crossunder"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(
                    expr_span,
                    "ta.crossunder expects two arguments",
                ));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::TaCrossunder,
                    args: vec![a0, a1],
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Bool)),
                },
                expr_span,
            ));
        }

        if path == ["math", "max"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "math.max expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            let ty = HirType::Series(Type::Primitive(PrimitiveType::Float));
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathMax,
                    args: vec![a0, a1],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "min"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "math.min expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            let ty = HirType::Series(Type::Primitive(PrimitiveType::Float));
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathMin,
                    args: vec![a0, a1],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "abs"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.abs expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathAbs,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "sqrt"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.sqrt expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathSqrt,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "round"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.round expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathRound,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "log"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.log expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathLog,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "exp"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.exp expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathExp,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "pow"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "math.pow expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            let ty = self.expr_ty_from_session_or_float_series(call_expr);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathPow,
                    args: vec![a0, a1],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "ceil"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.ceil expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathCeil,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "floor"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.floor expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathFloor,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
        }

        if path == ["math", "trunc"] {
            if args.len() != 1 {
                return Err(HirLowerError::at(expr_span, "math.trunc expects one argument"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let ty = self.expr_ty_from_session_or_float_series(&args[0].1);
            return Ok(self.alloc_expr(
                HirExpr::BuiltinCall {
                    kind: BuiltinKind::MathTrunc,
                    args: vec![a0],
                    ty,
                },
                expr_span,
            ));
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

        if path.as_slice() == ["plot"] {
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

/// Dotted callee path (`ta.sma`) or method form (`close.sma`), plus the receiver expression for the latter.
fn callee_path_with_receiver(callee: &Expr) -> Result<(Vec<String>, Option<&Expr>), HirLowerError> {
    match &callee.kind {
        ExprKind::IdentPath(p) => Ok((p.clone(), None)),
        ExprKind::Member { base, field } => {
            let mut path = path_tail_from_expr(base.as_ref()).ok_or_else(|| {
                HirLowerError::at(
                    callee.span,
                    "invalid method callee (expected a simple path before `.`)",
                )
            })?;
            path.push(field.clone());
            Ok((path, Some(base.as_ref())))
        }
        _ => Err(HirLowerError::at(
            callee.span,
            "only path or member call forms are supported",
        )),
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

fn is_barmerge_financial_gaps_expr(e: &Expr) -> bool {
    let Some(p) = path_tail_from_expr(e) else {
        return false;
    };
    p.len() == 2 && p[0] == "barmerge" && (p[1] == "gaps_on" || p[1] == "gaps_off")
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

/// Drawing / alert calls accepted by typecheck but not emitted to HIR yet (skipped like comments).
fn is_unlowered_viz_stmt(e: &Expr) -> bool {
    let ExprKind::Call { callee, .. } = &e.kind else {
        return false;
    };
    let ExprKind::IdentPath(p) = &callee.kind else {
        return false;
    };
    p.len() == 1 && matches!(p[0].as_str(), "plotshape" | "fill" | "alertcondition")
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
    let title = args
        .iter()
        .find(|(n, _)| n.as_deref() == Some("title"))
        .and_then(|(_, ex)| match &ex.kind {
            ExprKind::String(s) => Some(s.clone()),
            _ => None,
        })
        .or_else(|| {
            args.get(1).and_then(|(_, ex)| match &ex.kind {
                ExprKind::String(s) => Some(s.clone()),
                _ => None,
            })
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

fn string_kw_first_string(args: &[(Option<String>, Expr)], key: &str) -> Option<String> {
    args
        .iter()
        .find(|(n, _)| n.as_deref() == Some(key))
        .and_then(|(_, ex)| match &ex.kind {
            ExprKind::String(s) => Some(s.clone()),
            _ => None,
        })
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

fn float_default_from_expr(e: &Expr) -> Result<f64, HirLowerError> {
    match &e.kind {
        ExprKind::Float(x) => Ok(*x),
        _ => {
            if let Some(x) = try_input_float_default(e) {
                return Ok(x);
            }
            Err(HirLowerError::at(
                e.span,
                "input float declaration default must be a float literal or input.float(float)",
            ))
        }
    }
}

fn try_input_float_default(e: &Expr) -> Option<f64> {
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &e.kind
    else {
        return None;
    };
    if type_args.is_some() {
        return None;
    }
    let path = match &callee.kind {
        ExprKind::IdentPath(p) => p.as_slice(),
        _ => return None,
    };
    if path != ["input", "float"] {
        return None;
    }
    if args.len() == 1 {
        return match &args[0].1.kind {
            ExprKind::Float(f) => Some(*f),
            _ => None,
        };
    }
    args.iter()
        .find(|(nm, _)| nm.as_deref() == Some("defval"))
        .and_then(|(_, ex)| match &ex.kind {
            ExprKind::Float(f) => Some(*f),
            _ => None,
        })
}

/// `input(title=…, defval=…)` at top level (int / bool stored as 0/1 / float).
fn try_input_plain_defval_for_decl(e: &Expr) -> Option<HirInputKind> {
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &e.kind
    else {
        return None;
    };
    if type_args.is_some() {
        return None;
    }
    let ExprKind::IdentPath(p) = &callee.kind else {
        return None;
    };
    if p.as_slice() != ["input"] {
        return None;
    }
    let (_, ex) = args
        .iter()
        .find(|(nm, _)| nm.as_deref() == Some("defval"))?;
    match &ex.kind {
        ExprKind::Int(i) => Some(HirInputKind::Int(*i)),
        ExprKind::Bool(b) => Some(HirInputKind::Int(if *b { 1 } else { 0 })),
        ExprKind::Float(f) => Some(HirInputKind::Float(*f)),
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

    const SAMPLE_UNARY_CMP_TERNARY: &str = r#"//@version=6
indicator("exprs")
a = 1.0
a += 2.0
b = -close
c = close > 1.0
plot(true ? b : 0.0)
"#;

    #[test]
    fn golden_unary_compare_ternary_pipeline() {
        let script = parse_script("test", SAMPLE_UNARY_CMP_TERNARY).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    const SAMPLE_UNARY_NOT: &str = r#"//@version=6
indicator("not")
lit = not true
cmp = not (close > 1.0)
plot(lit ? 1.0 : 0.0)
plot(cmp ? close : 0.0)
"#;

    #[test]
    fn golden_unary_not_pipeline() {
        let script = parse_script("test", SAMPLE_UNARY_NOT).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        assert_debug_snapshot!(c.session.hir.as_ref().expect("hir"));
    }

    const SAMPLE_ARRAY_LITERAL: &str = r#"//@version=6
indicator("arr")
x = [1.0, 2.0, 3.0]
plot(close)
"#;

    #[test]
    fn array_literal_lowers_to_hir_array_expr() {
        use crate::hir::HirExpr;

        let script = parse_script("test", SAMPLE_ARRAY_LITERAL).expect("parse");
        check_script(&script).expect("semantic checks");
        let c = crate::analyze_to_hir_compiler(&script).expect("analyze + hir");
        let hir = c.session.hir.as_ref().expect("hir");
        assert!(
            hir.exprs
                .iter()
                .any(|e| matches!(e, HirExpr::Array { .. })),
            "expected HirExpr::Array in expr arena"
        );
    }
}
