//! High-level intermediate representation (HIR): normalized, typed, desugared program.
//!
//! # Layout (SOLID)
//!
//! - **Single responsibility:** one module per concern — identifiers ([`ids`]), types ([`ty`]),
//!   literals ([`literal`]), builtins ([`builtin`]), security calls ([`security`]), expressions
//!   ([`expr`]), statements ([`stmt`]), symbol table ([`symbols`]), script root ([`script`]),
//!   lowering seam ([`lowering`]).
//! - **Open/closed:** new expression or builtin shapes extend the relevant `enum` / [`builtin::BuiltinKind`]
//!   without rewriting unrelated modules; future passes implement [`lowering::LowerToHir`] or
//!   [`crate::semantic::passes::CompilerPass`].
//! - **Liskov:** [`lowering::LowerToHir`] implementations must return a consistent [`script::HirScript`]
//!   for the same input once typecheck is fixed.
//! - **Interface segregation:** small surface per module; dependents import only what they need.
//! - **Dependency inversion:** lowering and codegen depend on [`symbols::SymbolTable`] and node handles
//!   ([`ids::HirId`], [`ids::SymbolId`]), not on resolver internals.
//!
//! Node storage uses [`crate::session::CompilerSession::arena`] (`bumpalo::Bump`) plus dense `Vec`s
//! owned by a future builder; [`ids::HirId`] indexes those vectors.

mod builtin;
mod expr;
mod ids;
mod literal;
pub mod lowering;
mod security;
mod script;
mod stmt;
mod symbols;
mod ty;

pub use builtin::BuiltinKind;
pub use expr::HirExpr;
pub use ids::{HirId, SymbolId};
pub use literal::HirLiteral;
pub use lowering::LowerToHir;
pub use security::{GapMode, Lookahead, SecurityCall};
pub use script::{HirDeclaration, HirInputDecl, HirScript};
pub use stmt::HirStmt;
pub use symbols::SymbolTable;
pub use ty::HirType;
