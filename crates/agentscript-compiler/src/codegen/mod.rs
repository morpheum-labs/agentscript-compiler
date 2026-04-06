//! Phase 2: HIR → WebAssembly (guest strategy module).
//!
//! [`emit_hir_guest_wasm`] walks [`crate::hir::HirScript`] and emits host imports + `on_bar` body.
//! [`emit_minimal_guest_wasm_v0`] remains a tiny stub for tests. Guest import/export contracts are
//! checked by [`wasm::abi::validate_guest_abi_v0`].
//!
//! Higher-level emission goes through [`HirCodegenBackend`] (e.g. [`GuestWasmV0`]) so additional
//! backends can be added without changing the semantic driver.

mod backend;
mod builtin_wasm_emit;
mod hir_wasm;
mod wasm;

pub use backend::{GuestWasmV0, HirCodegenBackend};
pub use hir_wasm::emit_hir_guest_wasm;
pub use wasm::abi::{
    validate_guest_abi_v0, AbiValidationError, GuestAbiV0, GUEST_ABI_V0_EXPORTS,
    GUEST_ABI_V0_IMPORTS,
};
pub use wasm::error::HirWasmError;
pub use wasm::emit_minimal_guest_wasm_v0;
