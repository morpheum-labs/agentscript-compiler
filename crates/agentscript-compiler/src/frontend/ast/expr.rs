//! Expression AST with spans (parser phase; typecheck later).

use super::node::Span;
use super::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Pos,
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Expression with source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

impl Expr {
    #[must_use]
    pub fn new(span: impl Into<Span>, kind: ExprKind) -> Self {
        Self {
            span: span.into(),
            kind,
        }
    }

    /// For tests and synthetic nodes; avoid for real parse output.
    #[must_use]
    pub fn synthetic(kind: ExprKind) -> Self {
        Self {
            span: Span::DUMMY,
            kind,
        }
    }

    /// Structural equality ignoring spans (handy for tests and snapshots).
    #[must_use]
    pub fn shape_eq(&self, other: &Expr) -> bool {
        shape_eq_kind(&self.kind, &other.kind)
    }
}

fn shape_eq_kind(a: &ExprKind, b: &ExprKind) -> bool {
    use ExprKind::*;
    match (a, b) {
        (Int(x), Int(y)) => x == y,
        (Float(x), Float(y)) => (x.is_nan() && y.is_nan()) || x == y,
        (String(x), String(y)) => x == y,
        (Bool(x), Bool(y)) => x == y,
        (Na, Na) => true,
        (Color(x), Color(y)) => x == y,
        (HexColor(x), HexColor(y)) => x == y,
        (IdentPath(x), IdentPath(y)) => x == y,
        (Member { base: b1, field: f1 }, Member { base: b2, field: f2 }) => {
            f1 == f2 && b1.shape_eq(b2)
        }
        (
            Call {
                callee: c1,
                type_args: t1,
                args: a1,
            },
            Call {
                callee: c2,
                type_args: t2,
                args: a2,
            },
        ) => {
            c1.shape_eq(c2)
                && t1 == t2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|((n1, e1), (n2, e2))| n1 == n2 && e1.shape_eq(e2))
        }
        (Index { base: b1, index: i1 }, Index { base: b2, index: i2 }) => {
            b1.shape_eq(b2) && i1.shape_eq(i2)
        }
        (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(u, v)| u.shape_eq(v)),
        (Unary { op: o1, expr: e1 }, Unary { op: o2, expr: e2 }) => o1 == o2 && e1.shape_eq(e2),
        (
            Binary {
                op: o1,
                left: l1,
                right: r1,
            },
            Binary {
                op: o2,
                left: l2,
                right: r2,
            },
        ) => o1 == o2 && l1.shape_eq(l2) && r1.shape_eq(r2),
        (
            Ternary {
                cond: c1,
                then_b: t1,
                else_b: e1,
            },
            Ternary {
                cond: c2,
                then_b: t2,
                else_b: e2,
            },
        ) => c1.shape_eq(c2) && t1.shape_eq(t2) && e1.shape_eq(e2),
        (
            IfExpr {
                cond: c1,
                then_b: t1,
                else_b: e1,
            },
            IfExpr {
                cond: c2,
                then_b: t2,
                else_b: e2,
            },
        ) => c1.shape_eq(c2) && t1.shape_eq(t2) && e1.shape_eq(e2),
        _ => false,
    }
}

/// Expression shape without span.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Na,
    /// `color.red`, etc.
    Color(String),
    /// `#RRGGBB` or `#RRGGBBAA` (Pine-style).
    HexColor(String),
    /// Reference without a call suffix, e.g. `close`, `strategy.long`.
    IdentPath(Vec<String>),
    /// Field access on an arbitrary expression, e.g. `(a + b).field`.
    Member {
        base: Box<Expr>,
        field: String,
    },
    /// Call: `ta.sma(...)`, `(expr).m(...)`, or `matrix.new<float>(...)`.
    Call {
        callee: Box<Expr>,
        /// Generic type arguments before `(` when present (e.g. `matrix.new<float>`).
        type_args: Option<Vec<Type>>,
        args: Vec<(Option<String>, Expr)>,
    },
    /// Historical reference or indexing: `close[1]`.
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// Literal array: `[a, b, c]` or `[]`.
    Array(Vec<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Conditional (Pine / QAS `? :`), right-associative.
    Ternary {
        cond: Box<Expr>,
        then_b: Box<Expr>,
        else_b: Box<Expr>,
    },
    /// Pine expression `if cond thenExpr else elseExpr` (distinct from ternary `? :`).
    IfExpr {
        cond: Box<Expr>,
        then_b: Box<Expr>,
        else_b: Box<Expr>,
    },
}
