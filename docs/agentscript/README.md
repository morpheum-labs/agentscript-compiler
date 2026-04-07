# AgentScript language manual

This directory is the **syntax and language reference** for **AgentScript / QAS** (QuantAgent Script). It describes what you can write in a script and how it is interpreted by the language rules—not how any particular toolchain is built.

TradingView’s Pine Script manuals ([`spec/pinescriptv6/`](../../spec/pinescriptv6/)) are useful for **vocabulary and examples** where AgentScript follows Pine. When this manual or the normative grammar disagrees with TV docs, **AgentScript’s grammar and checker** are the reference for this project until parity work closes the gap.

## What AgentScript is

- **Surface syntax** is Pine Script v5/v6–aligned in many places (headers, keywords, dotted builtins) plus QAS-specific forms such as `f` user functions.
- **Typical source extensions:** `.pine`, `.qas` (case-insensitive).
- **Normative grammar:** EBNF in [`spec/agentscripts-v1.md`](../../spec/agentscripts-v1.md) (§§1–13).
- **Parser and semantics status:** [`spec/qas-v1-parser-status.md`](../../spec/qas-v1-parser-status.md).
- **Pine parity and roadmap:** [`ROADMAP.md`](../../ROADMAP.md) (including the “Pine v6 parity vs bundled docs” table).

## Index

| Document | Use when |
|----------|----------|
| [`LLM_MANIFEST.md`](LLM_MANIFEST.md) | Finding the smallest page for a given question (including tooling retrieval) |
| [`syntax/grammar.md`](syntax/grammar.md) | Where the formal EBNF lives and how it is organized |
| [`reference/program-structure.md`](reference/program-structure.md) | Top-level script shape (`indicator`, imports, `export`, functions, statements) |
| [`reference/directives.md`](reference/directives.md) | `//@version=` and `// @agentscript=` |
| [`reference/keywords.md`](reference/keywords.md) | Reserved words and constructs |
| [`reference/types.md`](reference/types.md) | Type syntax |
| [`reference/operators.md`](reference/operators.md) | Unary, binary, assignment, ternary, `if` expression |
| [`concepts/dialect-and-limitations.md`](concepts/dialect-and-limitations.md) | Braced dialect, intentional gaps vs TV Pine |
| [`concepts/tv-vs-agentscript-validation.md`](concepts/tv-vs-agentscript-validation.md) | TV vs AgentScript: what tends to pass the **checker** vs TV |

## Related (outside this manual)

- Guest WASM ABI: [`docs/agentscript-guest-abi.md`](../agentscript-guest-abi.md)
- HIR design: [`spec/hir.md`](../../spec/hir.md)
