//! Minimal `wasm32` module shape (guest ABI v0 stub).

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection,
};

/// Emit a valid WebAssembly module with guest exports `init` and `on_bar` (both `( ) -> ()`).
///
/// This is the Phase 2 **scaffold**: same bytes for any successfully lowered script until
/// `HirScript` drives real codegen.
#[must_use]
pub fn emit_minimal_guest_wasm_v0() -> Vec<u8> {
    let mut module = Module::new();

    let mut types = TypeSection::new();
    let ty_idx = types.len();
    types.ty().function([], []);

    let mut functions = FunctionSection::new();
    functions.function(ty_idx);
    functions.function(ty_idx);

    let mut exports = ExportSection::new();
    exports.export("init", ExportKind::Func, 0);
    exports.export("on_bar", ExportKind::Func, 1);

    let mut code = CodeSection::new();
    let empty = || {
        let mut f = Function::new([]);
        f.instruction(&Instruction::End);
        f
    };
    code.function(&empty());
    code.function(&empty());

    module.section(&types);
    module.section(&functions);
    module.section(&exports);
    module.section(&code);

    module.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmparser::{Parser, Payload};

    #[test]
    fn minimal_module_validates_and_exports_init_on_bar() {
        let wasm = emit_minimal_guest_wasm_v0();
        let mut saw_init = false;
        let mut saw_on_bar = false;
        for payload in Parser::new(0).parse_all(&wasm) {
            let Ok(p) = payload else { continue };
            if let Payload::ExportSection(reader) = p {
                for exp in reader {
                    let Ok(e) = exp else { continue };
                    match e.name {
                        "init" => saw_init = true,
                        "on_bar" => saw_on_bar = true,
                        _ => {}
                    }
                }
            }
        }
        assert!(saw_init && saw_on_bar, "exports init + on_bar");
        wasmparser::validate(&wasm).expect("wasm-encoder output should validate");
    }
}
