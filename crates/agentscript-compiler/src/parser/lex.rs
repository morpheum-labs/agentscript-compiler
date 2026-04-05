//! Whitespace, comments, Pine `//@version=`, and AgentScript `// @agentscript=` directives.

use chumsky::prelude::*;

use crate::version_policy::{qas_version_allowed, qas_version_unsupported_message};

pub(super) fn version_directive_suffix() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just('@')
        .ignore_then(just("version"))
        .ignore_then(just('='))
        .ignore_then(filter(|c: &char| c.is_ascii_digit()).repeated().at_least(1))
        .then_ignore(filter(|&c| c != '\n' && c != '\r').repeated())
        .ignored()
}

/// After `//`, matches ` `@agentscript=<digits>` rest-of-line (excludes this from generic `//` comments).
pub(super) fn agentscript_directive_suffix() -> impl Parser<char, (), Error = Simple<char>> + Clone
{
    one_of(" \t")
        .repeated()
        .at_least(1)
        .ignore_then(just('@'))
        .ignore_then(just("agentscript"))
        .ignore_then(just('='))
        .ignore_then(filter(|c: &char| c.is_ascii_digit()).repeated().at_least(1))
        .then_ignore(filter(|&c| c != '\n' && c != '\r').repeated())
        .ignored()
}

pub(super) fn version_directive() -> impl Parser<char, u32, Error = Simple<char>> + Clone {
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
            if qas_version_allowed(n) {
                Ok(n)
            } else {
                Err(Simple::custom(span, qas_version_unsupported_message()))
            }
        })
}

pub(super) fn agentscript_directive() -> impl Parser<char, u32, Error = Simple<char>> + Clone {
    just("//")
        .ignore_then(one_of(" \t").repeated().at_least(1))
        .ignore_then(just('@'))
        .ignore_then(just("agentscript"))
        .ignore_then(just('='))
        .ignore_then(text::int(10))
        .try_map(|s: String, span: std::ops::Range<usize>| {
            let n: u32 = match s.parse() {
                Ok(v) => v,
                Err(_) => return Err(Simple::custom(span, "invalid AgentScript version number")),
            };
            if n >= 1 {
                Ok(n)
            } else {
                Err(Simple::custom(
                    span,
                    "AgentScript version must be at least 1",
                ))
            }
        })
}

pub(super) fn line_comment() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    // One `not(choice(...))`: chained `a.not().ignore_then(b.not())` breaks `// @agentscript=` because the
    // first `not` consumes a byte when the inner parser fails, so the second `not` no longer sees leading spaces.
    let pine_or_agentscript_directive_shape =
        choice((version_directive_suffix(), agentscript_directive_suffix()));
    just("//")
        .ignore_then(pine_or_agentscript_directive_shape.not())
        .ignore_then(filter(|&c| c != '\n' && c != '\r').repeated())
        .ignored()
}

fn block_comment_rest() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    recursive(|inner| choice((just("*/").ignored(), any().ignore_then(inner).ignored())))
}

pub(super) fn block_comment() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just("/*").ignore_then(block_comment_rest()).ignored()
}

pub(super) fn pad() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    choice((one_of(" \t\r\n").to(()), line_comment(), block_comment()))
        .repeated()
        .to(())
}

pub(super) fn pad_non_empty() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    choice((one_of(" \t\r\n").to(()), line_comment(), block_comment()))
        .repeated()
        .at_least(1)
        .to(())
}

pub(super) fn optional_semicolon() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just(';').or_not().ignored()
}

/// Pine/QAS `=>` (two `Then` steps so a failed match does not leave a stray `=` like `just("=>")` can).
pub(super) fn fat_arrow() -> impl Parser<char, (), Error = Simple<char>> + Clone {
    just('=').ignore_then(just('>')).ignored()
}
