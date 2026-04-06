//! AgentScript (QAS) compiler front-end: parse → AST → lightweight semantic checks; typecheck + codegen later.
//!
//! Development progress: see repository root `ROADMAP.md`. Planned path: full typechecker, IR,
//! codegen, and `wasm32` output aligned with the Aether strategy guest ABI.

mod bindings;
mod codegen;
mod compiler;
mod error;
mod frontend;
pub mod hir;
mod semantic;
mod session;
mod visitor;

pub use compiler::Compiler;
pub use bindings::{NameBinding, SemanticSymbolId};
pub use frontend::ast::{
    assign_node_ids, max_node_id, AssignOp, BinOp, ElseBody, EnumDef, EnumVariant, ExportDecl, Expr,
    ExprKind, FnBody, FnDecl, FnParam, ForInPattern, IfStmt, ImportDecl, Item, NodeId,
    PrimitiveType, Script, ScriptDeclaration, ScriptKind, Span, Spanned, Stmt, StmtKind, Type,
    UdtField, UnaryOp, UserTypeDef, VarDecl, VarQualifier,
};
pub use error::{AnalyzeCompileError, CompileError, ParseFileError};
pub use frontend::parser::script_parser;
pub use codegen::{emit_hir_guest_wasm, emit_minimal_guest_wasm_v0, HirWasmError};
pub use semantic::{
    analyze_script, check_script, default_passes, default_passes_with_hir, lexical_resolve_script,
    lexical_resolve_script_in_session, resolve_script, resolve_script_in_session, typecheck_script,
    typecheck_script_in_session, AnalyzeError, BreakContinuePass, CompilerPass, EarlyAnalyzePass,
    HirLowerPass, LexicalResolvePass, ResolverPass, SemanticDiagnostic, TypecheckPass,
};
pub use hir::{
    lower_script_to_hir, lower_script_to_hir_in_bump, lower_script_to_hir_in_bump_with_session,
    AstHirLowerer, HirLowerError, HirScript, HirType, LowerToHir,
};
pub use session::{CompilerSession, SemanticDefSite};
pub use visitor::AstVisitor;

use chumsky::Parser;
use std::fs;
use std::path::Path;

/// Filename extensions the compiler treats as AgentScript / QAS source (Pine v6–aligned syntax).
///
/// Matching is case-insensitive (e.g. `.PINE` is accepted).
pub const AGENTSCRIPT_SOURCE_EXTENSIONS: &[&str] = &["pine", "qas"];

/// `true` if `path` has a recognized source extension ([`AGENTSCRIPT_SOURCE_EXTENSIONS`]).
pub fn is_agentscript_source_path(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            AGENTSCRIPT_SOURCE_EXTENSIONS
                .iter()
                .any(|&s| ext.eq_ignore_ascii_case(s))
        })
}

/// Read a UTF-8 source file and parse it. `path` is shown in diagnostics (use a `.pine` or `.qas`
/// name for clarity in error reports).
pub fn parse_script_file(path: impl AsRef<Path>) -> Result<Script, ParseFileError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)?;
    let label = path.to_string_lossy();
    parse_script(label.as_ref(), &source).map_err(ParseFileError::Compile)
}

/// Parse a full source file into a [`Script`].
pub fn parse_script(src_name: impl AsRef<str>, source: &str) -> Result<Script, CompileError> {
    let owned = source.to_string();
    if let Some(e) = frontend::parser::scan_leading_bad_directives(owned.as_str()) {
        return Err(error::compile_error_from_parse_errors(
            src_name,
            owned,
            vec![e],
        ));
    }
    match script_parser().parse(owned.as_str()) {
        Ok(mut ast) => {
            crate::frontend::ast::assign_node_ids(&mut ast);
            Ok(ast)
        }
        Err(errs) => Err(error::compile_error_from_parse_errors(
            src_name, owned, errs,
        )),
    }
}

