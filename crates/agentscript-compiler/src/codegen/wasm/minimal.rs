//! Minimal `wasm32` module shape (guest ABI scaffold for tests).

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
};

/// Emit a valid WebAssembly module with `init` **`() -> i32`** and `on_bar` **`(i32) -> i32`**
/// (guest ABI v1 export shapes; no `aether` imports — not a full strategy module).
#[must_use]
pub fn emit_minimal_guest_wasm_v0() -> Vec<u8> {
    let mut module = Module::new();

    let mut types = TypeSection::new();
    let ty_init = types.len();
    types.ty().function([], [ValType::I32]);
    let ty_step = types.len();
    types.ty().function([ValType::I32], [ValType::I32]);

    let mut functions = FunctionSection::new();
    functions.function(ty_init);
    functions.function(ty_step);

    let mut exports = ExportSection::new();
    exports.export("init", ExportKind::Func, 0);
    exports.export("on_bar", ExportKind::Func, 1);

    let mut code = CodeSection::new();
    let mut init = Function::new([]);
    init.instruction(&Instruction::I32Const(0));
    init.instruction(&Instruction::End);
    code.function(&init);

    // `(i32 bar_index) -> i32`: parameter is local 0; operand stack starts empty.
    let mut step = Function::new([]);
    step.instruction(&Instruction::I32Const(0));
    step.instruction(&Instruction::End);
    code.function(&step);

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
