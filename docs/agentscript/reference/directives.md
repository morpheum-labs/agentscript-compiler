# Compiler directives (headers)

AgentScript sources may begin with optional header lines before any top-level items. **Padding** (whitespace and comments) may appear before headers. Headers may repeat; **duplicates of the same kind** are rejected.

---

## `//@version=<n>` (Pine-shaped)

- **Form:** `//@version=` immediately after `//` (no space between `//` and `@`), then ASCII digits until end of line.
- **Accepted values:** **`5`** and **`6`** only. Any other number is rejected with a message along the lines of: *unsupported //@version (only Pine 5 and 6 are accepted)*.

This follows TradingView’s use of the directive as a Pine version marker. AgentScript / QAS does **not** use other numbers here for language revisions.

---

## `// @agentscript=<n>` (optional QAS metadata)

- **Form:** `//` then **at least one** space or tab, then `@agentscript=`, then digits until end of line.
- **Constraint:** The value must be **≥ 1**.

Whitespace after `//` is **required** so this line is not confused with `//@version=` (which has no space before `@`).

Optional `@agentscript` versions may be used by **tooling**; meaning beyond syntax is defined by the language roadmap ([`ROADMAP.md`](../../../ROADMAP.md)).

---

## Interaction with comments

Generic line comments `// …` that are **not** `//@version=` or `// @agentscript=` shapes are ordinary comments. Block comments `/* … */` are allowed in padding between tokens.
