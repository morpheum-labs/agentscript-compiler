//! Known Pine / QAS builtin namespace roots (first segment of `a.b` paths).
//!
//! Full signatures live in `pinescriptv6/reference/functions/`; this is only name-resolution glue.

use std::collections::HashSet;

/// First path segment accepted for dotted identifiers like `ta.sma`, `request.security`, `mcp.call`.
pub fn builtin_namespace_roots() -> HashSet<&'static str> {
    [
        "ta",
        "strategy",
        "math",
        "plot",
        "color",
        "input",
        "str",
        "array",
        "matrix",
        "map",
        "request",
        "syminfo",
        "timeframe",
        "barstate",
        "alert",
        "line",
        "label",
        "box",
        "table",
        "polyline",
        "linefill",
        "chart",
        "mcp",
        "session",
        "ticker",
        "dayofweek",
        "dividends",
        "earnings",
        "split",
        "text",
        "font",
        "size",
        "xloc",
        "yloc",
        "extend",
        "position",
        "shape",
        "display",
        "currency",
        "format",
    ]
    .into_iter()
    .collect()
}

/// Single-segment identifiers that resolve without a local binding (matches typecheck globals).
#[must_use]
pub fn is_unqualified_builtin_ident(name: &str) -> bool {
    matches!(
        name,
        "close" | "open" | "high" | "low" | "hl2" | "hlc3" | "ohlc4" | "hlcc4" | "volume"
            | "bar_index"
            | "time"
            | "timenow"
            | "true"
            | "false"
            // Bare `plot(x)` at statement / call sites (see typecheck `type_call`).
            | "plot"
            // Same for `nz(x, …)` (handled in `typecheck` for calls).
            | "nz"
    )
}
