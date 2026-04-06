//! Duplicate-definition checks (no types).

use indexmap::IndexMap;
use std::collections::HashSet;

use crate::frontend::ast::{
    EnumDef, ExportDecl, FnDecl, Item, Script, ScriptKind, Span, UserTypeDef,
};

use super::super::{AnalyzeError, SemanticDiagnostic};

pub fn analyze_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut issues = Vec::new();
    let mut def_counts: IndexMap<String, usize> = IndexMap::new();
    let mut import_counts: IndexMap<String, usize> = IndexMap::new();

    for item in &script.items {
        match item {
            Item::Import(i) => {
                *import_counts.entry(i.alias.clone()).or_insert(0) += 1;
            }
            Item::FnDecl(f) => {
                *def_counts.entry(f.name.clone()).or_insert(0) += 1;
            }
            Item::Export(ExportDecl::Fn(f)) => {
                *def_counts.entry(f.name.clone()).or_insert(0) += 1;
            }
            Item::Enum(e) => {
                *def_counts.entry(e.name.clone()).or_insert(0) += 1;
            }
            Item::Export(ExportDecl::Enum(e)) => {
                *def_counts.entry(e.name.clone()).or_insert(0) += 1;
            }
            Item::TypeDef(t) => {
                *def_counts.entry(t.name.clone()).or_insert(0) += 1;
            }
            Item::Export(ExportDecl::TypeDef(t)) => {
                *def_counts.entry(t.name.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    for (name, n) in &def_counts {
        if *n > 1 {
            issues.push(SemanticDiagnostic {
                message: format!(
                    "duplicate top-level definition `{name}` ({n} declarations)"
                ),
                span: Span::DUMMY,
            });
        }
    }
    for (alias, n) in &import_counts {
        if *n > 1 {
            issues.push(SemanticDiagnostic {
                message: format!("duplicate import alias `{alias}` ({n} imports)"),
                span: Span::DUMMY,
            });
        }
    }

    let primary_kind = script.items.iter().find_map(|it| match it {
        Item::ScriptDecl(d) => Some(d.kind),
        _ => None,
    });
    if primary_kind == Some(ScriptKind::Library) {
        for item in &script.items {
            if matches!(item, Item::Stmt(_)) {
                issues.push(SemanticDiagnostic {
                    message: "top-level executable statements are not allowed in `library()` scripts; use `export`"
                        .into(),
                    span: Span::DUMMY,
                });
            }
        }
    }

    for item in &script.items {
        match item {
            Item::FnDecl(f) => check_fn_params(f, &mut issues),
            Item::Export(ExportDecl::Fn(f)) => check_fn_params(f, &mut issues),
            Item::Enum(e) | Item::Export(ExportDecl::Enum(e)) => {
                check_enum_variants(e, &mut issues);
            }
            Item::TypeDef(t) | Item::Export(ExportDecl::TypeDef(t)) => {
                check_udt_fields(t, &mut issues);
            }
            _ => {}
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(AnalyzeError::new(issues))
    }
}

fn check_fn_params(f: &FnDecl, issues: &mut Vec<SemanticDiagnostic>) {
    let mut seen = HashSet::new();
    for p in &f.params {
        if !seen.insert(&p.name) {
            issues.push(SemanticDiagnostic {
                message: format!(
                    "duplicate parameter `{}` in function `{}`",
                    p.name, f.name
                ),
                span: Span::DUMMY,
            });
        }
    }
}

fn check_enum_variants(e: &EnumDef, issues: &mut Vec<SemanticDiagnostic>) {
    let mut seen = HashSet::new();
    for v in &e.variants {
        if !seen.insert(&v.name) {
            issues.push(SemanticDiagnostic {
                message: format!(
                    "duplicate variant `{}` in enum `{}`",
                    v.name, e.name
                ),
                span: Span::DUMMY,
            });
        }
    }
}

fn check_udt_fields(t: &UserTypeDef, issues: &mut Vec<SemanticDiagnostic>) {
    let mut seen = HashSet::new();
    for f in &t.fields {
        if !seen.insert(&f.name) {
            issues.push(SemanticDiagnostic {
                message: format!(
                    "duplicate field `{}` in type `{}`",
                    f.name, t.name
                ),
                span: Span::DUMMY,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::ast::{
        Expr, ExprKind, FnBody, FnParam, ScriptDeclaration, ScriptKind,
    };
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
        assert!(e.message().contains("duplicate"));
        assert!(e.message().contains("a"));
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
        assert!(e.message().contains("duplicate"), "{}", e.message());
        assert!(e.message().contains("dup"));
    }

    #[test]
    fn duplicate_import_alias_rejected() {
        let s = parse_script(
            "t",
            "import A/1 as x\nimport B/1 as x\nindicator(\"i\")\n",
        )
        .unwrap();
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message().contains("import"));
        assert!(e.message().contains("x"));
    }

    #[test]
    fn library_rejects_top_level_stmt() {
        let s = parse_script(
            "t",
            r#"//@version=6
library("L")
x = 1
"#,
        )
        .unwrap();
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message().contains("library"), "{}", e.message());
        assert!(e.message().contains("export"), "{}", e.message());
    }

    #[test]
    fn duplicate_param_rejected() {
        let s = Script {
            version: None,
            agentscript_version: None,
            items: vec![
                Item::ScriptDecl(ScriptDeclaration {
                    kind: ScriptKind::Indicator,
                    args: vec![(
                        None,
                        Expr::synthetic(ExprKind::String("t".into())),
                    )],
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
                    body: FnBody::Expr(Expr::synthetic(ExprKind::Int(0))),
                }),
            ],
        };
        let e = analyze_script(&s).unwrap_err();
        assert!(e.message().contains("parameter"));
        assert!(e.message().contains('n'));
    }
}
