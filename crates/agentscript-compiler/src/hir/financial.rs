//! `request.financial`-shaped host request (symbol, financial id, period).

use super::ids::HirId;
use super::ty::HirType;

/// Fundamental financial series request (not a plain `Call` node).
#[derive(Debug, Clone, PartialEq)]
pub struct FinancialCall {
    pub symbol: HirId,
    pub financial_id: HirId,
    pub period: HirId,
    /// Optional `ignore_invalid_symbol=` or positional 4th argument when lowered.
    pub ignore_invalid_symbol: Option<HirId>,
    pub ty: HirType,
}
