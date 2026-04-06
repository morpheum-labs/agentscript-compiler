# Program structure

A **program** is a sequence of **items** after optional header directives. This matches the AST [`Script`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) and the EBNF `program` / `item` rules in [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md).

Implementation: [`script_parser`](../../../crates/agentscript-compiler/src/frontend/parser/script.rs).

---

## Top-level items

| Form | AST |
|------|-----|
| `import` *path* `as` *alias* | [`Item::Import`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| `export` … | [`Item::Export`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| `indicator` `(` *args* `)` \| `strategy` `(` *args* `)` \| `library` `(` *args* `)` | [`Item::ScriptDecl`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| `enum` *name* `{` … `}` | [`Item::Enum`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| `type` *name* `{` … `}` | [`Item::TypeDef`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| User function (see below) | [`Item::FnDecl`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |
| Statement | [`Item::Stmt`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) |

Path segments for `import` are identifiers or numeric segments, separated by `/`, e.g. `User/Lib/1`.

---

## Script declaration

```text
indicator ( named_arg_list )
strategy ( named_arg_list )
library  ( named_arg_list )
```

Arguments are optional, comma-separated, with optional trailing comma. Each argument is either a positional expression or `name = expression` (Pine-style named actuals).

[`ScriptKind`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs): `Indicator`, `Strategy`, `Library`.

---

## User functions

Three shapes parse to the same [`FnDecl`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) (with `is_method` set only for `method`):

1. **Pine-style:** *name* `(` *params* `)` `=>` *expr* **or** *name* `(` *params* `)` `{` *stmts* `}`  
   Tried **before** the QAS `f` form so a function literally named `f` can use this shape.

2. **QAS-style:** `f` *name* `(` *params* `)` `=>` … or `{` … `}`

3. **Method:** `method` *name* `(` *params* `)` `=>` … or `{` … `}`  
   Parser sets `is_method: true`; full method dispatch typing is still evolving (see [`ROADMAP.md`](../../../ROADMAP.md)).

Parameters are optional type, name, optional `=` default, comma-separated.

Function body is either:

- **Expression body:** `=>` *expr* (fat arrow is two characters, see [`fat_arrow`](../../../crates/agentscript-compiler/src/frontend/parser/lex.rs)), or  
- **Block body:** `{` *statements* `}`

---

## `export` (libraries)

Inside a `library()` script, `export` may prefix:

- `enum` …  
- `type` …  
- A user function (`f`, `method`, or Pine-shaped function)  
- Variable declarations in the same forms as statements (`var` / `input` / typed, etc.)

See [`ExportDecl`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs).

---

## Statements at top level

Any statement allowed in a block may appear at top level as [`Item::Stmt`](../../../crates/agentscript-compiler/src/frontend/ast/decl.rs) (including `var` declarations, assignments, control flow, expression statements).

---

### Compiler note

**Parser vs semantics:** The parser builds a wide AST; resolver and typecheck enforce script-kind rules (for example, which builtins are valid in `indicator` vs `strategy`). See [`ROADMAP.md`](../../../ROADMAP.md) Phase 1 and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).
