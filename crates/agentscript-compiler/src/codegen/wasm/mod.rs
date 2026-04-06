//! WebAssembly helpers: minimal stub module, guest ABI v0 metadata / validation, and wasm codegen errors.

pub mod abi;
pub mod error;

mod minimal;

pub use minimal::emit_minimal_guest_wasm_v0;
