//! AgentScript (QAS) compiler front-end: parse → AST (typecheck + codegen later).
//!
//! Development progress: see repository root `ROADMAP.md`. Planned path: typechecker, IR,
//! codegen, and `wasm32` output aligned with the Aether strategy guest ABI.

mod ast;
mod error;
mod parser;

pub use ast::{
    AssignOp, BinOp, ElseBody, Expr, FnBody, FnDecl, FnParam, IfStmt, Item, PrimitiveType, Script,
    ScriptDeclaration, ScriptKind, Stmt, Type, UnaryOp, VarDecl, VarQualifier,
};
pub use error::CompileError;
pub use parser::script_parser;

use chumsky::Parser;
use chumsky::error::Simple;

/// Walk leading whitespace, `//` (non-version) lines, and `/* */` blocks; if the first `//@version=` line
/// is missing a number or not 1/6, return a parse error. (The main parser uses `or_not` on version, which
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
                if n != 1 && n != 6 {
                    return Some(Simple::custom(
                        num_start..num_end,
                        "unsupported //@version (QAS v1 allows only 1 or 6)",
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
