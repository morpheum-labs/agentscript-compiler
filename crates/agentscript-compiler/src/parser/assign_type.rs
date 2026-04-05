//! Assignment operator, type syntax, and variable qualifiers (`var` / `input` / …).

use chumsky::prelude::*;

use crate::ast::{AssignOp, PrimitiveType, Type, VarQualifier};

use super::lex::pad;

pub(super) fn assign_op() -> impl Parser<char, AssignOp, Error = Simple<char>> + Clone {
    choice((
        just(':').ignore_then(just('=')).to(AssignOp::ColonEq),
        // Single `=` assignment must not swallow the leading `=` of `==` or `=>` (Chumsky `Then` does not rewind).
        just('=')
            .ignore_then(choice((
                just('>').try_map(|_, span| {
                    Err(Simple::custom(
                        span,
                        "found `=>` in an assignment position; use a single `=` or `:=`",
                    ))
                }),
                just('=').try_map(|_, span| {
                    Err(Simple::custom(
                        span,
                        "use `==` for equality, not two `=` signs in an assignment",
                    ))
                }),
                empty().to(()),
            )))
            .map(|_| AssignOp::Eq),
    ))
}

pub(super) fn type_parser() -> impl Parser<char, Type, Error = Simple<char>> + Clone {
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

pub(super) fn var_qualifier() -> impl Parser<char, VarQualifier, Error = Simple<char>> + Clone {
    choice((
        text::keyword("varip").to(VarQualifier::Varip),
        text::keyword("var").to(VarQualifier::Var),
        text::keyword("const").to(VarQualifier::Const),
        text::keyword("input").to(VarQualifier::Input),
        text::keyword("simple").to(VarQualifier::Simple),
        text::keyword("series").to(VarQualifier::Series),
    ))
}
