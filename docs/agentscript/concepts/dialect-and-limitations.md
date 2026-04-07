# Dialect and limitations

AgentScript / QAS is a **Pine-shaped surface** with a **braced-block** grammar and additional QAS forms (`f` functions, optional `// @agentscript=`). It is **not** a byte-for-byte reimplementation of TradingView Pine Script v6.

---

## Braces, not indentation blocks

Control flow and `switch` use **`{ … }`** bodies. TradingView sometimes shows indentation-only bodies in examples; **that style is not** in the reference grammar. See [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

---

## `enum` and `type` bodies

Only **braced** `enum` / `type` forms are in scope for Phase 0. Unbraced TV-style bodies are intentionally out of scope (same status file).

---

## Plot and drawing calls

`plot`, `line.new`, and similar appear as **ordinary calls** or expression statements. The EBNF may describe some of these as statements for clarity; in the language they behave like other calls. See [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md) §9.

---

## Script kind and builtins

The grammar accepts a wide range of dotted identifiers (`ta.*`, `strategy.*`, `request.*`, …). **Static rules** tie some roots to script kind (for example `strategy.*` is only valid inside a `strategy()` script). Details evolve; see [`ROADMAP.md`](../../../ROADMAP.md) Phase 1.

---

## Pine v6 parity checklist

For a **feature-by-feature** comparison between bundled TV manual paths and this project (parser gaps, checker, lowering, WASM), use the table **“Pine v6 parity vs bundled docs (`pinescriptv6/`)”** in [`ROADMAP.md`](../../../ROADMAP.md). The bundled corpus [`spec/pinescriptv6/`](../../../spec/pinescriptv6/) is a **reference checklist**, not a normative guarantee of QAS behavior.

---

## Execution model

Bar-by-bar execution, full `var` / `varip` semantics, and historical series behavior are **runtime / IR** concerns. They are not described here as identical to TradingView; see ROADMAP Phase 2+ and Aether documentation.

For **which TV-accepted constructs pass static analysis vs fail**, see [`tv-vs-agentscript-validation.md`](tv-vs-agentscript-validation.md).

---

## Annotations

Pine compiler annotations (`//@description`, `//@function`, …) are typically **line comments** unless tooling preserves them separately. See [`spec/pinescriptv6/reference/annotations.md`](../../../spec/pinescriptv6/reference/annotations.md) for TV’s list; QAS tooling may add preservation later ([`ROADMAP.md`](../../../ROADMAP.md)).
