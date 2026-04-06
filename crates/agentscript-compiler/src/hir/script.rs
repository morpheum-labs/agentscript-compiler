//! Script-level HIR: version, declaration, inputs, body, symbols.
//!
//! **SRP:** top-level program shape; no per-node logic.

use std::fmt;

use crate::frontend::ast::{ScriptKind, Span};

use super::expr::HirExpr;
use super::ids::{HirId, SymbolId};
use super::stmt::HirStmt;
use super::symbols::SymbolTable;

#[derive(Debug, Clone, PartialEq)]
pub struct HirInputDecl {
    pub name: String,
    /// `input.int` / `input int` — minimal subset uses int defaults only.
    pub default_int: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirDeclaration {
    Indicator {
        title: Option<String>,
    },
    Strategy {
        title: Option<String>,
    },
    Library,
    /// Raw kind until lowering normalizes title extraction.
    FromAst(ScriptKind),
}

/// Lowered user-defined function (`f name(...) =>` or block body). [`SymbolId`] matches [`super::expr::HirExpr::UserCall`] callee.
#[derive(Debug, Clone, PartialEq)]
pub struct HirUserFunction {
    pub symbol: SymbolId,
    pub params: Vec<SymbolId>,
    /// Statements before the returned expression (block body); empty for `=>` expr bodies.
    pub body_stmts: Vec<HirStmt>,
    /// Result value (Pine-style last expression or inferred default).
    pub result: HirId,
}

#[derive(Clone, PartialEq)]
pub struct HirScript {
    pub version: u32,
    pub declaration: HirDeclaration,
    pub inputs: Vec<HirInputDecl>,
    /// Expression arena: [`super::ids::HirId`] indexes into this vector.
    pub exprs: Vec<HirExpr>,
    /// Source span per expression (same order/length as [`Self::exprs`]); codegen errors attach these.
    pub expr_spans: Vec<Span>,
    pub body: Vec<HirStmt>,
    pub user_functions: Vec<HirUserFunction>,
    pub symbols: SymbolTable,
}

impl fmt::Debug for HirScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Omit `expr_spans` so debug snapshots stay stable; spans are for diagnostics / wasm errors.
        f.debug_struct("HirScript")
            .field("version", &self.version)
            .field("declaration", &self.declaration)
            .field("inputs", &self.inputs)
            .field("exprs", &self.exprs)
            .field("body", &self.body)
            .field("user_functions", &self.user_functions)
            .field("symbols", &self.symbols)
            .finish()
    }
}
