# AgentScript strategy guest ABI (Aether)

This document is the contract between **`agentscript-compiler`** (QAS → WASM) and **Aether** (load, sandbox, backtest). Rust constants live in `aether-common::guest_abi`.

A **mirror** for compiler-side workflows lives at `agentscript-compiler/docs/agentscript-guest-abi.md` (same technical content; path relative to that repo).

## Versioning

- `guest_abi::VERSION` (`u32`): increment on breaking changes to exports or calling convention.
  - **2** — Guest export signatures are **`() -> i32`** (init) and **`(i32) -> i32`** (step); replaces the earlier preview where both were `() -> ()`.
  - **3** — Adds required import **`series_string_utf8`** `(i32 kind, i32 dst_off, i32 max_len) -> i32` so series `string` values (e.g. `syminfo.ticker`) can be written into guest memory before `request_security`; see import table below.
- Jobs may pin expected ABI version in `JobSpec` later; today only constants are defined.

## Module

- Logical name: `aether_strategy` (constant `guest_abi::MODULE_NAME` in `crates/aether-common/src/guest_abi.rs`).
- Target: `wasm32-unknown-unknown` unless MWVM documents otherwise.

## Exports (reserved)

| Symbol | Role | Signature |
|--------|------|-------------|
| `aether_strategy_init` | One-time setup (reset `var`/`varip` flags, future tables) | **`() -> i32`** — **`0` = success**, non-zero = reserved error |
| `aether_strategy_step` | One bar / decision point | **`(i32 bar_index) -> i32`** — **`0` = ok**; `bar_index` is the host’s **zero-based** bar ordinal |

Legacy aliases (same function indices, same signatures):

| Symbol | Same as |
|--------|---------|
| `init` | `aether_strategy_init` |
| `on_bar` | `aether_strategy_step` |

### `memory`

Guest modules export **`memory`** (index `0` in compiler emission today) for compile-time string pools, **optional zero-padded scratch** after the pool (two slots of up to **512** bytes each for symbol vs timeframe when `syminfo.*` is used in `request.security`), and future structured context. Hosts read/write via wasmtime linear memory APIs.

### Bar index vs OHLCV today

**v1** passes **`bar_index` only** into `step`. Series values (`series_close`, `series_hist`, etc.) still come from **`aether` imports**, with the host binding those calls to its current bar state (including `bar_index`). **Aether today:** `aether-mwvm::run_guest_strategy_bar_loop_with_limits` accepts `GuestReplay::Ohlcv` so wasmtime stubs read the committed dataset row for `current_bar` before each `aether_strategy_step` (`series_*`, `series_hist*`, and host-side `ta_sma` / `ta_ema` / `ta_tr` / `ta_atr` / `ta_crossover` / `ta_crossunder`). **`GuestReplay::Synthetic`** keeps neutral zeros for CI. See `crates/aether-mwvm/src/lib.rs` and `bar_series_host.rs`. **Future ABI bumps** may add pointer/length pairs or a fixed struct in linear memory for OHLCV batches; document any change here and bump `guest_abi::VERSION`.

### Host invocation sequence

1. Instantiate module + link all **`aether`** imports (stubs or real host).
2. Call **`aether_strategy_init` / `init`** once; check **`i32` return** is `0`.
3. For each bar in replay order, call **`aether_strategy_step` / `on_bar`** with the bar index; check return is `0` (non-zero reserved for future fatals).

**Status:** MWVM preflight can instantiate and link stubs. **`TeeRunner`** (WASM jobs) runs **`init` → `step`×N** with **`GuestReplay::Ohlcv`** so imports see the job’s OHLCV replay; reported **`BacktestResult`** metrics/trades remain a **placeholder** until guest fills are read back. Without WASM, **`VectorBacktestEngine`** still drives the demo vector path.

