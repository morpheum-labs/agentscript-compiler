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

/// Generated from [`data/builtins.json`](../../data/builtins.json) by `build.rs`. Edit JSON and rebuild.
mod generated {
    include!(concat!(env!("OUT_DIR"), "/builtin_entries_gen.rs"));
}

pub use generated::BUILTIN_ENTRIES;

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