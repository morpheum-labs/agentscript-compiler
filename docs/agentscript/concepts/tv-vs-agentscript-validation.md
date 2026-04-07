# TradingView Pine vs AgentScript: what validates here

On **TradingView**, Pine Script v6 accepts a large surface; many scripts **run** with full bar semantics, builtins, and chart integration. **AgentScript / QAS** reuses a Pine-shaped syntax but implements a **smaller** semantic and codegen path.

This page lists representative **TV-accurate behaviors** that are **not** matched in AgentScript yet, and states whether a script using them still **passes AgentScript static analysis** (the usual parse + checker pipeline in this project). That is the sense of **“validated”** in the table: **static** acceptance, **not** TradingView parity and **not** successful WASM emission (codegen can be stricter; see [`ROADMAP.md`](../../../ROADMAP.md)).

| Outcome | Meaning |
|---------|---------|
| **Analyze passes** | Static analysis succeeds; resolver and type rules satisfied for the constructs used. |
| **Analyze fails** | Parse may succeed, but the checker reports an error (resolver, typecheck, loop placement, early checks, …). |
| **Parse fails** | Source does not match the **braced** AgentScript grammar (see [`dialect-and-limitations.md`](dialect-and-limitations.md)). |

Normative progress for semantics is still [`ROADMAP.md`](../../../ROADMAP.md) (“Semantics — development progress”). Parser alignment is [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

**Parser note:** Pine-style **leading-zero integers** and **trailing-dot floats** (`0.`) are accepted; optional **function parameter** types use a prefix rule so names like `MAType` are not mistaken for types (see [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md) § function parameter type prefixes). **Braces** are still required for TV examples that use indentation-only bodies.

---

## Validation ladder (what “passes” means at each stage)

AgentScript validation is **layered**: a script can pass an earlier stage and still fail a later one. From coarsest to strictest:

1. **Parse** — [`parse_script`](../../../crates/agentscript-compiler/src/lib.rs) / the braced QAS grammar ([`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md) via [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md)). Success means a well-formed [`Script`](../../../crates/agentscript-compiler/src/frontend/ast/mod.rs) AST.
2. **Static analysis (`check_script`)** — [`check_script`](../../../crates/agentscript-compiler/src/lib.rs) runs the default semantic passes (resolve, typecheck, early checks, etc.). The library helper [`parse_and_analyze`](../../../crates/agentscript-compiler/src/lib.rs) combines parse + analysis. This is what most of the **“Analyze passes”** cells in the tables below refer to.
3. **HIR (`--emit=hir`)** — The CLI [`agentscriptc`](../../../crates/agentscript-compiler/src/main.rs) runs `check_script`, then [`analyze_to_hir_compiler`](../../../crates/agentscript-compiler/src/lib.rs) and prints HIR. Lowering can still be **stricter** than the typechecker for constructs not yet represented in HIR.
4. **WASM (`--emit=wasm`)** — Same entrypoint runs through [`compile_script_to_wasm_v0`](../../../crates/agentscript-compiler/src/lib.rs) (guest subset + [`docs/agentscript-guest-abi.md`](../../agentscript-guest-abi.md)). Integration tests such as `examples_uptrend_pine_parse_and_analyze` exercise **analysis** without requiring a full WASM compile of the same source.

---

## Syntax and dialect (TV accepts; AgentScript differs)

| On TradingView | In AgentScript today | Typical static check |
|----------------|----------------------|----------------------|
| **Indentation-only** bodies for `if` / `while` / etc. | **Braced** blocks only (`{ … }`); indent bodies are **not** in the grammar | **Parse fails** — rewrite with braces |
| **Unbraced** `enum` / `type` bodies (TV-style) | Only **braced** `enum` / `type` forms in scope | **Parse fails** |
| `//@version=` **1–4** or other values | Only **`5`** and **`6`** | **Parse fails** (unsupported version directive) |
| Same script using **v5 vs v6**-specific behavior | Header accepted, but **no** semantic split between v5 and v6 bodies yet | **Analyze passes** if the body is otherwise valid; behavior is **not** TV-split |

---

## Builtins and namespaces (TV full registry; partial here)

| On TradingView | In AgentScript today | Typical static check |
|----------------|----------------------|----------------------|
| Most of **`ta.*`** | **Partial:** a small subset is fully wired through checker and lowering; **many** `ta.*` names are still missing | **Analyze fails** for unknown `ta.*` (unresolved dotted root / signature) |
| **`strategy.*`** in a **strategy** script | Allowed in `strategy()` scripts; **no** full strategy semantics or WASM imports yet ([`ROADMAP.md`](../../../ROADMAP.md)) | **Analyze passes** in `strategy()` scripts for names the checker knows; **no** execution parity |
| **`strategy.*`** in an **indicator** script | **Rejected** in `indicator()` scripts | **Analyze fails** |
| **`request.security` / `request.financial`** with **dynamic** symbols, series args, full MTF rules | **Partial:** literals / restricted shapes may lower to WASM v0; dynamic `syminfo.*` / full merge semantics **open** | **Analyze fails** when args fall outside supported shapes; **passes** for supported literal patterns |
| **Other `request.*`** (e.g. economic, dividend, …) | **Not implemented** | **Analyze fails** (unknown root / builtin) |
| **`mcp.*`** | **Not implemented** | **Analyze fails** |
| **`syminfo.*` / `timeframe.*`** (full set) | **Partial** typing for a **small** `syminfo.*` slice; **no** full parity or lowering for most paths | **Varies:** known paths may **pass**; unknown paths **fail** |
| **`input.*`** beyond **`input.int` / `input.float`** (and the bare `input(...)` forms called out in ROADMAP) | **Partial** — other factories **not** modeled | **Analyze fails** when the builtin is unknown or untyped |
| **`plot`**, **`plotshape`**, **`fill`**, **`alertcondition`**, … | **`plot(...)`** as a **statement** or **expression** (e.g. RHS of `let`) lowers on supported paths; several drawing/alert calls are accepted by the checker but **skipped** in lowering ([`ROADMAP.md`](../../../ROADMAP.md) plot/drawing row) | **Analyze passes** for many supported forms; **WASM v0** still side-effect oriented (`plot` import); **no** full chart/drawing IR |

---

## Types, modules, and execution model

| On TradingView | In AgentScript today | Typical static check |
|----------------|----------------------|----------------------|
| **`import` / `export`** module linking | Host can link a `library()` script with [`register_import_library`](../../../crates/agentscript-compiler/src/lib.rs) ([`ROADMAP.md`](../../../ROADMAP.md), [`aether-integration-gap.md`](../../aether-integration-gap.md) §2.1); **no** TV cloud registry | **`import`:** **Analyze passes** when the alias is unused, bare alias only, or the host registered a library and the call matches an **`export` function** signature. Otherwise qualified use **analyze fails** with a library-linking / unknown-member diagnostic. **HIR** may still fail on linked library **calls** (not lowered yet). **`export`:** same static rules as the matching non-export top-level forms in the library unit |
| Surface **`array<>` / `matrix<>` / `map<>`** enforcement | Type syntax parsed; **not** fully checked ([`ROADMAP.md`](../../../ROADMAP.md) types row) | **Often passes**; generic typing is **incomplete** vs TV |
| **`var` / `varip`** bar persistence | **Partial:** lowering maps persist symbols to globals for a **subset**; full TV bar/`varip` rules **open** | **Passes** when the checker accepts; **semantics differ** from TV |
| **`barstate.*`**, tick replay, session rules | **Not** full TV execution model ([`ROADMAP.md`](../../../ROADMAP.md) bar execution row) | **Varies** — unknown globals **fail**; partial stubs may **pass** without TV behavior |
| **Ternary** `? :` **lazy** branches | Typed and lowered; **lazy TV semantics** still open ([`ROADMAP.md`](../../../ROADMAP.md)) | **Analyze passes**; **may not match TV** evaluation order |
| **Array literals / values** in guest **WASM** | Checker + lowering may build arrays; **WASM v0** may still reject array **values** in the guest path ([`ROADMAP.md`](../../../ROADMAP.md) array row) | **Analyze may pass**; **WASM emit fails** for scripts that require array values in codegen |

---

## How this relates to “will be validated”

- **Today:** “Validated” in this table means **AgentScript’s** static checker, not TradingView’s compiler.
- **Future:** Stricter checks (stricter Pine typing, dynamic `request.*` rules, corpus tests against `spec/pinescriptv6/`) will **narrow** cases that currently **analyze pass** but are **wrong vs TV**; track that work in [`ROADMAP.md`](../../../ROADMAP.md) and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

For syntax-only constraints (braces, headers), see [`dialect-and-limitations.md`](dialect-and-limitations.md). For guest binary rules, see [`docs/agentscript-guest-abi.md`](../../agentscript-guest-abi.md).
