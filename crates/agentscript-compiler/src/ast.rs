//! Abstract syntax tree for AgentScript / QAS (parser phases; typecheck later).

/// Parsed AgentScript / QAS compilation unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// `//@version=5` or `6` when present (Pine-shaped scripts; same header as TradingView).
    pub version: Option<u32>,
    /// `// @agentscript=<n>` when present (requires whitespace after `//` so Pine `//@…` is unchanged).
    pub agentscript_version: Option<u32>,
    /// Top-level declarations and statements.
    pub items: Vec<Item>,
}

impl Script {
    pub fn empty() -> Self {
        Self {
            version: None,
            agentscript_version: None,
            items: Vec::new(),
        }
    }
}

/// `import User/Lib/1 as alias` (Pine-style library path).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// Path segments, e.g. `["TradingView", "ta", "5"]`.
    pub path: Vec<String>,
    pub alias: String,
}

/// `export f ...` or `export var ...` in a `library()` script.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportDecl {
    Fn(FnDecl),
    Var(VarDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// `import path as alias`.
    Import(ImportDecl),
    /// `export` declaration for libraries.
    Export(ExportDecl),
    /// `indicator(...)`, `strategy(...)`, or `library(...)`.
    ScriptDecl(ScriptDeclaration),
    /// User function: Pine `name(...) =>`, `method name(...) =>`, or QAS `f name(...) =>` / block.
    FnDecl(FnDecl),
    /// Executable statement (includes variable declarations at top level).
    Stmt(Stmt),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// Pine `method foo(...)` declarations; QAS `f` / Pine bare `foo(...)` are `false`.
    pub is_method: bool,
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
        /// `for i = a to b by step` — Pine-style step (optional).
        by: Option<Expr>,
        body: Vec<Stmt>,
    },
    Switch {
        scrutinee: Expr,
        cases: Vec<(Expr, Stmt)>,
        default: Option<Box<Stmt>>,
    },
    /// `while cond { ... }`
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    /// `break` — only valid inside `for` / `while` (enforced by semantic [`check_script`](crate::check_script)).
    Break,
    /// `continue` — only valid inside `for` / `while`.
    Continue,
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
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
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
}
