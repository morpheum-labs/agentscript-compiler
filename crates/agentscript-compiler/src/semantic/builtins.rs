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
