# AgentScript compiler ↔ Aether integration gap

This document tracks **what is missing** to go from **QAS source** to **Aether backtests** with a **pinned, reproducible WASM** artifact. It complements [`ROADMAP.md`](../ROADMAP.md) (compiler phases) and Aether’s [`ROADMAP.md`](../../aether/ROADMAP.md) (runtime phases).

**Contract of record:** [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) + `aether_common::guest_abi` (Rust).

---

## 1. End-to-end artifact gap

| Milestone | Compiler | Aether |
|-----------|----------|--------|
| **Stable `.wasm` bytes** | Production `wasm32-unknown-unknown` (or agreed triple) emission; deterministic builds for `wasm_sha256` pins | Consumes `JobSpec::wasm_sha256` + bytes; preflight today |
| **Guest exports** | Emit `aether_strategy_init` / `aether_strategy_step` (names per ABI doc; finalize **step** signature) | **Does not call exports yet** — `VectorBacktestEngine` still drives results |
| **Guest imports** | Lower `request.*`, `strategy.*`, etc. to WASM **import** declarations matching host | wasmtime path accepts import-less modules; **MWVM imports** need `mwvm-full` / linker story |

**Gap:** No **single** pipeline is “done” until: compiler emits modules that **instantiate under the same rules** Aether uses, **and** the host **invokes** exports and **links** imports (or documents stubs).

---

## 2. Compiler-side gaps (agentscript-compiler)

Roughly ordered by dependency.

1. **HIR coverage** — Today: indicator slice (`input.int`, `close`, `ta.sma`, **`ta.ema`**, `request.security`, `plot`, `close[k]`). **Gap:** rest of typed surface, user functions in HIR, full `request.*` shapes, strategy bodies.
2. **WASM codegen** — **Progress:** `wasm-encoder` emission in [`hir_wasm.rs`](../crates/agentscript-compiler/src/codegen/hir_wasm.rs) for that slice. **Gap:** extend with language coverage; MWVM linker story for all `aether` imports.
3. **Guest ABI in emitted code** — **Progress:** dual exports + `aether` import table documented in [`agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md). **Gap:** evolve `init`/`step` signatures (today v0 preview is `() -> ()`); finalize memory/buffer convention for `step`.
4. **Determinism story** — **Gap:** FP rules, fixed codegen options, optional `cargo_lock_hash` / toolchain metadata for job pins (see Aether ROADMAP optional item).
5. **Semantics vs Pine v6** — **Gap:** bar model, `var`/`varip`, full builtin registry; see ROADMAP semantics table and `pinescriptv6/` checklist.
6. **Tooling** — **Progress:** `--emit=wasm` / `hir` / `ast`. **Gap:** `-o`, JSON diagnostics (ROADMAP Phase 3).

---

## 3. Aether-side gaps (aether)

1. **Invoke guest exports** after preflight — **Gap:** call `init` / `step` (or agreed batch export) and feed OHLCV / bar index per finalized ABI.
2. **Contract tests** — **Gap:** CI test: load compiler-emitted (or pinned fixture) WASM → assert exports exist → optional hash match → **call sequence** smoke.
3. **Host imports** — **Gap:** implement or stub `request.security` / `request.financial` / `strategy.*` as WASM imports wired to oracle / vector engine (stubs first).
4. **ABI doc completion** — **Gap:** finalize `aether_strategy_step` signature (linear memory layout, ptr/len, or fixed struct).

---

## 4. Shared / process gaps

- **Cross-repo tests:** Same WASM bytes verified in **compiler** (emit + validate) and **Aether** (instantiate + export smoke). Ideally one **pinned** `.wasm` fixture in tests.
- **Naming drift:** ABI doc lists `aether_strategy_init` / `aether_strategy_step`; compiler/codegen and `guest_abi` constants must stay in lockstep.

---

## 5. Checklist (close the gap)

Working backlog — track progress here (markdown checkboxes only).

- [ ] Finalize **step** calling convention in `agentscript-guest-abi.md` (memory + types).
- [x] Compiler: **emit** WASM with **exports** matching reserved ABI names (v0 preview signatures `() -> ()`; see ABI doc).
- [x] Compiler: **integration test** — emit → `wasmparser` validate → import/export name checks ([`lib.rs` tests](../crates/agentscript-compiler/src/lib.rs)).
- [x] Compiler: **wasmtime smoke** — `wasmtime::Module::new` accepts emitted bytes (same [`lib.rs`](../crates/agentscript-compiler/src/lib.rs); no host imports linked).
- [ ] Aether: **integration test** — pinned WASM → instantiate → **call `init` / `step`** (stub memory if needed).
- [ ] Define **import** module names and function signatures for `request.*` / `strategy.*` in ABI doc.
- [ ] Compiler: lower at least one **`request.security`** path to an **import call**; Aether: stub host implementation for backtest.
- [ ] Document **one** end-to-end command sequence: `.qas` → `agentscriptc` → `.wasm` → `aether-cli --wasm` (when CLI flags exist).

---

## 6. References

| Doc / code | Role |
|------------|------|
| [`spec/hir.md`](../spec/hir.md) | HIR shape |
| [`ROADMAP.md`](../ROADMAP.md) | Compiler phases & semantics table |
| [`aether/ROADMAP.md`](../../aether/ROADMAP.md) | Sandbox, network, product phases |
| [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) | Export/import contract |
| `vaulted-knowledge-protocol/backtesting-infra` | Tiers, economics (orthogonal to technical gap above) |
