//! `break` / `continue` may appear only inside `for` or `while` bodies.

use crate::ast::{ElseBody, ExportDecl, FnBody, FnDecl, IfStmt, Item, Script, Stmt};

use super::AnalyzeError;

pub fn check_break_continue(script: &Script) -> Result<(), AnalyzeError> {
    let mut issues = Vec::new();
    for item in &script.items {
        match item {
            Item::Stmt(s) => walk_stmt(s, 0, &mut issues),
            Item::FnDecl(f) | Item::Export(ExportDecl::Fn(f)) => walk_fn(f, &mut issues),
            Item::ScriptDecl(_) | Item::Export(ExportDecl::Var(_)) | Item::Import(_) => {}
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(AnalyzeError {
            message: issues.join("\n"),
        })
    }
}

fn walk_fn(f: &FnDecl, issues: &mut Vec<String>) {
    if let FnBody::Block(stmts) = &f.body {
        for s in stmts {
            walk_stmt(s, 0, issues);
        }
    }
}

fn walk_stmt(s: &Stmt, loop_depth: u32, issues: &mut Vec<String>) {
    match s {
        Stmt::Break | Stmt::Continue => {
            if loop_depth == 0 {
                issues.push(format!(
                    "`{}` is only valid inside a `for` or `while` loop",
                    match s {
                        Stmt::Break => "break",
                        Stmt::Continue => "continue",
                        _ => unreachable!(),
                    }
                ));
            }
        }
        Stmt::Block(stmts) => {
            for x in stmts {
                walk_stmt(x, loop_depth, issues);
            }
        }
        Stmt::If(i) => walk_if(i, loop_depth, issues),
        Stmt::For { body, .. } => {
            for x in body {
                walk_stmt(x, loop_depth.saturating_add(1), issues);
            }
        }
        Stmt::While { body, .. } => {
            for x in body {
                walk_stmt(x, loop_depth.saturating_add(1), issues);
            }
        }
        Stmt::Switch {
            cases,
            default,
            ..
        } => {
            for (_, arm) in cases {
                walk_stmt(arm, loop_depth, issues);
            }
            if let Some(d) = default {
                walk_stmt(d, loop_depth, issues);
            }
        }
        Stmt::VarDecl(_)
        | Stmt::Assign { .. }
        | Stmt::Expr(_) => {}
    }
}

fn walk_if(i: &IfStmt, loop_depth: u32, issues: &mut Vec<String>) {
    for x in &i.then_body {
        walk_stmt(x, loop_depth, issues);
    }
    if let Some(else_b) = &i.else_body {
        match else_b {
            ElseBody::If(inner) => walk_if(inner, loop_depth, issues),
            ElseBody::Block(stmts) => {
                for x in stmts {
                    walk_stmt(x, loop_depth, issues);
                }
            }
        }
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
        assert!(e.message.contains("break"));
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
}
