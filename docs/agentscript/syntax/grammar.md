# Grammar (normative specification)

The **full** QuantAgent Script v1 grammar is written as EBNF in **[`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md)** (sections **§1 Lexical** through **§13** and following). That document is the **contract** for the intended surface language. Intentional gaps and “planned” productions are called out in comments there and in [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

This page does **not** duplicate the EBNF. It explains how to use the spec and how it relates to other docs.

---

## What the EBNF sections cover (guide)

| Area | Typical § range (see spec headings) |
|------|-------------------------------------|
| Lexical structure, comments, tokens | §1 and related |
| Types, qualifiers, declarations | §3–4 (exact numbering in spec) |
| Expressions, operators, calls | Expression productions in spec |
| Statements, control flow | `if` / `for` / `while` / `switch`, blocks |
| Top-level program: imports, `export`, `indicator` / `strategy` / `library`, `enum` / `type`, functions | `program` / `item` rules |

Always use **`spec/agentscripts-v1.md`** for authoritative spellings and ordering. Use **`spec/qas-v1-parser-status.md`** when you need to know whether a production is fully enforced by the checker yet.

---

## Relationship to Pine v6 manual

[`spec/pinescriptv6/`](../../../spec/pinescriptv6/) mirrors TradingView’s manual for **checklist and vocabulary**. AgentScript language documentation lives under **[`docs/agentscript/`](../README.md)**. When the manuals disagree, **AgentScript’s grammar and checker** win until parity work lands.
