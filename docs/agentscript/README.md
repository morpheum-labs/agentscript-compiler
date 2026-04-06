# AgentScript syntax manual

This directory is the **compiler-grounded** syntax reference for **AgentScript / QAS** as implemented by [`agentscript-compiler`](../../crates/agentscript-compiler/). It uses the same *navigation pattern* as the bundled TradingView Pine v6 mirror ([`spec/pinescriptv6/`](../../spec/pinescriptv6/)), but the content reflects **this repository’s parser and AST**, not TradingView’s product manual.

## What AgentScript is here

- **Surface syntax** is Pine Script v5/v6–aligned (headers, many keywords, dotted builtins) plus QAS forms such as `f` user functions.
- **File extensions** recognized by the library API: `.pine`, `.qas` (case-insensitive). See [`AGENTSCRIPT_SOURCE_EXTENSIONS`](../../crates/agentscript-compiler/src/lib.rs).
- **Normative grammar:** EBNF in [`spec/agentscripts-v1.md`](../../spec/agentscripts-v1.md) (§§1–13).
- **Implementation tracker:** [`spec/qas-v1-parser-status.md`](../../spec/qas-v1-parser-status.md).
- **Pine parity and roadmap:** [`ROADMAP.md`](../../ROADMAP.md) (including the “Pine v6 parity vs bundled docs” table).

## Index

| Document | Use when |
|----------|----------|
| [`LLM_MANIFEST.md`](LLM_MANIFEST.md) | Routing large prompts to the smallest relevant file |
| [`syntax/grammar.md`](syntax/grammar.md) | Where the EBNF lives and which Rust modules implement it |
| [`reference/program-structure.md`](reference/program-structure.md) | Top-level script shape (`indicator`, imports, `export`, functions, statements) |
| [`reference/directives.md`](reference/directives.md) | `//@version=` and `// @agentscript=` |
| [`reference/keywords.md`](reference/keywords.md) | Reserved words and constructs |
| [`reference/types.md`](reference/types.md) | Type syntax |
| [`reference/operators.md`](reference/operators.md) | Unary, binary, assignment, ternary, `if` expression |
| [`concepts/dialect-and-limitations.md`](concepts/dialect-and-limitations.md) | Braced dialect, intentional gaps vs TV Pine |
| [`concepts/tv-vs-agentscript-validation.md`](concepts/tv-vs-agentscript-validation.md) | TV vs AgentScript: what works on TV, what validates here (`check_script`) |

## Related (not syntax)

- Guest WASM ABI: [`docs/agentscript-guest-abi.md`](../agentscript-guest-abi.md)
- HIR design: [`spec/hir.md`](../../spec/hir.md)
