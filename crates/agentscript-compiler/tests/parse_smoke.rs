use agentscript_compiler::{
    parse_and_analyze, parse_script, AssignOp, BinOp, ElseBody, ExportDecl, Expr, ExprKind,
    FnBody, ForInPattern, ImportDecl, Item, PrimitiveType, ScriptDeclaration, ScriptKind, Span,
    Stmt, StmtKind, Type, UnaryOp, VarQualifier,
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
        span,
        ..
    }) = &s.items[0]
    else {
        panic!("expected indicator decl");
    };
    assert_ne!(*span, Span::DUMMY, "script decl should carry a source span");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, None);
    assert_eq!(args[0].1.kind, ExprKind::String("x".into()));
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
    assert_eq!(d.args[0].0, None);
    assert_eq!(d.args[0].1.kind, ExprKind::String("My".into()));
    assert_eq!(d.args[1].0, Some("overlay".into()));
    assert_eq!(d.args[1].1.kind, ExprKind::Bool(true));
    assert_eq!(d.args[2].0, Some("initial_capital".into()));
    assert_eq!(d.args[2].1.kind, ExprKind::Int(100_000));
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, op, value }, .. }) = &s.items[1] else {
        panic!("expected assignment");
    };
    assert_eq!(name, "x");
    assert_eq!(*op, AssignOp::Eq);
    assert_eq!(value.kind, ExprKind::Int(1));
}

#[test]
fn qualified_ident_positional() {
    let src = "strategy(\"x\", strategy.long)\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::ScriptDecl(d) = &s.items[0] else {
        panic!("expected decl");
    };
    assert_eq!(
        d.args[1].1.kind,
        ExprKind::IdentPath(vec!["strategy".into(), "long".into()])
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign a");
    };
    assert_eq!(value.kind, ExprKind::Array(vec![]));
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[2] else {
        panic!("expected assign b");
    };
    assert!(value.shape_eq(&Expr::synthetic(ExprKind::Array(vec![
        Expr::synthetic(ExprKind::Int(1)),
        Expr::synthetic(ExprKind::Binary {
            op: BinOp::Add,
            left: Box::new(Expr::synthetic(ExprKind::Int(2))),
            right: Box::new(Expr::synthetic(ExprKind::Int(3))),
        }),
        Expr::synthetic(ExprKind::IdentPath(vec!["x".into()])),
    ]))));
}

#[test]
fn array_literal_then_index() {
    let src = r#"indicator("I")
c = [10, 20][1]
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert!(value.shape_eq(&Expr::synthetic(ExprKind::Index {
        base: Box::new(Expr::synthetic(ExprKind::Array(vec![
            Expr::synthetic(ExprKind::Int(10)),
            Expr::synthetic(ExprKind::Int(20)),
        ]))),
        index: Box::new(Expr::synthetic(ExprKind::Int(1))),
    })));
}

#[test]
fn assign_colon_eq_and_binary() {
    let src = r#"indicator("I")
a := 1 + 2 * 3
b = a == 2
"#;
    let s = parse_script("t.pine", src).unwrap();
    assert_eq!(s.items.len(), 3);
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, op, value }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert_eq!(name, "a");
    assert_eq!(*op, AssignOp::ColonEq);
    let ExprKind::Binary { op, left, right } = &value.kind else {
        panic!("expected binary +");
    };
    assert_eq!(*op, BinOp::Add);
    assert!(left.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(1))));
    let ExprKind::Binary {
        op,
        left: l,
        right: r,
    } = &right.as_ref().kind
    else {
        panic!("expected *");
    };
    assert_eq!(*op, BinOp::Mul);
    assert!(l.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(2))));
    assert!(r.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(3))));

    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, value, .. }, .. }) = &s.items[2] else {
        panic!("expected assign b");
    };
    assert_eq!(name, "b");
    let ExprKind::Binary { op, .. } = &value.kind else {
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, value, .. }, .. }) = &s.items[1] else {
        panic!("expected y assign");
    };
    assert_eq!(name, "y");
    let ExprKind::Call { callee, args, .. } = &value.kind else {
        panic!("expected call");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "ta".into(),
        "sma".into(),
    ]))));
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, None);
    assert!(args[0].1.shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["close".into()]))));
    assert_eq!(args[1].0, None);
    assert!(args[1].1.shape_eq(&Expr::synthetic(ExprKind::Int(20))));

    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, value, .. }, .. }) = &s.items[2] else {
        panic!("expected z assign");
    };
    assert_eq!(name, "z");
    let ExprKind::Index { base, index } = &value.kind else {
        panic!("expected index");
    };
    assert!(base.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["close".into()]))));
    assert!(index.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(1))));
}

