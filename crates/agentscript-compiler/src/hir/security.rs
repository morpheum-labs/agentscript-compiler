//! `request.security`-shaped context switch (symbol, timeframe, inner expression).
//!
//! **SRP:** security-call metadata only; shared by lowering and WASM codegen docs.

use super::ids::HirId;
use super::ty::HirType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GapMode {
    NoGaps,
    WithGaps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lookahead {
    Off,
    On,
}

/// First-class security request: not a plain `Call` node.
#[derive(Debug, Clone, PartialEq)]
pub struct SecurityCall {
    pub symbol: HirId,
    pub timeframe: HirId,
    pub expression: HirId,
    pub gaps: GapMode,
    pub lookahead: Lookahead,
    pub ty: HirType,
}
