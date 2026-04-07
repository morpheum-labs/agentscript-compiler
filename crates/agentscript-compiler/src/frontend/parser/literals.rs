//! String, numeric, and hex-color literals.

use chumsky::prelude::*;

use crate::frontend::ast::{Expr, ExprKind};

pub(super) fn string_literal() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    let double_quoted = just('"')
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
        .then_ignore(just('"'));
    let single_quoted = just('\'')
        .ignore_then(
            choice((
                just("\\'").to('\''),
                just("\\\\").to('\\'),
                just("\\n").to('\n'),
                just("\\t").to('\t'),
                filter(|&c: &char| c != '\'' && c != '\\').map(|c| c),
            ))
            .repeated()
            .collect::<String>(),
        )
        .then_ignore(just('\''));
    choice((double_quoted, single_quoted)).map_with_span(|s, span| Expr::new(span, ExprKind::String(s)))
}

pub(super) fn number_literal() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    // Pine allows leading zeros (`00`, `007`) and trailing-dot floats (`0.`, `1.`).
    // `text::int(10)` rejects leading zeros; we accept any non-empty digit run and parse in Rust.
    let int_digits = filter(|&c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect::<String>();
    let frac_after_dot = just('.')
        .ignore_then(text::digits(10).or_not())
        .map(|frac_o| frac_o.unwrap_or_default());
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
    let with_int = int_digits
        .then(frac_after_dot.or_not())
        .then(exp.or_not())
        .try_map(|((int_s, frac_dot_o), exp_o), span: std::ops::Range<usize>| {
            let mut s = int_s;
            let mut is_float = frac_dot_o.is_some() || exp_o.is_some();
            if let Some(frac) = frac_dot_o {
                s.push('.');
                s.push_str(&frac);
            }
            if let Some(exp_s) = exp_o {
                is_float = true;
                s.push_str(&exp_s);
            }
            if !is_float {
                let n: i64 = s
                    .parse()
                    .map_err(|_| Simple::custom(span.clone(), "invalid integer"))?;
                Ok(Expr::new(span, ExprKind::Int(n)))
            } else {
                let v: f64 = s
                    .parse()
                    .map_err(|_| Simple::custom(span.clone(), "invalid float literal"))?;
                Ok(Expr::new(span, ExprKind::Float(v)))
            }
        });
    // `.5`, `.5e2` (common in Pine / math-heavy scripts)
    let leading_dot = just('.')
        .ignore_then(text::digits(10))
        .then(exp.or_not())
        .try_map(|(frac, exp_o), span: std::ops::Range<usize>| {
            let mut s = String::from('.');
            s.push_str(&frac);
            if let Some(exp_s) = exp_o {
                s.push_str(&exp_s);
            }
            let v: f64 = s
                .parse()
                .map_err(|_| Simple::custom(span.clone(), "invalid float literal"))?;
            Ok(Expr::new(span, ExprKind::Float(v)))
        });
    choice((with_int, leading_dot))
}

/// `#RRGGBB` or `#RRGGBBAA` (Pine-style hex color).
pub(super) fn hex_color_literal() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    just('#').ignore_then(
        filter(|&c: &char| c.is_ascii_hexdigit())
            .repeated()
            .at_least(6)
            .at_most(8)
            .collect::<String>()
            .try_map(|s, span| {
                if s.len() == 6 || s.len() == 8 {
                    Ok(Expr::new(span, ExprKind::HexColor(s)))
                } else {
                    Err(Simple::custom(
                        span,
                        "hex color must be exactly 6 or 8 hex digits",
                    ))
                }
            }),
    )
}