**MWVM preflight:** `aether-mwvm` (non–`mwvm-full`) registers wasmtime linker stubs for the full `aether` import table via `link_aether_guest_abi_v0` (`aether-mwvm` crate) so compiled strategy WASM from `agentscript-compiler` can **instantiate** in CI; stubs return neutral values / Rust `f64` `ln`/`exp`/`powf` for `math_*` (not Pine-identical until the real host lands). **Export smoke:** [`crates/aether-mwvm/tests/strategy_guest_smoke.rs`](../crates/aether-mwvm/tests/strategy_guest_smoke.rs) calls `init`/`step` on a pinned `.wasm` (see [`tests/fixtures/README.md`](../crates/aether-mwvm/tests/fixtures/README.md)).

## Imports (`aether` module)

The compiler emits a single import module **`aether`**. Names and **stable indices** are defined in `agentscript-compiler` `crates/agentscript-compiler/src/codegen/wasm/abi.rs` (`GUEST_ABI_V0_IMPORTS` — label retained for index stability).

| Import | WASM signature (summary) | Role |
|--------|--------------------------|------|
| `series_close` | `() -> f64` | Current bar close |
| `input_int` | `(i32 idx) -> i32` | Integer input by index |
| `ta_sma` | `(i32 src_kind, i32 period) -> f64` | SMA on host stream |
| `request_security` | `(i32 sym_off, i32 sym_len, i32 tf_off, i32 tf_len, f64 inner) -> f64` | MTF / foreign series; strings in guest memory |
| `plot` | `(f64) -> ()` | Plot side effect |
| `series_hist` | `(i32 offset) -> f64` | `close[offset]` (legacy) |
| `ta_ema` | `(i32 src_kind, i32 period) -> f64` | EMA |
| `input_float` | `(i32 idx) -> f64` | Float input |
| `ta_crossover` / `ta_crossunder` | `(f64, f64) -> f64` | Stateful crosses; bool as `f64` |
| `series_open` / `series_high` / `series_low` / `series_volume` / `series_time` | `() -> f64` | Current bar fields |
| `series_hist_at` | `(i32 series_kind, i32 offset) -> f64` | Historical OHLCV by kind |
| `ta_tr` | `() -> f64` | True range (current bar) |
| `ta_atr` | `(i32 period) -> f64` | ATR |
| `nz` | `(f64, f64) -> f64` | `nz`-style replacement |
| `math_log` / `math_exp` | `(f64) -> f64` | Transcendentals |
| `math_pow` | `(f64, f64) -> f64` | Power |
| `request_financial` | **10× `i32` → `f64`** | Symbol / id / period / currency string slices + `gaps` / `ignore` flags in guest memory (v0 literal-oriented lowering) |
| `series_string_utf8` | `(i32 kind, i32 dst_off, i32 max_len) -> i32` | Host writes UTF-8 for the current bar’s series string; `kind` **`0`** = `syminfo.ticker`, **`1`** = `syminfo.prefix`; returns bytes written (truncates to `max_len`), **`-1`** = na / skip |

**`request_financial`:** `(i32×10) -> f64` — symbol / financial id / period / optional currency string slices, `gaps` `0`/`1`, `ignore` `0`/`1`, `currency` sentinels per compiler docs; MWVM stubs may return `0.0` until the financial oracle is wired.

**`series_string_utf8`:** Called from the guest immediately before passing `dst_off` / returned length into `request_security` when the symbol or timeframe expression is a series `string` builtin. Production hosts supply chart symbol / prefix from job context.

**`ta_crossover` / `ta_crossunder`:** stateful on the host across `step` calls.

## Imports (MWVM / `morpheum`)

Strategy modules may need **MWVM host imports** (`morpheum::*`) when built for full `mwvm-sdk` linking. The wasmtime-only preflight in `aether-mwvm` accepts modules **without** those imports; modules that import `morpheum_*` require **`aether-mwvm` with feature `mwvm-full`** for instantiation.

## Security

- `JobSpec::wasm_sha256` commits to bytecode; the runner passes the same bytes into the sandbox.
- `SandboxLimits` (see `aether-mwvm`) caps per-job linear memory growth and fuel during preflight; execution limits will apply the same knobs when guest calls are wired.
