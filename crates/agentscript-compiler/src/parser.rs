use chumsky::prelude::*;

use crate::ast::{
    AssignOp, BinOp, ElseBody, Expr, FnBody, FnDecl, FnParam, IfStmt, Item, PrimitiveType, Script,
    ScriptDeclaration, ScriptKind, Stmt, Type, UnaryOp, VarDecl, VarQualifier,
};

fn version_directive_suffix() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just('@')
        .ignore_then(just("version"))
        .ignore_then(just('='))
        .ignore_then(filter(|c: &char| c.is_ascii_digit()).repeated().at_least(1))
        .then_ignore(filter(|&c| c != '\n' && c != '\r').repeated())
        .ignored()
}

fn version_directive() -> impl Parser<char, u32, Error = Simple<char>> + Clone {
    just("//")
        .ignore_then(just('@'))
        .ignore_then(just("version"))
        .ignore_then(just('='))
        .ignore_then(text::int(10))
        .try_map(|s: String, span: std::ops::Range<usize>| {
            let n: u32 = match s.parse() {
                Ok(v) => v,
                Err(_) => return Err(Simple::custom(span, "invalid version number")),
            };
            if n == 1 || n == 6 {
                Ok(n)
            } else {
                Err(Simple::custom(
                    span,
                    "unsupported //@version (QAS v1 allows only 1 or 6)",
                ))
            }
        })
}

fn line_comment() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just("//")
        .ignore_then(version_directive_suffix().not())
        .ignore_then(filter(|&c| c != '\n' && c != '\r').repeated())
        .ignored()
}

fn block_comment_rest() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    recursive(|inner| {
        choice((
            just("*/").ignored(),
            any().ignore_then(inner).ignored(),
        ))
    })
}

fn block_comment() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just("/*").ignore_then(block_comment_rest()).ignored()
}

fn pad() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    choice((
        one_of(" \t\r\n").to(()),
        line_comment(),
        block_comment(),
    ))
    .repeated()
    .to(())
}

fn pad_non_empty() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    choice((
        one_of(" \t\r\n").to(()),
        line_comment(),
        block_comment(),
    ))
    .repeated()
    .at_least(1)
    .to(())
}

fn optional_semicolon() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just(';').or_not().ignored()
}

fn string_literal() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    just('"')
        .ignore_then(
            choice((
                just("\\\"").to('"'),
                just("\\\\").to('\\'),
                just("\\n").to('\n'),
                just("\\t").to('\t'),
                filter(|&c: &char| c != '"' && c != '\\').map(|c| c),
            ))
            .repeated()
            .collect::<String>(),
        )
        .then_ignore(just('"'))
        .map(Expr::String)
}

fn number_literal() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    let frac = just('.').ignore_then(text::digits(10));
    let exp = choice((just('e'), just('E')))
        .ignore_then(just('+').or(just('-')).or_not())
        .then(text::int(10))
        .map(|(sign_o, exp_digits)| {
            let mut out = String::from("e");
            if let Some(sgn) = sign_o {
                out.push(sgn);
            }
            out.push_str(&exp_digits);
            out
        });
    text::int(10)
        .then(frac.or_not())
        .then(exp.or_not())
        .try_map(|((int_s, frac_o), exp_o), span: std::ops::Range<usize>| {
            let mut s = int_s;
            let mut is_float = frac_o.is_some() || exp_o.is_some();
            if let Some(frac) = frac_o {
                s.push('.');
                s.push_str(&frac);
            }
            if let Some(exp_s) = exp_o {
                is_float = true;
                s.push_str(&exp_s);
            }
            if !is_float {
                let n: i64 = s.parse().map_err(|_| Simple::custom(span.clone(), "invalid integer"))?;
                Ok(Expr::Int(n))
            } else {
                let v: f64 = s
                    .parse()
                    .map_err(|_| Simple::custom(span, "invalid float literal"))?;
                Ok(Expr::Float(v))
            }
        })
}

fn assign_op() -> impl Parser<char, AssignOp, Error = Simple<char>> + Clone {
    choice((
        just(':').ignore_then(just('=')).to(AssignOp::ColonEq),
        just('=')
            .then(just('=').or_not())
            .try_map(|(_, second_eq), span| {
                if second_eq.is_some() {
                    Err(Simple::custom(
                        span,
                        "expected `:=` or `=` for assignment",
                    ))
                } else {
                    Ok(AssignOp::Eq)
                }
            }),
    ))
}

