//! Semantic passes after parsing (Phase 1 groundwork).
//!
//! Today: duplicate definitions, dotted-path roots, and `strategy.*` script-kind rules.
//! Later: full symbol tables, types, builtin signatures.

use crate::Script;

mod builtins;
mod early;
mod resolve;

pub use early::analyze_script;
pub use resolve::resolve_script;

/// Semantic analysis failed (no source spans on the AST yet; messages are textual).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AnalyzeError {
    pub message: String,
}

/// Early checks + path / script-kind resolution.
pub fn check_script(script: &Script) -> Result<(), AnalyzeError> {
    early::analyze_script(script)?;
    resolve::resolve_script(script)?;
    Ok(())
}
