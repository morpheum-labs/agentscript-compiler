//! Type annotations on HIR nodes after typechecking.
//!
//! **SRP:** HIR-level type shape (`Simple` vs `Series`) separate from AST surface syntax.

use crate::frontend::ast::Type as AstType;

/// Fully inferred type for codegen: every value is either a per-bar scalar/simple value or a series.
#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Simple(AstType),
    Series(AstType),
    /// Homogeneous array (`[a, b]` or `float[]`): element type carries series/simple per Pine rules.
    Array(Box<HirType>),
    /// Row-major matrix type surface (`matrix<float>`): element series/simple like arrays.
    Matrix(Box<HirType>),
}
