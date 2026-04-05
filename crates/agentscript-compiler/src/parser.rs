use chumsky::prelude::*;

use crate::ast::{
    AssignOp, BinOp, Expr, Item, Script, ScriptDeclaration, ScriptKind, Stmt, UnaryOp,
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
    choice((
        text::int(10)
            .then(just('.').ignore_then(text::digits(10)))
            .map(|(int_s, frac)| {
                let v: f64 = format!("{int_s}.{frac}").parse().unwrap_or(0.0);
                Expr::Float(v)
            }),
        text::int(10).map(|s: String| Expr::Int(s.parse().unwrap_or(0))),
    ))
}

fn assign_op() -> impl Parser<char, AssignOp, Error = Simple<char>> + Clone {
    choice((
        just(':').ignore_then(just('=')).to(AssignOp::ColonEq),
        just('=')
            .ignore_then(just('=').not().rewind())
            .to(AssignOp::Eq),
    ))
}

/// Version line, padding, then declarations and statements until EOF.
pub fn script_parser() -> impl Parser<char, Script, Error = Simple<char>> {
    let expr = recursive(|expr| {
        let expr = expr.boxed();

        let named_arg = {
            let e = expr.clone();
            choice((
                text::ident()
                    .then_ignore(pad())
                    .then_ignore(just('='))
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

        let path = text::ident()
            .then(just('.').ignore_then(text::ident()).repeated())
            .map(|(head, tail)| {
                let mut segs = vec![head];
                segs.extend(tail);
                segs
            });

        let call_or_ident = path
            .then(call_args.or_not())
            .map(|(path, args)| match args {
                Some(args) => Expr::Call { path, args },
                None => Expr::IdentPath(path),
            });

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
            just('-').to(UnaryOp::Neg),
            text::keyword("not").to(UnaryOp::Not),
        ));

        let unary = unary_op
            .repeated()
            .then(postfix)
            .foldr(|op, e| Expr::Unary {
                op,
                expr: Box::new(e),
            });

        let product = unary
            .clone()
            .then(
                choice((
                    just('*').to(BinOp::Mul),
                    just('/').to(BinOp::Div),
                    just('%').to(BinOp::Mod),
                ))
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
                choice((just('+').to(BinOp::Add), just('-').to(BinOp::Sub)))
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
                choice((
                    just("==").to(BinOp::Eq),
                    just("!=").to(BinOp::Ne),
                    just("<=").to(BinOp::Le),
                    just(">=").to(BinOp::Ge),
                    just('<').to(BinOp::Lt),
                    just('>').to(BinOp::Gt),
                ))
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
                text::keyword("and")
                    .ignore_then(pad())
                    .ignore_then(cmp.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::Binary {
                op: BinOp::And,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        and_expr
            .clone()
            .then(
                text::keyword("or")
                    .ignore_then(pad())
                    .ignore_then(and_expr.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::Binary {
                op: BinOp::Or,
                left: Box::new(lhs),
                right: Box::new(rhs),
            })
            .boxed()
    });

    let decl_args = {
        let e = expr.clone();
        let na = choice((
            text::ident()
                .then_ignore(pad())
                .then_ignore(just('='))
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

    let stmt_assign = text::ident()
        .then_ignore(pad())
        .then(assign_op())
        .then_ignore(pad())
        .then(expr.clone())
        .map(|((name, op), value)| {
            Item::Stmt(Stmt::Assign { name, op, value })
        });

    let stmt = choice((
        stmt_assign,
        expr.clone().map(|e| Item::Stmt(Stmt::Expr(e))),
    ))
    .boxed();

    let item = choice((decl.clone(), stmt)).boxed();

    pad()
        .ignore_then(version_directive().then_ignore(pad()).or_not())
        .then(item.then_ignore(pad()).repeated())
        .map(|(version, items)| Script { version, items })
        .then_ignore(end())
}
