# AgentScript compiler ↔ Aether integration gap

This document tracks **what is missing** to go from **QAS source** to **Aether backtests** with a **pinned, reproducible WASM** artifact. It complements [`ROADMAP.md`](../ROADMAP.md) (compiler phases) and Aether’s [`ROADMAP.md`](../../aether/ROADMAP.md) (runtime phases).

**Contract of record:** [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) + `aether_common::guest_abi` (Rust).

---

## 1. End-to-end artifact gap

| Milestone | Compiler | Aether |
|-----------|----------|--------|
| **Stable `.wasm` bytes** | Production `wasm32-unknown-unknown` (or agreed triple) emission; deterministic builds for `wasm_sha256` pins | Consumes `JobSpec::wasm_sha256` + bytes; preflight today |
| **Guest exports** | Emit `aether_strategy_init` **`() -> i32`** / `aether_strategy_step` **`(i32) -> i32`** (names per ABI doc) | **Production** still uses `VectorBacktestEngine` without guest `step`; **MWVM CI** calls `init`/`step` on a pinned fixture ([`aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs)); compiler [`wasmtime_guest_instantiate.rs`](../crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) does the same on freshly emitted WASM |
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

### 2.1 Pine-style `import` / library resolution (compiler contract)

TradingView resolves `import user/lib/1 as m` against a **published library** in their cloud. This repo has **no** TV publisher; the compiler needs an explicit **host-supplied** mapping before qualified `m.*` can be type-checked or lowered.

**Contract (v0):**

| Input | Role |
|-------|------|
| **Main script** | Parsed `Script`; may contain `import … as alias` lines. |
| **Library script** | A second parsed `Script` whose top-level declaration is `library(...)`. The host (CLI, Aether job loader, or tests) calls [`register_import_library`](../crates/agentscript-compiler/src/lib.rs) on the compile [`CompilerSession`](../crates/agentscript-compiler/src/session.rs) **before** running semantic passes on the main script, passing the **same string** as the import alias (e.g. `m`). |
| **Export surface** | Only **`export` functions** participate in the linked map today. The library script runs the usual early → resolver → lexical → **typecheck** pipeline **alone**; the session then stores each export as [`LibraryLinkedExport`](../crates/agentscript-compiler/src/session.rs) (**inferred signature + cloned `FnDecl`**). Node ids in that clone are cleared so consumer analysis maps stay aligned with the main script only. **`export method`** is rejected at registration (same restriction as ordinary user `method` lowering). |

**Non-goals (v0):** matching TV’s `user/lib/version` registry, private vs public publish flags, or multi-unit graphs beyond host-registered aliases. **Progress (HIR / WASM):** on the consumer script, HIR lowering registers each linked export under a **mangled** top-level name `__import__{alias}__{member}` (reserved; treat as compiler-internal), lowers `alias.member(...)` to [`HirExpr::UserCall`](../crates/agentscript-compiler/src/hir/script.rs) on that symbol, and emits it like any other user function in [`hir_wasm.rs`](../crates/agentscript-compiler/src/codegen/hir_wasm.rs) (`UserCall` → `call`). Nested `otherAlias.foo` **inside** a library body is out of scope unless that alias is also registered on the same session.

**Future options** (not committed here): JSON manifest of exports only (no source), workspace-relative path glob, or Aether passing pre-parsed `Script` blobs per job dependency.

---

## 3. Aether-side gaps (aether)

1. **Invoke guest exports** after preflight — **Gap:** call `init` / `step` (or agreed batch export) and feed OHLCV / bar index per finalized ABI.
2. **Contract tests** — **Progress:** [`aether-mwvm/tests/strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs) loads pinned [`tiny_strategy_guest.wasm`](../../aether/crates/aether-mwvm/tests/fixtures/tiny_strategy_guest.wasm) → link stubs → **call `init` / `step`**. **Gap:** production runner + hash-pinned cross-repo fixture workflow (optional).
3. **Host imports** — **Progress:** `aether-mwvm` stubs **`request.security`** (identity on inner) and **`request.financial`** (`0.0`). **Gap:** real oracle / vector engine wiring; `strategy.*` and remaining `request.*`.
4. **ABI doc completion** — **Progress:** v1 `step(bar_index: i32) -> i32` documented. **Gap:** optional ptr/len / struct layout for batched OHLCV (next bump).

---

## 4. Shared / process gaps

- **Cross-repo tests:** Compiler emits + validates; Aether MWVM uses a **checked-in** `.wasm` ([`aether/crates/aether-mwvm/tests/fixtures/`](../../aether/crates/aether-mwvm/tests/fixtures/README.md)) regenerated via `agentscriptc` (see §5.1). `agentscript-compiler` [`tests/wasmtime_guest_instantiate.rs`](../crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) compiles minimal scripts at test time — stub linker must stay aligned with `aether-mwvm` `link_aether_guest_abi_v0` (a compiler dev-dependency on `aether-mwvm` was avoided: some toolchains fail the combined graph). When `request_financial` or other import **signatures** change, update duplicated stub closures in that test, in [`aether_guest_stubs.rs`](../../aether/crates/aether-mwvm/src/aether_guest_stubs.rs), and regenerate the pinned WASM if needed.
- **Issue stubs:** [github-backlog.md](github-backlog.md) mirrors ROADMAP bullets for GitHub / PR checklists.
- **Naming drift:** ABI doc lists `aether_strategy_init` / `aether_strategy_step`; compiler/codegen and `guest_abi` constants must stay in lockstep.

---

## 5. Checklist (close the gap)

Working backlog — track progress here (markdown checkboxes only).

- [x] Finalize **step** calling convention in `agentscript-guest-abi.md` — **v1:** `(i32 bar_index) -> i32`; memory OHLCV deferred.
- [x] Compiler: **emit** WASM with **exports** matching reserved ABI names (**v1:** `() -> i32` init, `(i32) -> i32` step; see ABI doc).
- [x] Compiler: **integration test** — emit → `wasmparser` validate → import/export name checks ([`lib.rs` tests](../crates/agentscript-compiler/src/lib.rs)).
- [x] Compiler: **wasmtime smoke** — `wasmtime::Module::new` accepts emitted bytes (same [`lib.rs`](../crates/agentscript-compiler/src/lib.rs); no host imports linked).
- [x] Aether: **integration test** — pinned WASM → instantiate → **call `init` / `step`** ([`strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs), fixture [`tiny_strategy_guest.wasm`](../../aether/crates/aether-mwvm/tests/fixtures/tiny_strategy_guest.wasm)).
- [x] Define **`aether` import** names and signatures for current lowered surface (`request_security`, `request_financial`, …) in ABI doc; **`strategy.*`** still open when codegen lands.
- [x] Compiler: lower **`request.security`** to **`request_security`** import; Aether MWVM: stub (pass-through inner `f64`).
- [x] Compiler: lower **`request.financial`** v0 to **`request_financial`** import; Aether MWVM: stub (`0.0`).
- [x] Document **end-to-end compile path** (below); unified **`aether-cli --wasm`** — **TBD** (no such binary in-tree today).

### 5.1 End-to-end: source → `.wasm` → run (stub level)

1. **Compile** (from [`agentscript-compiler`](../) workspace; stdout is raw WASM — there is no `-o` yet):

   ```bash
   cargo run -p agentscript-compiler --bin agentscriptc -- --emit=wasm path/to/script.pine > out.wasm
   ```

   Use `.pine` or `.qas` sources the parser accepts. **Validate** emitted bytes in Rust with [`validate_guest_abi_v1`](../crates/agentscript-compiler/src/codegen/wasm/abi.rs) (as in compiler tests) or rely on `wasmparser::validate`.

2. **Load in Aether MWVM** (programmatic): [`instantiate_job_wasm`](../../aether/crates/aether-mwvm/src/lib.rs) compiles and instantiates with `aether` stubs linked; it does **not** yet invoke `init`/`step`. For a full **export smoke**, use the same pattern as [`strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs) (`link_aether_guest_abi_v0` → `instantiate` → typed `init` / `step`).

3. **Production backtest** wiring (`VectorBacktestEngine`, oracle-backed imports) remains **separate** — see [`aether/docs/data-back.md`](../../aether/docs/data-back.md).

---

## 6. References

| Doc / code | Role |
|------------|------|
| [`spec/hir.md`](../spec/hir.md) | HIR shape |
| [`ROADMAP.md`](../ROADMAP.md) | Compiler phases & semantics table |
| [`aether/ROADMAP.md`](../../aether/ROADMAP.md) | Sandbox, network, product phases |
| [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) | Export/import contract |
| [`aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs) | Pinned WASM + `init`/`step` smoke |
| [`ROADMAP.md`](../ROADMAP.md) “Next chapter” | Follow-on: **`request.security` dynamics** (typecheck → HIR → wasm → ABI); dedicated PR |
| `vaulted-knowledge-protocol/backtesting-infra` | Tiers, economics (orthogonal to technical gap above) |
