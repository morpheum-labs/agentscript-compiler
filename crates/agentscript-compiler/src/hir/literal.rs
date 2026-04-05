//! Constant values in HIR (no span; lowering attaches [`crate::hir::ty::HirType`]).
//!
//! **SRP:** literal payloads only.

#[derive(Debug, Clone, PartialEq)]
pub enum HirLiteral {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Color(u32),
}
