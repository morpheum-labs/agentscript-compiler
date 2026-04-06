//! HIR expression tree (normalized, desugared).
//!
//! **SRP:** expression variants only; statements live in [`super::stmt`].

use crate::frontend::ast::BinOp;

use super::builtin::BuiltinKind;
use super::ids::{HirId, SymbolId};
use super::literal::HirLiteral;
use super::security::SecurityCall;
use super::ty::HirType;

#[derive(Debug, Clone, PartialEq)]
pub enum HirExpr {
    Literal(HirLiteral, HirType),
    Variable(SymbolId, HirType),
    Binary {
        op: BinOp,
        lhs: HirId,
        rhs: HirId,
        ty: HirType,
    },
    BuiltinCall {
        kind: BuiltinKind,
        args: Vec<HirId>,
        ty: HirType,
    },
    /// Call to a user-defined function (symbol names the function in [`super::symbols::SymbolTable`]).
    UserCall {
        callee: SymbolId,
        args: Vec<HirId>,
        ty: HirType,
    },
    /// `close[1]`-style history access (offset from current bar).
    SeriesAccess {
        base: HirId,
        offset: i32,
        ty: HirType,
    },
    Security(Box<SecurityCall>),
    /// Inline plot when lowered as an expression-shaped construct (if the surface allows).
    Plot {
        expr: HirId,
        title: Option<String>,
    },
}
