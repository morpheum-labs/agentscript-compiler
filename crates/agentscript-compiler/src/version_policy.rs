//! Pine-shaped `//@version=` rules (parser + leading-trivia preflight).
//!
//! TradingView Pine Script only defines **v5** and **v6** on this directive. AgentScript / QAS
//! source uses the same header as Pine so imports and tooling stay compatible; language or ABI
//! revisions are **not** expressed as `//@version=1` (that would collide with meaningless Pine
//! version numbers).

pub(crate) fn qas_version_allowed(n: u32) -> bool {
    matches!(n, 5 | 6)
}

pub(crate) fn qas_version_unsupported_message() -> &'static str {
    "unsupported //@version (only Pine 5 and 6 are accepted)"
}
