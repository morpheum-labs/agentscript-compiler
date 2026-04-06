//! Duplicate-definition checks (no types).

use std::collections::HashSet;

use crate::frontend::ast::{
    EnumDef, ExportDecl, FnDecl, Item, Script, ScriptKind, Span, UserTypeDef,
};

use super::super::{AnalyzeError, SemanticDiagnostic};

pub fn analyze_script(script: &Script) -> Result<(), AnalyzeError> {
    let mut issues = Vec::new();

    // Imports: report at the second (and later) occurrence with that import's span.
    let mut seen_import_alias: HashSet<String> = HashSet::new();
    for item in &script.items {
        if let Item::Import(i) = item {
            if !seen_import_alias.insert(i.alias.clone()) {
                issues.push(SemanticDiagnostic {
                    message: format!("duplicate import alias `{}`", i.alias),
                    span: i.span,
                });
            }
        }
    }

    // Top-level defs: first declaration wins; duplicate reports at the conflicting declaration.
    let mut seen_def: HashSet<String> = HashSet::new();
    for item in &script.items {
        let dup: Option<(String, Span)> = match item {
            Item::FnDecl(f) => {
                if !seen_def.insert(f.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", f.name),
                        f.span,
                    ))
                } else {
                    None
                }
            }
            Item::Export(ExportDecl::Fn(f)) => {
                if !seen_def.insert(f.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", f.name),
                        f.span,
                    ))
                } else {
                    None
                }
            }
            Item::Enum(e) => {
                if !seen_def.insert(e.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", e.name),
                        e.span,
                    ))
                } else {
                    None
                }
            }
            Item::Export(ExportDecl::Enum(e)) => {
                if !seen_def.insert(e.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", e.name),
                        e.span,
                    ))
                } else {
                    None
                }
            }
            Item::TypeDef(t) => {
                if !seen_def.insert(t.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", t.name),
                        t.span,
                    ))
                } else {
                    None
                }
            }
            Item::Export(ExportDecl::TypeDef(t)) => {
                if !seen_def.insert(t.name.clone()) {
                    Some((
                        format!("duplicate top-level definition `{}`", t.name),
                        t.span,
                    ))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some((message, span)) = dup {
            issues.push(SemanticDiagnostic { message, span });
        }
    }

    let primary_kind = script.items.iter().find_map(|it| match it {
        Item::ScriptDecl(d) => Some(d.kind),
        _ => None,
    });
    if primary_kind == Some(ScriptKind::Library) {
        for item in &script.items {
            if let Item::Stmt(s) = item {
                issues.push(SemanticDiagnostic {
                    message: "top-level executable statements are not allowed in `library()` scripts; use `export`"
                        .into(),
                    span: s.span,
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
                span: f.span,
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
                span: e.span,
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
                span: t.span,
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
                    span: Span::DUMMY,
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
