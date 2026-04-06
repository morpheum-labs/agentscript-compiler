# GitHub backlog (from ROADMAP)

Copy into issues as needed. Canonical detail lives in [ROADMAP.md](../ROADMAP.md) and [aether-integration-gap.md](aether-integration-gap.md).

## Sprint order (April 2026)

1. **Span coverage** — Prefer real AST/HIR spans on every user-facing error; [`HirScript::source_span`](../crates/agentscript-compiler/src/hir/script.rs) backs [`expr_span`](../crates/agentscript-compiler/src/codegen/hir_wasm.rs) when expression arenas are inconsistent. Audit `typecheck`, `ast_lower`, and remaining codegen paths.
2. **`request.*` parity** — Full Pine v6 parameters, dynamic args in WASM, prefetch metadata; compiler progress: `request.financial` gaps + currency literals (v0 host ABI `i32×10`).
3. **Type system + modules** — Import/export graph, surface `array<>` / `map<>` / `matrix<>` enforcement.
4. **HIR/WASM widen** — Non-`close` series, nested `plot`, more `ta.*`, arrays in guest ABI.

## Issue stubs

### Span audit (Phase 1 / UX)

- **Title:** Audit semantic and codegen diagnostics for non-`DUMMY` spans  
- **Body:** Grep `Span::DUMMY`, `HirWasmError::at`, `HirLowerError::at`, `AnalyzeError` sites; thread declaration/call spans; document pattern in ROADMAP “Diagnostics” row.  
- **DoD:** `cargo test -p agentscript-compiler` green; spot-check CLI `agentscriptc` on a script that fails codegen.

### `request.security` / `request.financial` host parity

- **Title:** Extend `request.*` to Pine v6 + real host (beyond MWVM stubs)  
- **Body:** Typecheck + HIR + `wasm/abi` + `aether_guest_stubs` + `tests/wasmtime_guest_instantiate.rs` alignment; Aether golden tests when host exists.  
- **DoD:** Documented ABI in `aether/docs/agentscript-guest-abi.md`; compiler tests cover new import shape.

### Guest ABI `init` / `step`

- **Title:** Finalize and invoke `aether_strategy_init` / `aether_strategy_step`  
- **Body:** Memory layout, `aether-common::guest_abi` vs `wasm/abi.rs` import indices; MWVM calls exports after instantiate.  
- **DoD:** Aether integration test calls `step` at least once.

### CLI polish

- **Title:** `agentscriptc -o out.wasm` and optional JSON diagnostics  
- **Body:** See ROADMAP Phase 3.  
- **DoD:** `--emit=wasm -o` writes file; documented in `--help`.

## PR checklist (short)

- [ ] `cargo test -p agentscript-compiler`  
- [ ] If guest imports changed: update [wasmtime_guest_instantiate.rs](../crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) stub linker and `aether-mwvm` [aether_guest_stubs.rs](../../aether/crates/aether-mwvm/src/aether_guest_stubs.rs)  
- [ ] If ABI constants changed: [wasm/abi.rs](../crates/agentscript-compiler/src/codegen/wasm/abi.rs) + Aether ABI doc
