//! HIR statements (let, control flow, strategy commands, …).
//!
//! **SRP:** statement shapes; expressions referenced by [`super::ids::HirId`].

use super::ids::{HirId, SymbolId};

#[derive(Debug, Clone, PartialEq)]
pub enum HirStmt {
    Let {
        symbol: SymbolId,
        value: HirId,
    },
    Plot {
        expr: HirId,
        title: Option<String>,
    },
    /// Conditional execution (`if` / `else if` chains lower to nested [`HirStmt::If`] in `else_branch`).
    If {
        cond: HirId,
        then_branch: Vec<HirStmt>,
        else_branch: Option<Vec<HirStmt>>,
    },
    Block(Vec<HirStmt>),
    /// `var` / `varip`: initializer runs once; value lives in a wasm global across bars.
    VarInit {
        symbol: SymbolId,
        value: HirId,
    },
}
