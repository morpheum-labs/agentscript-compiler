//! Declarative builtin metadata — **single source of truth** for “known” dotted calls.
//!
//! **Follow-up:** replace [`BUILTIN_ENTRIES`] with generated output from `PinescriptV6-docs-crawler/`
//! (or an equivalent doc scrape) once that pipeline emits Rust or JSON that the build can import.
//! Until then, grow this curated slice only when new typecheck / HIR tests need real signatures.

use crate::frontend::ast::{PrimitiveType, Type as AstType};
use crate::hir::HirType;

/// Result shape for a registry builtin (Pine-style: most indicators are series float).
#[allow(dead_code)] // Variants reserved for future registry rows from codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinResultKind {
    SeriesFloat,
    SeriesInt,
    SeriesBool,
    SimpleFloat,
    SimpleInt,
    SimpleBool,
    SimpleString,
}

impl BuiltinResultKind {
    #[must_use]
    pub fn to_hir(self) -> HirType {
        match self {
            BuiltinResultKind::SeriesFloat => {
                HirType::Series(AstType::Primitive(PrimitiveType::Float))
            }
            BuiltinResultKind::SeriesInt => HirType::Series(AstType::Primitive(PrimitiveType::Int)),
            BuiltinResultKind::SeriesBool => {
                HirType::Series(AstType::Primitive(PrimitiveType::Bool))
            }
            BuiltinResultKind::SimpleFloat => {
                HirType::Simple(AstType::Primitive(PrimitiveType::Float))
            }
            BuiltinResultKind::SimpleInt => HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            BuiltinResultKind::SimpleBool => HirType::Simple(AstType::Primitive(PrimitiveType::Bool)),
            BuiltinResultKind::SimpleString => {
                HirType::Simple(AstType::Primitive(PrimitiveType::String))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinEntry {
    pub dotted_name: &'static str,
    pub min_args: usize,
    pub result: BuiltinResultKind,
    /// When set, the typechecker applies moving-average rules on `arg_tys[0]` and `arg_tys[1]`.
    pub moving_average: bool,
    /// `math.pow(a,b)`-style: both numeric, result uses Pine promotion (series if either operand is series).
    pub binary_numeric: bool,
    /// `math.round(x)`-style: one numeric arg; preserves simple vs series.
    pub unary_numeric: bool,
    /// `str.tonumber(s)`: one string arg → float scalar.
    pub unary_string_to_float: bool,
    /// `ta.crossover(a,b)`: both bool-like (simple or series).
    pub bool_binary: bool,
    /// If result is [`BuiltinResultKind`] simple primitive and any argument is series-shaped, lift to series.
    pub series_from_args: bool,
}

/// Replace this slice with generated output from the docs crawler when wired up.
pub static BUILTIN_ENTRIES: &[BuiltinEntry] = &[
    BuiltinEntry {
        dotted_name: "ta.sma",
        min_args: 2,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: true,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.ema",
        min_args: 2,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: true,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.wma",
        min_args: 2,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: true,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.rma",
        min_args: 2,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: true,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.rsi",
        min_args: 2,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: true,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.macd",
        min_args: 4,
        result: BuiltinResultKind::SeriesFloat,
        moving_average: false,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.crossover",
        min_args: 2,
        result: BuiltinResultKind::SeriesBool,
        moving_average: false,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: true,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "ta.crossunder",
        min_args: 2,
        result: BuiltinResultKind::SeriesBool,
        moving_average: false,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: true,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "math.pow",
        min_args: 2,
        result: BuiltinResultKind::SimpleFloat,
        moving_average: false,
        binary_numeric: true,
        unary_numeric: false,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: false,
    },
    BuiltinEntry {
        dotted_name: "math.round",
        min_args: 1,
        result: BuiltinResultKind::SimpleFloat,
        moving_average: false,
        binary_numeric: false,
        unary_numeric: true,
        unary_string_to_float: false,
        bool_binary: false,
        series_from_args: true,
    },
    BuiltinEntry {
        dotted_name: "str.tonumber",
        min_args: 1,
        result: BuiltinResultKind::SimpleFloat,
        moving_average: false,
        binary_numeric: false,
        unary_numeric: false,
        unary_string_to_float: true,
        bool_binary: false,
        series_from_args: false,
    },
];

#[must_use]
pub fn lookup_dotted(name: &str) -> Option<&'static BuiltinEntry> {
    BUILTIN_ENTRIES.iter().find(|e| e.dotted_name == name)
}

/// Namespace roots (`ta`, `math`, …) derived from registry plus static Pine/QAS-only roots.
#[must_use]
pub fn namespace_roots_from_registry() -> std::collections::HashSet<&'static str> {
    use std::collections::HashSet;
    let mut s = HashSet::new();
    for e in BUILTIN_ENTRIES {
        if let Some(seg) = e.dotted_name.split('.').next() {
            s.insert(seg);
        }
    }
    s
}