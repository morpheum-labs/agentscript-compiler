//! AgentScript (QAS) compiler front-end: parse → AST (typecheck + codegen later).
//!
//! Roadmap matches `spec/rust-implementation.md`: Chumsky parser, AST, then typechecker,
//! runtime, and WASM.

mod ast;
mod error;
mod parser;

pub use ast::{Item, Script};
pub use error::CompileError;
pub use parser::script_parser;

use chumsky::Parser;

/// Parse a full source file into a [`Script`].
pub fn parse_script(src_name: impl Into<String>, source: &str) -> Result<Script, CompileError> {
    let name = src_name.into();
    let owned = source.to_string();
    match script_parser().parse(owned.as_str()) {
        Ok(ast) => Ok(ast),
        Err(errs) => Err(error::compile_error_from_parse_errors(name, owned, errs)),
    }
}
