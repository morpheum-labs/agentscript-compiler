//! One-shot embedder API ([`WasmCompiler`]). Session-based workflows still use [`crate::driver::Compiler`].

use crate::pipeline::compile_to_wasm;
use crate::CompileOrAnalyzeError;

/// Minimal façade: compile a full script string to validated guest WASM.
pub struct WasmCompiler;

impl WasmCompiler {
    /// Same as [`crate::pipeline::compile_to_wasm`] with `src_name` `"main"`.
    pub fn compile(source: &str) -> Result<Vec<u8>, CompileOrAnalyzeError> {
        compile_to_wasm("main", source)
    }
}
