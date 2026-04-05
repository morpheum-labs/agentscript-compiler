# AgentScript compiler roadmap

## Primary goal

**Turn AgentScript / QAS (`.qas`) into validated, reproducible `wasm32` modules** that downstream runtimes—**Aether** (`aether-mwvm`, wasmtime / MWVM)—can load under a **shared strategy guest ABI**. The compiler is the language front end; execution policy and job orchestration live in Aether and **MWVM**.

## Current status

**Done today**

- [x] **Parse → AST** (Chumsky): `//@version=1` or `//@version=6`, script declarations (`indicator` / `strategy` / `library`), statements (assign `=` / `:=`, expressions).
- [x] **Diagnostics**: miette-backed `CompileError` with source spans.
- [x] **CLI**: read a file path or stdin (`-`), print debug representation of `Script` on success.
- [x] **Tests**: parser / error cases in `crates/agentscript-compiler/tests/`.

**Not started**

- [ ] Typechecker (scopes, builtins, strategy vs library rules).
- [ ] IR and lowering.
- [ ] Codegen to **`wasm32-unknown-unknown`** (or agreed target triple).
- [ ] **Guest ABI** alignment with Aether (`aether-common` / ABI doc): exports, calling convention, host imports for data and backtest hooks.

## Downstream alignment

| Consumer | What we owe them |
|----------|------------------|
| **Aether** | Stable ABI + `.wasm` bytes + deterministic build story so jobs can pin `wasm_sha256`. |
| **MWVM** | WASM that matches the same ABI and host expectations as other agent guests, where applicable. |

Spec and economics context: **`vaulted-knowledge-protocol/backtesting-infra`**.

## Phase 0 — Parser & AST (current)

- [x] Chumsky grammar for QAS v1/v6 surface aligned with EBNF.
- [x] AST types for declarations, statements, expressions.
- [ ] Expand coverage: edge cases, larger fixtures, error messages for common mistakes.

## Phase 1 — Semantic analysis

- [ ] Symbol tables and name resolution.
- [ ] Type system for core expressions (numbers, series, calls).
- [ ] Script-kind rules (`strategy` vs `indicator` vs `library`).
- [ ] Rich diagnostics (second pass after typecheck).

## Phase 2 — IR & codegen

- [ ] Internal IR suited for lowering and optimization passes.
- [ ] WASM emission (likely `wasm-encoder` / `wasmparser` validation, or another chosen stack).
- [ ] **ABI contract** implemented in codegen (documented in-repo + mirrored types in Aether where useful).

## Phase 3 — Tooling & integration

- [ ] CLI flags: `--emit-ast`, `--emit-wasm`, `-o`, quiet / JSON diagnostics (as needed).
- [ ] **Documented loop**: `.qas` → `agentscript-compiler` → `.wasm` → `aether` run (when Aether’s WASM path is ready).
- [ ] Optional: `cargo` integration or `build.rs` helper for strategy crates.

## Success criteria by phase

| Phase | Done when |
|-------|-----------|
| **0** | `cargo test` green; real-world-ish `.qas` samples parse with clear errors on invalid input. |
| **1** | Ill-typed scripts fail fast with actionable diagnostics; well-typed scripts have a stable semantic model. |
| **2** | Valid strategies compile to **loadable** WASM that satisfies the **written guest ABI** (verified against Aether/MWVM smoke tests). |
| **3** | Builders can compile and run end-to-end without reading compiler internals. |

## Repository layout

| Piece | Location |
|-------|----------|
| Library API | `crates/agentscript-compiler` (`parse_script`, AST, errors) |
| CLI | `crates/agentscript-compiler/src/main.rs` |
