//! Shared QAS / AgentScript `//@version=` rules (parser + preflight).

pub(crate) fn qas_version_allowed(n: u32) -> bool {
    matches!(n, 1 | 5 | 6)
}

pub(crate) fn qas_version_unsupported_message() -> &'static str {
    "unsupported //@version (QAS allows 1, 5, or 6)"
}
