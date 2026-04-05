use agentscript_compiler::{
    parse_script, AssignOp, BinOp, ElseBody, ExportDecl, Expr, FnBody, ImportDecl, Item,
    PrimitiveType, ScriptDeclaration, ScriptKind, Stmt, Type, UnaryOp, VarQualifier,
};

#[test]
fn empty_script() {
    let s = parse_script("empty", "").unwrap();
    assert_eq!(s.version, None);
    assert_eq!(s.agentscript_version, None);
    assert!(s.items.is_empty());
}

#[test]
fn version_and_indicator() {
    let src = "//@version=6\nindicator(\"x\")\n";
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(6));
    assert_eq!(s.agentscript_version, None);
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
fn version_five_no_newline_before_decl() {
    let src = "//@version=5 strategy(\"S\")";
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(5));
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
    let src = r#"//@version=5
/* head */
strategy("My", overlay=true, initial_capital=100000)
// tail
x = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.version, Some(5));
    assert_eq!(s.items.len(), 2);
    let Item::ScriptDecl(d) = &s.items[0] else {
        panic!("expected strategy decl");
    };
    assert_eq!(d.kind, ScriptKind::Strategy);
    assert_eq!(d.args.len(), 3);
    assert_eq!(d.args[0], (None, Expr::String("My".into())));
    assert_eq!(d.args[1], (Some("overlay".into()), Expr::Bool(true)));
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
    assert!(parse_script("t", "//@version=7").is_err());
}

#[test]
fn pine_version_one_rejected() {
    let r = parse_script("t.pine", "//@version=1\nindicator(\"x\")\n");
    assert!(r.is_err());
}

#[test]
fn version_five_accepted() {
    let s = parse_script("t", "//@version=5\nindicator(\"v5\")\n").unwrap();
    assert_eq!(s.version, Some(5));
}

#[test]
fn agentscript_directive_with_pine() {
    let src = "//@version=6\n// @agentscript=1\nindicator(\"x\")\n";
    let s = parse_script("t.qas", src).unwrap();
    assert_eq!(s.version, Some(6));
    assert_eq!(s.agentscript_version, Some(1));
}

#[test]
fn agentscript_directive_before_pine_version() {
    let src = "// @agentscript=2\n//@version=5\nindicator(\"x\")\n";
    let s = parse_script("t.qas", src).unwrap();
    assert_eq!(s.version, Some(5));
    assert_eq!(s.agentscript_version, Some(2));
}

#[test]
fn agentscript_without_pine_version() {
    let src = "// @agentscript=1\nindicator(\"x\")\n";
    let s = parse_script("t.qas", src).unwrap();
    assert_eq!(s.version, None);
    assert_eq!(s.agentscript_version, Some(1));
}

#[test]
fn duplicate_agentscript_directive_rejected() {
    let src = "// @agentscript=1\n// @agentscript=2\nindicator(\"x\")\n";
    assert!(parse_script("t", src).is_err());
}

#[test]
fn agentscript_version_zero_rejected() {
    assert!(parse_script("t", "// @agentscript=0\nindicator(\"x\")\n").is_err());
}

#[test]
fn agentscript_missing_number_rejected() {
    assert!(parse_script("t", "// @agentscript=\nindicator(\"x\")\n").is_err());
}

#[test]
fn agentscript_requires_space_after_slashes() {
    let src = "//@agentscript=1\nindicator(\"x\")\n";
    let s = parse_script("t", src).unwrap();
    assert_eq!(s.agentscript_version, None);
    assert_eq!(s.version, None);
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
fn array_literal_empty_and_elements() {
    let src = r#"indicator("I")
a = []
b = [1, 2 + 3, x]
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign a");
    };
    assert_eq!(*value, Expr::Array(vec![]));
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[2] else {
        panic!("expected assign b");
    };
    assert_eq!(
        *value,
        Expr::Array(vec![
            Expr::Int(1),
            Expr::Binary {
                op: BinOp::Add,
                left: Box::new(Expr::Int(2)),
                right: Box::new(Expr::Int(3)),
            },
            Expr::IdentPath(vec!["x".into()]),
        ])
    );
}

