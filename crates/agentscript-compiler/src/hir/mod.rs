//! High-level IR after lowering (Phase 1+).
//!
//! Until a dedicated HIR is introduced, this aliases the concrete [`Script`] AST so passes and
//! drivers can already depend on a single “post-parse script” type.

pub type Script = crate::frontend::ast::Script;
