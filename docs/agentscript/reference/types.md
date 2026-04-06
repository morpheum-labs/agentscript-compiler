# Types (surface syntax)

Type syntax is parsed into [`Type`](../../../crates/agentscript-compiler/src/frontend/ast/types.rs). Implementation: [`assign_type.rs`](../../../crates/agentscript-compiler/src/frontend/parser/assign_type.rs) (`type_parser`, `type_parser_decl_root`, `var_qualifier`).

Normative EBNF: [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md) §3–4.

---

## Primitives

| Syntax | AST |
|--------|-----|
| `int` | `Primitive(Int)` |
| `float` | `Primitive(Float)` |
| `bool` | `Primitive(Bool)` |
| `string` | `Primitive(String)` |
| `color` | `Primitive(Color)` |

---

## Arrays

- **Bracket form (Pine):** `int[]`, `float[]`, `bool[]`, `string[]`, `color[]` — equivalent to a one-dimensional array of that primitive (see `type_parser_core` in `assign_type.rs`).
- **Generic form:** `array<` *type* `>` → `Type::Array(Box<Type>)`.

---

## Matrix and map

- `matrix<` *type* `>` → `Matrix`
- `map<` *key* `,` *value* `>` → `Map`

---

## Object / drawing-related types

Parsed as dedicated variants (not `Named`):

- `label`, `line`, `box`, `table`, `polyline`, `linefill`, `volume_row`
- `chart.point` → `ChartPoint`

---

## Named types

A bare identifier at the end of the `type` parser chain is a **named** type (user `enum` or `type` name), e.g. for `map<symbols, float>` where `symbols` is an enum.

---

## Variable qualifiers (not types, but type-like positions)

Used with declarations: `var`, `varip`, `const`, `input`, `simple`, `series` — see [`VarQualifier`](../../../crates/agentscript-compiler/src/frontend/ast/types.rs) and [`reference/keywords.md`](keywords.md).

---

## Known gaps

- The EBNF and roadmap mention types such as **`footprint`** used with some `request.*` APIs; the **full** surface for every TV v6 type name is not guaranteed implemented. Track [`ROADMAP.md`](../../../ROADMAP.md) and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

---

### Compiler note

**Parser vs typecheck:** The parser accepts a wide set of type shapes; the typechecker only understands a **subset** for diagnostics and HIR lowering. Underestimating or overstating parity with TradingView is avoided here; use ROADMAP Phase 1 rows for status.
