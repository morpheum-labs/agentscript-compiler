//! AST → HIR lowering for a **small** supported subset (indicator body: inputs, `ta.sma`,
//! `request.security`, `plot`).
//!
//! Run [`check_script`](crate::semantic::check_script) before lowering so resolution / early rules
//! have already run.

use std::collections::{HashMap, HashSet};

use crate::frontend::ast::{
    AssignOp, Expr, ExprKind, Item, PrimitiveType, Script, ScriptDeclaration, ScriptKind, Stmt,
    StmtKind, Type, VarQualifier,
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
#[error("HIR lowering: {0}")]
pub struct HirLowerError(pub String);

impl HirLowerError {
    fn unsupported(msg: impl Into<String>) -> Self {
        Self(msg.into())
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
                Item::Import(_) | Item::Export(_) | Item::FnDecl(_) | Item::Enum(_) | Item::TypeDef(_) => {
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
                    return Err(HirLowerError::unsupported(
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
                Err(HirLowerError::unsupported(
                    "only `plot(...)` expression statements are supported in this pass",
                ))
            }
            _ => Err(HirLowerError::unsupported(
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
            ExprKind::IdentPath(path) => self.lower_ident_path(path),
            ExprKind::Call {
                callee,
                type_args,
                args,
            } => {
                if type_args.is_some() {
                    return Err(HirLowerError::unsupported(
                        "generic calls are not supported in this HIR lowering pass",
                    ));
                }
                self.lower_call(callee.as_ref(), args)
            }
            _ => Err(HirLowerError::unsupported(
                "expression kind not supported by this HIR lowering pass",
            )),
        }
    }

    fn lower_ident_path(&mut self, path: &[String]) -> Result<HirId, HirLowerError> {
        if path.len() == 1 {
            let name = &path[0];
            let id = *self.names.get(name.as_str()).ok_or_else(|| {
                HirLowerError::unsupported(format!("unknown identifier `{name}`"))
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
        Err(HirLowerError::unsupported(format!(
            "qualified identifier `{}` not supported",
            path.join(".")
        )))
    }

    fn lower_call(
        &mut self,
        callee: &Expr,
        args: &[(Option<String>, Expr)],
    ) -> Result<HirId, HirLowerError> {
        let path = match &callee.kind {
            ExprKind::IdentPath(p) => p.as_slice(),
            _ => {
                return Err(HirLowerError::unsupported(
                    "only simple path callees are supported",
                ));
            }
        };

        if path == ["input", "int"] {
            if args.len() != 1 {
                return Err(HirLowerError::unsupported("input.int expects one argument"));
            }
            let n = match &args[0].1.kind {
                ExprKind::Int(i) => *i,
                _ => {
                    return Err(HirLowerError::unsupported(
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
                return Err(HirLowerError::unsupported("ta.sma expects two arguments"));
            }
            let a0 = self.lower_expr(&args[0].1)?;
            let a1 = self.lower_expr(&args[1].1)?;
            return Ok(self.alloc_expr(HirExpr::BuiltinCall {
                kind: BuiltinKind::TaSma,
                args: vec![a0, a1],
                ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
            }));
        }

        if path == ["request", "security"] {
            if args.len() != 3 {
                return Err(HirLowerError::unsupported(
                    "request.security expects three arguments in this pass",
                ));
            }
            let sym = self.lower_expr(&args[0].1)?;
            let tf = self.lower_expr(&args[1].1)?;
            let inner = self.lower_expr(&args[2].1)?;
            let sec = SecurityCall {
                symbol: sym,
                timeframe: tf,
                expression: inner,
                gaps: GapMode::NoGaps,
                lookahead: Lookahead::Off,
                ty: HirType::Series(Type::Primitive(PrimitiveType::Float)),
            };
            return Ok(self.alloc_expr(HirExpr::Security(Box::new(sec))));
        }

        if path == ["plot"] {
            return Err(HirLowerError::unsupported(
                "use `plot(expr)` as a statement, not as a nested expression",
            ));
        }

        Err(HirLowerError::unsupported(format!(
            "call `{}` not supported",
            path.join(".")
        )))
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
        return Err(HirLowerError::unsupported("plot needs at least one argument"));
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
            Err(HirLowerError::unsupported(
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
}
