use chumsky::error::Simple;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// User-facing compiler error (parse phase today; typecheck/runtime later).
#[derive(Debug, Error, Diagnostic)]
#[error("failed to parse AgentScript source")]
pub struct CompileError {
    #[source_code]
    pub source: miette::NamedSource<String>,
    #[related]
    pub labels: Vec<ParseLabel>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct ParseLabel {
    #[label]
    pub span: SourceSpan,
    pub message: String,
}

pub(crate) fn compile_error_from_parse_errors(
    src_name: impl Into<String>,
    source: String,
    errs: Vec<Simple<char>>,
) -> CompileError {
    let labels = errs
        .into_iter()
        .filter_map(|e| {
            let span = e.span();
            let range = span.start..span.end;
            if range.start >= range.end {
                return None;
            }
            Some(ParseLabel {
                span: (range.start, range.end - range.start).into(),
                message: e.to_string(),
            })
        })
        .collect();
    CompileError {
        source: miette::NamedSource::new(src_name, source),
        labels,
    }
}
