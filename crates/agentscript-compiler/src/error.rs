#![allow(unused_assignments)] // miette/thiserror derive triggers false positives on field reads

use chumsky::error::Simple;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::frontend::ast::Span;
use crate::semantic::AnalyzeError;

/// User-facing compiler error (parse phase today; typecheck/runtime later).
#[derive(Debug, Error, Diagnostic)]
#[error("failed to parse AgentScript source")]
pub struct CompileError {
    #[source_code]
    pub src: miette::NamedSource<String>,
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

/// Reading a source file from disk failed, or the file parsed with errors.
#[derive(Debug, Error)]
pub enum ParseFileError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

/// Semantic / type errors with optional source spans (miette-friendly).
#[derive(Debug, Error, Diagnostic)]
#[error("{summary}")]
pub struct AnalyzeCompileError {
    /// Concatenated diagnostic messages (stable for logs and plain Display).
    pub summary: String,
    #[source_code]
    pub src: miette::NamedSource<String>,
    #[related]
    pub labels: Vec<SemanticLabel>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct SemanticLabel {
    #[label]
    pub span: SourceSpan,
    pub message: String,
}

impl AnalyzeCompileError {
    #[must_use]
    pub fn from_analyze_error(src_name: impl AsRef<str>, source: String, err: AnalyzeError) -> Self {
        let summary = err
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let labels = err
            .diagnostics
            .into_iter()
            .filter_map(|d| {
                if d.span == Span::DUMMY {
                    return None;
                }
                let len = d.span.end.saturating_sub(d.span.start);
                if len == 0 {
                    return None;
                }
                Some(SemanticLabel {
                    span: (d.span.start, len).into(),
                    message: d.message,
                })
            })
            .collect();
        Self {
            summary,
            src: miette::NamedSource::new(src_name.as_ref(), source),
            labels,
        }
    }
}

pub(crate) fn compile_error_from_parse_errors(
    src_name: impl AsRef<str>,
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
        src: miette::NamedSource::new(src_name.as_ref(), source),
        labels,
    }
}
