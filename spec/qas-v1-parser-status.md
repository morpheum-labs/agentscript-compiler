# QAS v1 — parser implementation status

This file tracks how the **`agentscript-compiler`** crate lines up with [`agentscripts-v1.md`](agentscripts-v1.md). The **EBNF block (§§1–13)** in that document follows the **reference parser** (program shape, headers, imports/exports, `enum`/`type`, control flow, expression sketch). The Rust sources under [`frontend/parser/`](../crates/agentscript-compiler/src/frontend/parser/) are the fine-grained source of truth for precedence and token boundaries.

## Key source files

| Concern | Location |
|---------|----------|
| Script / items / stmts | [`frontend/parser/script.rs`](../crates/agentscript-compiler/src/frontend/parser/script.rs) |
| Expressions | [`frontend/parser/expr.rs`](../crates/agentscript-compiler/src/frontend/parser/expr.rs) |
| Types, qualifiers, assign ops | [`frontend/parser/assign_type.rs`](../crates/agentscript-compiler/src/frontend/parser/assign_type.rs) |
| Lexer, padding, directives | [`frontend/parser/lex.rs`](../crates/agentscript-compiler/src/frontend/parser/lex.rs), [`leading_scan.rs`](../crates/agentscript-compiler/src/frontend/parser/leading_scan.rs) |
| `//@version=` policy | [`frontend/parser/version_policy.rs`](../crates/agentscript-compiler/src/frontend/parser/version_policy.rs) |
| AST types | [`frontend/ast/`](../crates/agentscript-compiler/src/frontend/ast/) (`Expr`, `Stmt`, `Item`, …) |

## Phase 0 exit criteria (parser & AST)

Phase 0 is **“good enough”** when all of the following hold (see also [`ROADMAP.md`](ROADMAP.md) success row for Phase 0):

- [x] **`cargo test -p agentscript-compiler`** passes (parser + analyze + HIR golden, etc.).
- [x] **Braced QAS/TV-shaped** programs in scope parse to a consistent AST; **unsupported** syntax is rejected with a parse error (miette-backed where applicable).
- [x] **Spec tracker**: this file and [`agentscripts-v1.md`](agentscripts-v1.md) §§1–13 stay aligned on *intentional* gaps (below).
- [ ] **Optional stretch:** corpus or sampled real `.pine` / `.qas` files, fuzzing, or sharper messages for frequent mistakes (still Phase 0–friendly).

Indent-only TV bodies, unbraced `enum`/`type`, and locked-in **`map.from`** are **explicit non-blockers** for calling Phase 0 “done” on the current dialect.

## Version headers

| Source | Allowed `//@version=` |
|--------|------------------------|
| [`agentscripts-v1.md`](agentscripts-v1.md) §1 `VERSION_LINE` | **`5` \| `6`** |
| [`version_policy.rs`](../crates/agentscript-compiler/src/frontend/parser/version_policy.rs) + lexer | **`5` and `6`** |

Optional **`// @agentscript=<n>`** (`n` ≥ 1): see §1 `AGENTSCRIPT_LINE` in the spec and **`agentscript_directive`** in [`lex.rs`](../crates/agentscript-compiler/src/frontend/parser/lex.rs).

## EBNF §11 “literals & collections” vs AST

| Spec rule | Implemented? | Notes |
|-----------|----------------|-------|
| `matrix_literal` → `matrix.new<…>(…)` | **Yes** | [`ExprKind::Call`](../crates/agentscript-compiler/src/frontend/ast/expr.rs) with generic type args; `generic_call_matrix_new` in [`tests/parse_smoke.rs`](../crates/agentscript-compiler/tests/parse_smoke.rs). |
| `map_literal` → `map.new<…,…>(…)` | **Yes** | Same; `map_named_key_type` in `parse_smoke.rs`. |
| `array_factory_literal` → `array.from(…)` | **Yes** | Dotted call; `array_from_call` in `parse_smoke.rs`. |
| `bracket_array_literal` → `[a, b]` | **Yes** | [`ExprKind::Array`](../crates/agentscript-compiler/src/frontend/ast/expr.rs). |
| `map.from(…)` | **TBD** | Not finalized in spec §11; today parses as an ordinary call; specialize when Pine reference + tests lock the shape. |

## Spec vs parser — residual gaps

- **§9 plot/drawing** productions are **declarative** in the spec; the parser accepts the same calls as **expression statements** without dedicated statement AST variants for each `plot*` form.
- **Indent-only** TradingView bodies are **not** in the grammar (`{ … }` first).
- **`footprint` type** and full **`request.*` typing** are Phase 1+.
- **EBNF** nonterminals like `path_or_call`, `label_new`, … are **documentation**; combinators in `frontend/parser/` decide what actually parses.

## Still out of scope for Phase 0 (parser)

- Indentation-only blocks (TV-style).
- Unbraced TV-style `enum` / `type` bodies.
- Dedicated statement AST for every `plot*` / drawing form.
- **`map.from`** as a first-class literal/factory rule until spec §11 + tests are fixed.

## How to extend this doc

When adding syntax, update [`agentscripts-v1.md`](agentscripts-v1.md) §§ as needed and add tests in [`tests/parse_smoke.rs`](../crates/agentscript-compiler/tests/parse_smoke.rs) (or a dedicated fixture file under `tests/` if the case deserves its own source file).
