//! Linear orchestration: parse → semantic passes (incl. HIR) → guest WASM v0.

use crate::compile_script_to_wasm_v0;
use crate::parse_script;
use crate::CompileOrAnalyzeError;

/// Parse `source`, run the default semantic pipeline with HIR lowering, emit `wasm32` guest bytes.
pub fn compile_to_wasm(src_name: &str, source: &str) -> Result<Vec<u8>, CompileOrAnalyzeError> {
    let script = parse_script(src_name, source)?;
    compile_script_to_wasm_v0(&script).map_err(Into::into)
}
