//! `request.financial`-shaped host request (symbol, financial id, period).

use super::ids::HirId;
use super::security::GapMode;
use super::ty::HirType;

/// Fundamental financial series request (not a plain `Call` node).
#[derive(Debug, Clone, PartialEq)]
pub struct FinancialCall {
    pub symbol: HirId,
    pub financial_id: HirId,
    pub period: HirId,
    /// Pine `gaps=` / 4th positional `barmerge.gaps_*` (default `gaps_off`).
    pub gaps: GapMode,
    /// Optional `ignore_invalid_symbol=` or positional bool after optional `gaps`.
    pub ignore_invalid_symbol: Option<HirId>,
    /// Optional `currency=` or last positional string (WASM v0: string literal only).
    pub currency: Option<HirId>,
    pub ty: HirType,
}
