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