#[test]
fn expr_stmt_call() {
    let src = r#"indicator("x")
plot(close)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Expr(e), .. }) = &s.items[1] else {
        panic!("expected expr stmt");
    };
    let ExprKind::Call { callee, args, .. } = &e.kind else {
        panic!("expected plot call");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["plot".into()]))));
    assert_eq!(args.len(), 1);
    assert!(args[0].1.shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["close".into()]))));
}

#[test]
fn logical_and_or_not() {
    let src = r#"indicator("x")
ok = not a and b or c
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    // Parsed as (not a) and b or c  —  or lowest:  ((not a) and b) or c
    let ExprKind::Binary {
        op: BinOp::Or,
        left,
        right,
    } = &value.kind
    else {
        panic!("expected or at root: {value:#?}");
    };
    assert!(right.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["c".into()]))));
    let ExprKind::Binary {
        op: BinOp::And,
        left: l,
        right: r,
    } = &left.as_ref().kind
    else {
        panic!("expected and: {left:#?}");
    };
    assert!(r.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["b".into()]))));
    let ExprKind::Unary {
        op: agentscript_compiler::UnaryOp::Not,
        expr: inner,
    } = &l.as_ref().kind
    else {
        panic!("expected not a: {l:#?}");
    };
    assert!(inner.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["a".into()]))));
}

#[test]
fn ternary_simple() {
    let src = r#"indicator("x")
x = true ? 1 : 2
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Ternary {
        cond,
        then_b,
        else_b,
    } = &value.kind
    else {
        panic!("expected ternary: {value:#?}");
    };
    assert!(cond.as_ref().shape_eq(&Expr::synthetic(ExprKind::Bool(true))));
    assert!(then_b.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(1))));
    assert!(else_b.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(2))));
}

#[test]
fn ternary_right_associative() {
    let src = r#"indicator("x")
x = a ? b : c ? d : e
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Ternary {
        cond,
        then_b,
        else_b,
    } = &value.kind
    else {
        panic!("expected outer ternary: {value:#?}");
    };
    assert!(cond.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["a".into()]))));
    assert!(then_b.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["b".into()]))));
    let ExprKind::Ternary {
        cond: c2,
        then_b: t2,
        else_b: e2,
    } = &else_b.as_ref().kind
    else {
        panic!("expected nested ternary in else: {else_b:#?}");
    };
    assert!(c2.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["c".into()]))));
    assert!(t2.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["d".into()]))));
    assert!(e2.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["e".into()]))));
}

#[test]
fn var_decl() {
    let src = r#"indicator("x")
var fast = ta.sma(close, 10)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected var decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Var));
    assert_eq!(v.name, "fast");
    let ExprKind::Call { callee, args, .. } = &v.value.kind else {
        panic!("expected call");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "ta".into(),
        "sma".into(),
    ]))));
    assert_eq!(args.len(), 2);
}

#[test]
fn varip_decl() {
    let src = r#"indicator("x")
varip ticks = 0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected varip decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Varip));
    assert_eq!(v.name, "ticks");
    assert_eq!(v.value.kind, ExprKind::Int(0));
}

#[test]
fn varname_is_assign_not_var_keyword() {
    let src = r#"indicator("x")
varname = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign to varname");
    };
    assert_eq!(name, "varname");
    assert_eq!(value.kind, ExprKind::Int(1));
}

#[test]
fn typed_decl_primitive_float() {
    let src = r#"indicator("x")
float len = 14
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected typed var decl");
    };
    assert_eq!(v.qualifier, None);
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "len");
    assert_eq!(v.value.kind, ExprKind::Int(14));
}

#[test]
fn qualified_const_untyped() {
    let src = r#"indicator("x")
const n = 0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected const decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Const));
    assert_eq!(v.ty, None);
    assert_eq!(v.name, "n");
    assert_eq!(v.value.kind, ExprKind::Int(0));
}

