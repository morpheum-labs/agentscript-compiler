# AgentScript strategy guest ABI (compiler mirror)

This file **mirrors** the canonical contract in the Aether repository:

`aether/docs/agentscript-guest-abi.md`

Keep them in sync when changing export signatures, import tables, or versioning. **`aether-common::guest_abi::VERSION`** is the numeric pin hosts may check (**3** adds required import **`series_string_utf8`** for series `string` in `request.security` args — see canonical Aether doc).

## Quick reference (guest ABI v1)

| Export | Signature |
|--------|-----------|
| `aether_strategy_init` / `init` | **`() -> i32`** (`0` = success) |
| `aether_strategy_step` / `on_bar` | **`(i32 bar_index) -> i32`** (`0` = ok) |

| Also required | Notes |
|---------------|--------|
| `memory` | String pool + future context |

**Imports:** module **`aether`**, exact names and order in `crates/agentscript-compiler/src/codegen/wasm/abi.rs` (`GUEST_ABI_V0_IMPORTS`).

**Validation:** [`validate_guest_abi_v1`](../crates/agentscript-compiler/src/codegen/wasm/abi.rs) on emitted bytes (imports, exports, **export function signatures**).

**Integration tests:** Compiler [`tests/wasmtime_guest_instantiate.rs`](../crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs); Aether [`aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs`](../../aether/crates/aether-mwvm/tests/strategy_guest_smoke.rs) (pinned WASM + `init`/`step`). Import stubs must match `link_aether_guest_abi_v0` in both places.
