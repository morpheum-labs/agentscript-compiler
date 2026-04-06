//! Shared compiler state: allocation arena, name resolution side maps, last HIR snapshot.

use bumpalo::Bump;

use crate::bindings::NameBinding;
use crate::frontend::ast::{max_node_id, NodeId, Script};
use crate::hir::{HirScript, HirType};

/// Long-lived state for a compile session (arena-backed AST in later phases).
pub struct CompilerSession {
    pub arena: Bump,
    /// Filled by [`crate::semantic::passes::HirLowerPass`] when lowering succeeds for the script.
    pub hir: Option<HirScript>,
    /// Indexed by [`NodeId::0`]. Populated by [`crate::semantic::passes::lexical`] and [`crate::semantic::passes::resolver`].
    pub name_bindings: Vec<Option<NameBinding>>,
    /// Inferred [`HirType`] per expression node; filled by [`crate::semantic::passes::typecheck`].
    pub expr_types: Vec<Option<HirType>>,
}

impl CompilerSession {
    #[must_use]
    pub fn new() -> Self {
        Self {
            arena: Bump::new(),
            hir: None,
            name_bindings: Vec::new(),
            expr_types: Vec::new(),
        }
    }

    /// Clear arena and HIR (e.g. before recompiling another unit in the same session).
    pub fn reset(&mut self) {
        self.arena = Bump::new();
        self.hir = None;
        self.name_bindings.clear();
        self.expr_types.clear();
    }

    /// Allocate side maps for `script` (from [`max_node_id`]).
    pub fn prepare_analysis(&mut self, script: &Script) {
        let n = max_node_id(script) as usize;
        let len = n.saturating_add(1);
        self.name_bindings = vec![None; len];
        self.expr_types = vec![None; len];
    }

    /// Record resolution for an expression node (no-op if `id` is out of range).
    pub fn set_name_binding(&mut self, id: NodeId, binding: NameBinding) {
        let i = id.0 as usize;
        if i < self.name_bindings.len() {
            self.name_bindings[i] = Some(binding);
        }
    }

    /// Record inferred type for an expression node (no-op if `id` is out of range).
    pub fn set_expr_type(&mut self, id: NodeId, ty: HirType) {
        let i = id.0 as usize;
        if i < self.expr_types.len() {
            self.expr_types[i] = Some(ty);
        }
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