#[test]
fn var_with_primitive_type() {
    let src = r#"indicator("x")
var float y = 1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected var + type decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Var));
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "y");
    assert_eq!(v.value.kind, ExprKind::Int(1));
}

#[test]
fn input_dotted_call_is_expr_not_decl() {
    let src = r#"indicator("x")
x = input.int(9, "Lots")
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign, not misparsed input decl");
    };
    assert_eq!(name, "x");
    let ExprKind::Call { callee, args, .. } = &value.kind else {
        panic!("expected call: {value:#?}");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "input".into(),
        "int".into(),
    ]))));
    assert_eq!(args.len(), 2);
}

#[test]
fn input_qualifier_decl_with_type() {
    let src = r#"indicator("x")
input float x = 1.0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected input decl");
    };
    assert_eq!(v.qualifier, Some(VarQualifier::Input));
    assert_eq!(v.ty, Some(Type::Primitive(PrimitiveType::Float)));
    assert_eq!(v.name, "x");
    assert!(v
        .value
        .shape_eq(&Expr::synthetic(ExprKind::Float(1.0))));
}

#[test]
fn scientific_float() {
    let src = r#"indicator("x")
y = 1.5e-2
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Float(v) = &value.kind else {
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    assert_eq!(
        value.kind,
        ExprKind::IdentPath(vec!["color".into(), "red".into()])
    );
}

#[test]
fn unary_plus() {
    let src = r#"indicator("x")
z = +1
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Unary {
        op: UnaryOp::Pos,
        expr,
    } = &value.kind
    else {
        panic!("expected unary +: {value:#?}");
    };
    assert!(expr.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(1))));
}

#[test]
fn array_from_call() {
    let src = r#"indicator("x")
a = array.from(1, 2, 3)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &value.kind
    else {
        panic!("expected call: {value:#?}");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "array".into(),
        "from".into(),
    ]))));
    assert!(type_args.is_none());
    assert_eq!(args.len(), 3);
    assert_eq!(args[0].0, None);
    assert!(args[0].1.shape_eq(&Expr::synthetic(ExprKind::Int(1))));
    assert_eq!(args[1].0, None);
    assert!(args[1].1.shape_eq(&Expr::synthetic(ExprKind::Int(2))));
    assert_eq!(args[2].0, None);
    assert!(args[2].1.shape_eq(&Expr::synthetic(ExprKind::Int(3))));
}

#[test]
fn generic_call_matrix_new() {
    let src = r#"indicator("x")
m = matrix.new<float>(2, 3)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Call {
        callee,
        type_args,
        args,
    } = &value.kind
    else {
        panic!("expected call");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "matrix".into(),
        "new".into(),
    ]))));
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
    let Item::Stmt(Stmt { kind: StmtKind::If(if_s), .. }) = &s.items[1] else {
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
    let Item::Stmt(Stmt { kind: StmtKind::If(outer), .. }) = &s.items[1] else {
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
fn break_continue_in_loops() {
    let src = r#"indicator("x")
while true {
  break
}
for i = 0 to 1 {
  continue
}
"#;
    parse_script("t.pine", src).expect("break/continue in loops should parse");
    agentscript_compiler::parse_and_analyze("t.pine", src).expect("analyze should accept");
}

#[test]
fn break_outside_loop_rejected_by_analyze() {
    let src = "indicator(\"x\")\nbreak\n";
    let e = agentscript_compiler::parse_and_analyze("t.pine", src).unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("break") && (msg.contains("for") || msg.contains("while")),
        "{msg}"
    );
}

#[test]
fn switch_without_scrutinee_braced() {
    let src = r#"indicator("x")
switch {
  a => { x = 1 }
  => { y = 2 }
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Switch { scrutinee, .. }, .. }) = &s.items[1] else {
        panic!("expected switch");
    };
    assert!(scrutinee.is_none());
}

#[test]
fn for_in_single_element() {
    let src = r#"indicator("x")
for el in arr {
  y = el
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::ForIn {
        pattern,
        iterable,
        body,
    }, .. }) = &s.items[1]
    else {
        panic!("expected for-in");
    };
    assert_eq!(pattern, &ForInPattern::Name("el".into()));
    assert!(iterable.shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["arr".into()]))));
    assert_eq!(body.len(), 1);
}

