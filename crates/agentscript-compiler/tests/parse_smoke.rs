use agentscript_compiler::{
    parse_script, Expr, Item, ScriptDeclaration, ScriptKind,
};

#[test]
fn empty_script() {
    let s = parse_script("empty", "").unwrap();
    assert_eq!(s.version, None);
    assert!(s.items.is_empty());
}

#[test]
fn version_and_indicator() {
    let src = "//@version=6\nindicator(\"x\")\n";
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(6));
    assert_eq!(s.items.len(), 1);
    let Item::ScriptDecl(ScriptDeclaration {
        kind: ScriptKind::Indicator,
        args,
    }) = &s.items[0]
    else {
        panic!("expected indicator decl");
    };
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, None);
    assert_eq!(args[0].1, Expr::String("x".into()));
}

#[test]
fn version_one_no_newline_before_decl() {
    let src = "//@version=1 strategy(\"S\")";
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(1));
    assert!(matches!(
        &s.items[0],
        Item::ScriptDecl(ScriptDeclaration {
            kind: ScriptKind::Strategy,
            ..
        })
    ));
}

#[test]
fn strategy_named_args_and_comment() {
    let src = r#"//@version=1
/* head */
strategy("My", overlay=true, initial_capital=100000)
// tail
x = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(1));
    let Item::ScriptDecl(d) = &s.items[0];
    assert_eq!(d.kind, ScriptKind::Strategy);
    assert_eq!(d.args.len(), 3);
    assert_eq!(d.args[0], (None, Expr::String("My".into())));
    assert_eq!(
        d.args[1],
        (Some("overlay".into()), Expr::Bool(true))
    );
    assert_eq!(
        d.args[2],
        (Some("initial_capital".into()), Expr::Int(100_000))
    );
}

#[test]
fn qualified_ident_positional() {
    let src = "strategy(\"x\", strategy.long)\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::ScriptDecl(d) = &s.items[0];
    assert_eq!(
        d.args[1].1,
        Expr::IdentPath(vec!["strategy".into(), "long".into()])
    );
}

#[test]
fn invalid_version_rejected() {
    assert!(parse_script("t", "//@version=5").is_err());
}

#[test]
fn multiple_decls() {
    let src = r#"library("L")
indicator("I")
"#;
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.items.len(), 2);
    assert!(matches!(&s.items[0], Item::ScriptDecl(d) if d.kind == ScriptKind::Library));
    assert!(matches!(&s.items[1], Item::ScriptDecl(d) if d.kind == ScriptKind::Indicator));
}
