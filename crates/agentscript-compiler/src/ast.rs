//! Abstract syntax tree for AgentScript / QAS (parser phases; typecheck later).

/// Parsed AgentScript / QAS compilation unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// `//@version=1` or `//@version=6` when present (QAS EBNF).
    pub version: Option<u32>,
    /// Top-level declarations and statements.
    pub items: Vec<Item>,
}

impl Script {
    pub fn empty() -> Self {
        Self {
            version: None,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// `indicator(...)`, `strategy(...)`, or `library(...)`.
    ScriptDecl(ScriptDeclaration),
    /// Executable line: assignment or bare expression (e.g. future `plot(...)`).
    Stmt(Stmt),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `name = expr` (first assignment) or `name := expr` (reassignment).
    Assign {
        name: String,
        op: AssignOp,
        value: Expr,
    },
    /// Expression used as a statement (calls, etc.).
    Expr(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignOp {
    Eq,
    ColonEq,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptDeclaration {
    pub kind: ScriptKind,
    /// Named (`Some(name)`, value) or positional (`None`, value) actual arguments.
    pub args: Vec<(Option<String>, Expr)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptKind {
    Indicator,
    Strategy,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
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

/// Expression (Phase 3: calls, subscripts, binary/unary; more forms later).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Na,
    /// Reference without a call suffix, e.g. `close`, `strategy.long`.
    IdentPath(Vec<String>),
    /// Function / method call: `ta.sma(...)`, `input.int(...)`.
    Call {
        path: Vec<String>,
        args: Vec<(Option<String>, Expr)>,
    },
    /// Historical reference or indexing: `close[1]`.
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}
