//! **Source of truth** for the AgentScript / QAS Chumsky parser: trivia, literals, types, expressions, script grammar, and leading-trivia checks.

mod assign_type;
mod expr;
mod leading_scan;
mod lex;
mod literals;
mod script;

pub use script::script_parser;

pub(crate) use leading_scan::scan_leading_bad_directives;
