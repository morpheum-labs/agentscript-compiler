//! Stable node handles into the HIR arena (`HirId`) and resolved names (`SymbolId`).
//!
//! **SRP:** identifiers only; no expression or type semantics here.

/// Index into the HIR expression / statement arena (bumpalo or `Vec`, owned by the builder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirId(pub u32);

/// Resolved binding or builtin slot in [`super::symbols::SymbolTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub u32);
