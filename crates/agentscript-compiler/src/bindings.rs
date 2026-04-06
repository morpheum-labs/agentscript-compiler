//! Resolved name metadata for AST nodes (semantic layer, Pine/QAS parity path).
//!
//! **SRP:** binding kinds only. [`crate::session::CompilerSession`] stores
//! `Vec<Option<NameBinding>>` indexed by [`crate::frontend::ast::NodeId`].

/// Stable id for a user-defined binding (local, parameter, top-level def) within one compile unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SemanticSymbolId(pub u32);

/// Result of resolving an identifier use site (Phase 1: unqualified locals + builtins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameBinding {
    /// User-defined symbol (local, parameter, or hoisted top-level name).
    Local(SemanticSymbolId),
    /// Known single-segment builtin (`close`, `plot`, …).
    UnqualifiedBuiltin(String),
    /// Multi-segment path with a valid root (`ta.sma`, `strategy.long`, import `m.foo`, UDT `Side.buy`).
    QualifiedPath(String),
}
