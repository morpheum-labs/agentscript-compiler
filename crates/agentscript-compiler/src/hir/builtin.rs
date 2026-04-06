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
    /// Square root (`math.sqrt`); emitted as wasm `f64.sqrt` (no host import).
    MathSqrt,
    /// `math.round` → wasm `f64.nearest`.
    MathRound,
    /// `math.log` — host `(f64) -> f64` (natural log; NaN policy on host).
    MathLog,
    /// `math.exp` — host `(f64) -> f64`.
    MathExp,
    /// `math.pow` — host `(f64, f64) -> f64`.
    MathPow,
    /// `math.ceil` → wasm `f64.ceil`.
    MathCeil,
    /// `math.floor` → wasm `f64.floor`.
    MathFloor,
    /// `math.trunc` → wasm `f64.trunc`.
    MathTrunc,
    /// True range series (`ta.tr`).
    TaTr,
    /// Average true range (`ta.atr(length)`).
    TaAtr,
    /// `nz(x, y)` — replacement value when `x` is na (host NaN policy).
    Nz,
    /// `syminfo.ticker` — series string; wasm only as `request.security` arg (host fills scratch via `series_string_utf8`).
    SyminfoTicker,
    /// `syminfo.prefix` — series string; same restrictions as [`SyminfoTicker`].
    SyminfoPrefix,
}
