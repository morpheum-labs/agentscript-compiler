# Keywords

These spellings are **reserved** in keyword positions (they cannot be used as ordinary identifiers where the grammar expects a keyword). Identifiers elsewhere follow the lexical rules in [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md).

This page lists language keywords and notes **partial vs TradingView** behavior where it matters.

Sources for the exact grammar: [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md) and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

---

## Script and modules

### `indicator`, `strategy`, `library`

Introduce a script declaration with a parenthesized argument list. See [program-structure.md](program-structure.md).

**Note:** Script kind affects which builtin namespaces are valid (e.g. `strategy.*` in strategies). See [`ROADMAP.md`](../../../ROADMAP.md).

### `import`

`import` *path* `as` *alias* — Pine-style library path.

### `export`

Valid in `library()` scripts; see [program-structure.md](program-structure.md).

---

## Types and user-defined types

### `enum`

`enum` *Name* `{` *variant* `=` *expr* `;` … `}` — braced body required in this dialect.

### `type`

`type` *Name* `{` *fields* `}` — user-defined type with field defaults.

---

## Functions

### `f`

QAS user-function introducer: `f` *name* `(` *params* `)` `=>` … or `{` … `}`.

### `method`

`method` *name* `(` *params* `)` … — method-style declaration. Parser and checker support may still lag TradingView in some cases ([`ROADMAP.md`](../../../ROADMAP.md)).

---

## Variable declarations

Qualifiers (see [types.md](types.md) for qualifier positions):

| Keyword | Role |
|---------|------|
| `var` | Persistent variable |
| `varip` | Intrabar persistent |
| `const` | Constant |
| `input` | Input (also standalone `input` declaration form) |
| `simple` | Simple type qualifier (Pine) |
| `series` | Series qualifier (Pine) |

Forms combine *qualifier?* *type?* *name* *assign_op* *expr*; see EBNF in [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md) §3.

---

## Control flow

| Keyword | Meaning |
|---------|---------|
| `if` | Statement: `if` *cond* `{` … `}` with optional `else` / `else if`. Expression: see [operators.md](operators.md) (`if`–`else` expression). |
| `else` | Part of `if` statement or `if` expression |
| `for` | Numeric `for` *i* `=` *from* `to` *to* [`by` *step*] `{` … `}` **or** `for` *x* `in` *expr* `{` … `}` **or** `for` `[` *i* `,` *v* `]` `in` *expr* `{` … `}` |
| `to`, `by` | Parts of numeric `for` (`to` required; `by` optional) |
| `in` | `for … in` only |
| `while` | `while` *cond* `{` … `}` |
| `switch` | `switch` *scrutinee?* `{` *arms* `}` — arms use `=>` (see below) |
| `break` | Exits innermost `for` / `while` (must appear inside a loop) |
| `continue` | Next iteration of innermost `for` / `while` |

---

## Logical operators (expression keywords)

| Keyword | Role |
|---------|------|
| `not` | Logical not (unary) |
| `and` | Logical and (short-circuiting) |
| `or` | Logical or (short-circuiting) |

---

## Literals

| Keyword | Meaning |
|---------|---------|
| `true`, `false` | Boolean literals |
| `na` | “Not available” / null-like sentinel |

---

## `switch` arms

Switch bodies use **fat arrow** `=>` between condition and statement: either *expr* `=>` *stmt*, or a default `=>` *stmt*. The scrutinee may be omitted for strategy-style switching.

---

## Examples (minimal)

```pine
//@version=6
indicator("Kw", overlay = true)

f add(float a, float b) =>
    a + b

for i = 0 to 10 by 2 {
    continue
}

switch close {
    1.0 => 1
    => na
}
```

---

### Builtins vs keywords

Names like `ta`, `close`, or `strategy` may appear as **identifier paths** when not in a keyword position. Builtin libraries are not exhaustively documented in this directory; see [`LLM_MANIFEST.md`](../LLM_MANIFEST.md) section 4 and [`ROADMAP.md`](../../../ROADMAP.md).