#[test]
fn for_in_index_value_pair() {
    let src = r#"indicator("x")
for [i, v] in m {
  y = v
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::ForIn { pattern, .. }, .. }) = &s.items[1] else {
        panic!("expected for-in");
    };
    assert_eq!(
        pattern,
        &ForInPattern::Pair("i".into(), "v".into())
    );
}

#[test]
fn if_expression_else_if_chain() {
    let src = r#"indicator("x")
y = if a 1 else if b 2 else 3
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::IfExpr { cond, then_b, else_b } = &value.kind else {
        panic!("expected if expr, got {value:#?}");
    };
    assert!(cond.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["a".into()]))));
    assert!(then_b.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(1))));
    let ExprKind::IfExpr {
        cond: c2,
        then_b: t2,
        else_b: e2,
    } = &else_b.as_ref().kind
    else {
        panic!("expected else-if as nested IfExpr");
    };
    assert!(c2.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec!["b".into()]))));
    assert!(t2.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(2))));
    assert!(e2.as_ref().shape_eq(&Expr::synthetic(ExprKind::Int(3))));
}

#[test]
fn tuple_destructure_assign() {
    let src = r#"indicator("x")
[a, b, c] = t
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::TupleAssign { names, op, value }, .. }) = &s.items[1] else {
        panic!("expected tuple assign");
    };
    assert_eq!(
        names,
        &vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ]
    );
    assert_eq!(*op, AssignOp::Eq);
    assert_eq!(
        value.kind,
        ExprKind::IdentPath(vec!["t".into()])
    );
}

#[test]
fn float_array_bracket_type() {
    let src = r#"indicator("x")
float[] xs = array.new<float>(0)
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected var decl");
    };
    assert_eq!(
        v.ty,
        Some(Type::Array(Box::new(Type::Primitive(PrimitiveType::Float))))
    );
    assert_eq!(v.name, "xs");
}

#[test]
fn enum_braced_string_variants() {
    let src = r#"indicator("x")
enum tz {
  utc = "UTC"
  ny = "America/New_York"
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Enum(e) = &s.items[1] else {
        panic!("expected enum");
    };
    assert_eq!(e.name, "tz");
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.variants[0].name, "utc");
    assert_eq!(e.variants[0].value.kind, ExprKind::String("UTC".into()));
    assert_eq!(
        e.variants[1].value.kind,
        ExprKind::String("America/New_York".into())
    );
}

#[test]
fn type_udt_float_fields() {
    let src = r#"indicator("x")
type bar {
  float o = open
  float c = close
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::TypeDef(t) = &s.items[1] else {
        panic!("expected type def");
    };
    assert_eq!(t.name, "bar");
    assert_eq!(t.fields.len(), 2);
    assert_eq!(t.fields[0].name, "o");
    assert!(matches!(
        t.fields[0].ty,
        Type::Primitive(PrimitiveType::Float)
    ));
    assert!(t.fields[0].default.shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "open".into(),
    ]))));
}

#[test]
fn export_enum_in_library() {
    let src = r#"//@version=6
library("L")
export enum sym {
  a = "A"
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Export(ExportDecl::Enum(e)) = &s.items[1] else {
        panic!("expected export enum");
    };
    assert_eq!(e.name, "sym");
    assert_eq!(e.variants.len(), 1);
}

#[test]
fn map_named_key_type() {
    let src = r#"indicator("x")
map<symbols, float> m = map.new<symbols, float>()
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::VarDecl(v), .. }) = &s.items[1] else {
        panic!("expected var");
    };
    assert_eq!(
        v.ty,
        Some(Type::Map(
            Box::new(Type::Named("symbols".into())),
            Box::new(Type::Primitive(PrimitiveType::Float))
        ))
    );
}

#[test]
fn udt_field_varip_qualifier() {
    let src = r#"indicator("x")
type b {
  varip int ticks = -1
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::TypeDef(t) = &s.items[1] else {
        panic!("expected type");
    };
    assert_eq!(t.fields[0].qualifier, Some(VarQualifier::Varip));
    assert_eq!(t.fields[0].name, "ticks");
    assert_eq!(t.fields[0].default.kind, ExprKind::Int(-1));
}

#[test]
fn for_loop_by_step() {
    let src = r#"indicator("x")
for i = 0 to 9 by 2 {
  y = i
}
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::For {
        var,
        from,
        to,
        by,
        body,
    }, .. }) = &s.items[1]
    else {
        panic!("expected for");
    };
    assert_eq!(var, "i");
    assert_eq!(from.kind, ExprKind::Int(0));
    assert_eq!(to.kind, ExprKind::Int(9));
    assert!(by
        .as_ref()
        .is_some_and(|e| e.kind == ExprKind::Int(2)));
    assert_eq!(body.len(), 1);
}

