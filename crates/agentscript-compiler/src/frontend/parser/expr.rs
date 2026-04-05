//! Expression grammar (`expr` recursive parser).

use chumsky::prelude::*;

use crate::frontend::ast::{BinOp, Expr, ExprKind, Span, UnaryOp};

use super::assign_type::type_parser;
use super::lex::{pad, pad_non_empty};
use super::literals::{hex_color_literal, number_literal, string_literal};

pub(super) fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    recursive(|expr| {
        let expr = expr.boxed();

        let named_arg = {
            let e = expr.clone();
            choice((
                text::ident()
                    .then_ignore(choice((just('='), pad_non_empty().ignore_then(just('=')))))
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
            .then(
                just(',')
                    .ignore_then(pad())
                    .ignore_then(type_parser())
                    .or_not(),
            )
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
            .try_map(
                |((path, type_args), args_o), span| match (type_args, args_o) {
                    (None, None) => Ok(Expr::new(span, ExprKind::IdentPath(path))),
                    (Some(_), None) => {
                        Err(Simple::custom(span, "expected `(` after type arguments"))
                    }
                    (ta, Some(args)) => {
                        let span_s: Span = span.clone().into();
                        Ok(Expr::new(
                            span,
                            ExprKind::Call {
                                callee: Box::new(Expr::new(span_s, ExprKind::IdentPath(path))),
                                type_args: ta,
                                args,
                            },
                        ))
                    }
                },
            );

        let color_lit = text::keyword("color")
            .ignore_then(just('.'))
            .ignore_then(text::ident())
            .map_with_span(|name, span| Expr::new(span, ExprKind::Color(name)));

        let paren = just('(')
            .ignore_then(pad())
            .then(expr.clone())
            .then_ignore(pad())
            .then_ignore(just(')'))
            .map_with_span(|(_, inner), span| Expr::new(span, inner.kind));

        let array_lit = just('[')
            .ignore_then(pad())
            .then(
                expr.clone()
                    .separated_by(just(',').ignore_then(pad()))
                    .allow_trailing(),
            )
            .then_ignore(pad())
            .then_ignore(just(']'))
            .map_with_span(|(_, elements), span| Expr::new(span, ExprKind::Array(elements)));

        let atom_base = choice((
            string_literal(),
            number_literal(),
            hex_color_literal(),
            text::keyword("true").map_with_span(|_, span| Expr::new(span, ExprKind::Bool(true))),
            text::keyword("false").map_with_span(|_, span| Expr::new(span, ExprKind::Bool(false))),
            text::keyword("na").map_with_span(|_, span| Expr::new(span, ExprKind::Na)),
            color_lit,
            array_lit,
            paren,
            call_or_ident,
        ));

        #[derive(Clone)]
        enum PostfixPiece {
            Index(Expr),
            Field {
                name: String,
                args: Option<Vec<(Option<String>, Expr)>>,
            },
        }

        let dot_field_or_call = just('.')
            .ignore_then(text::ident())
            .then(
                pad()
                    .ignore_then(just('('))
                    .ignore_then(
                        named_arg
                            .clone()
                            .separated_by(just(',').ignore_then(pad()))
                            .allow_trailing(),
                    )
                    .then_ignore(pad())
                    .then_ignore(just(')'))
                    .or_not(),
            )
            .map(|(name, args)| PostfixPiece::Field { name, args });

        let postfix_op = choice((
            just('[')
                .ignore_then(pad())
                .ignore_then(expr.clone())
                .then_ignore(pad())
                .then_ignore(just(']'))
                .map(PostfixPiece::Index),
            dot_field_or_call,
        ));

        let postfix = atom_base
            .then(postfix_op.repeated())
            .foldl(|e, piece| match piece {
                PostfixPiece::Index(idx) => Expr::new(
                    Span::merge(e.span, idx.span),
                    ExprKind::Index {
                        base: Box::new(e),
                        index: Box::new(idx),
                    },
                ),
                PostfixPiece::Field { name, args: None } => Expr::new(
                    e.span,
                    ExprKind::Member {
                        base: Box::new(e),
                        field: name,
                    },
                ),
                PostfixPiece::Field {
                    name,
                    args: Some(args),
                } => Expr::new(
                    e.span,
                    ExprKind::Call {
                        callee: Box::new(Expr::new(
                            e.span,
                            ExprKind::Member {
                                base: Box::new(e),
                                field: name,
                            },
                        )),
                        type_args: None,
                        args,
                    },
                ),
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
                ops.into_iter().rev().fold(e, |acc, op| match (op, acc) {
                    (UnaryOp::Neg, Expr {
                        span,
                        kind: ExprKind::Int(n),
                    }) => Expr::new(span, ExprKind::Int(-n)),
                    (UnaryOp::Neg, Expr {
                        span,
                        kind: ExprKind::Float(x),
                    }) => Expr::new(span, ExprKind::Float(-x)),
                    (op, acc) => Expr::new(
                        acc.span,
                        ExprKind::Unary {
                            op,
                            expr: Box::new(acc),
                        },
                    ),
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
            .foldl(|lhs, (op, rhs)| Expr::new(
                Span::merge(lhs.span, rhs.span),
                ExprKind::Binary {
                    op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            ));

        let sum = product
            .clone()
            .then(
                pad()
                    .ignore_then(choice((just('+').to(BinOp::Add), just('-').to(BinOp::Sub))))
                    .then_ignore(pad())
                    .then(product.clone())
                    .repeated(),
            )
            .foldl(|lhs, (op, rhs)| Expr::new(
                Span::merge(lhs.span, rhs.span),
                ExprKind::Binary {
                    op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            ));

        let cmp = sum
            .clone()
            .then(
                pad()
                    .ignore_then(choice((
                        just('=').ignore_then(just('=')).to(BinOp::Eq),
                        just('!').ignore_then(just('=')).to(BinOp::Ne),
                        just('<').ignore_then(just('=')).to(BinOp::Le),
                        just('>').ignore_then(just('=')).to(BinOp::Ge),
                        just('<').to(BinOp::Lt),
                        just('>').to(BinOp::Gt),
                    )))
                    .then_ignore(pad())
                    .then(sum.clone())
                    .repeated(),
            )
            .foldl(|lhs, (op, rhs)| Expr::new(
                Span::merge(lhs.span, rhs.span),
                ExprKind::Binary {
                    op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            ));

        let and_expr = cmp
            .clone()
            .then(
                pad()
                    .ignore_then(text::keyword("and"))
                    .ignore_then(pad())
                    .ignore_then(cmp.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::new(
                Span::merge(lhs.span, rhs.span),
                ExprKind::Binary {
                    op: BinOp::And,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            ));

        let or_expr = and_expr
            .clone()
            .then(
                pad()
                    .ignore_then(text::keyword("or"))
                    .ignore_then(pad())
                    .ignore_then(and_expr.clone())
                    .repeated(),
            )
            .foldl(|lhs, rhs| Expr::new(
                Span::merge(lhs.span, rhs.span),
                ExprKind::Binary {
                    op: BinOp::Or,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
            ));

        let ternary_expr = or_expr
            .clone()
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
                Some((then_b, else_b)) => Expr::new(
                    Span::merge(cond.span, else_b.span),
                    ExprKind::Ternary {
                        cond: Box::new(cond),
                        then_b: Box::new(then_b),
                        else_b: Box::new(else_b),
                    },
                ),
            });

        let if_expr = text::keyword("if")
            .ignore_then(pad().ignore_then(expr.clone()))
            .then_ignore(pad())
            .then(expr.clone())
            .then_ignore(pad())
            .then_ignore(text::keyword("else"))
            .then_ignore(pad())
            .then(expr.clone())
            .map_with_span(|((cond, then_b), else_b), span| {
                Expr::new(
                    span,
                    ExprKind::IfExpr {
                        cond: Box::new(cond),
                        then_b: Box::new(then_b),
                        else_b: Box::new(else_b),
                    },
                )
            });

        choice((if_expr, ternary_expr)).boxed()
    })
}
