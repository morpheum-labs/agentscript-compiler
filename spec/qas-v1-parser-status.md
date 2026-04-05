# QAS v1 — parser implementation status

This file tracks how the **`agentscript-compiler`** crate (Chumsky parsers under `crates/agentscript-compiler/src/parser/`) lines up with the narrative + EBNF in [`agentscripts-v1.md`](agentscripts-v1.md). The spec mixes product context with a **compiler-oriented EBNF** (§§1–13 around line 331); that EBNF is **aspirational** in places and does not yet list every construct the compiler accepts.

## Version headers

| Document / policy | Allowed `//@version=` |
|-------------------|----------------------|
| [`agentscripts-v1.md`](agentscripts-v1.md) EBNF (§1) | `1` \| `6` |
| [`version_policy.rs`](../crates/agentscript-compiler/src/version_policy.rs) + lexer | **`5` and `6`** (QAS / AgentScript alignment) |

Treat **`5`/`6`** as authoritative for this repository until the spec EBNF is revised to match.

## EBNF §11 “literals & collections” vs AST

| Spec rule | Implemented? | Notes |
|-----------|----------------|-------|
| `matrix_literal` → `matrix.new<…>(…)` | **Yes** | Parsed as [`Expr::Call`](../crates/agentscript-compiler/src/ast.rs) + type args; see `generic_call_matrix_new` in `tests/parse_smoke.rs`. |
| `map_literal` → `map.new<…,…>(…)` | **Yes** | Same; see `map_named_key_type` in `parse_smoke.rs`. |
| `array_literal` → `array.from(…)` | **Yes** | Parsed as a dotted call (`array` · `from`); not a separate AST variant. See `array_from_call` in `parse_smoke.rs`. |
| `array_literal` → `[a, b]` | **Yes** | Pine-style bracket literals; **extra** vs the spec’s single `array.from` line. |
| `map.from(…)` | **Not in spec body** | EBNF shows `map.from '(' ... ')'` as incomplete; no dedicated tests until grammar is finalized. |

## Major syntax present in the compiler but absent or narrower in the spec EBNF

The following are **implemented** in [`script.rs`](../crates/agentscript-compiler/src/parser/script.rs) / [`expr.rs`](../crates/agentscript-compiler/src/parser/expr.rs) but **not** reflected in the short EBNF excerpt in `agentscripts-v1.md`:

- `import` / `export` (including `export enum`, `export type`, `export` functions and vars)
- `enum` and `type` (UDT) braced declarations
- Pine-style `name(…) =>` / block functions and `method` declarations; QAS `f name(…)`
- `for … in`, `for [i, v] in`, `for` … `to` … [`by`], `while`, `break`, `continue`
- `switch` with optional scrutinee and `=>` arms
- Tuple destructuring assign `[a, b] = expr`
- Compound assignments `+=`, `-=`, …
- `if` as expression (`Expr::IfExpr`) and ternary
- Optional `// @agentscript=<n>` header (≥ 1)

## Still out of scope for Phase 0 (parser)

- Indentation-only blocks (TV-style); compiler is **`{ … }`-first**
- Unbraced TV-style `enum` / `type` bodies
- Dedicated AST / parse rules for every `plot*` / drawing statement as **statements** (many parse today as generic calls / expr stmts depending on form)
- `footprint` type keyword and full `request.*` typing (Phase 1+)

## How to extend this doc

When adding syntax, update the table above and add or extend a test in `crates/agentscript-compiler/tests/parse_smoke.rs` (or a fixture under `crates/agentscript-compiler/tests/fixtures/`, e.g. `minimal_strategy.pine`, wired by `fixture_minimal_strategy_parse_and_analyze`).