#[test]
fn array_literal_then_index() {
    let src = r#"indicator("I")
c = [10, 20][1]
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert_eq!(
        *value,
        Expr::Index {
            base: Box::new(Expr::Array(vec![Expr::Int(10), Expr::Int(20)])),
            index: Box::new(Expr::Int(1)),
        }
    );
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
    let Expr::Binary {
        op,
        left: l,
        right: r,
    } = right.as_ref()
    else {
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
    let Expr::Call { callee, args, .. } = value else {
        panic!("expected call");
    };
    assert_eq!(**callee, Expr::IdentPath(vec!["ta".into(), "sma".into()]));
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
    let Expr::Call { callee, args, .. } = e else {
        panic!("expected plot call");
    };
    assert_eq!(**callee, Expr::IdentPath(vec!["plot".into()]));
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
    let Expr::Binary {
        op: BinOp::Or,
        left,
        right,
    } = value
    else {
        panic!("expected or at root: {value:#?}");
    };
    assert_eq!(**right, Expr::IdentPath(vec!["c".into()]));
    let Expr::Binary {
        op: BinOp::And,
        left: l,
        right: r,
    } = left.as_ref()
    else {
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
    let Expr::Ternary {
        cond,
        then_b,
        else_b,
    } = value
    else {
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
    let Expr::Ternary {
        cond,
        then_b,
        else_b,
    } = value
    else {
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
    let Expr::Call { callee, args, .. } = &v.value else {
        panic!("expected call");
    };
    assert_eq!(**callee, Expr::IdentPath(vec!["ta".into(), "sma".into()]));
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
    let Expr::Call { callee, args, .. } = value else {
        panic!("expected call: {value:#?}");
    };
    assert_eq!(
        **callee,
        Expr::IdentPath(vec!["input".into(), "int".into()])
    );
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
        callee,
        type_args,
        args,
    } = value
    else {
        panic!("expected call");
    };
    assert_eq!(
        **callee,
        Expr::IdentPath(vec!["matrix".into(), "new".into()])
    );
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
fn else_if_chain() {
    let src = r#"indicator("x")
if a {
  x = 1
} else if b {
  x = 2
} else {
  x = 3
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::If(outer)) = &s.items[1] else {
        panic!("expected if");
    };
    let Some(ElseBody::If(inner)) = &outer.else_body else {
        panic!("expected else if: {:?}", outer.else_body);
    };
    assert_eq!(inner.then_body.len(), 1);
    assert!(matches!(
        inner.else_body,
        Some(ElseBody::Block(ref b)) if b.len() == 1
    ));
}

#[test]
fn for_loop_by_step() {
    let src = r#"indicator("x")
for i = 0 to 9 by 2 {
  y = i
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::For {
        var,
        from,
        to,
        by,
        body,
    }) = &s.items[1]
    else {
        panic!("expected for");
    };
    assert_eq!(var, "i");
    assert_eq!(*from, Expr::Int(0));
    assert_eq!(*to, Expr::Int(9));
    assert_eq!(*by, Some(Expr::Int(2)));
    assert_eq!(body.len(), 1);
}

#[test]
fn leading_dot_float() {
    let src = r#"indicator("x")
x = .25 + .5e0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Binary {
        op: BinOp::Add,
        left,
        right,
    } = value
    else {
        panic!("expected add");
    };
    assert_eq!(**left, Expr::Float(0.25));
    assert_eq!(**right, Expr::Float(0.5));
}

#[test]
fn switch_default_only_braced() {
    let src = r#"indicator("x")
switch z {
  => {
    b = 2
  }
}
"#;
    parse_script("t.pine", src).expect("default-only switch should parse");
}

#[test]
fn switch_case_int_literal() {
    let src = r#"indicator("x")
switch z {
  1 => { a = 1 }
}
"#;
    parse_script("t.pine", src).expect("int case label should parse");
}

#[test]
fn switch_two_cases_no_default() {
    let src = r#"indicator("x")
switch z {
  x => { a = 1 }
  y => { b = 2 }
}
"#;
    parse_script("t.pine", src).expect("two switch arms should parse");
}

#[test]
fn switch_case_plus_default_minimal() {
    let src = "switch z {\n  x => { a = 1 }\n  => { b = 2 }\n}\n";
    let r = parse_script("t.pine", src);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn switch_case_plus_default_after_indicator() {
    let src = "indicator(\"x\")\nswitch z {\n  x => { a = 1 }\n  => { b = 2 }\n}\n";
    let r = parse_script("t.pine", src);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn switch_default_block_two_stmts() {
    let src =
        "indicator(\"x\")\nswitch z {\n  x => { a = 1 }\n  => {\n    b = 2\n    c = 3\n  }\n}\n";
    let r = parse_script("t.pine", src);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn switch_default_multiline_block_one_stmt() {
    let src = "indicator(\"x\")\nswitch z {\n  x => { a = 1 }\n  => {\n    b = 2\n  }\n}\n";
    let r = parse_script("t.pine", src);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn switch_default_braced_block() {
    let src = r#"indicator("x")
switch z {
  x => { a = 1 }
  => {
    b = 2
    c = 3
  }
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Switch { default, .. }) = &s.items[1] else {
        panic!("expected switch");
    };
    let Some(d) = default else {
        panic!("expected default");
    };
    let Stmt::Block(stmts) = d.as_ref() else {
        panic!("expected block default: {d:#?}");
    };
    assert_eq!(stmts.len(), 2);
}

#[test]
fn mcp_namespace_call() {
    let src = r#"indicator("x")
r = mcp.call("tool", syminfo.ticker)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let Expr::Call { callee, args, .. } = value else {
        panic!("expected call");
    };
    assert_eq!(**callee, Expr::IdentPath(vec!["mcp".into(), "call".into()]));
    assert_eq!(args.len(), 2);
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
    assert!(!f.is_method);
    assert_eq!(f.name, "add");
    assert_eq!(f.params.len(), 2);
    assert!(matches!(f.body, FnBody::Expr(Expr::Binary { .. })));
    assert!(matches!(&s.items[2], Item::Stmt(Stmt::Assign { .. })));
}

#[test]
fn pine_function_without_f_keyword() {
    let src = r#"indicator("x")
add(int a, int b) => a + b
y = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::FnDecl(f) = &s.items[1] else {
        panic!("expected Pine-style fn decl");
    };
    assert!(!f.is_method);
    assert_eq!(f.name, "add");
}

#[test]
fn pine_function_named_f_uses_pine_form() {
    let src = r#"indicator("x")
f() => 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::FnDecl(f) = &s.items[1] else {
        panic!("expected fn named f");
    };
    assert_eq!(f.name, "f");
    assert!(f.params.is_empty());
}

#[test]
fn method_declaration_sets_flag() {
    let src = r#"indicator("x")
method push(array<float> id, float v) => id
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::FnDecl(f) = &s.items[1] else {
        panic!("expected method decl");
    };
    assert!(f.is_method);
    assert_eq!(f.name, "push");
}

#[test]
fn compound_assignment_plus_eq() {
    let src = r#"indicator("x")
n = 0
n += 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { name, op, .. }) = &s.items[2] else {
        panic!("expected assign");
    };
    assert_eq!(name, "n");
    assert_eq!(*op, AssignOp::PlusEq);
}

#[test]
fn pine_import_line() {
    let src = "import TradingView/ta/5 as ta\nindicator(\"x\")\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Import(ImportDecl { path, alias }) = &s.items[0] else {
        panic!("expected import");
    };
    assert_eq!(
        path,
        &vec!["TradingView".to_string(), "ta".to_string(), "5".to_string(),]
    );
    assert_eq!(alias, "ta");
}

#[test]
fn pine_export_function() {
    let src = "library(\"L\")\nexport f inc(float x) => x + 1\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Export(ExportDecl::Fn(f)) = &s.items[1] else {
        panic!("expected export f");
    };
    assert_eq!(f.name, "inc");
    assert_eq!(f.params.len(), 1);
}

#[test]
fn pine_export_without_f_keyword() {
    let src = "library(\"L\")\nexport inc(float x) => x + 1\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Export(ExportDecl::Fn(f)) = &s.items[1] else {
        panic!("expected export fn");
    };
    assert!(!f.is_method);
    assert_eq!(f.name, "inc");
}

#[test]
fn pine_export_var() {
    let src = "library(\"L\")\nexport var N = 42\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Export(ExportDecl::Var(v)) = &s.items[1] else {
        panic!("expected export var");
    };
    assert_eq!(v.name, "N");
    assert_eq!(v.value, Expr::Int(42));
}

#[test]
fn while_loop_parses() {
    let src = "indicator(\"x\")\nwhile i < 10 {\n  i := i + 1\n}\n";
    parse_script("t.pine", src).expect("while loop");
}

#[test]
fn hex_color_rrggbb() {
    let src = "indicator(\"x\")\nc = #ff00Aa\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("assign");
    };
    assert_eq!(*value, Expr::HexColor("ff00Aa".into()));
}

#[test]
fn postfix_call_on_grouped_expr() {
    let src = "indicator(\"x\")\ny = (close + open).m()\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("assign");
    };
    let Expr::Call { callee, args, .. } = value else {
        panic!("expected call, got {value:#?}");
    };
    assert!(args.is_empty());
    let Expr::Member { base, field } = callee.as_ref() else {
        panic!("member callee: {callee:#?}");
    };
    assert_eq!(field, "m");
    assert!(matches!(base.as_ref(), Expr::Binary { .. }));
}

#[test]
fn dotted_ident_stays_ident_path() {
    let src = "indicator(\"x\")\ny = syminfo.ticker\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt::Assign { value, .. }) = &s.items[1] else {
        panic!("assign");
    };
    assert_eq!(
        *value,
        Expr::IdentPath(vec!["syminfo".into(), "ticker".into()])
    );
}
