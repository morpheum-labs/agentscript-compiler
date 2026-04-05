//! Name resolution output consumed by lowering and later phases.
//!
//! **SRP + DIP:** passes depend on this table abstraction, not on resolver internals.

/// Maps [`super::ids::SymbolId`] to display names and future metadata (readonly, series, …).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SymbolTable {
    pub names: Vec<String>,
}

impl SymbolTable {
    #[must_use]
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }

    pub fn push(&mut self, name: impl Into<String>) -> super::ids::SymbolId {
        let id = super::ids::SymbolId(self.names.len() as u32);
        self.names.push(name.into());
        id
    }

    #[must_use]
    pub fn name(&self, id: super::ids::SymbolId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }
}
