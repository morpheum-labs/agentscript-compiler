# Types (surface syntax)

Normative EBNF: [`spec/agentscripts-v1.md`](../../../spec/agentscripts-v1.md) §3–4.

---

## Primitives

| Syntax | Meaning |
|--------|---------|
| `int` | Integer |
| `float` | Floating-point |
| `bool` | Boolean |
| `string` | String |
| `color` | Color |

---

## Arrays

- **Bracket form (Pine):** `int[]`, `float[]`, `bool[]`, `string[]`, `color[]` — one-dimensional array of that primitive.
- **Generic form:** `array<` *type* `>` — generic array type.

---

## Matrix and map

- `matrix<` *type* `>` — matrix of the element type  
- `map<` *key* `,` *value* `>` — map from key type to value type  

---

## Object / drawing-related types

These are parsed as dedicated type names:

- `label`, `line`, `box`, `table`, `polyline`, `linefill`, `volume_row`
- `chart.point` — chart point type

---

## Named types

A bare identifier at the end of a type can denote a **user** `enum` or `type` name, e.g. in `map<symbols, float>` where `symbols` is an enum.

---

## Variable qualifiers (not types, but type-like positions)

Used with declarations: `var`, `varip`, `const`, `input`, `simple`, `series` — see [`reference/keywords.md`](keywords.md).

---

## Known gaps

The EBNF and roadmap mention types such as **`footprint`** with some `request.*` APIs; the **full** surface for every TV v6 type name is not guaranteed available yet. Track [`ROADMAP.md`](../../../ROADMAP.md) and [`spec/qas-v1-parser-status.md`](../../../spec/qas-v1-parser-status.md).

---

### Checker coverage

The grammar accepts a wide set of type shapes; the **checker** only understands a **subset** for diagnostics and downstream lowering. For parity with TradingView, use [`ROADMAP.md`](../../../ROADMAP.md) Phase 1 rows rather than assuming every parsed type is fully modeled.
