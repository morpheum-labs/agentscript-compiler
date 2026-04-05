use agentscript_compiler::{
    parse_script, AssignOp, BinOp, ElseBody, Expr, FnBody, Item, PrimitiveType, ScriptDeclaration,
    ScriptKind, Stmt, Type, UnaryOp, VarQualifier,
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
    assert_eq!(s.items.len(), 2);
    let Item::ScriptDecl(d) = &s.items[0] else {
        panic!("expected strategy decl");
    };
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
    let Item::Stmt(Stmt::Assign { name, op, value }) = &s.items[1] else {
        panic!("expected assignment");
    };
    assert_eq!(name, "x");
    assert_eq!(*op, AssignOp::Eq);
    assert_eq!(*value, Expr::Int(1));
}

#[test]
fn qualified_ident_positional() {
    let src = "strategy(\"x\", strategy.long)\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::ScriptDecl(d) = &s.items[0] else {
        panic!("expected decl");
    };
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

#[test]
fn assign_colon_eq_and_binary() {
    let src = r#"indicator("I")
a := 1 + 2 * 3
b = a == 2
"#;
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.items.len(), 3);
    let Item::Stmt(Stmt::Assign { name, op, value }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert_eq!(name, "a");
    assert_eq!(*op, AssignOp::ColonEq);
    let Expr::Binary { op, left, right } = value else {
        panic!("expected binary +");
    };
    assert_eq!(*op, BinOp::Add);
    assert_eq!(**left, Expr::Int(1));
    let Expr::Binary { op, left: l, right: r } = right.as_ref() else {
        panic!("expected *");
    };
    assert_eq!(*op, BinOp::Mul);
    assert_eq!(**l, Expr::Int(2));
    assert_eq!(**r, Expr::Int(3));

    let Item::Stmt(Stmt::Assign { name, value, .. }) = &s.items[2] else {
        panic!("expected assign b");
    };
    assert_eq!(name, "b");
    let Expr::Binary { op, .. } = value else {
        panic!("expected ==");
    };
    assert_eq!(*op, BinOp::Eq);
}

#[test]
fn call_and_subscript() {
    let src = r#"indicator("x")
y = ta.sma(close, 20)
z = close[1]
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { name, value, .. }) = &s.items[1] else {
        panic!("expected y assign");
    };
    assert_eq!(name, "y");
    let Expr::Call { path, args, .. } = value else {
        panic!("expected call");
    };
    assert_eq!(path, &vec!["ta".to_string(), "sma".to_string()]);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, None);
    assert_eq!(args[0].1, Expr::IdentPath(vec!["close".into()]));
    assert_eq!(args[1].0, None);
    assert_eq!(args[1].1, Expr::Int(20));

    let Item::Stmt(Stmt::Assign { name, value, .. }) = &s.items[2] else {
        panic!("expected z assign");
    };
    assert_eq!(name, "z");
    let Expr::Index { base, index } = value else {
        panic!("expected index");
    };
    assert_eq!(**base, Expr::IdentPath(vec!["close".into()]));
    assert_eq!(**index, Expr::Int(1));
}

#[test]
fn expr_stmt_call() {
    let src = r#"indicator("x")
plot(close)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Expr(e)) = &s.items[1] else {
        panic!("expected expr stmt");
    };
    let Expr::Call { path, args, .. } = e else {
        panic!("expected plot call");
    };
    assert_eq!(path, &vec!["plot".to_string()]);
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].1, Expr::IdentPath(vec!["close".into()]));
}

#[test]
fn logical_and_or_not() {
    let src = r#"indicator("x")
ok = not a and b or c
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    // Parsed as (not a) and b or c  —  or lowest:  ((not a) and b) or c
    let Expr::Binary { op: BinOp::Or, left, right } = value else {
        panic!("expected or at root: {value:#?}");
    };
    assert_eq!(**right, Expr::IdentPath(vec!["c".into()]));
    let Expr::Binary { op: BinOp::And, left: l, right: r } = left.as_ref() else {
        panic!("expected and: {left:#?}");
    };
    assert_eq!(**r, Expr::IdentPath(vec!["b".into()]));
    let Expr::Unary {
        op: agentscript_compiler::UnaryOp::Not,
        expr: inner,
    } = l.as_ref()
    else {
        panic!("expected not a: {l:#?}");
    };
    assert_eq!(**inner, Expr::IdentPath(vec!["a".into()]));
}

#[test]
fn ternary_simple() {
    let src = r#"indicator("x")
x = true ? 1 : 2
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Ternary { cond, then_b, else_b } = value else {
        panic!("expected ternary: {value:#?}");
    };
    assert_eq!(**cond, Expr::Bool(true));
    assert_eq!(**then_b, Expr::Int(1));
    assert_eq!(**else_b, Expr::Int(2));
}

