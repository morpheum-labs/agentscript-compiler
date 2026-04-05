//! Abstract syntax tree (Phase 1: script shell + declarations grow here).

/// Parsed AgentScript / QAS compilation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// `//@version=N` when present.
    pub version: Option<u32>,
    /// Top-level items (indicators, strategies, functions, etc.) — expanded in later phases.
    pub items: Vec<Item>,
}

impl Script {
    pub fn empty() -> Self {
        Self {
            version: None,
            items: Vec::new(),
        }
    }
}

/// Placeholder until the full grammar is wired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// Raw span preserved for forward-compatible parsing.
    Stub,
}
