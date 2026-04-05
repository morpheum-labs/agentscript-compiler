//! Script-level HIR: version, declaration, inputs, body, symbols.
//!
//! **SRP:** top-level program shape; no per-node logic.

use crate::frontend::ast::ScriptKind;

use super::stmt::HirStmt;
use super::symbols::SymbolTable;

#[derive(Debug, Clone, PartialEq)]
pub struct HirInputDecl {
    pub name: String,
    // default value, type, … — filled when lowering exists
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
    pub body: Vec<HirStmt>,
    pub symbols: SymbolTable,
}
