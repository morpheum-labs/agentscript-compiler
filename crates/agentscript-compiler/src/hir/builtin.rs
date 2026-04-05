//! Intrinsics that are not ordinary calls in codegen (`ta.sma`, `plot`, …).
//!
//! **OCP:** new builtins extend this enum (or migrate to a registry) without changing unrelated HIR nodes.

/// Builtin / intrinsic discriminant for [`super::expr::HirExpr::BuiltinCall`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinKind {
    TaSma,
    // Expand: TaEma, Plot, InputInt, …
}