#[test]
fn ternary_right_associative() {
    let src = r#"indicator("x")
x = a ? b : c ? d : e
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Ternary { cond, then_b, else_b } = value else {
        panic!("expected outer ternary: {value:#?}");
    };
    assert_eq!(**cond, Expr::IdentPath(vec!["a".into()]));
    assert_eq!(**then_b, Expr::IdentPath(vec!["b".into()]));
    let Expr::Ternary {
        cond: c2,
        then_b: t2,
        else_b: e2,
    } = else_b.as_ref()
    else {
        panic!("expected nested ternary in else: {else_b:#?}");
    };
    assert_eq!(**c2, Expr::IdentPath(vec!["c".into()]));
    assert_eq!(**t2, Expr::IdentPath(vec!["d".into()]));
    assert_eq!(**e2, Expr::IdentPath(vec!["e".into()]));
}

#[test]
fn var_decl() {
    let src = r#"indicator("x")
var fast = ta.sma(close, 10)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected var decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Var));
    assert_eq!(v.name, "fast");
    let Expr::Call { path, args, .. } = &v.value else {
        panic!("expected call");
    };
    assert_eq!(path, &vec!["ta".to_string(), "sma".to_string()]);
    assert_eq!(args.len(), 2);
}

#[test]
fn varip_decl() {
    let src = r#"indicator("x")
varip ticks = 0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected varip decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Varip));
    assert_eq!(v.name, "ticks");
    assert_eq!(v.value, Expr::Int(0));
}

#[test]
fn varname_is_assign_not_var_keyword() {
    let src = r#"indicator("x")
varname = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { name, value, .. }) = &s.items[1] else {
        panic!("expected assign to varname");
    };
    assert_eq!(name, "varname");
    assert_eq!(*value, Expr::Int(1));
}

#[test]
fn typed_decl_primitive_float() {
    let src = r#"indicator("x")
float len = 14
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected typed var decl");
    };
    assert_eq!(v.qualifier, None);
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "len");
    assert_eq!(v.value, Expr::Int(14));
}

#[test]
fn qualified_const_untyped() {
    let src = r#"indicator("x")
const n = 0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected const decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Const));
    assert_eq!(v.ty, None);
    assert_eq!(v.name, "n");
    assert_eq!(v.value, Expr::Int(0));
}

#[test]
fn var_with_primitive_type() {
    let src = r#"indicator("x")
var float y = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected var + type decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Var));
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "y");
    assert_eq!(v.value, Expr::Int(1));
}

#[test]
fn input_dotted_call_is_expr_not_decl() {
    let src = r#"indicator("x")
x = input.int(9, "Lots")
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { name, value, .. }) = &s.items[1] else {
        panic!("expected assign, not misparsed input decl");
    };
    assert_eq!(name, "x");
    let Expr::Call { path, args, .. } = value else {
        panic!("expected call: {value:#?}");
    };
    assert_eq!(path, &vec!["input".to_string(), "int".to_string()]);
    assert_eq!(args.len(), 2);
}

#[test]
fn input_qualifier_decl_with_type() {
    let src = r#"indicator("x")
input float x = 1.0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::VarDecl(v)) = &s.items[1] else {
        panic!("expected input decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Input));
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "x");
    assert_eq!(v.value, Expr::Float(1.0));
}

#[test]
fn scientific_float() {
    let src = r#"indicator("x")
y = 1.5e-2
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Float(v) = value else {
        panic!("expected float");
    };
    assert!((v - 0.015).abs() < 1e-9);
}

#[test]
fn color_literal() {
    let src = r#"indicator("x")
c = color.red
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert_eq!(*value, Expr::Color("red".into()));
}

#[test]
fn unary_plus() {
    let src = r#"indicator("x")
z = +1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Unary {
        op: UnaryOp::Pos,
        expr,
    } = value
    else {
        panic!("expected unary +: {value:#?}");
    };
    assert_eq!(**expr, Expr::Int(1));
}

#[test]
fn generic_call_matrix_new() {
    let src = r#"indicator("x")
m = matrix.new<float>(2, 3)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Call {
        path,
        type_args,
        args,
    } = value
    else {
        panic!("expected call");
    };
    assert_eq!(path, &vec!["matrix".to_string(), "new".to_string()]);
    assert_eq!(
        type_args.as_deref(),
        Some(&[Type::Primitive(PrimitiveType::Float)][..])
    );
    assert_eq!(args.len(), 2);
}

#[test]
fn if_else_blocks() {
    let src = r#"indicator("x")
if true {
  x = 1
} else {
  x = 2
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::If(if_s)) = &s.items[1] else {
        panic!("expected if");
    };
    assert_eq!(if_s.then_body.len(), 1);
    assert!(matches!(
        if_s.else_body,
        Some(ElseBody::Block(ref b)) if b.len() == 1
    ));
}

#[test]
fn fn_short_form() {
    let src = r#"indicator("x")
f add(int a, int b) => a + b
y = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::FnDecl(f) = &s.items[1] else {
        panic!("expected fn decl");
    };
    assert_eq!(f.name, "add");
    assert_eq!(f.params.len(), 2);
    assert!(matches!(f.body, FnBody::Expr(Expr::Binary { .. })));
    assert!(matches!(&s.items[2], Item::Stmt(Stmt::Assign { .. })));
}
