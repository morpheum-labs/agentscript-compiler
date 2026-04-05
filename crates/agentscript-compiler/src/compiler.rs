//! Thin orchestration: parse hook + ordered semantic passes.

use crate::frontend::ast::Script;
use crate::semantic::passes::{default_passes, CompilerPass};
use crate::semantic::AnalyzeError;
use crate::session::CompilerSession;

/// Configurable compiler driver (arena + pass list).
pub struct Compiler {
    pub session: CompilerSession,
    passes: Vec<Box<dyn CompilerPass>>,
}

impl Compiler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: CompilerSession::new(),
            passes: default_passes(),
        }
    }

    /// Replace the default Phase-0 semantic pipeline.
    pub fn with_passes(passes: Vec<Box<dyn CompilerPass>>) -> Self {
        Self {
            session: CompilerSession::new(),
            passes,
        }
    }

    /// Run all registered semantic passes.
    pub fn run_semantic_passes(&mut self, script: &Script) -> Result<(), AnalyzeError> {
        for p in &mut self.passes {
            p.run(&mut self.session, script)?;
        }
        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}
