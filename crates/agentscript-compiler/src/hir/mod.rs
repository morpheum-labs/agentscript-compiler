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

mod ast_lower;
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

pub use ast_lower::{
    lower_script_to_hir, lower_script_to_hir_in_bump, lower_script_to_hir_in_bump_with_session,
    AstHirLowerer, HirLowerError,
};
pub use builtin::BuiltinKind;
pub use expr::HirExpr;
pub use ids::{HirId, SymbolId};
pub use literal::HirLiteral;
pub use lowering::LowerToHir;
pub use security::{GapMode, Lookahead, SecurityCall};
pub use script::{HirDeclaration, HirInputDecl, HirInputKind, HirScript, HirUserFunction};
pub use stmt::HirStmt;
pub use symbols::SymbolTable;
pub use ty::{
    assignable, binary_numeric_result, coerce_simple_to_series, index_result_type, is_bool_like,
    is_integral, is_numeric, is_series_shape, is_stringish, promote_numeric_series,
    request_security_result_type, type_compatible_eq, unify_branch_types, HirType,
};
