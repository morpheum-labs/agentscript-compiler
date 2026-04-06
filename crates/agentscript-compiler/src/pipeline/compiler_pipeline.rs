use crate::driver::Compiler;
use crate::semantic::passes::{default_passes, default_passes_with_hir, CompilerPass};

/// Builder for [`Compiler`] with the same defaults as [`Compiler::new`] and
/// [`Compiler::with_hir_lowering`].
#[derive(Default)]
pub struct CompilerPipeline {
    passes: Option<Vec<Box<dyn CompilerPass>>>,
}

impl CompilerPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self { passes: None }
    }

    /// Use [`default_passes`] (semantic analysis only; no HIR).
    #[must_use]
    pub fn with_default_semantic(mut self) -> Self {
        self.passes = Some(default_passes());
        self
    }

    /// Semantic passes plus [`crate::semantic::passes::HirLowerPass`].
    #[must_use]
    pub fn with_hir_lowering(mut self) -> Self {
        self.passes = Some(default_passes_with_hir());
        self
    }

    /// Full control over pass order and contents.
    #[must_use]
    pub fn with_passes(mut self, passes: Vec<Box<dyn CompilerPass>>) -> Self {
        self.passes = Some(passes);
        self
    }

    #[must_use]
    pub fn build(self) -> Compiler {
        match self.passes {
            Some(p) => Compiler::with_passes(p),
            None => Compiler::new(),
        }
    }
}
