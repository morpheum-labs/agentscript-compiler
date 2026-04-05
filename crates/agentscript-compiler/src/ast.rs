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
    /// User function: `f name(...) => expr` or `f name(...) { ... }`.
    FnDecl(FnDecl),
    /// Executable statement (includes variable declarations at top level).
    Stmt(Stmt),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<FnParam>,
    pub body: FnBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnParam {
    pub ty: Option<Type>,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnBody {
    Expr(Expr),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub qualifier: Option<VarQualifier>,
    pub ty: Option<Type>,
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarQualifier {
    Var,
    Varip,
    Const,
    Input,
    Simple,
    Series,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    Array(Box<Type>),
    Matrix(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Label,
    Line,
    BoxType,
    Table,
    Polyline,
    Linefill,
    ChartPoint,
    VolumeRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Int,
    Float,
    Bool,
    String,
    Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `qualifier? type? name = expr` (declaration).
    VarDecl(VarDecl),
    /// `name = expr` (first assignment) or `name := expr` (reassignment).
    Assign {
        name: String,
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
        body: Vec<Stmt>,
    },
    Switch {
        scrutinee: Expr,
        cases: Vec<(Expr, Stmt)>,
        default: Option<Box<Stmt>>,
    },
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

/// Expression (parser phase; typecheck later).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Na,
    /// `color.red`, etc.
    Color(String),
    /// Reference without a call suffix, e.g. `close`, `strategy.long`.
    IdentPath(Vec<String>),
    /// Function / method call: `ta.sma(...)`, `matrix.new<float>(...)`.
    Call {
        path: Vec<String>,
        /// Generic type arguments before `(` when present (e.g. `matrix.new<float>`).
        type_args: Option<Vec<Type>>,
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
    /// Conditional (Pine / QAS `? :`), right-associative.
    Ternary {
        cond: Box<Expr>,
        then_b: Box<Expr>,
        else_b: Box<Expr>,
    },
}