#[test]
fn leading_dot_float() {
    let src = r#"indicator("x")
x = .25 + .5e0
"#;
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Binary {
        op: BinOp::Add,
        left,
        right,
    } = &value.kind
    else {
        panic!("expected add");
    };
    assert!(left.as_ref().shape_eq(&Expr::synthetic(ExprKind::Float(0.25))));
    assert!(right.as_ref().shape_eq(&Expr::synthetic(ExprKind::Float(0.5))));
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
    let Item::Stmt(Stmt { kind: StmtKind::Switch { default, .. }, .. }) = &s.items[1] else {
        panic!("expected switch");
    };
    let Some(d) = default else {
        panic!("expected default");
    };
    let Stmt { kind: StmtKind::Block(stmts), .. } = d.as_ref() else {
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("expected assign");
    };
    let ExprKind::Call { callee, args, .. } = &value.kind else {
        panic!("expected call");
    };
    assert!(callee.as_ref().shape_eq(&Expr::synthetic(ExprKind::IdentPath(vec![
        "mcp".into(),
        "call".into(),
    ]))));
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
    assert!(matches!(f.body, FnBody::Expr(Expr { kind: ExprKind::Binary { .. }, .. })));
    assert!(matches!(&s.items[2], Item::Stmt(Stmt { kind: StmtKind::Assign { .. }, .. })));
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { name, op, .. }, .. }) = &s.items[2] else {
        panic!("expected assign");
    };
    assert_eq!(name, "n");
    assert_eq!(*op, AssignOp::PlusEq);
}

#[test]
fn pine_import_line() {
    let src = "import TradingView/ta/5 as ta\nindicator(\"x\")\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Import(ImportDecl { path, alias, .. }) = &s.items[0] else {
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
    assert_eq!(v.value.kind, ExprKind::Int(42));
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
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("assign");
    };
    assert_eq!(value.kind, ExprKind::HexColor("ff00Aa".into()));
}

#[test]
fn postfix_call_on_grouped_expr() {
    let src = "indicator(\"x\")\ny = (close + open).m()\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("assign");
    };
    let ExprKind::Call { callee, args, .. } = &value.kind else {
        panic!("expected call, got {value:#?}");
    };
    assert!(args.is_empty());
    let ExprKind::Member { base, field } = &callee.as_ref().kind else {
        panic!("member callee: {callee:#?}");
    };
    assert_eq!(field, "m");
    assert!(matches!(&base.as_ref().kind, ExprKind::Binary { .. }));
}

#[test]
fn dotted_ident_stays_ident_path() {
    let src = "indicator(\"x\")\ny = syminfo.ticker\n";
    let s = parse_script("t.pine", src).unwrap();
    let Item::Stmt(Stmt { kind: StmtKind::Assign { value, .. }, .. }) = &s.items[1] else {
        panic!("assign");
    };
    assert_eq!(
        value.kind,
        ExprKind::IdentPath(vec!["syminfo".into(), "ticker".into()])
    );
}

#[test]
fn examples_uptrend_pine_parse_and_analyze() {
    let src = include_str!("../../../examples/uptrend.pine");
    parse_and_analyze("examples/uptrend.pine", src).expect("repo example parse + analyze");
}

#[test]
fn fixture_minimal_strategy_parse_and_analyze() {
    let src = include_str!("fixtures/minimal_strategy.pine");
    let s = parse_and_analyze("minimal_strategy.pine", src).expect("fixture parse + analyze");
    assert_eq!(s.version, Some(6));
    assert!(matches!(
        &s.items[0],
        Item::ScriptDecl(ScriptDeclaration {
            kind: ScriptKind::Strategy,
            ..
        })
    ));
    assert!(matches!(&s.items[1], Item::Import(_)));
    assert!(s.items.len() >= 5);
}