/// Parse and run [`check_script`] (early checks, resolver, minimal typecheck). For HIR + WASM use
/// [`Compiler::with_hir_lowering`] or [`compile_script_to_wasm_v0`].
pub fn parse_and_analyze(
    src_name: impl AsRef<str>,
    source: &str,
) -> Result<Script, CompileOrAnalyzeError> {
    let script = parse_script(src_name.as_ref(), source)?;
    check_script(&script).map_err(CompileOrAnalyzeError::Analyze)?;
    Ok(script)
}

/// Run [`Compiler::run_semantic_passes`] with the default semantic pipeline (no HIR).
pub fn analyze_to_compiler(script: &Script) -> Result<Compiler, AnalyzeError> {
    let mut c = Compiler::new();
    c.run_semantic_passes(script)?;
    Ok(c)
}

/// Semantic passes plus HIR lowering; on success [`CompilerSession::hir`] is set.
pub fn analyze_to_hir_compiler(script: &Script) -> Result<Compiler, AnalyzeError> {
    let mut c = Compiler::with_hir_lowering();
    c.run_semantic_passes(script)?;
    Ok(c)
}

/// Borrow the lowered [`HirScript`] after a successful [`analyze_to_hir_compiler`] (or any
/// [`Compiler::with_hir_lowering`] run that completed without error).
#[must_use]
pub fn session_hir(compiler: &Compiler) -> Option<&HirScript> {
    compiler.session.hir.as_ref()
}

/// Lower + emit a guest `wasm32` module (`memory`, `init`, `on_bar`) using [`codegen::emit_hir_guest_wasm`].
/// Requires the current HIR subset; wasm emit errors map to [`AnalyzeError`].
pub fn compile_script_to_wasm_v0(script: &Script) -> Result<Vec<u8>, AnalyzeError> {
    let mut c = Compiler::with_hir_lowering();
    c.run_semantic_passes(script)?;
    let hir = c
        .session
        .hir
        .as_ref()
        .expect("hir present after successful hir lowering");
    codegen::emit_hir_guest_wasm(hir).map_err(|e| AnalyzeError::single(e.message, e.span))
}

