//! Abstract syntax tree for AgentScript / QAS (parser phases; typecheck later).

mod assign_ids;
mod decl;
mod expr;
mod node;
mod stmt;
mod types;

pub use assign_ids::{assign_node_ids, max_node_id};
pub use decl::{
    EnumDef, EnumVariant, ExportDecl, FnBody, FnDecl, FnParam, ImportDecl, Item, Script,
    ScriptDeclaration, ScriptKind, UserTypeDef, UdtField,
};
pub use expr::{BinOp, Expr, ExprKind, UnaryOp};
pub use node::{NodeId, Span, Spanned};
pub use stmt::{
    AssignOp, ElseBody, ForInPattern, IfStmt, Stmt, StmtKind, VarDecl,
};
pub use types::{PrimitiveType, Type, VarQualifier};
