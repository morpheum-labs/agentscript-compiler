//! AST → HIR lowering for a **small** supported subset (indicator body: inputs, `ta.sma` /
//! `ta.ema`, `request.security`, `plot`).
//!
//! Run [`check_script`](crate::semantic::check_script) before lowering so resolution / early rules
//! have already run.

use std::collections::{HashMap, HashSet};

use crate::frontend::ast::{
    AssignOp, BinOp, Expr, ExprKind, Item, PrimitiveType, Script, ScriptDeclaration, ScriptKind,
    Span, Stmt, StmtKind, Type, VarQualifier,
};

use super::builtin::BuiltinKind;
use super::expr::HirExpr;
use super::ids::HirId;
use super::literal::HirLiteral;
use super::lowering::LowerToHir;
use super::script::{HirDeclaration, HirInputDecl, HirScript};
use super::security::{GapMode, Lookahead, SecurityCall};
use super::stmt::HirStmt;
use super::symbols::SymbolTable;
use super::ty::HirType;

/// Lowering failed: construct not in the supported subset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("HIR lowering: {message}")]
pub struct HirLowerError {
    pub message: String,
    pub span: Span,
}

impl HirLowerError {
    fn unsupported(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            span: Span::DUMMY,
        }
    }

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

/// Parse-time script → HIR for the supported subset.
pub fn lower_script_to_hir(script: &Script) -> Result<HirScript, HirLowerError> {
    let mut lower = LowerCtx::new();
    lower.lower_script(script)
}

struct LowerCtx {
    exprs: Vec<HirExpr>,
    symbols: SymbolTable,
    names: HashMap<String, super::ids::SymbolId>,
    /// Names introduced by `input.int` / `input int` (typed as simple int in HIR).
    input_int_names: HashSet<String>,
}

impl LowerCtx {
    fn new() -> Self {
        Self {
            exprs: Vec::new(),
            symbols: SymbolTable::new(),
            names: HashMap::new(),
            input_int_names: HashSet::new(),
        }
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

    fn alloc_expr(&mut self, e: HirExpr) -> HirId {
        let id = HirId(self.exprs.len() as u32);
        self.exprs.push(e);
        id
    }

    fn expr_hir_type(&self, id: HirId) -> HirType {
        match &self.exprs[id.0 as usize] {
            HirExpr::Literal(_, ty) => ty.clone(),
            HirExpr::Variable(_, ty) => ty.clone(),
            HirExpr::Binary { ty, .. } => ty.clone(),
            HirExpr::BuiltinCall { ty, .. } => ty.clone(),
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
                Item::Export(_) | Item::FnDecl(_) | Item::Enum(_) | Item::TypeDef(_) => {
                    return Err(HirLowerError::unsupported(
                        "only indicator/strategy declarations and statements are supported in this HIR lowering pass",
                    ));
                }
            }
        }

        Ok(HirScript {
            version,
            declaration,
            inputs,
            exprs: std::mem::take(&mut self.exprs),
            body,
            symbols: std::mem::take(&mut self.symbols),
        })
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
            ScriptKind::Library => Err(HirLowerError::unsupported(
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
        match &stmt.kind {
            StmtKind::VarDecl(v) => {
                if v.qualifier == Some(VarQualifier::Input) {
                    let def = int_default_from_expr(&v.value)?;
                    inputs.push(HirInputDecl {
                        name: v.name.clone(),
                        default_int: def,
                    });
                    self.register_input_int(&v.name);
                    self.intern_name(&v.name);
                    return Ok(());
                }
                if v.qualifier.is_some() || v.ty.is_some() {
                    return Err(HirLowerError::at(
                        v.span,
                        "only plain `name = expr` or `input …` declarations are supported",
                    ));
                }
                let sym = self.intern_name(&v.name);
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
                    self.intern_name(name);
                    return Ok(());
                }
                let sym = self.intern_name(name);
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
            _ => Err(HirLowerError::at(
                stmt.span,
                "statement kind not supported by this HIR lowering pass",
            )),
        }
    }

    fn lower_expr(&mut self, e: &Expr) -> Result<HirId, HirLowerError> {
        match &e.kind {
            ExprKind::Int(i) => Ok(self.alloc_expr(HirExpr::Literal(
                HirLiteral::Int(*i),
                HirType::Simple(Type::Primitive(PrimitiveType::Int)),
            ))),
            ExprKind::Float(f) => Ok(self.alloc_expr(HirExpr::Literal(
                HirLiteral::Float(*f),
                HirType::Simple(Type::Primitive(PrimitiveType::Float)),
            ))),
            ExprKind::String(s) => Ok(self.alloc_expr(HirExpr::Literal(
                HirLiteral::String(s.clone()),
                HirType::Simple(Type::Primitive(PrimitiveType::String)),
            ))),
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
                Ok(self.alloc_expr(HirExpr::SeriesAccess {
                    base: base_id,
                    offset,
                    ty,
                }))
            }
            ExprKind::IdentPath(path) => self.lower_ident_path(path, e.span),
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
                Ok(self.alloc_expr(HirExpr::Binary {
                    op: *op,
                    lhs,
                    rhs,
                    ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
                }))
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

    fn lower_ident_path(&mut self, path: &[String], span: Span) -> Result<HirId, HirLowerError> {
        if path.len() == 1 {
            let name = &path[0];
            let id = *self.names.get(name.as_str()).ok_or_else(|| {
                HirLowerError::at(span, format!("unknown identifier `{name}`"))
            })?;
            let ty = if name == "close" {
                HirType::Series(Type::Primitive(PrimitiveType::Float))
            } else if self.input_int_names.contains(name) {
                HirType::Simple(Type::Primitive(PrimitiveType::Int))
            } else {
                HirType::Series(Type::Primitive(PrimitiveType::Float))
            };
            return Ok(self.alloc_expr(HirExpr::Variable(id, ty)));
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
            let lit = self.alloc_expr(HirExpr::Literal(
                HirLiteral::Int(n),
                HirType::Simple(Type::Primitive(PrimitiveType::Int)),
            ));
            return Ok(self.alloc_expr(HirExpr::BuiltinCall {
                kind: BuiltinKind::InputInt,
                args: vec![lit],
                ty: HirType::Simple(Type::Primitive(PrimitiveType::Int)),
            }));
        }

        if path == ["ta", "sma"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "ta.sma expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(HirExpr::BuiltinCall {
                kind: BuiltinKind::TaSma,
                args: vec![a0, a1],
                ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
            }));
        }

        if path == ["ta", "ema"] {
            if args.len() != 2 {
                return Err(HirLowerError::at(expr_span, "ta.ema expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(HirExpr::BuiltinCall {
                kind: BuiltinKind::TaEma,
                args: vec![a0, a1],
                ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
            }));
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
            let sec = SecurityCall {
                symbol: sym,
                timeframe: tf,
                expression: inner,
                gaps,
                lookahead,
                ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
            };
            return Ok(self.alloc_expr(HirExpr::Security(Box::new(sec))));
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
    use super::*;
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
        let hir = lower_script_to_hir(&script).expect("lower");
        assert_debug_snapshot!(hir);
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
        let hir = lower_script_to_hir(&script).expect("lower");
        assert_debug_snapshot!(hir);
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
        let hir = lower_script_to_hir(&script).expect("lower");
        assert_debug_snapshot!(hir);
    }
}
