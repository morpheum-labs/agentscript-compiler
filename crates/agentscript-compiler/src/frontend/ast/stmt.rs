//! Statements with spans.

use super::expr::Expr;
use super::node::{NodeId, Span};
use super::types::{Type, VarQualifier};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignOp {
    Eq,
    ColonEq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
}

/// Statement with source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub id: NodeId,
    pub span: Span,
    pub kind: StmtKind,
}

impl Stmt {
    #[must_use]
    pub fn new(span: impl Into<Span>, kind: StmtKind) -> Self {
        Self {
            id: NodeId::UNASSIGNED,
            span: span.into(),
            kind,
        }
    }
}

/// Statement shape without span.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `qualifier? type? name = expr` (declaration).
    VarDecl(VarDecl),
    /// `name = expr` (first assignment) or `name := expr` (reassignment).
    Assign {
        name: String,
        op: AssignOp,
        value: Expr,
    },
    /// `[a, b, ...] = expr` / `:=` (Pine tuple destructuring).
    TupleAssign {
        names: Vec<String>,
        op: AssignOp,
        value: Expr,
    },
    /// Expression used as a statement (calls, etc.).
    Expr(Expr),
    /// `{ ... }`
    Block(Vec<Stmt>),
    If(IfStmt),
    For {
        var: String,
        from: Expr,
        to: Expr,
        /// `for i = a to b by step` — Pine-style step (optional).
        by: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// `for x in arr` or `for [i, v] in mat` (Pine `for…in`).
    ForIn {
        pattern: ForInPattern,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Switch {
        /// `None` for strategy-style `switch { cond => ... }` with no scrutinee.
        scrutinee: Option<Expr>,
        cases: Vec<(Expr, Stmt)>,
        default: Option<Box<Stmt>>,
    },
    /// `while cond { ... }`
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    /// `break` — only valid inside `for` / `while` (enforced by semantic checks).
    Break,
    /// `continue` — only valid inside `for` / `while`.
    Continue,
}

/// Binding pattern for [`StmtKind::ForIn`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ForInPattern {
    /// `for name in iterable`
    Name(String),
    /// `for [index, value] in iterable`
    Pair(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_body: Vec<Stmt>,
    pub else_body: Option<ElseBody>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElseBody {
    If(Box<IfStmt>),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    /// Span of the full declaration (matches enclosing [`Stmt::span`] when parsed as a statement).
    pub span: Span,
    pub qualifier: Option<VarQualifier>,
    pub ty: Option<Type>,
    pub name: String,
    pub value: Expr,
}
