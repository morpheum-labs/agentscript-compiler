//! Pluggable HIR → bytes backends (guest WASM v0 today).

use crate::hir::HirScript;

use super::hir_wasm::emit_hir_guest_wasm;
use super::wasm::error::HirWasmError;

/// Emit executable output from a lowered [`HirScript`].
pub trait HirCodegenBackend {
    /// Binary module or other artifact for the chosen target.
    fn emit(&self, hir: &HirScript) -> Result<Vec<u8>, HirWasmError>;
}

/// Aether guest strategy module: `aether` imports + `init` **`() -> i32`** / `on_bar` **`(i32) -> i32`** (guest ABI v1 exports).
#[derive(Debug, Clone, Copy, Default)]
pub struct GuestWasmV0;

impl HirCodegenBackend for GuestWasmV0 {
    fn emit(&self, hir: &HirScript) -> Result<Vec<u8>, HirWasmError> {
        emit_hir_guest_wasm(hir)
    }
}
