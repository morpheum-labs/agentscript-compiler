//! Errors produced while lowering [`crate::hir::HirScript`] to WebAssembly.

use crate::frontend::ast::Span;

/// Wasm codegen failed for this HIR; [`Self::span`] is the best source range (often the offending expression).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("HIR wasm: {message}")]
pub struct HirWasmError {
    pub message: String,
    pub span: Span,
}

impl HirWasmError {
    pub(crate) fn at(span: Span, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub(crate) fn dummy(message: impl Into<String>) -> Self {
        Self::at(Span::DUMMY, message)
    }
}