fn type_parser() -> impl Parser<char, Type, Error = Simple<char>> + Clone {
    recursive(|ty| {
        let ty = ty.boxed();
        let primitive = choice((
            text::keyword("int").to(Type::Primitive(PrimitiveType::Int)),
            text::keyword("float").to(Type::Primitive(PrimitiveType::Float)),
            text::keyword("bool").to(Type::Primitive(PrimitiveType::Bool)),
            text::keyword("string").to(Type::Primitive(PrimitiveType::String)),
            text::keyword("color").to(Type::Primitive(PrimitiveType::Color)),
        ));
        let object = choice((
            text::keyword("label").to(Type::Label),
            text::keyword("line").to(Type::Line),
            text::keyword("box").to(Type::BoxType),
            text::keyword("table").to(Type::Table),
            text::keyword("polyline").to(Type::Polyline),
            text::keyword("linefill").to(Type::Linefill),
            text::keyword("volume_row").to(Type::VolumeRow),
            text::keyword("chart")
                .ignore_then(just('.'))
                .ignore_then(text::keyword("point"))
                .to(Type::ChartPoint),
        ));
        let generic = choice((
            text::keyword("array")
                .ignore_then(just('<'))
                .ignore_then(pad())
                .ignore_then(ty.clone())
                .then_ignore(pad())
                .then_ignore(just('>'))
                .map(|t| Type::Array(Box::new(t))),
            text::keyword("matrix")
                .ignore_then(just('<'))
                .ignore_then(pad())
                .ignore_then(ty.clone())
                .then_ignore(pad())
                .then_ignore(just('>'))
                .map(|t| Type::Matrix(Box::new(t))),
            text::keyword("map")
                .ignore_then(just('<'))
                .ignore_then(pad())
                .ignore_then(ty.clone())
                .then(just(',').ignore_then(pad()).ignore_then(ty.clone()))
                .then_ignore(pad())
                .then_ignore(just('>'))
                .map(|(a, b)| Type::Map(Box::new(a), Box::new(b))),
        ));
        choice((generic, object, primitive))
    })
}

fn var_qualifier() -> impl Parser<char, VarQualifier, Error = Simple<char>> + Clone {
    choice((
        text::keyword("varip").to(VarQualifier::Varip),
        text::keyword("var").to(VarQualifier::Var),
        text::keyword("const").to(VarQualifier::Const),
        text::keyword("input").to(VarQualifier::Input),
        text::keyword("simple").to(VarQualifier::Simple),
        text::keyword("series").to(VarQualifier::Series),
    ))
}

fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    recursive(|expr| {
        let expr = expr.boxed();

        let named_arg = {
            let e = expr.clone();
            choice((
                text::ident()
                    .then_ignore(choice((
                        just('='),
                        pad_non_empty().ignore_then(just('=')),
                    )))
                    .then_ignore(pad())
                    .then(e.clone())
                    .map(|(name, v)| (Some(name), v)),
                e.clone().map(|v| (None, v)),
            ))
            .boxed()
        };

        let call_args = named_arg
            .clone()
            .separated_by(just(',').ignore_then(pad()))
            .allow_trailing()
            .delimited_by(just('(').ignore_then(pad()), pad().ignore_then(just(')')));

        let type_args_angle = just('<')
            .ignore_then(pad())
            .ignore_then(type_parser())
            .then(just(',').ignore_then(pad()).ignore_then(type_parser()).or_not())
            .then_ignore(pad())
            .then_ignore(just('>'))
            .map(|(t1, t2o)| match t2o {
                None => vec![t1],
                Some(t2) => vec![t1, t2],
            });

        let path = text::ident()
            .then(just('.').ignore_then(text::ident()).repeated())
            .map(|(head, tail)| {
                let mut segs = vec![head];
                segs.extend(tail);
                segs
            });

        let call_or_ident = path
            .then(type_args_angle.or_not())
            .then(call_args.or_not())
            .try_map(|((path, type_args), args_o), span| match (type_args, args_o) {
                (None, None) => Ok(Expr::IdentPath(path)),
                (Some(_), None) => Err(Simple::custom(
                    span,
                    "expected `(` after type arguments",
                )),
                (ta, Some(args)) => Ok(Expr::Call {
                    path,
                    type_args: ta,
                    args,
                }),
            });

        let color_lit = text::keyword("color")
            .ignore_then(just('.'))
            .ignore_then(text::ident())
            .map(Expr::Color);

        let paren = just('(')
            .ignore_then(pad())
            .ignore_then(expr.clone())
            .then_ignore(pad())
            .then_ignore(just(')'));

        let atom_base = choice((
            string_literal(),
            number_literal(),
            text::keyword("true").to(Expr::Bool(true)),
            text::keyword("false").to(Expr::Bool(false)),
            text::keyword("na").to(Expr::Na),
            color_lit,
            paren,
            call_or_ident,
        ));

        let postfix = atom_base
            .then(
                just('[')
                    .ignore_then(pad())
                    .ignore_then(expr.clone())
                    .then_ignore(pad())
                    .then_ignore(just(']'))
                    .repeated(),
            )
            .foldl(|e, idx| Expr::Index {
                base: Box::new(e),
                index: Box::new(idx),
            });

        let unary_op = choice((
            just('+').to(UnaryOp::Pos),
            just('-').to(UnaryOp::Neg),
            text::keyword("not").to(UnaryOp::Not),
        ));

        let unary = pad()
            .ignore_then(unary_op)
            .repeated()
            .then(pad().ignore_then(postfix))
            .map(|(ops, e)| {
                ops.into_iter().rev().fold(e, |acc, op| Expr::Unary {
                    op,
                    expr: Box::new(acc),
                })
            });

        let product = unary
            .clone()
            .then(
                pad()
                    .ignore_then(choice((
                        just('*').to(BinOp::Mul),
                        just('/').to(BinOp::Div),
                        just('%').to(BinOp::Mod),
                    )))
                    .then_ignore(pad())
                    .then(unary.clone())
                    .repeated(),
            )
            .foldl(|lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        let sum = product
            .clone()
            .then(
                pad()
                    .ignore_then(choice((
                        just('+').to(BinOp::Add),
                        just('-').to(BinOp::Sub),
                    )))
                    .then_ignore(pad())
                    .then(product.clone())
                    .repeated(),
            )
            .foldl(|lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        let cmp = sum
            .clone()
            .then(
                pad()
                    .ignore_then(choice((
                        just("==").to(BinOp::Eq),
                        just("!=").to(BinOp::Ne),
                        just("<=").to(BinOp::Le),
                        just(">=").to(BinOp::Ge),
                        just('<').to(BinOp::Lt),
                        just('>').to(BinOp::Gt),
                    )))
                    .then_ignore(pad())
                    .then(sum.clone())
                    .repeated(),
            )
            .foldl(|lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        let and_expr = cmp
            .clone()
            .then(
                pad()
                    .ignore_then(text::keyword("and"))
                    .ignore_then(pad())
                    .ignore_then(cmp.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::Binary {
                op: BinOp::And,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        let or_expr = and_expr
            .clone()
            .then(
                pad()
                    .ignore_then(text::keyword("or"))
                    .ignore_then(pad())
                    .ignore_then(and_expr.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::Binary {
                op: BinOp::Or,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        or_expr
            .then(
                pad()
                    .ignore_then(just('?'))
                    .ignore_then(pad())
                    .ignore_then(expr.clone())
                    .then(
                        pad()
                            .ignore_then(just(':'))
                            .ignore_then(pad())
                            .ignore_then(expr.clone()),
                    )
                    .or_not(),
            )
            .map(|(cond, opt)| match opt {
                None => cond,
                Some((then_b, else_b)) => Expr::Ternary {
                    cond: Box::new(cond),
                    then_b: Box::new(then_b),
                    else_b: Box::new(else_b),
                },
            })
            .boxed()
    })
}

/// Version line, padding, then declarations and statements until EOF.
pub fn script_parser() -> impl Parser<char, Script, Error = Simple<char>> {
    let expr = expr_parser().boxed();

    let decl_args = {
        let e = expr.clone();
        let na = choice((
            text::ident()
                .then_ignore(choice((
                    just('='),
                    pad_non_empty().ignore_then(just('=')),
                )))
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
    let stmt = recursive(move |stmt| {
        let stmt = stmt.boxed();
        let expr_if = expr_for_stmt.clone();

        let compound_vec = || {
            just('{')
                .ignore_then(pad())
                .ignore_then(stmt.clone().repeated())
                .then_ignore(pad())
                .then_ignore(just('}'))
        };

        let then_part = || {
            just('{')
                .ignore_then(pad())
                .ignore_then(stmt.clone().repeated())
                .then_ignore(pad())
                .then_ignore(just('}'))
        };

        // `else if` chains are parsed as `else { if ... }` in a follow-up pass if needed; nested `recursive`
        // here caused mutual-recursion stack overflows with chumsky 0.9.
        let if_stmt = text::keyword("if")
            .ignore_then(pad())
            .ignore_then(expr_if.clone())
            .then_ignore(pad())
            .then(then_part())
            .then(
                pad()
                    .ignore_then(text::keyword("else"))
                    .ignore_then(pad())
                    .ignore_then(then_part())
                    .map(ElseBody::Block)
                    .or_not(),
            )
            .map(|((cond, then_body), else_body)| {
                Stmt::If(IfStmt {
                    cond,
                    then_body,
                    else_body,
                })
            });

        let for_stmt = text::keyword("for")
            .ignore_then(pad())
            .ignore_then(text::ident())
            .then_ignore(pad())
            .then_ignore(just('='))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then_ignore(text::keyword("to"))
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then(compound_vec())
            .map(|(((var, from), to), body)| Stmt::For { var, from, to, body });

        let switch_case = expr_for_stmt
            .clone()
            .then_ignore(pad())
            .then_ignore(just("=>"))
            .then_ignore(pad())
            .then(stmt.clone());

        let switch_default = just("=>")
            .ignore_then(pad())
            .ignore_then(stmt.clone())
            .map(Box::new);

        let switch_body = just('{')
            .ignore_then(pad())
            .ignore_then(switch_case.repeated())
            .then(switch_default.or_not())
            .then_ignore(pad())
            .then_ignore(just('}'));

        let switch_stmt = text::keyword("switch")
            .ignore_then(pad())
            .ignore_then(expr_for_stmt.clone())
            .then_ignore(pad())
            .then(switch_body)
            .map(|(scrutinee, (cases, default))| Stmt::Switch {
                scrutinee,
                cases,
                default,
            });

        let block_stmt = compound_vec().map(Stmt::Block);

        let var_decl_qualified = var_qualifier()
            .then_ignore(pad())
            .then(type_parser().or_not())
            .then_ignore(pad())
            .then(text::ident())
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map(|((((qual, ty), name), _op), value)| {
                Stmt::VarDecl(VarDecl {
                    qualifier: Some(qual),
                    ty,
                    name,
                    value,
                })
            });

        let var_decl_input = text::keyword("input")
            .ignore_then(pad())
            .ignore_then(type_parser().or_not())
            .then_ignore(pad())
            .then(text::ident())
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map(|(((ty, name), _op), value)| {
                Stmt::VarDecl(VarDecl {
                    qualifier: Some(VarQualifier::Input),
                    ty,
                    name,
                    value,
                })
            });

        let var_decl_typed = type_parser()
            .then_ignore(pad())
            .then(text::ident())
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map(|(((ty, name), _op), value)| {
                Stmt::VarDecl(VarDecl {
                    qualifier: None,
                    ty: Some(ty),
                    name,
                    value,
                })
            });

        let stmt_assign = text::ident()
            .then_ignore(pad())
            .then(assign_op())
            .then_ignore(pad())
            .then(expr_for_stmt.clone())
            .map(|((name, op), value)| Stmt::Assign { name, op, value });

        let expr_stmt = expr_for_stmt.clone().map(Stmt::Expr);

        choice((
            block_stmt,
            if_stmt,
            for_stmt,
            switch_stmt,
            var_decl_qualified,
            var_decl_input,
            var_decl_typed,
            stmt_assign,
            expr_stmt,
        ))
        .then_ignore(optional_semicolon())
        .boxed()
    });

    let param = type_parser()
        .or_not()
        .then_ignore(pad())
        .then(text::ident())
        .then(
            pad()
                .ignore_then(just('='))
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
        .ignore_then(stmt.clone().repeated())
        .then_ignore(pad())
        .then_ignore(just('}'))
        .map(FnBody::Block);

    let fn_decl = text::keyword("f")
        .ignore_then(pad())
        .ignore_then(text::ident())
        .then_ignore(pad())
        .then(param_list)
        .then_ignore(pad())
        .then(choice((
            just('=')
                .ignore_then(just('>'))
                .ignore_then(pad())
                .ignore_then(expr.clone())
                .map(FnBody::Expr),
            fn_body_block.clone(),
        )))
        .map(|((name, params), body)| Item::FnDecl(FnDecl { name, params, body }));

    let item = choice((decl.clone(), fn_decl.clone(), stmt.clone().map(Item::Stmt))).boxed();

    pad()
        .ignore_then(version_directive().then_ignore(pad()).or_not())
        .then(item.then_ignore(pad()).repeated())
        .map(|(version, items)| Script { version, items })
        .then_ignore(end())
}
