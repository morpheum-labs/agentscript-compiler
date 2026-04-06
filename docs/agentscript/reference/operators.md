# Operators

Expression parsing is implemented in [`expr.rs`](../../../crates/agentscript-compiler/src/frontend/parser/expr.rs). AST enums: [`UnaryOp`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs), [`BinOp`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs), [`AssignOp`](../../../crates/agentscript-compiler/src/frontend/ast/stmt.rs).

Precedence (high to low) in the reference parser:

1. Postfix: `[` *expr* `]` (history/index), `.` field / method-style call
2. Unary: `+`, `-`, `not`
3. `*`, `/`, `%`
4. `+`, `-`
5. `==`, `!=`, `<`, `>`, `<=`, `>=`
6. `and`
7. `or`
8. Ternary: `?` `:` (right-associative)
9. Special: Pine-style `if` *cond* *thenExpr* `else` *elseExpr* is parsed as a single expression form competing with the ternary chain (see below)

Assignment operators are parsed in statement position only ([`assign_op`](../../../crates/agentscript-compiler/src/frontend/parser/assign_type.rs)).

---

## Unary

| Op | Meaning |
|----|---------|
| `+` | Unary plus |
| `-` | Negation (constant folding for numeric literals) |
| `not` | Logical not |

---

## Binary

| Op | Meaning |
|----|---------|
| `*`, `/`, `%` | Multiply, divide, modulo |
| `+`, `-` | Add, subtract |
| `==`, `!=` | Equality |
| `<`, `>`, `<=`, `>=` | Ordering |
| `and`, `or` | Short-circuiting boolean (parsed as keywords, not `&&` / `||`) |

---

## Ternary

*condition* `?` *then* `:` *else* — [`ExprKind::Ternary`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs).

---

## `if` expression (Pine-style)

```text
if cond thenExpr else elseExpr
```

No `then` keyword: after `if` and the condition expression, the **next** expression is the then-branch, then `else`, then the else-branch. AST: [`ExprKind::IfExpr`](../../../crates/agentscript-compiler/src/frontend/ast/expr.rs).

This is distinct from the ternary operator.

---

## Assignment (statements)

| Op | Meaning |
|----|---------|
| `=` | Assign or initial binding (context-dependent) |
| `:=` | Reassignment |
| `+=`, `-=`, `*=`, `/=`, `%=` | Compound assignment |

**Parse detail:** A single `=` is not allowed to absorb the start of `==` or `=>`; the parser reports tailored errors if those appear in assignment position ([`assign_op`](../../../crates/agentscript-compiler/src/frontend/parser/assign_type.rs)).

Tuple and simple assignment forms are in [`StmtKind`](../../../crates/agentscript-compiler/src/frontend/ast/stmt.rs): `Assign`, `TupleAssign`, `VarDecl`.

---

### Compiler note

HIR lowering may rewrite compound assignments into read/modify/write forms where supported; see [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md) and [`ROADMAP.md`](../../../ROADMAP.md).
