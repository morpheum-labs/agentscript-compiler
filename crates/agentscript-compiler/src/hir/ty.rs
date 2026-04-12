//! Type annotations on HIR nodes after typechecking.
//!
//! **SRP:** HIR-level type shape (`Simple` vs `Series`) separate from AST surface syntax.
//! Assignability, numeric promotion, and branch unification for the typechecker live here so
//! [`crate::semantic::passes::typecheck`] stays orchestration-heavy, not rule-heavy.
//!
//! Runtime `na` is not modeled in [`HirType`] yet; equality and comparisons follow the minimal
//! rules documented in `spec/hir.md` until a dedicated `na` lattice lands.

use crate::frontend::ast::PrimitiveType;
use crate::frontend::ast::Type as AstType;

/// Fully inferred type for codegen: every value is either a per-bar scalar/simple value or a series.
#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Simple(AstType),
    Series(AstType),
    /// Homogeneous array (`[a, b]` or `float[]`): element type carries series/simple per Pine rules.
    Array(Box<HirType>),
    /// Row-major matrix type surface (`matrix<float>`): element series/simple like arrays.
    Matrix(Box<HirType>),
    /// `import … as alias` namespace (not a TV string; use for bare-alias typing until linking supplies members).
    ImportNamespace { alias: String },
}

#[inline]
#[must_use]
pub fn is_series_shape(t: &HirType) -> bool {
    t.is_series_shape()
}

impl HirType {
    #[must_use]
    pub fn is_series_shape(&self) -> bool {
        matches!(self, Self::Series(_))
    }

    #[must_use]
    pub fn is_simple_shape(&self) -> bool {
        matches!(self, Self::Simple(_) | Self::ImportNamespace { .. })
    }

    /// Element type for array/matrix wrappers; scalars return `None`.
    #[must_use]
    pub fn element_type(&self) -> Option<&HirType> {
        match self {
            Self::Array(inner) | Self::Matrix(inner) => Some(inner.as_ref()),
            _ => None,
        }
    }
}

#[must_use]
pub fn is_numeric(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float))
            | HirType::Series(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float))
    )
}

#[must_use]
pub fn is_integral(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Int))
            | HirType::Series(AstType::Primitive(PrimitiveType::Int))
    )
}

#[must_use]
pub fn is_bool_like(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::Bool))
            | HirType::Series(AstType::Primitive(PrimitiveType::Bool))
    )
}

#[must_use]
pub fn is_stringish(t: &HirType) -> bool {
    matches!(
        t,
        HirType::Simple(AstType::Primitive(PrimitiveType::String))
            | HirType::Series(AstType::Primitive(PrimitiveType::String))
    )
}

#[must_use]
pub fn coerce_simple_to_series(t: HirType) -> HirType {
    match t {
        HirType::Simple(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(p)),
        o => o,
    }
}

#[must_use]
pub fn promote_numeric_series(t: HirType) -> HirType {
    match t {
        HirType::Simple(AstType::Primitive(PrimitiveType::Int)) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        HirType::Series(AstType::Primitive(PrimitiveType::Int)) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        o => o,
    }
}

#[must_use]
pub fn numeric_prim(t: &HirType) -> Option<PrimitiveType> {
    match t {
        HirType::Simple(AstType::Primitive(p)) | HirType::Series(AstType::Primitive(p)) => {
            Some(*p)
        }
        _ => None,
    }
}

/// Pine-style numeric binops: if either operand is a series, the result is a series; `int`×`int` simple stays `int` unless float appears.
pub fn binary_numeric_result(l: &HirType, r: &HirType) -> Result<HirType, String> {
    if !is_numeric(l) || !is_numeric(r) {
        return Err("numeric operator expects numeric operands".into());
    }
    let series = l.is_series_shape() || r.is_series_shape();
    let pl = numeric_prim(l).unwrap();
    let pr = numeric_prim(r).unwrap();
    let prim = match (pl, pr) {
        (PrimitiveType::Float, _) | (_, PrimitiveType::Float) => PrimitiveType::Float,
        _ => PrimitiveType::Int,
    };
    Ok(if series {
        HirType::Series(AstType::Primitive(prim))
    } else {
        HirType::Simple(AstType::Primitive(prim))
    })
}

