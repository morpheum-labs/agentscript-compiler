use chumsky::prelude::*;

use crate::ast::Script;

/// Optional `//@version=N` at the start of the file (Pine-style).
fn version_directive() -> impl Parser<char, u32, Error = Simple<char>> {
    just("//")
        .ignore_then(just('@'))
        .ignore_then(just("version"))
        .padded()
        .ignore_then(just('='))
        .padded()
        .ignore_then(text::int(10))
        .map(|digits: String| digits.parse::<u32>().unwrap_or(0))
}

fn line_ending() -> impl Parser<char, (), Error = Simple<char>> {
    choice((
        just('\r').ignore_then(just('\n')).to(()),
        just('\n').to(()),
    ))
}

/// Phase 1: header only; remaining syntax lands in follow-up PRs.
pub fn script_parser() -> impl Parser<char, Script, Error = Simple<char>> {
    let header = version_directive()
        .then_ignore(line_ending().or_not())
        .or_not();

    header
        .map(|v| Script {
            version: v,
            items: Vec::new(),
        })
        .then_ignore(end())
}
