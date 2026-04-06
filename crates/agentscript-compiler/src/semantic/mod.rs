//! Semantic passes after parsing (Phase 1 groundwork).
//!
//! **DIP / ISP:** the pluggable seam is [`CompilerPass`] (also exported as [`SemanticPass`]); the
//! driver holds `Vec<Box<dyn CompilerPass>>` and does not hard-code pass order beyond defaults.
//!
//! Today: duplicate definitions, dotted-path roots, `strategy.*` script-kind rules, lexical binding
//! for unqualified identifiers ([`crate::semantic::passes::lexical`]), and a minimal typechecker
//! ([`crate::semantic::passes::typecheck`]). Diagnostics carry [`Span`] when the AST provides it.

use std::fmt;

use crate::frontend::ast::Script;
use crate::frontend::ast::Span;

pub mod builtin_registry;
mod builtins;
pub mod passes;

pub use passes::{
    analyze_script, default_passes, default_passes_with_hir, lexical_resolve_script,
    lexical_resolve_script_in_session, resolve_script, resolve_script_in_session, typecheck_script,
    typecheck_script_in_session, BreakContinuePass, CompilerPass, EarlyAnalyzePass, HirLowerPass,
    LexicalResolvePass, ResolverPass, TypecheckPass,
};

/// Second name for [`CompilerPass`] (semantic analysis pipeline step).
pub use passes::CompilerPass as SemanticPass;

/// One semantic issue at a concrete source range (or [`Span::DUMMY`] when unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub message: String,
    pub span: Span,
}

/// Semantic analysis failed. May contain multiple [`SemanticDiagnostic`] entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzeError {
    pub diagnostics: Vec<SemanticDiagnostic>,
}

impl AnalyzeError {
    #[must_use]
    pub fn new(diagnostics: Vec<SemanticDiagnostic>) -> Self {
        Self { diagnostics }
    }

    #[must_use]
    pub fn single(message: impl Into<String>, span: Span) -> Self {
        Self {
            diagnostics: vec![SemanticDiagnostic {
                message: message.into(),
                span,
            }],
        }
    }

    /// Concatenated messages (stable for tests and simple CLIs).
    #[must_use]
    pub fn message(&self) -> String {
        self.diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, d) in self.diagnostics.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            if d.span == Span::DUMMY {
                write!(f, "{}", d.message)?;
            } else {
                write!(f, "[{}..{}] {}", d.span.start, d.span.end, d.message)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for AnalyzeError {}

/// Early checks + semantic pipeline via the default [`crate::Compiler`] driver.
pub fn check_script(script: &Script) -> Result<(), AnalyzeError> {
    crate::Compiler::new().run_semantic_passes(script)
}
