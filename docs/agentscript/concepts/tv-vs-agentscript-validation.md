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
| **`plot`**, **`plotshape`**, **`fill`**, **`alertcondition`**, … | **`plot(expr)`** lowers in some paths; several drawing/alert calls are accepted by the checker but **skipped** in lowering ([`ROADMAP.md`](../../../ROADMAP.md) plot/drawing row) | **Analyze passes** for many statement-level forms; **no** chart/drawing IR |

---

## Types, modules, and execution model

| On TradingView | In AgentScript today | Typical static check |
|----------------|----------------------|----------------------|
| **`import` / `export`** module linking | Parsed; **no** module graph or link step ([`ROADMAP.md`](../../../ROADMAP.md) imports/exports row) | **`import`:** **Analyze passes** when the alias is unused (or only the bare alias is referenced); **qualified** `alias.member` or `alias.fn(...)` **analyze fails** with an explicit library-linking diagnostic. **`export`:** same static rules as the matching non-export top-level forms; **no** cross-module validation |
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
