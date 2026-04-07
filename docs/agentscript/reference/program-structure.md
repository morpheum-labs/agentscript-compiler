# Program structure

A **program** is a sequence of **items** after optional header directives. The shape matches the EBNF `program` / `item` rules in [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md).

---

## Top-level items

| Form | Role |
|------|------|
| `import` *path* `as` *alias* | Library import (Pine-style path) |
| `export` … | Export from a `library()` script (see below) |
| `indicator` `(` *args* `)` \| `strategy` `(` *args* `)` \| `library` `(` *args* `)` | Script kind and metadata |
| `enum` *name* `{` … `}` | Enumeration |
| `type` *name* `{` … `}` | User-defined type |
| User function (see below) | Callable defined in the script |
| Statement | Any statement valid inside a block, at top level |

Path segments for `import` are identifiers or numeric segments, separated by `/`, e.g. `User/Lib/1`.

---

## Script declaration

```text
indicator ( named_arg_list )
strategy ( named_arg_list )
library  ( named_arg_list )
```

Arguments are optional, comma-separated, with optional trailing comma. Each argument is either a positional expression or `name = expression` (Pine-style named actuals).

Each form selects a **script kind**: indicator, strategy, or library. Static checking ties some builtins (for example `strategy.*`) to the script kind.

---

## User functions

Three shapes are accepted (the Pine-style form is tried first so a function literally named `f` can use it):

1. **Pine-style:** *name* `(` *params* `)` `=>` *expr* **or** *name* `(` *params* `)` `{` *stmts* `}`

2. **QAS-style:** `f` *name* `(` *params* `)` `=>` … or `{` … `}`

3. **Method:** `method` *name* `(` *params* `)` `=>` … or `{` … `}`  
   Full method dispatch and typing may still differ from TradingView where noted in [`ROADMAP.md`](../../../ROADMAP.md).

Parameters are optional type, name, optional `=` default, comma-separated.

Function body is either:

- **Expression body:** `=>` *expr* — the fat arrow is the two-character token `=>`, not `=` followed by `>`, or  
- **Block body:** `{` *statements* `}`

---

## `export` (libraries)

Inside a `library()` script, `export` may prefix:

- `enum` …  
- `type` …  
- A user function (`f`, `method`, or Pine-shaped function)  
- Variable declarations in the same forms as statements (`var` / `input` / typed, etc.)

---

## Statements at top level

Any statement allowed in a block may appear at top level (including `var` declarations, assignments, control flow, expression statements).

---

### Static checking

The grammar allows a wide range of dotted names (`ta.*`, `strategy.*`, `request.*`, …). **Script-kind and builtin rules** restrict what is valid in a given script (for example, `strategy.*` in an `indicator()`). See [`ROADMAP.md`](../../../ROADMAP.md) Phase 1 and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).
