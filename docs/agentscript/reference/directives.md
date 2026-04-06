# Compiler directives (headers)

AgentScript sources may begin with optional header lines before any top-level items. The reference parser accepts **padding** (whitespace and comments) before headers, and headers may repeat; **duplicates of the same kind** are rejected.

Implementation: [`script_parser`](../../../crates/agentscript-compiler/src/frontend/parser/script.rs) (`HeaderDirective`, `fold_header_directives`), lexer helpers in [`lex.rs`](../../../crates/agentscript-compiler/src/frontend/parser/lex.rs), version policy in [`version_policy.rs`](../../../crates/agentscript-compiler/src/frontend/parser/version_policy.rs).

---

## `//@version=<n>` (Pine-shaped)

- **Form:** `//@version=` immediately after `//` (no space between `//` and `@`), then ASCII digits until end of line.
- **Accepted values:** **`5`** and **`6`** only. Any other number is a parse error with message: *unsupported //@version (only Pine 5 and 6 are accepted)*.

This matches TradingView’s use of the directive as a Pine version marker; AgentScript / QAS **does not** use other numbers here for language revisions (see comments in `version_policy.rs`).

---

## `// @agentscript=<n>` (optional QAS metadata)

- **Form:** `//` then **at least one** space or tab, then `@agentscript=`, then digits until end of line.
- **Constraint:** Parsed value must be **≥ 1**.

Whitespace after `//` is **required** so this line is not confused with `//@version=` (which has no space before `@`).

---

## Interaction with comments

Generic line comments `// …` that are **not** `//@version=` or `// @agentscript=` shapes are ordinary comments. Block comments `/* … */` are handled in padding between tokens.

---

### Compiler note

Parsed values are stored on [`Script`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) as `version: Option<u32>` and `agentscript_version: Option<u32>`. Semantics of `agentscript_version` for tooling or codegen evolve with the roadmap; the **syntax** is fixed as above.
