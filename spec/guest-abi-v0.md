# Guest WASM ABI (compiler emission v0)

This describes what **`agentscript-compiler`** emits today from [`emit_hir_guest_wasm`](../crates/agentscript-compiler/src/codegen/hir_wasm.rs). The long-term contract with Aether is documented in **`aether/docs/agentscript-guest-abi.md`** and `aether_common::guest_abi`; this file tracks the **current** bytecode shape so hosts can stub imports and dual-export names stay in sync.

## Exports

| Name | Same index as | WASM signature (today) |
|------|----------------|-------------------------|
| `memory` | — | `memory` min 1 page |
| `init` | `aether_strategy_init` | `() -> ()` |
| `on_bar` | `aether_strategy_step` | `(locals…) -> ()` |

`aether_strategy_*` names match [`aether_common::guest_abi`](../../aether/crates/aether-common/src/guest_abi.rs). Planned signature change to `() -> i32` for init (per Aether doc) is **not** implemented in the compiler yet.

## Imports (`module: "aether"`)

| Name | Signature | Purpose |
|------|-----------|---------|
| `series_close` | `() -> f64` | Current bar close |
| `input_int` | `(i32 idx) -> i32` | Script input by index in `HirScript::inputs` |
| `ta_sma` | `(i32 period) -> f64` | SMA of host primary series |
| `request_security` | `(i32 sym_off, sym_len, tf_off, tf_len, f64 inner) -> f64` | Strings in guest linear memory |
| `plot` | `(f64) -> ()` | Plot side effect |
| `series_hist` | `(i32 offset) -> f64` | Primary series value `offset` bars ago (v0: lowered only for `close[offset]` in HIR) |

Full import order and additional functions (`request_financial`, `series_string_utf8`, …) are in [`codegen/wasm/abi.rs`](../crates/agentscript-compiler/src/codegen/wasm/abi.rs) `GUEST_ABI_V0_IMPORTS`.

## Host responsibilities

- **`series_hist`**: For offset `0`, behavior should align with `series_close`; for `k > 0`, return the historical close `k` bars back (or `na` policy as defined by the runtime).
- **String arguments** to `request_security`: UTF-8 in guest memory starting at `sym_off` / `tf_off` with given lengths (same buffer as the module `data` section uses at address 0 today).
- **`series_string_utf8`**: `(i32 kind, i32 dst_off, i32 max_len) -> i32` — host writes series `string` builtins (e.g. `syminfo.ticker`) into scratch at `dst_off` before `request_security` reads those offsets; see `aether/docs/agentscript-guest-abi.md`.

## Versioning

When this layout changes, bump expectations in **`hir_wasm` contract tests** and coordinate with `guest_abi::VERSION` in Aether.
