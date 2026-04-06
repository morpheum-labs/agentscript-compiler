//! Top-level script: header directives, imports, exports, declarations, statements.

use chumsky::prelude::*;

use crate::frontend::ast::{
    ElseBody, EnumDef, EnumVariant, ExportDecl, Expr, FnBody, FnDecl, FnParam, ForInPattern,
    IfStmt, ImportDecl, Item, NodeId, Script, ScriptDeclaration, ScriptKind, Span, Stmt, StmtKind,
    Type, UdtField, UserTypeDef, VarDecl, VarQualifier,
};

use super::assign_type::{assign_op, type_parser, type_parser_decl_root, var_qualifier};
use super::expr::expr_parser;
use super::lex::{
    agentscript_directive, fat_arrow, optional_semicolon, pad, pad_non_empty, version_directive,
};

#[derive(Clone, Copy)]
enum HeaderDirective {
    Pine(u32),
    AgentScript(u32),
}

fn fold_header_directives(
    pieces: Vec<HeaderDirective>,
    span: std::ops::Range<usize>,
) -> Result<(Option<u32>, Option<u32>), Simple<char>> {
    let mut pine = None;
    let mut agentscript = None;
    for p in pieces {
        match p {
            HeaderDirective::Pine(v) => {
                if pine.replace(v).is_some() {
                    return Err(Simple::custom(
                        span.clone(),
                        "duplicate //@version= directive",
                    ));
                }
            }
            HeaderDirective::AgentScript(v) => {
                if agentscript.replace(v).is_some() {
                    return Err(Simple::custom(
                        span.clone(),
                        "duplicate // @agentscript= directive",
                    ));
                }
            }
        }
    }
    Ok((pine, agentscript))
}

