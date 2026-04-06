//! Phase 2: HIR → WebAssembly (guest strategy module).
//!
//! v0 emits a **minimal valid** `wasm32` module with stable export names for the Aether / MWVM
//! guest ABI. Instruction bodies are stubs until lowering walks [`crate::hir::HirScript`].

mod wasm;

pub use wasm::emit_minimal_guest_wasm_v0;
