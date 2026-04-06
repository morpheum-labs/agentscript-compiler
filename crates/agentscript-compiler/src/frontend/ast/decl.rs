//! Top-level script, imports, exports, and declarations.

use super::expr::Expr;
use super::node::{NodeId, Span};
use super::stmt::{Stmt, VarDecl};
use super::types::{Type, VarQualifier};

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
    /// Dense id for session side maps and diagnostics ([`assign_node_ids`]).
    pub id: NodeId,
    pub span: Span,
    /// Path segments, e.g. `["TradingView", "ta", "5"]`.
    pub path: Vec<String>,
    pub alias: String,
}

/// `export f ...`, `export var ...`, `export enum ...`, or `export type ...` in a `library()` script.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportDecl {
    Fn(FnDecl),
    Var(VarDecl),
    Enum(EnumDef),
    TypeDef(UserTypeDef),
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
    /// Pine `enum name { a = expr, ... }` (braced QAS/TV-style body).
    Enum(EnumDef),
    /// Pine `type name { fields... }` user-defined type.
    TypeDef(UserTypeDef),
    /// Executable statement (includes variable declarations at top level).
    Stmt(Stmt),
}

/// Enumeration definition (`enum tz { ... }`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub span: Span,
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Expr,
}

/// User-defined type (`type bar { float o = open ... }`).
#[derive(Debug, Clone, PartialEq)]
pub struct UserTypeDef {
    pub span: Span,
    pub name: String,
    pub fields: Vec<UdtField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UdtField {
    pub qualifier: Option<VarQualifier>,
    pub ty: Type,
    pub name: String,
    pub default: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub span: Span,
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
