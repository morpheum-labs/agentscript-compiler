//! Script-level HIR: version, declaration, inputs, body, symbols.
//!
//! **SRP:** top-level program shape; no per-node logic.

use std::collections::HashSet;
use std::fmt;

use crate::frontend::ast::{ScriptKind, Span};

use super::expr::HirExpr;
use super::ids::{HirId, SymbolId};
use super::stmt::HirStmt;
use super::symbols::SymbolTable;

#[derive(Debug, Clone, PartialEq)]
pub enum HirInputKind {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirInputDecl {
    pub name: String,
    pub kind: HirInputKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirDeclaration {
    Indicator {
        title: Option<String>,
        /// Pine `indicator(..., timeframe=...)` (empty string allowed).
        timeframe: Option<String>,
    },
    Strategy {
        title: Option<String>,
        timeframe: Option<String>,
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
    /// Script header span (`indicator` / `strategy` / `library` through closing `)`); used when an expression span is missing.
    pub source_span: Span,
    pub declaration: HirDeclaration,
    pub inputs: Vec<HirInputDecl>,
    /// Expression arena: [`super::ids::HirId`] indexes into this vector.
    pub exprs: Vec<HirExpr>,
    /// Source span per expression (same order/length as [`Self::exprs`]); codegen errors attach these.
    pub expr_spans: Vec<Span>,
    pub body: Vec<HirStmt>,
    pub user_functions: Vec<HirUserFunction>,
    pub symbols: SymbolTable,
    /// Symbols declared with `var` / `varip` (persist across `on_bar` via wasm globals).
    pub persist_symbols: HashSet<SymbolId>,
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
            .field("persist_symbols", &self.persist_symbols)
            .finish()
    }
}
