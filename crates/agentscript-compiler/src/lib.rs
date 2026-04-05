//! AgentScript (QAS) compiler front-end: parse → AST (typecheck + codegen later).
//!
//! Development progress: see repository root `ROADMAP.md`. Planned path: typechecker, IR,
//! codegen, and `wasm32` output aligned with the Aether strategy guest ABI.

mod ast;
mod error;
mod parser;
mod version_policy;

pub use ast::{
    AssignOp, BinOp, ElseBody, ExportDecl, Expr, FnBody, FnDecl, FnParam, IfStmt, ImportDecl, Item,
    PrimitiveType, Script, ScriptDeclaration, ScriptKind, Stmt, Type, UnaryOp, VarDecl,
    VarQualifier,
};
pub use error::{CompileError, ParseFileError};
pub use parser::script_parser;

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
    if let Some(e) = parser::scan_leading_bad_directives(owned.as_str()) {
        return Err(error::compile_error_from_parse_errors(
            src_name,
            owned,
            vec![e],
        ));
    }
    match script_parser().parse(owned.as_str()) {
        Ok(ast) => Ok(ast),
        Err(errs) => Err(error::compile_error_from_parse_errors(
            src_name, owned, errs,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
