//! Shared compiler state: allocation arena, name resolution side maps, last HIR snapshot.

use std::collections::HashMap;

use bumpalo::Bump;

use crate::bindings::{NameBinding, SemanticSymbolId};
use crate::frontend::ast::{max_node_id, NodeId, Script, Span};
use crate::hir::{HirScript, HirType};

/// Definition site recorded by typecheck (mirrors lexical scope stack for tooling / LSP).
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticDefSite {
    pub def_span: Span,
    pub ty: HirType,
}

/// Long-lived state for a compile session (`Bump` backs the HIR expr pool during [`crate::semantic::passes::HirLowerPass`]; AST arena migration is later).
pub struct CompilerSession {
    pub arena: Bump,
    /// Filled by [`crate::semantic::passes::HirLowerPass`] when lowering succeeds for the script.
    pub hir: Option<HirScript>,
    /// Indexed by [`NodeId::0`]. Populated by [`crate::semantic::passes::lexical`] and [`crate::semantic::passes::resolver`].
    pub name_bindings: Vec<Option<NameBinding>>,
    /// Inferred [`HirType`] per expression node; filled by [`crate::semantic::passes::typecheck`].
    pub expr_types: Vec<Option<HirType>>,
    /// Typed binding stack aligned with typechecker scopes (innermost vector last).
    pub symbol_def_stack: Vec<HashMap<String, SemanticDefSite>>,
    /// Definition sites in **lexical walk order** (hoisted `fn`/`enum`/`type` names, then `walk_item`
    /// `define_*`). Used by HIR lowering to align [`SemanticSymbolId`] with [`crate::hir::SymbolId`].
    /// Import aliases are excluded (HIR does not lower imports yet).
    pub def_semantic_ids: Vec<SemanticSymbolId>,
}

impl CompilerSession {
    #[must_use]
    pub fn new() -> Self {
        Self {
            arena: Bump::new(),
            hir: None,
            name_bindings: Vec::new(),
            expr_types: Vec::new(),
            symbol_def_stack: Vec::new(),
            def_semantic_ids: Vec::new(),
        }
    }

    /// Clear arena and HIR (e.g. before recompiling another unit in the same session).
    pub fn reset(&mut self) {
        self.arena = Bump::new();
        self.hir = None;
        self.name_bindings.clear();
        self.expr_types.clear();
        self.symbol_def_stack.clear();
        self.def_semantic_ids.clear();
    }

    /// Allocate side maps for `script` (from [`max_node_id`]).
    pub fn prepare_analysis(&mut self, script: &Script) {
        let n = max_node_id(script) as usize;
        let len = n.saturating_add(1);
        self.name_bindings = vec![None; len];
        self.expr_types = vec![None; len];
        self.symbol_def_stack = vec![HashMap::new()];
        self.def_semantic_ids.clear();
    }

    /// Push a scope frame for typed symbol definitions (call with [`Self::push_symbol_scope`] from typecheck).
    pub fn push_symbol_scope(&mut self) {
        self.symbol_def_stack.push(HashMap::new());
    }

    pub fn pop_symbol_scope(&mut self) {
        let _ = self.symbol_def_stack.pop();
    }

    /// Record a binding in the innermost symbol scope (typecheck only).
    pub fn record_symbol_def(&mut self, span: Span, name: &str, ty: HirType) {
        if let Some(m) = self.symbol_def_stack.last_mut() {
            m.insert(
                name.to_string(),
                SemanticDefSite {
                    def_span: span,
                    ty,
                },
            );
        }
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
