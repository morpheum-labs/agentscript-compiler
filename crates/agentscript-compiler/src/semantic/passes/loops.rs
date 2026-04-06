//! `break` / `continue` may appear only inside `for` or `while` bodies.

use crate::frontend::ast::{Script, Stmt, StmtKind};
use crate::visitor::{AstWalk, VisitExpr, VisitStmt};

use super::super::{AnalyzeError, SemanticDiagnostic};

struct BreakContinueChecker {
    loop_depth: u32,
    issues: Vec<SemanticDiagnostic>,
}

impl VisitExpr for BreakContinueChecker {}

impl VisitStmt for BreakContinueChecker {
    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), ()> {
        match &stmt.kind {
            StmtKind::Break | StmtKind::Continue => {
                if self.loop_depth == 0 {
                    let kw = match &stmt.kind {
                        StmtKind::Break => "break",
                        StmtKind::Continue => "continue",
                        _ => unreachable!(),
                    };
                    self.issues.push(SemanticDiagnostic {
                        message: format!(
                            "`{kw}` is only valid inside a `for` or `while` loop",
                        ),
                        span: stmt.span,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl AstWalk for BreakContinueChecker {
    fn push_loop_frame(&mut self) {
        self.loop_depth = self.loop_depth.saturating_add(1);
    }

    fn pop_loop_frame(&mut self) {
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }
}

pub fn check_break_continue(script: &Script) -> Result<(), AnalyzeError> {
    let mut c = BreakContinueChecker {
        loop_depth: 0,
        issues: Vec::new(),
    };
    let Ok(()) = c.walk_script(script) else {
        unreachable!("break/continue walk does not surface errors");
    };
    if c.issues.is_empty() {
        Ok(())
    } else {
        Err(AnalyzeError::new(c.issues))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_script;

    #[test]
    fn break_inside_while_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\nwhile true {\n  break\n}\n",
        )
        .unwrap();
        check_break_continue(&s).unwrap();
    }

    #[test]
    fn break_at_top_level_rejected() {
        let s = parse_script("t.pine", "indicator(\"x\")\nbreak\n").unwrap();
        let e = check_break_continue(&s).unwrap_err();
        assert!(e.message().contains("break"));
    }

    #[test]
    fn continue_in_for_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\nfor i = 0 to 1 {\n  continue\n}\n",
        )
        .unwrap();
        check_break_continue(&s).unwrap();
    }

    #[test]
    fn continue_in_for_in_ok() {
        let s = parse_script(
            "t.pine",
            "indicator(\"x\")\nfor x in arr {\n  continue\n}\n",
        )
        .unwrap();
        check_break_continue(&s).unwrap();
    }
}