/// Optional Pine `//@version=` and/or `// @agentscript=` lines, then declarations and statements until EOF.
pub fn script_parser() -> impl Parser<char, Script, Error = Simple<char>> {
    let expr = expr_parser().boxed();

    let decl_args = {
        let e = expr.clone();
        let na = choice((
            text::ident()
                .then_ignore(choice((just('='), pad_non_empty().ignore_then(just('=')))))
                .then_ignore(pad())
                .then(e.clone())
                .map(|(name, v)| (Some(name), v)),
            e.clone().map(|v| (None, v)),
        ))
        .boxed();
        na.separated_by(just(',').ignore_then(pad()))
            .allow_trailing()
            .delimited_by(just('(').ignore_then(pad()), pad().ignore_then(just(')')))
    };

    let decl = choice((
        text::keyword("indicator").to(ScriptKind::Indicator),
        text::keyword("strategy").to(ScriptKind::Strategy),
        text::keyword("library").to(ScriptKind::Library),
    ))
    .then_ignore(pad())
    .then(decl_args)
    .map(|(kind, args)| Item::ScriptDecl(ScriptDeclaration { kind, args }))
    .boxed();

    let expr_for_stmt = expr.clone();

    // After `var`/`const`/…, optional type must not use a bare `Type::Named` at the root, or we
    // consume the variable name (`var fast = …` would parse `fast` as the type). UDT form uses
    // two idents: `var MyType x = …`.
    let name_after_qual = choice((
        type_parser_decl_root()
            .then_ignore(pad())
            .then(text::ident())
            .map(|(ty, name)| (Some(ty), name)),
        text::ident()
            .then_ignore(pad())
            .then(text::ident())
            .map(|(ty_name, name)| (Some(Type::Named(ty_name)), name)),
        text::ident().map(|name| (None, name)),
    ))
    .boxed();

    let var_decl_qualified = var_qualifier()
        .then_ignore(pad())
        .then(name_after_qual.clone())
        .then_ignore(pad())
        .then(assign_op())
        .then_ignore(pad())
        .then(expr_for_stmt.clone())
        .map_with_span(|(((qual, (ty, name)), _op), value), span| {
            let sp: Span = span.into();
            Stmt::new(
                sp,
                StmtKind::VarDecl(VarDecl {
                    span: sp,
                    qualifier: Some(qual),
                    ty,
                    name,
                    value,
                }),
            )
        })
        .boxed();

    let var_decl_input = text::keyword("input")
        .ignore_then(pad())
        .ignore_then(type_parser().or_not())
        .then_ignore(pad())
        .then(text::ident())
        .then_ignore(pad())
        .then(assign_op())
        .then_ignore(pad())
        .then(expr_for_stmt.clone())
        .map_with_span(|(((ty, name), _op), value), span| {
            let sp: Span = span.into();
            Stmt::new(
                sp,
                StmtKind::VarDecl(VarDecl {
                    span: sp,
                    qualifier: Some(VarQualifier::Input),
                    ty,
                    name,
                    value,
                }),
            )
        })
        .boxed();

    let var_decl_typed = type_parser()
        .then_ignore(pad())
        .then(text::ident())
        .then_ignore(pad())
        .then(assign_op())
        .then_ignore(pad())
        .then(expr_for_stmt.clone())
        .map_with_span(|(((ty, name), _op), value), span| {
            let sp: Span = span.into();
            Stmt::new(
                sp,
                StmtKind::VarDecl(VarDecl {
                    span: sp,
                    qualifier: None,
                    ty: Some(ty),
                    name,
                    value,
                }),
            )
        })
        .boxed();

    let var_decl_qualified_stmt = var_decl_qualified.clone();
    let var_decl_input_stmt = var_decl_input.clone();
    let var_decl_typed_stmt = var_decl_typed.clone();

    let stmt = recursive(move |stmt| {
        let stmt = stmt.boxed();
        let expr_if = expr_for_stmt.clone();

        let compound_vec = || {
            just('{')
                .ignore_then(pad())
                .ignore_then(pad().ignore_then(stmt.clone()).repeated())
                .then_ignore(pad())
                .then_ignore(just('}'))
        };

        // `else if` via recursive [`IfStmt`]: `else` + (`if` + … | `{` … `}`).
        let if_stmt_ast = recursive(|if_ast| {
            text::keyword("if")
                .ignore_then(pad())
                .ignore_then(expr_if.clone())
                .then_ignore(pad())
                .then(compound_vec())
                .then(
                    pad()
                        .ignore_then(text::keyword("else"))
                        .ignore_then(pad())
                        .ignore_then(choice((
                            if_ast.clone().map(|inner| ElseBody::If(Box::new(inner))),
                            compound_vec().map(ElseBody::Block),
                        )))
                        .or_not(),
                )
                .map(|((cond, then_body), else_body)| IfStmt {
                    cond,
                    then_body,
                    else_body,
                })
        });
        let if_stmt =
            if_stmt_ast.map_with_span(|if_stmt, span| Stmt::new(span, StmtKind::If(if_stmt)));

        let for_classic = text::ident()
            .then_ignore(pad())
            .then_ignore(just('='))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then_ignore(text::keyword("to"))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then(
                pad()
                    .ignore_then(text::keyword("by"))
                    .ignore_then(pad())
                    .ignore_then(expr_for_stmt.clone())
                    .or_not(),
            )
            .then_ignore(pad())
            .then(compound_vec())
            .map_with_span(|((((var, from), to), by), body), span| {
                Stmt::new(
                    span,
                    StmtKind::For {
                        var,
                        from,
                        to,
                        by,
                        body,
                    },
                )
            });

        let for_in_pair = just('[')
            .ignore_then(pad().ignore_then(text::ident()))
            .then_ignore(pad())
            .then_ignore(just(','))
            .then_ignore(pad())
            .then(text::ident())
            .then_ignore(pad())
            .then_ignore(just(']'))
            .then_ignore(pad())
            .then_ignore(text::keyword("in"))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then(compound_vec())
            .map_with_span(|(((idx, val), iterable), body), span| {
                Stmt::new(
                    span,
                    StmtKind::ForIn {
                        pattern: ForInPattern::Pair(idx, val),
                        iterable,
                        body,
                    },
                )
            });

        let for_in_single = text::ident()
            .then_ignore(pad())
            .then_ignore(text::keyword("in"))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then(compound_vec())
            .map_with_span(|((var, iterable), body), span| {
                Stmt::new(
                    span,
                    StmtKind::ForIn {
                        pattern: ForInPattern::Name(var),
                        iterable,
                        body,
                    },
                )
            });

        let for_stmt = text::keyword("for").ignore_then(
            pad().ignore_then(choice((for_in_pair, for_in_single, for_classic))),
        );

        let switch_arm = choice((
            compound_vec().map_with_span(|mut v, span| match v.len() {
                0 => Stmt::new(span, StmtKind::Block(vec![])),
                1 => v.pop().expect("one stmt"),
                _ => Stmt::new(span, StmtKind::Block(v)),
            }),
            stmt.clone(),
        ));

        // `=>` default must be tried before another `case =>`: otherwise `expr` may consume
        // the leading `=` of `=>` while attempting `==`, leaving `>` and a bogus error.
        #[derive(Clone)]
        enum SwitchEl {
            Case((Expr, Stmt)),
            Default(Stmt),
        }

        let switch_el = pad().ignore_then(choice((
            fat_arrow()
                .ignore_then(pad())
                .ignore_then(switch_arm.clone())
                .map(SwitchEl::Default),
            expr_for_stmt
                .clone()
                .then_ignore(pad())
                .then_ignore(fat_arrow())
                .then_ignore(pad())
                .then(switch_arm.clone())
                .map(|(e, s)| SwitchEl::Case((e, s))),
        )));

        let switch_body = just('{')
            .ignore_then(pad())
            .ignore_then(switch_el.repeated())
            .then_ignore(pad())
            .then_ignore(just('}'))
            .try_map(|elements, span| {
                let mut cases = Vec::new();
                let mut default: Option<Stmt> = None;
                for el in elements {
                    match el {
                        SwitchEl::Case(pair) => {
                            if default.is_some() {
                                return Err(Simple::custom(
                                    span.clone(),
                                    "switch cases may not follow the default (`=>`) arm",
                                ));
                            }
                            cases.push(pair);
                        }
                        SwitchEl::Default(arm) => {
                            if default.is_some() {
                                return Err(Simple::custom(
                                    span,
                                    "duplicate default arm in switch",
                                ));
                            }
                            default = Some(arm);
                        }
                    }
                }
                Ok((cases, default.map(Box::new)))
            });

        let switch_stmt = text::keyword("switch")
            .ignore_then(
                pad().ignore_then(expr_for_stmt.clone().then_ignore(pad()).or_not()),
            )
            .then(switch_body)
            .map_with_span(|(scrutinee, (cases, default)), span| {
                Stmt::new(
                    span,
                    StmtKind::Switch {
                        scrutinee,
                        cases,
                        default,
                    },
                )
            });

        let while_stmt = text::keyword("while")
            .ignore_then(pad())
            .ignore_then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then(compound_vec())
            .map_with_span(|(cond, body), span| {
                Stmt::new(span, StmtKind::While { cond, body })
            });

        let break_stmt =
            text::keyword("break").map_with_span(|_, span| Stmt::new(span, StmtKind::Break));
        let continue_stmt = text::keyword("continue")
            .map_with_span(|_, span| Stmt::new(span, StmtKind::Continue));

        let block_stmt = compound_vec()
            .map_with_span(|stmts, span| Stmt::new(span, StmtKind::Block(stmts)));

        let tuple_assign = just('[')
            .ignore_then(pad().ignore_then(text::ident()))
            .then(
                just(',')
                    .ignore_then(pad().ignore_then(text::ident()))
                    .repeated(),
            )
            .then_ignore(pad())
            .then_ignore(just(']'))
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map_with_span(|(((first, rest), op), value), span| {
                let mut names = vec![first];
                names.extend(rest);
                Stmt::new(
                    span,
                    StmtKind::TupleAssign { names, op, value },
                )
            });

        let stmt_assign = text::ident()
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map_with_span(|((name, op), value), span| {
                Stmt::new(span, StmtKind::Assign { name, op, value })
            });

        let expr_stmt = expr_for_stmt
            .clone()
            .map(|e| Stmt::new(e.span, StmtKind::Expr(e)));

        choice((
            block_stmt,
            if_stmt,
            for_stmt,
            switch_stmt,
            while_stmt,
            break_stmt,
            continue_stmt,
            var_decl_qualified_stmt.clone(),
            var_decl_input_stmt.clone(),
            var_decl_typed_stmt.clone(),
            tuple_assign,
            stmt_assign,
            expr_stmt,
        ))
        .then_ignore(optional_semicolon())
        .boxed()
    });

    let enum_variant_line = text::ident()
        .then_ignore(pad())
        .then_ignore(just('='))
        .then_ignore(pad())
        .then(expr.clone())
        .then_ignore(optional_semicolon())
        .map(|(name, value)| EnumVariant { name, value });

    let enum_brace_body = just('{')
        .ignore_then(pad())
        .ignore_then(pad().ignore_then(enum_variant_line).repeated())
        .then_ignore(pad())
        .then_ignore(just('}'));

    let enum_name_and_body = text::ident()
        .then_ignore(pad())
        .then(enum_brace_body.clone())
        .map_with_span(|(name, variants), span| EnumDef {
            span: span.into(),
            name,
            variants,
        });

    let enum_item = text::keyword("enum")
        .ignore_then(pad().ignore_then(enum_name_and_body.clone()))
        .map(Item::Enum)
        .boxed();

    let udt_field_line = var_qualifier()
        .or_not()
        .then_ignore(pad())
        .then(type_parser())
        .then_ignore(pad())
        .then(text::ident())
        .then_ignore(pad())
        .then_ignore(just('='))
        .then_ignore(pad())
        .then(expr.clone())
        .then_ignore(optional_semicolon())
        .map(|(((qual, ty), name), default)| UdtField {
            qualifier: qual,
            ty,
            name,
            default,
        });

    let typedef_brace_body = just('{')
        .ignore_then(pad())
        .ignore_then(udt_field_line.repeated())
        .then_ignore(pad())
        .then_ignore(just('}'));

    let typedef_name_fields = text::ident()
        .then_ignore(pad())
        .then(typedef_brace_body.clone())
        .map_with_span(|(name, fields), span| UserTypeDef {
            span: span.into(),
            name,
            fields,
        });

    let typedef_item = text::keyword("type")
        .ignore_then(pad().ignore_then(typedef_name_fields.clone()))
        .map(Item::TypeDef)
        .boxed();

    let param = type_parser()
        .or_not()
        .then_ignore(pad())
        .then(text::ident())
        .then(
            just('=')
                .ignore_then(pad())
                .ignore_then(expr.clone())
                .or_not(),
        )
        .map(|((ty, name), default)| FnParam { ty, name, default });

    let param_list = param
        .separated_by(just(',').ignore_then(pad()))
        .allow_trailing()
        .delimited_by(just('(').ignore_then(pad()), pad().ignore_then(just(')')));

    let fn_body_block = just('{')
        .ignore_then(pad())
        .ignore_then(pad().ignore_then(stmt.clone()).repeated())
        .then_ignore(pad())
        .then_ignore(just('}'))
        .map(FnBody::Block);

    let fn_body_arrow = just('=')
        .ignore_then(just('>'))
        .ignore_then(pad())
        .ignore_then(expr.clone())
        .map(FnBody::Expr);

    let fn_after_params = param_list
        .then_ignore(pad())
        .then(choice((fn_body_arrow, fn_body_block.clone())))
        .boxed();

    // Pine `foo() =>` before QAS `f foo() =>` so a function named `f` can use Pine form.
    let fn_decl_pine = text::ident()
        .then_ignore(pad())
        .then(fn_after_params.clone())
        .map_with_span(|(name, (params, body)), span| FnDecl {
            span: span.into(),
            is_method: false,
            name,
            params,
            body,
        })
        .boxed();

    let fn_decl_f = text::keyword("f")
        .ignore_then(pad())
        .then(text::ident())
        .then_ignore(pad())
        .then(fn_after_params.clone())
        .map_with_span(|((_, name), (params, body)), span| FnDecl {
            span: span.into(),
            is_method: false,
            name,
            params,
            body,
        })
        .boxed();

    let fn_decl_method = text::keyword("method")
        .ignore_then(pad())
        .then(text::ident())
        .then_ignore(pad())
        .then(fn_after_params.clone())
        .map_with_span(|((_, name), (params, body)), span| FnDecl {
            span: span.into(),
            is_method: true,
            name,
            params,
            body,
        })
        .boxed();

    let fn_decl_any = choice((
        fn_decl_pine.clone(),
        fn_decl_f.clone(),
        fn_decl_method.clone(),
    ));

    let fn_decl = fn_decl_any.map(Item::FnDecl);

    let import_decl = text::keyword("import")
        .ignore_then(pad())
        .ignore_then(
            choice((text::ident(), text::int(10)))
                .separated_by(just('/').ignore_then(pad()))
                .at_least(1),
        )
        .then_ignore(pad())
        .then_ignore(text::keyword("as"))
        .then_ignore(pad())
        .then(text::ident())
        .map_with_span(|(path, alias), span| {
            Item::Import(ImportDecl {
                id: NodeId::UNASSIGNED,
                span: span.into(),
                path,
                alias,
            })
        })
        .boxed();

    let export_var_decl = choice((
        var_decl_qualified.clone(),
        var_decl_input.clone(),
        var_decl_typed.clone(),
    ))
    .then_ignore(optional_semicolon())
    .map(|stmt| match &stmt.kind {
        StmtKind::VarDecl(v) => ExportDecl::Var(v.clone()),
        _ => unreachable!("export var parsers only yield VarDecl"),
    })
    .boxed();

    let export_decl = text::keyword("export")
        .ignore_then(pad())
        .ignore_then(choice((
            text::keyword("enum")
                .ignore_then(pad().ignore_then(enum_name_and_body.clone()))
                .map(ExportDecl::Enum),
            text::keyword("type")
                .ignore_then(pad().ignore_then(typedef_name_fields.clone()))
                .map(ExportDecl::TypeDef),
            fn_decl_f.clone().map(ExportDecl::Fn),
            fn_decl_method.clone().map(ExportDecl::Fn),
            fn_decl_pine.clone().map(ExportDecl::Fn),
            export_var_decl.clone(),
        )))
        .map(Item::Export)
        .boxed();

    let item = choice((
        import_decl.clone(),
        export_decl.clone(),
        decl.clone(),
        enum_item.clone(),
        typedef_item.clone(),
        fn_decl.clone(),
        stmt.clone().map(Item::Stmt),
    ))
    .boxed();

    let header_piece = choice((
        version_directive().map(HeaderDirective::Pine),
        agentscript_directive().map(HeaderDirective::AgentScript),
    ));

    pad()
        .ignore_then(
            header_piece
                .then_ignore(pad())
                .repeated()
                .try_map(fold_header_directives),
        )
        .then(item.then_ignore(pad()).repeated())
        .map(|((version, agentscript_version), items)| Script {
            version,
            agentscript_version,
            items,
        })
        .then_ignore(end())
}
