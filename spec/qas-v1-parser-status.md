# QAS v1 — parser implementation status

This file tracks how the **`agentscript-compiler`** crate (Chumsky parsers under `crates/agentscript-compiler/src/parser/`) lines up with [`agentscripts-v1.md`](agentscripts-v1.md). The **EBNF block (§§1–13)** in that document was revised to follow the **reference parser** (program shape, headers, imports/exports, `enum`/`type`, control flow, expression sketch). Remaining mismatches are called out below.

## Version headers

| Source | Allowed `//@version=` |
|--------|------------------------|
| [`agentscripts-v1.md`](agentscripts-v1.md) §1 `VERSION_LINE` | **`5` \| `6`** |
| [`version_policy.rs`](../crates/agentscript-compiler/src/version_policy.rs) + lexer | **`5` and `6`** |

Optional **`// @agentscript=<n>`** (`n` ≥ 1): see §1 `AGENTSCRIPT_LINE` in the spec and `agentscript_directive` in `parser/lex.rs`.

## EBNF §11 “literals & collections” vs AST

| Spec rule | Implemented? | Notes |
|-----------|----------------|-------|
| `matrix_literal` → `matrix.new<…>(…)` | **Yes** | [`Expr::Call`](../crates/agentscript-compiler/src/ast.rs) + type args; `generic_call_matrix_new` in `tests/parse_smoke.rs`. |
| `map_literal` → `map.new<…,…>(…)` | **Yes** | Same; `map_named_key_type` in `parse_smoke.rs`. |
| `array_factory_literal` → `array.from(…)` | **Yes** | Dotted call; `array_from_call` in `parse_smoke.rs`. |
| `bracket_array_literal` → `[a, b]` | **Yes** | `Expr::Array`; not a separate spec name before the EBNF update. |
| `map.from(…)` | **TBD** | Not finalized in spec §11; parse as ordinary calls once signatures are fixed. |

## Spec vs parser — residual gaps

- **§9 plot/drawing** productions are **declarative** in the spec; the reference parser often accepts the same calls as **expression statements** without dedicated statement AST variants.
- **Indent-only** TradingView bodies are **not** in the reference grammar (`{ … }` first).
- **`footprint` type** and full **`request.*` typing** are Phase 1+.
- **EBNF** uses nonterminals like `path_or_call`, `label_new`, … as **documentation**; the Rust code is the fine-grained source of truth for precedence and token boundaries.

## Still out of scope for Phase 0 (parser)

- Indentation-only blocks (TV-style).
- Unbraced TV-style `enum` / `type` bodies.
- Dedicated statement AST for every `plot*` / drawing form.
- `map.from` until Pine v6 reference + tests lock the shape.

## How to extend this doc

When adding syntax, update [`agentscripts-v1.md`](agentscripts-v1.md) §§ as needed and add tests in `crates/agentscript-compiler/tests/parse_smoke.rs` or fixtures under `crates/agentscript-compiler/tests/fixtures/`.
