//! Known Pine / QAS builtin namespace roots (first segment of `a.b` paths).
//!
//! Roots are merged from [`super::builtin_registry`] (data-driven, codegen-ready) plus static
//! namespaces that exist in Pine but are not yet listed in the registry.

use std::collections::HashSet;

use super::builtin_registry::namespace_roots_from_registry;

/// First path segment accepted for dotted identifiers like `ta.sma`, `request.security`, `mcp.call`.
pub fn builtin_namespace_roots() -> HashSet<&'static str> {
    let mut s = namespace_roots_from_registry();
    for root in [
        "barmerge",
        "strategy",
        "plot",
        "color",
        "input",
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
    ] {
        s.insert(root);
    }
    s
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