/// Parse failure ([`CompileError`]) or post-parse semantic failure ([`AnalyzeError`]).
#[derive(Debug, thiserror::Error)]
pub enum CompileOrAnalyzeError {
    #[error(transparent)]
    Parse(#[from] CompileError),
    #[error(transparent)]
    Analyze(#[from] AnalyzeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TINY_INDICATOR: &str = r#"//@version=6
indicator("Test Agent")

len = input.int(14)
sma = ta.sma(close, len)
htf = request.security("AAPL", "D", sma)
plot(htf)
"#;

    #[test]
    fn compile_script_to_wasm_v0_smoke() {
        let script = parse_script("t", TINY_INDICATOR).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("compile");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }

    #[test]
    fn session_hir_set_after_analyze_to_hir_compiler() {
        let script = parse_script("t", TINY_INDICATOR).expect("parse");
        let c = analyze_to_hir_compiler(&script).expect("hir");
        assert!(session_hir(&c).is_some());
    }

    #[test]
    fn wasmtime_accepts_compiled_tiny_indicator_module() {
        use wasmtime::{Engine, Module};

        let script = parse_script("t", TINY_INDICATOR).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("compile");
        let engine = Engine::default();
        Module::new(&engine, &wasm).expect("wasmtime parses and compiles module");
    }

    #[test]
    fn compile_wasm_v0_user_call_smoke() {
        const SRC: &str = r#"//@version=6
indicator("x")
f(float x) => x
plot(f(close))
"#;
        let script = parse_script("t", SRC).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("user-call wasm");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }

    #[test]
    fn compile_wasm_v0_block_user_fn_smoke() {
        const SRC: &str = r#"//@version=6
indicator("x")
g(float a) {
  t = a * 2.0
  t
}
plot(g(close))
"#;
        let script = parse_script("t", SRC).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("block user-fn wasm");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }

    #[test]
    fn compile_wasm_v0_unary_compare_ternary_smoke() {
        const SRC: &str = r#"//@version=6
indicator("exprs")
a = 1.0
a += 2.0
b = -close
c = close > 1.0
plot(true ? b : 0.0)
"#;
        let script = parse_script("t", SRC).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("unary/compare/ternary wasm");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }

    /// Contract: imports (`aether`, …), dual exports (`init` + `aether_strategy_init`, etc.), and `series_hist` when HIR uses `close[k]`.
    #[test]
    fn compile_script_to_wasm_v0_guest_abi_contract() {
        use wasmparser::{Parser, Payload};

        const WITH_CLOSE_HIST: &str = r#"//@version=6
indicator("t")
plot(close[1])
"#;

        let script = parse_script("t", WITH_CLOSE_HIST).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("compile");
        wasmparser::validate(&wasm).expect("valid wasm module");

        let mut imports: Vec<(String, String)> = Vec::new();
        let mut exports: Vec<String> = Vec::new();
        for payload in Parser::new(0).parse_all(&wasm) {
            let Ok(p) = payload else { continue };
            match p {
                Payload::ImportSection(reader) => {
                    for imp in reader {
                        let Ok(i) = imp else { continue };
                        imports.push((i.module.to_string(), i.name.to_string()));
                    }
                }
                Payload::ExportSection(reader) => {
                    for exp in reader {
                        let Ok(e) = exp else { continue };
                        exports.push(e.name.to_string());
                    }
                }
                _ => {}
            }
        }

        for (module, name) in [
            ("aether", "series_close"),
            ("aether", "input_int"),
            ("aether", "ta_sma"),
            ("aether", "request_security"),
            ("aether", "plot"),
            ("aether", "series_hist"),
            ("aether", "ta_ema"),
        ] {
            assert!(
                imports.iter().any(|(m, n)| m == module && n == name),
                "missing import `{module}::{name}`, have {imports:?}"
            );
        }

        for name in [
            "memory",
            "init",
            "on_bar",
            "aether_strategy_init",
            "aether_strategy_step",
        ] {
            assert!(
                exports.iter().any(|e| e == name),
                "missing export `{name}`, have {exports:?}"
            );
        }
    }

    /// Regression: `ta.ema` lowering must emit a validating module (includes `ta_ema` import).
    #[test]
    fn compile_ta_ema_sample_to_wasm_validates() {
        const SAMPLE_EMA: &str = r#"//@version=6
indicator("EMA")
len = input.int(14)
ema = ta.ema(close, len)
plot(ema)
"#;
        let script = parse_script("t", SAMPLE_EMA).expect("parse");
        let wasm = compile_script_to_wasm_v0(&script).expect("compile");
        wasmparser::validate(&wasm).expect("valid wasm module");
    }

    #[test]
    fn agentscript_source_extensions_recognize_pine_and_qas_case_insensitive() {
        assert!(is_agentscript_source_path("x.pine"));
        assert!(is_agentscript_source_path("x.PINE"));
        assert!(is_agentscript_source_path("x.qas"));
        assert!(is_agentscript_source_path(PathBuf::from("dir/strat.pine")));
        assert!(!is_agentscript_source_path("x.rs"));
        assert!(!is_agentscript_source_path("pine")); // no extension
    }

    #[test]
    fn parse_script_file_accepts_pine_extension() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("agentscript_test_{nanos}.pine"));
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "//@version=6\nindicator(\"t\")").unwrap();
        drop(f);
        assert!(is_agentscript_source_path(&p));
        let script = parse_script_file(&p).expect("valid .pine should parse");
        let Item::ScriptDecl(ScriptDeclaration {
            kind: ScriptKind::Indicator,
            ..
        }) = &script.items[0]
        else {
            panic!("expected indicator decl");
        };
        let _ = std::fs::remove_file(&p);
    }
}
