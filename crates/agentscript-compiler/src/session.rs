//! Shared compiler state: allocation arena, name resolution side maps, last HIR snapshot.

use std::collections::HashMap;

use bumpalo::Bump;

use crate::bindings::{NameBinding, SemanticSymbolId};
use crate::frontend::ast::{max_node_id, NodeId, Script, Span};
use crate::hir::{HirScript, HirType};

/// Inferred signature for a top-level or exported library function (params + return after typecheck).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryExportFnSig {
    pub params: Vec<HirType>,
    pub ret: HirType,
}

/// Read linked `import` library export signatures supplied by the host ([`CompilerSession::linked_library_exports`]).
pub trait LibraryExportsRead {
    fn library_export_fn_sig(&self, import_alias: &str, member: &str) -> Option<LibraryExportFnSig>;

    /// True if the host registered a library for this import alias ([`crate::register_import_library`]).
    fn import_library_is_linked(&self, import_alias: &str) -> bool;
}

/// Sink for the typechecker to record top-level function signatures after inference.
pub trait TypecheckedFnSigSink {
    fn replace_typechecked_fn_sigs(&mut self, m: HashMap<String, LibraryExportFnSig>);
}

/// Narrow interface for passes that only record qualified-path resolutions.
pub trait NameBindingSink {
    fn set_name_binding(&mut self, id: NodeId, binding: NameBinding);
}

/// Narrow interface for passes that only store inferred expression types.
pub trait ExprTypeSink {
    fn set_expr_type(&mut self, id: NodeId, ty: HirType);
}

/// Read inferred types after typecheck (e.g. return-type inference over a block).
pub trait ExprTypesRead {
    fn expr_types(&self) -> &[Option<HirType>];
}

/// Typed definition scopes (minimal typechecker surface).
pub trait SymbolDefRecorder {
    fn push_symbol_scope(&mut self);
    fn pop_symbol_scope(&mut self);
    fn record_symbol_def(&mut self, span: Span, name: &str, ty: HirType);
}

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
    /// `import_alias` → (`export_name` → signature). Filled by [`crate::register_import_library`].
    pub linked_library_exports: HashMap<String, HashMap<String, LibraryExportFnSig>>,
    /// `import_alias` → lowered library [`HirScript`] (from the same registration). Used to splice
    /// `alias.member(...)` calls into the consumer HIR as merged user functions.
    pub linked_library_hir: HashMap<String, HirScript>,
    /// Top-level function signatures after the last successful [`typecheck_script_in_session`] on this session.
    pub typechecked_fn_sigs: HashMap<String, LibraryExportFnSig>,
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
            linked_library_exports: HashMap::new(),
            linked_library_hir: HashMap::new(),
            typechecked_fn_sigs: HashMap::new(),
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
        self.linked_library_exports.clear();
        self.linked_library_hir.clear();
        self.typechecked_fn_sigs.clear();
    }

    /// Allocate side maps for `script` (from [`max_node_id`]).
    pub fn prepare_analysis(&mut self, script: &Script) {
        let n = max_node_id(script) as usize;
        let len = n.saturating_add(1);
        self.name_bindings = vec![None; len];
        self.expr_types = vec![None; len];
        self.symbol_def_stack = vec![HashMap::new()];
        self.def_semantic_ids.clear();
        self.typechecked_fn_sigs.clear();
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

impl NameBindingSink for CompilerSession {
    fn set_name_binding(&mut self, id: NodeId, binding: NameBinding) {
        CompilerSession::set_name_binding(self, id, binding);
    }
}

impl ExprTypeSink for CompilerSession {
    fn set_expr_type(&mut self, id: NodeId, ty: HirType) {
        CompilerSession::set_expr_type(self, id, ty);
    }
}

impl ExprTypesRead for CompilerSession {
    fn expr_types(&self) -> &[Option<HirType>] {
        &self.expr_types
    }
}

impl SymbolDefRecorder for CompilerSession {
    fn push_symbol_scope(&mut self) {
        CompilerSession::push_symbol_scope(self);
    }

    fn pop_symbol_scope(&mut self) {
        CompilerSession::pop_symbol_scope(self);
    }

    fn record_symbol_def(&mut self, span: Span, name: &str, ty: HirType) {
        CompilerSession::record_symbol_def(self, span, name, ty);
    }
}

impl LibraryExportsRead for CompilerSession {
    fn library_export_fn_sig(&self, import_alias: &str, member: &str) -> Option<LibraryExportFnSig> {
        self.linked_library_exports
            .get(import_alias)?
            .get(member)
            .cloned()
    }

    fn import_library_is_linked(&self, import_alias: &str) -> bool {
        self.linked_library_exports.contains_key(import_alias)
    }
}

impl TypecheckedFnSigSink for CompilerSession {
    fn replace_typechecked_fn_sigs(&mut self, m: HashMap<String, LibraryExportFnSig>) {
        self.typechecked_fn_sigs = m;
    }
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}
