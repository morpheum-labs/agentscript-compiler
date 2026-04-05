//! Single-responsibility semantic passes (Phase 1 groundwork).

use crate::frontend::ast::Script;
use crate::session::CompilerSession;

use super::AnalyzeError;

mod early;
mod loops;
mod resolver;

pub use early::analyze_script;
pub use resolver::resolve_script;

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

    fn run(&mut self, _session: &mut CompilerSession, script: &Script) -> Result<(), AnalyzeError> {
        resolver::resolve_script(script)
    }
}

/// Default Phase-0 semantic pipeline (order matters).
pub fn default_passes() -> Vec<Box<dyn CompilerPass>> {
    vec![
        Box::new(EarlyAnalyzePass),
        Box::new(BreakContinuePass),
        Box::new(ResolverPass),
    ]
}
