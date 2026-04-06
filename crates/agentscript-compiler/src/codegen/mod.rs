//! Phase 2: HIR → WebAssembly (guest strategy module).
//!
//! [`emit_hir_guest_wasm`] walks [`crate::hir::HirScript`] and emits host imports + `on_bar` body.
//! [`emit_minimal_guest_wasm_v0`] remains a tiny stub for tests.

mod hir_wasm;
mod wasm;

pub use hir_wasm::{
    emit_hir_guest_wasm, HirWasmError, IMPORT_INPUT_INT, IMPORT_PLOT, IMPORT_REQUEST_SECURITY,
    IMPORT_SERIES_CLOSE, IMPORT_TA_SMA,
};
pub use wasm::emit_minimal_guest_wasm_v0;
