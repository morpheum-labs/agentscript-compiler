//! Early semantic passes (Phase 1 groundwork): duplicate definitions, etc.
//!
//! Full typechecking and builtin resolution live in later milestones.

use std::collections::{HashMap, HashSet};

use crate::ast::{ExportDecl, FnDecl, Item, Script};

/// Semantic analysis failed (no source spans on the AST yet; messages are textual).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AnalyzeError {
    pub message: String,
}

/// Check script-wide consistency that does not require types or builtins.
pub fn analyze_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut issues = Vec::new();
    let mut fn_counts: HashMap<String, usize> = HashMap::new();
    let mut import_counts: HashMap<String, usize> = HashMap::new();

    for item in &script.items {
        match item {
            Item::Import(i) => {
                *import_counts.entry(i.alias.clone()).or_insert(0) += 1;
            }
            Item::FnDecl(f) => {
                *fn_counts.entry(f.name.clone()).or_insert(0) += 1;
            }
            Item::Export(ExportDecl::Fn(f)) => {
                *fn_counts.entry(f.name.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    for (name, n) in &fn_counts {
        if *n > 1 {
            issues.push(format!(
                "duplicate function definition `{name}` ({n} declarations)"
            ));
        }
    }
    for (alias, n) in &import_counts {
        if *n > 1 {
            issues.push(format!(
                "duplicate import alias `{alias}` ({n} imports)"
            ));
        }
    }

    for item in &script.items {
        match item {
            Item::FnDecl(f) => check_fn_params(f, &mut issues),
            Item::Export(ExportDecl::Fn(f)) => check_fn_params(f, &mut issues),
            _ => {}
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

fn check_fn_params(f: &FnDecl, issues: &mut Vec<String>) {
    let mut seen = HashSet::new();
    for p in &f.params {
        if !seen.insert(&p.name) {
            issues.push(format!(
                "duplicate parameter `{}` in function `{}`",
                p.name, f.name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FnBody, FnParam, ScriptDeclaration, ScriptKind};
    use crate::{parse_script, Item};

    #[test]
    fn analyze_ok_minimal() {
        let s = parse_script("t", "indicator(\"x\")\n").unwrap();
        analyze_script(&s).unwrap();
    }

    #[test]
    fn duplicate_top_level_fn_rejected() {
        let s = parse_script(
            "t",
            "indicator(\"x\")\nf a() => 1\nf a() => 2\n",
        )
        .unwrap();
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message.contains("duplicate function"));
        assert!(e.message.contains("a"));
    }

    #[test]
    fn duplicate_export_fn_counts_with_plain_fn() {
        let s = parse_script(
            "t",
            r#"//@version=6
library("L")
export f dup() => 1
f dup() => 2
"#,
        )
        .unwrap();
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message.contains("dup"));
    }

    #[test]
    fn duplicate_import_alias_rejected() {
        let s = parse_script(
            "t",
            "import A/1 as x\nimport B/1 as x\nindicator(\"i\")\n",
        )
        .unwrap();
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message.contains("import"));
        assert!(e.message.contains("x"));
    }

    #[test]
    fn duplicate_param_rejected() {
        let s = Script {
            version: None,
            agentscript_version: None,
            items: vec![
                Item::ScriptDecl(ScriptDeclaration {
                    kind: ScriptKind::Indicator,
                    args: vec![(None, crate::ast::Expr::String("t".into()))],
                }),
                Item::FnDecl(FnDecl {
                    is_method: false,
                    name: "bad".into(),
                    params: vec![
                        FnParam {
                            ty: None,
                            name: "n".into(),
                            default: None,
                        },
                        FnParam {
                            ty: None,
                            name: "n".into(),
                            default: None,
                        },
                    ],
                    body: FnBody::Expr(crate::ast::Expr::Int(0)),
                }),
            ],
        };
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message.contains("parameter"));
        assert!(e.message.contains('n'));
    }
}
