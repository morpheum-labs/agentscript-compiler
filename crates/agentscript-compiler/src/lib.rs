//! AgentScript (QAS) compiler front-end: parse → AST (typecheck + codegen later).
//!
//! Development progress: see repository root `ROADMAP.md`. Planned path: typechecker, IR,
//! codegen, and `wasm32` output aligned with the Aether strategy guest ABI.

mod ast;
mod error;
mod parser;

pub use ast::{
    AssignOp, BinOp, ElseBody, ExportDecl, Expr, FnBody, FnDecl, FnParam, IfStmt, ImportDecl, Item,
    PrimitiveType, Script, ScriptDeclaration, ScriptKind, Stmt, Type, UnaryOp, VarDecl,
    VarQualifier,
};
pub use error::{CompileError, ParseFileError};
pub use parser::script_parser;

use chumsky::Parser;
use chumsky::error::Simple;
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

/// Walk leading whitespace, `//` (non-version) lines, and `/* */` blocks; if the first `//@version=` line
/// is missing a number or not 1/5/6, return a parse error. (The main parser uses `or_not` on version, which
/// would otherwise swallow `try_map` failures after the digits were consumed.)
fn scan_leading_bad_version_directive(source: &str) -> Option<Simple<char>> {
    let mut i = 0usize;
    let b = source.as_bytes();
    while i < b.len() {
        let c = b[i];
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
            continue;
        }
        if i + 1 < b.len() && c == b'/' && b[i + 1] == b'/' {
            let line_start = i;
            let mut j = i + 2;
            while j < b.len() && b[j] != b'\n' && b[j] != b'\r' {
                j += 1;
            }
            let line = source.get(line_start..j).unwrap_or("");
            if let Some(_rest) = line.strip_prefix("//@version=") {
                let num_start = line_start + "//@version=".len();
                let mut num_end = num_start;
                for ch in source.get(num_start..j)?.chars() {
                    if ch.is_ascii_digit() {
                        num_end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if num_end == num_start {
                    return Some(Simple::custom(
                        num_start..num_start.saturating_add(1).min(source.len()),
                        "missing version number after //@version=",
                    ));
                }
                let digits = source.get(num_start..num_end).unwrap_or("0");
                let n: u32 = digits.parse().unwrap_or(0);
                if n != 1 && n != 5 && n != 6 {
                    return Some(Simple::custom(
                        num_start..num_end,
                        "unsupported //@version (QAS allows 1, 5, or 6)",
                    ));
                }
                return None;
            }
            i = j;
            if i < b.len() && b[i] == b'\r' {
                i += 1;
            }
            if i < b.len() && b[i] == b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < b.len() && c == b'/' && b[i + 1] == b'*' {
            i += 2;
            let mut closed = false;
            while i + 1 < b.len() {
                if b[i] == b'*' && b[i + 1] == b'/' {
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                break;
            }
            continue;
        }
        break;
    }
    None
}

/// Parse a full source file into a [`Script`].
pub fn parse_script(src_name: impl AsRef<str>, source: &str) -> Result<Script, CompileError> {
    let owned = source.to_string();
    if let Some(e) = scan_leading_bad_version_directive(owned.as_str()) {
        return Err(error::compile_error_from_parse_errors(
            src_name,
            owned,
            vec![e],
        ));
    }
    match script_parser().parse(owned.as_str()) {
        Ok(ast) => Ok(ast),
        Err(errs) => Err(error::compile_error_from_parse_errors(src_name, owned, errs)),
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
