//! Fluent builder for [`crate::driver::Compiler`] and the default semantic pass lists.
//!
//! **SRP:** this module only constructs the driver; parsing stays on [`crate::parse_script`], and
//! pass logic lives under [`crate::semantic::passes`] ([`crate::semantic::CompilerPass`] /
//! [`crate::semantic::SemanticPass`] seam).

mod compiler_pipeline;

pub use compiler_pipeline::CompilerPipeline;

use crate::CompileOrAnalyzeError;

/// Parse `source` and compile to guest WASM v0 (same as [`crate::compile_script_to_wasm_v0`] after [`crate::parse_script`]).
pub fn compile_to_wasm(src_name: &str, source: &str) -> Result<Vec<u8>, CompileOrAnalyzeError> {
    let script = crate::parse_script(src_name, source)?;
    crate::compile_script_to_wasm_v0(&script).map_err(CompileOrAnalyzeError::Analyze)
}

#[cfg(test)]
mod tests {
    use super::{compile_to_wasm, CompilerPipeline};
    use crate::{parse_script, session_hir};

    const TINY_INDICATOR: &str = r#"//@version=6
indicator("Test Agent")

len = input.int(14)
sma = ta.sma(close, len)
htf = request.security("AAPL", "D", sma)
plot(htf)
"#;

    #[test]
    fn compiler_pipeline_hir_smoke() {
        let script = parse_script("t", TINY_INDICATOR).expect("parse");
        let mut c = CompilerPipeline::new()
            .with_hir_lowering()
            .build();
        c.run_semantic_passes(&script).expect("pipeline");
        assert!(session_hir(&c).is_some());
    }

    #[test]
    fn compile_to_wasm_matches_tiny_indicator() {
        let wasm = compile_to_wasm("t", TINY_INDICATOR).expect("wasm");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }
}
