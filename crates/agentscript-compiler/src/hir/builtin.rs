//! Intrinsics that are not ordinary calls in codegen (`ta.sma`, `plot`, …).
//!
//! **OCP:** new builtins extend this enum (or migrate to a registry) without changing unrelated HIR nodes.
//! Surface typing for dotted calls is driven by [`crate::semantic::builtin_registry`]; this enum only
//! tracks shapes that need dedicated HIR / wasm lowering.

/// Builtin / intrinsic discriminant for [`super::expr::HirExpr::BuiltinCall`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinKind {
    TaSma,
    TaEma,
    /// `input.int(default)` surface call (lowering may also record [`crate::hir::script::HirInputDecl`]).
    InputInt,
    /// `input.float(default)` — literal default in this lowering pass.
    InputFloat,
    /// Host compares current `(a,b)` to previous bar values; returns bool as f64.
    TaCrossover,
    TaCrossunder,
    MathMax,
    MathMin,
    MathAbs,
    /// True range series (`ta.tr`).
    TaTr,
    /// Average true range (`ta.atr(length)`).
    TaAtr,
    /// `nz(x, y)` — replacement value when `x` is na (host NaN policy).
    Nz,
}
