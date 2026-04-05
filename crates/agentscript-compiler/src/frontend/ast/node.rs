//! Source spans and lightweight wrappers for the AST.

use std::ops::Range;

/// Byte offsets into the original source (`start` inclusive, `end` exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Placeholder when no source range is available (e.g. tests building AST by hand).
    pub const DUMMY: Self = Self {
        start: 0,
        end: 0,
    };

    #[must_use]
    pub fn merge(a: Self, b: Self) -> Self {
        Self {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

impl From<Range<usize>> for Span {
    fn from(r: Range<usize>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

/// Attach a span to an arbitrary payload (used by visitors and future arena migration).
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    #[must_use]
    pub fn new(span: impl Into<Span>, value: T) -> Self {
        Self {
            span: span.into(),
            value,
        }
    }
}
