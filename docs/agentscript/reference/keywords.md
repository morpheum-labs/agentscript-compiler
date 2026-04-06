# Keywords

Chumsky’s `text::keyword` is used for reserved spellings in the parser. Identifiers use `text::ident()` and **must not** match these spellings in keyword positions. This page lists the **compiler-facing** keywords and notes **parser vs later phases** where relevant.

Sources: [`script.rs`](../../../crates/agentscript-compiler/src/frontend/parser/script.rs), [`expr.rs`](../../../crates/agentscript-compiler/src/frontend/parser/expr.rs), [`assign_type.rs`](../../../crates/agentscript-compiler/src/frontend/parser/assign_type.rs).

---

## Script and modules

### `indicator`, `strategy`, `library`

Introduce a script declaration with a parenthesized argument list. See [program-structure.md](program-structure.md).

**Compiler note:** Resolver restricts certain builtin namespaces by script kind (e.g. `strategy.*` in strategies). See [`ROADMAP.md`](../../../ROADMAP.md).

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

`method` *name* `(` *params* `)` … — sets [`FnDecl.is_method`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs).

**Compiler note:** Parser accepts methods; full dispatch and typechecking match Pine/TV only where implemented ([`ROADMAP.md`](../../../ROADMAP.md)).

---

## Variable declarations

Qualifiers ([`VarQualifier`](../../../crates/agentscript-compiler/src/frontend/ast/types.rs)):

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
| `break` | Exits innermost `for` / `while` (semantic check: [`loops.rs`](../../../crates/agentscript-compiler/src/semantic/passes/loops.rs)) |
| `continue` | Next iteration of innermost `for` / `while` |

---

## Logical operators (expression keywords)

| Keyword | AST |
|---------|-----|
| `not` | Unary [`UnaryOp::Not`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs) |
| `and` | Binary [`BinOp::And`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs) |
| `or` | Binary [`BinOp::Or`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs) |

---

## Literals

| Keyword | Meaning |
|---------|---------|
| `true`, `false` | Boolean literals |
| `na` | “Not available” / null-like sentinel ([`ExprKind::Na`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs)) |

---

## `switch` arms

Switch bodies use **fat arrow** `=>` between condition and statement (see [`script.rs`](../../../crates/agentscript-compiler/src/frontend/parser/script.rs)): either *expr* `=>` *stmt*, or a default `=>` *stmt*. Scrutinee may be omitted for strategy-style switching.

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

### Compiler note

**Keywords vs builtins:** Names like `ta`, `close`, or `strategy` may appear as **identifier paths** when not in a keyword position. Builtins are not documented exhaustively in this directory; see [`LLM_MANIFEST.md`](../LLM_MANIFEST.md) section 4.
