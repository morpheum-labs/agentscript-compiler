//! Semantic passes after parsing (Phase 1 groundwork).
//!
//! Today: duplicate definitions, dotted-path roots, and `strategy.*` script-kind rules.
//! Later: full symbol tables and richer builtin signatures. A minimal typechecker runs after
//! resolution ([`crate::semantic::passes::typecheck`]).

use crate::frontend::ast::Script;

mod builtins;
pub mod passes;

pub use passes::{
    analyze_script, default_passes, lexical_resolve_script, resolve_script, typecheck_script,
    BreakContinuePass, CompilerPass, EarlyAnalyzePass, LexicalResolvePass, ResolverPass, TypecheckPass,
};

/// Semantic analysis failed (spans exist on the AST; richer reporting can use them later).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AnalyzeError {
    pub message: String,
}

/// Early checks + path / script-kind resolution via the default [`crate::compiler::Compiler`] pipeline.
pub fn check_script(script: &Script) -> Result<(), AnalyzeError> {
    crate::compiler::Compiler::new().run_semantic_passes(script)
}
