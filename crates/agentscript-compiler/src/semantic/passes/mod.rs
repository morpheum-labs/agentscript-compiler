//! Single-responsibility semantic passes (Phase 1 groundwork).

use crate::frontend::ast::Script;
use crate::hir::lower_script_to_hir_in_bump_with_session;
use crate::session::CompilerSession;

use super::AnalyzeError;

mod early;
mod lexical;
mod loops;
mod resolver;
mod typecheck;

pub use early::analyze_script;
pub use lexical::{lexical_resolve_script, lexical_resolve_script_in_session};
pub use resolver::{resolve_script, resolve_script_in_session};
pub use typecheck::{typecheck_script, typecheck_script_in_session};

/// Pluggable pipeline step; add new passes without editing the driver’s control flow.
pub trait CompilerPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError>;
}

/// Duplicate definitions and malformed function / enum / UDT shapes.
pub struct EarlyAnalyzePass;

impl CompilerPass for EarlyAnalyzePass {
    fn name(&self) -> &'static str {
        "early_analyze"
    }

    fn run(&mut self, _session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        early::analyze_script(script)
    }
}

/// `break` / `continue` placement.
pub struct BreakContinuePass;

impl CompilerPass for BreakContinuePass {
    fn name(&self) -> &'static str {
        "break_continue"
    }

    fn run(&mut self, _session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        loops::check_break_continue(script)
    }
}

/// Dotted-path roots and `strategy.*` script kind.
pub struct ResolverPass;

impl CompilerPass for ResolverPass {
    fn name(&self) -> &'static str {
        "resolve"
    }

    fn run(&mut self, session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        resolver::resolve_script_in_session(session, script)
    }
}

/// Unqualified identifier binding (locals, hoisted fns, imports, builtins).
pub struct LexicalResolvePass;

impl CompilerPass for LexicalResolvePass {
    fn name(&self) -> &'static str {
        "lexical_resolve"
    }

    fn run(&mut self, session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        lexical::lexical_resolve_script_in_session(session, script)
    }
}

/// Minimal typecheck (`series` vs `simple`, builtins, scoping).
pub struct TypecheckPass;

impl CompilerPass for TypecheckPass {
    fn name(&self) -> &'static str {
        "typecheck"
    }

    fn run(&mut self, session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        typecheck::typecheck_script_in_session(session, script)
    }
}

/// AST → HIR for the supported lowering subset; fails the pipeline if lowering is not implemented yet.
pub struct HirLowerPass;

impl CompilerPass for HirLowerPass {
    fn name(&self) -> &'static str {
        "hir_lower"
    }

    fn run(&mut self, session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        match lower_script_to_hir_in_bump_with_session(&session.arena, script, Some(session)) {
            Ok(hir) => {
                session.hir = Some(hir);
                Ok(())
            }
            Err(e) => Err(AnalyzeError::single(e.message, e.span)),
        }
    }
}

/// Default Phase-1 semantic pipeline (order matters). Does **not** run [`HirLowerPass`] so
/// [`check_script`] stays usable for scripts outside the current HIR subset.
pub fn default_passes() -> Vec<Box<dyn CompilerPass>> {
    vec![
        Box::new(EarlyAnalyzePass),
        Box::new(BreakContinuePass),
        Box::new(ResolverPass),
        Box::new(LexicalResolvePass),
        Box::new(TypecheckPass),
    ]
}

/// Same as [`default_passes`] plus HIR lowering (Phase 2 driver).
pub fn default_passes_with_hir() -> Vec<Box<dyn CompilerPass>> {
    let mut v = default_passes();
    v.push(Box::new(HirLowerPass));
    v
}
