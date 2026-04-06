//! Pipeline driver: owns [`CompilerSession`] and an ordered list of semantic passes.
//!
//! Parsing stays in [`crate::parse_script`]; this module only **coordinates** analysis after AST construction.

use crate::frontend::ast::Script;
use crate::semantic::passes::{default_passes, default_passes_with_hir, CompilerPass};
use crate::semantic::AnalyzeError;
use crate::session::CompilerSession;

/// Configurable compiler driver: shared session state plus the pass pipeline.
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

    /// Semantic pipeline plus [`crate::semantic::passes::HirLowerPass`] (AST → HIR for the supported subset).
    #[must_use]
    pub fn with_hir_lowering() -> Self {
        Self {
            session: CompilerSession::new(),
            passes: default_passes_with_hir(),
        }
    }

    /// Run all registered semantic passes.
    pub fn run_semantic_passes(&mut self, script: &Script) -> Result<(), AnalyzeError> {
        self.session.hir = None;
        self.session.prepare_analysis(script);
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
