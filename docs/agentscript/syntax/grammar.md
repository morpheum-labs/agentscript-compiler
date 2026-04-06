# Grammar (where it lives)

The **full** QuantAgent Script v1 EBNF lives in **[`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md)** (sections **§1 Lexical** through **§13** and following). That file is the **human-readable contract** for the intended surface language; intentional gaps and “planned” productions are called out in comments there and in [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

This page does **not** duplicate the EBNF block; it points to the **Rust modules** that implement it.

---

## Reference implementation layout

| Concern | Crate path |
|---------|------------|
| Program, items, statements, `switch` / `for` / `while`, function declarations, `enum` / `type`, headers | [`frontend/parser/script.rs`](../../../crates/agentscript-compiler/src/frontend/parser/script.rs) (`script_parser`) |
| Expressions, precedence, calls, indexing, ternary, `if` expr | [`frontend/parser/expr.rs`](../../../crates/agentscript-compiler/src/frontend/parser/expr.rs) (`expr_parser`) |
| Types, qualifiers, assignment operators | [`frontend/parser/assign_type.rs`](../../../crates/agentscript-compiler/src/frontend/parser/assign_type.rs) |
| Whitespace, comments, `//@version`, `// @agentscript`, `=>` | [`frontend/parser/lex.rs`](../../../crates/agentscript-compiler/src/frontend/parser/lex.rs) |
| Leading-directive preflight / bad directives | [`frontend/parser/leading_scan.rs`](../../../crates/agentscript-compiler/src/frontend/parser/leading_scan.rs) |
| Pine `//@version=` allowed values | [`frontend/parser/version_policy.rs`](../../../crates/agentscript-compiler/src/frontend/parser/version_policy.rs) |
| Literals (numbers, strings, hex colors) | [`frontend/parser/literals.rs`](../../../crates/agentscript-compiler/src/frontend/parser/literals.rs) |

AST shapes: [`frontend/ast/`](../../../crates/agentscript-compiler/src/frontend/ast/).

---

## Tests

Regression coverage for parse behavior includes [`tests/parse_smoke.rs`](../../../crates/agentscript-compiler/tests/parse_smoke.rs) and related integration tests under `crates/agentscript-compiler/tests/`.

---

## Relationship to Pine v6 manual

[`spec/pinescriptv6/`](../../../spec/pinescriptv6/) mirrors TradingView’s manual for **checklist and vocabulary**. AgentScript syntax documentation is **[`docs/agentscript/`](../README.md)**; when the manuals disagree, **this compiler and `agentscripts-v1.md`** win until parity work lands.
