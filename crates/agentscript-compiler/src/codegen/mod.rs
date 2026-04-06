//! Phase 2: HIR → WebAssembly (guest strategy module).
//!
//! [`emit_hir_guest_wasm`] walks [`crate::hir::HirScript`] and emits host imports + `on_bar` body.
//! [`emit_minimal_guest_wasm_v0`] remains a tiny stub for tests.

mod hir_wasm;
mod wasm;

pub use hir_wasm::{
    emit_hir_guest_wasm, HirWasmError, GUEST_EXPORT_INIT_ABI, GUEST_EXPORT_INIT_LEGACY,
    GUEST_EXPORT_STEP_ABI, GUEST_EXPORT_STEP_LEGACY, IMPORT_SERIES_HIST,
};
pub use wasm::emit_minimal_guest_wasm_v0;