/// Assignment and parameter passing: `series float` accepts `simple int/float` etc.
#[must_use]
pub fn assignable(from: &HirType, to: &HirType) -> bool {
    if from == to {
        return true;
    }
    match (from, to) {
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (
            HirType::Series(AstType::Primitive(PrimitiveType::Int)),
            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Int)),
            HirType::Series(AstType::Primitive(PrimitiveType::Int | PrimitiveType::Float)),
        ) => true,
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Float)),
            HirType::Series(AstType::Primitive(PrimitiveType::Float)),
        ) => true,
        (
            HirType::Simple(AstType::Primitive(PrimitiveType::Color)),
            HirType::Series(AstType::Primitive(PrimitiveType::Color)),
        ) => true,
        (HirType::Array(f), HirType::Array(t)) => assignable(f, t),
        (HirType::Matrix(f), HirType::Matrix(t)) => assignable(f, t),
        (
            &HirType::ImportNamespace { alias: ref a },
            &HirType::ImportNamespace { alias: ref b },
        ) => a == b,
        _ => false,
    }
}

/// Equality operand compatibility; see `spec/hir.md` (“Typing notes: equality and `na`”).
#[must_use]
pub fn type_compatible_eq(a: &HirType, b: &HirType) -> bool {
    if assignable(a, b) || assignable(b, a) || (is_numeric(a) && is_numeric(b)) {
        return true;
    }
    matches!(
        (a, b),
        (
            HirType::Simple(AstType::Named(n1)),
            HirType::Simple(AstType::Named(n2))
        ) if n1 == n2
    ) || matches!(
        (a, b),
        (
            HirType::Series(AstType::Named(n1)),
            HirType::Series(AstType::Named(n2))
        ) if n1 == n2
    ) || matches!(
        (a, b),
        (
            HirType::ImportNamespace { alias: ref a },
            HirType::ImportNamespace { alias: ref b }
        ) if a == b
    )
}

/// Unify types for `?:`, `if` expressions, and homogeneous array literals (greatest lower bound).
#[must_use]
pub fn unify_branch_types(a: &HirType, b: &HirType) -> Option<HirType> {
    if assignable(a, b) {
        return Some(b.clone());
    }
    if assignable(b, a) {
        return Some(a.clone());
    }
    if let (HirType::Array(ea), HirType::Array(eb)) = (a, b) {
        return unify_branch_types(ea, eb).map(|e| HirType::Array(Box::new(e)));
    }
    if let (HirType::Matrix(ea), HirType::Matrix(eb)) = (a, b) {
        return unify_branch_types(ea, eb).map(|e| HirType::Matrix(Box::new(e)));
    }
    if let (
        HirType::ImportNamespace { alias: ref a },
        HirType::ImportNamespace { alias: ref b },
    ) = (a, b)
    {
        if a == b {
            return Some(HirType::ImportNamespace { alias: a.clone() });
        }
    }
    binary_numeric_result(a, b).ok()
}

/// `request.security`: result is a series whose element type follows the expression argument.
#[must_use]
pub fn request_security_result_type(expr_ty: &HirType) -> HirType {
    match expr_ty {
        HirType::Simple(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(*p)),
        HirType::Series(AstType::Primitive(p)) => HirType::Series(AstType::Primitive(*p)),
        HirType::Array(_) | HirType::Matrix(_) => {
            HirType::Series(AstType::Primitive(PrimitiveType::Float))
        }
        _ => HirType::Series(AstType::Primitive(PrimitiveType::Float)),
    }
}

pub fn index_result_type(base: &HirType) -> Result<HirType, ()> {
    match base {
        HirType::Series(a) => Ok(HirType::Series(a.clone())),
        HirType::Simple(AstType::Primitive(p)) => Ok(HirType::Simple(AstType::Primitive(*p))),
        HirType::Array(elem) | HirType::Matrix(elem) => Ok((**elem).clone()),
        HirType::ImportNamespace { .. } => Err(()),
        _ => Err(()),
    }
}
