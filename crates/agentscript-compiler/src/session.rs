//! Shared compiler state: allocation arena and future source maps / symbol tables.

use bumpalo::Bump;

/// Long-lived state for a compile session (arena-backed AST in later phases).
pub struct CompilerSession {
    pub arena: Bump,
}

impl CompilerSession {
    #[must_use]
    pub fn new() -> Self {
        Self {
            arena: Bump::new(),
        }
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
