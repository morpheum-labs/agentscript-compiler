# Operators

Precedence below is **high to low** for AgentScript expressions (Pine-shaped). For exact productions, see [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md).

1. Postfix: `[` *expr* `]` (history/index), `.` field / method-style call  
2. Unary: `+`, `-`, `not`  
3. `*`, `/`, `%`  
4. `+`, `-`  
5. `==`, `!=`, `<`, `>`, `<=`, `>=`  
6. `and`  
7. `or`  
8. Ternary: `?` `:` (right-associative)  
9. Special: Pine-style `if` *cond* *thenExpr* `else` *elseExpr* competes with the ternary chain (see below)

Assignment operators appear in **statement** position only, not inside arbitrary expressions.

---

## Unary

| Op | Meaning |
|----|---------|
| `+` | Unary plus |
| `-` | Negation |
| `not` | Logical not |

---

## Binary

| Op | Meaning |
|----|---------|
| `*`, `/`, `%` | Multiply, divide, modulo |
| `+`, `-` | Add, subtract |
| `==`, `!=` | Equality |
| `<`, `>`, `<=`, `>=` | Ordering |
| `and`, `or` | Short-circuiting boolean (keywords, not `&&` / `||`) |

---

## Ternary

*condition* `?` *then* `:` *else*

---

## `if` expression (Pine-style)

```text
if cond thenExpr else elseExpr
```

There is no `then` keyword: after `if` and the condition, the **next** expression is the then-branch, then `else`, then the else-branch.

This is distinct from the ternary operator.

---

## Assignment (statements)

| Op | Meaning |
|----|---------|
| `=` | Assign or initial binding (context-dependent) |
| `:=` | Reassignment |
| `+=`, `-=`, `*=`, `/=`, `%=` | Compound assignment |

Using `=` where `==` or `=>` is required typically produces a **targeted error** in assignment position.

Simple assignment, tuple assignment, and declarations use these operators in statement forms defined in the EBNF.

---

### Lowering note

Downstream lowering may rewrite compound assignments into read/modify/write forms where supported. See [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md) and [`ROADMAP.md`](../../../ROADMAP.md).
