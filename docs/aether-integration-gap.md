# AgentScript compiler ↔ Aether integration gap

This document tracks **what is missing** to go from **QAS source** to **Aether backtests** with a **pinned, reproducible WASM** artifact. It complements [`ROADMAP.md`](../ROADMAP.md) (compiler phases) and Aether’s [`ROADMAP.md`](../../aether/ROADMAP.md) (runtime phases).

**Contract of record:** [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) + `aether_common::guest_abi` (Rust).

---

## 1. End-to-end artifact gap

| Milestone | Compiler | Aether |
|-----------|----------|--------|
| **Stable `.wasm` bytes** | Production `wasm32-unknown-unknown` (or agreed triple) emission; deterministic builds for `wasm_sha256` pins | Consumes `JobSpec::wasm_sha256` + bytes; preflight today |
| **Guest exports** | Emit `aether_strategy_init` **`() -> i32`** / `aether_strategy_step` **`(i32) -> i32`** (names per ABI doc) | **Does not call exports in production** — `VectorBacktestEngine` still drives results; compiler wasmtime test calls `init`/`step` |
| **Guest imports** | Lower `request.*`, `strategy.*`, etc. to WASM **import** declarations matching host | wasmtime path accepts import-less modules; **MWVM imports** need `mwvm-full` / linker story |

**Gap:** No **single** pipeline is “done” until: compiler emits modules that **instantiate under the same rules** Aether uses, **and** the host **invokes** exports and **links** imports (or documents stubs).

---

## 2. Compiler-side gaps (agentscript-compiler)

Roughly ordered by dependency.

1. **HIR coverage** — Today: indicator slice (`input.int`, `close`, `ta.sma`, **`ta.ema`**, `request.security`, **`request.financial`** (v0 literals), `plot`, `close[k]`). **Gap:** rest of typed surface, user functions in HIR, full `request.*` shapes (gaps, currency, …), strategy bodies.
2. **WASM codegen** — **Progress:** `wasm-encoder` emission in [`hir_wasm.rs`](../crates/agentscript-compiler/src/codegen/hir_wasm.rs) for that slice. **Gap:** extend with language coverage; MWVM linker story for all `aether` imports.
3. **Guest ABI in emitted code** — **Progress:** dual exports + `aether` import table documented in [`agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) and mirrored in [`agentscript-guest-abi.md`](agentscript-guest-abi.md). **Done (v1):** `init` **`() -> i32`**, `step` **`(i32 bar_index) -> i32`**; [`validate_guest_abi_v1`](../crates/agentscript-compiler/src/codegen/wasm/abi.rs). **Still open:** optional linear-memory OHLCV layout for a future ABI bump.
4. **Determinism story** — **Gap:** FP rules, fixed codegen options, optional `cargo_lock_hash` / toolchain metadata for job pins (see Aether ROADMAP optional item).
5. **Semantics vs Pine v6** — **Gap:** bar model, `var`/`varip`, full builtin registry; see ROADMAP semantics table and `pinescriptv6/` checklist.
6. **Tooling** — **Progress:** `--emit=wasm` / `hir` / `ast`. **Gap:** `-o`, JSON diagnostics (ROADMAP Phase 3).

---

## 3. Aether-side gaps (aether)

1. **Invoke guest exports** after preflight — **Gap:** call `init` / `step` (or agreed batch export) and feed OHLCV / bar index per finalized ABI.
2. **Contract tests** — **Gap:** CI test: load compiler-emitted (or pinned fixture) WASM → assert exports exist → optional hash match → **call sequence** smoke.
3. **Host imports** — **Progress:** `aether-mwvm` stubs **`request.security`** (identity on inner) and **`request.financial`** (`0.0`). **Gap:** real oracle / vector engine wiring; `strategy.*` and remaining `request.*`.
4. **ABI doc completion** — **Progress:** v1 `step(bar_index: i32) -> i32` documented. **Gap:** optional ptr/len / struct layout for batched OHLCV (next bump).

---

## 4. Shared / process gaps

- **Cross-repo tests:** Same WASM bytes verified in **compiler** (emit + validate) and **Aether** (instantiate + export smoke). Ideally one **pinned** `.wasm` fixture in tests. **Today:** `agentscript-compiler` integration test [`tests/wasmtime_guest_instantiate.rs`](../crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) compiles minimal scripts and **instantiates** with a stub linker that must stay aligned with `aether-mwvm` `link_aether_guest_abi_v0` (an `aether-mwvm` dev-dependency on the compiler was avoided: some toolchains fail building the combined graph via `ar_archive_writer` / rustc features). When `request_financial` or other import **signatures** change, update the duplicated stub closures in that test and in `aether-mwvm` (`10×i32` args as of 2026-04).
- **Issue stubs:** [github-backlog.md](github-backlog.md) mirrors ROADMAP bullets for GitHub / PR checklists.
- **Naming drift:** ABI doc lists `aether_strategy_init` / `aether_strategy_step`; compiler/codegen and `guest_abi` constants must stay in lockstep.

---

## 5. Checklist (close the gap)

Working backlog — track progress here (markdown checkboxes only).

- [x] Finalize **step** calling convention in `agentscript-guest-abi.md` — **v1:** `(i32 bar_index) -> i32`; memory OHLCV deferred.
- [x] Compiler: **emit** WASM with **exports** matching reserved ABI names (**v1:** `() -> i32` init, `(i32) -> i32` step; see ABI doc).
- [x] Compiler: **integration test** — emit → `wasmparser` validate → import/export name checks ([`lib.rs` tests](../crates/agentscript-compiler/src/lib.rs)).
- [x] Compiler: **wasmtime smoke** — `wasmtime::Module::new` accepts emitted bytes (same [`lib.rs`](../crates/agentscript-compiler/src/lib.rs); no host imports linked).
- [ ] Aether: **integration test** — pinned WASM → instantiate → **call `init` / `step`** (stub memory if needed).
- [x] Define **`aether` import** names and signatures for current lowered surface (`request_security`, `request_financial`, …) in ABI doc; **`strategy.*`** still open when codegen lands.
- [x] Compiler: lower **`request.security`** to **`request_security`** import; Aether MWVM: stub (pass-through inner `f64`).
- [x] Compiler: lower **`request.financial`** v0 to **`request_financial`** import; Aether MWVM: stub (`0.0`).
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
