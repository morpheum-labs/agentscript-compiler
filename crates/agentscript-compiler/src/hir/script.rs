//! Script-level HIR: version, declaration, inputs, body, symbols.
//!
//! **SRP:** top-level program shape; no per-node logic.

use crate::frontend::ast::ScriptKind;

use super::expr::HirExpr;
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

#[derive(Debug, Clone, PartialEq)]
pub struct HirScript {
    pub version: u32,
    pub declaration: HirDeclaration,
    pub inputs: Vec<HirInputDecl>,
    /// Expression arena: [`super::ids::HirId`] indexes into this vector.
    pub exprs: Vec<HirExpr>,
    pub body: Vec<HirStmt>,
    pub symbols: SymbolTable,
}
