//! **Dependency inversion:** consumers of lowering (tests, drivers, future codegen) depend on
//! [`LowerToHir`], not on a single concrete visitor implementation.
//!
//! The semantic pipeline continues to extend via [`crate::semantic::passes::CompilerPass`]; a future
//! `HirLowerer` can implement both that trait and this one, or delegate to a type that implements
//! [`LowerToHir`].

use crate::frontend::ast::Script;

use super::script::HirScript;

/// Produces a [`HirScript`] from the analyzed surface AST (after resolver + typecheck in later phases).
pub trait LowerToHir {
    type Err;
    fn lower(&mut self, script: &Script) -> Result<HirScript, Self::Err>;
}
