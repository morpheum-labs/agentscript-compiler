# AgentScript documentation manifest

**Purpose:** Route retrieval for tools and humans: pick the **smallest** file that answers the question, then open linked normative sources if needed.

**Normative grammar:** [`spec/agentscripts-v1.md`](../../spec/agentscripts-v1.md) (QAS v1 EBNF §§1–13).

**Parser and semantics status:** [`spec/qas-v1-parser-status.md`](../../spec/qas-v1-parser-status.md).

**Roadmap / Pine checklist:** [`ROADMAP.md`](../../ROADMAP.md).

---

## 1. Syntax and program shape

Use when the user asks how a script is structured, what may appear at top level, or how headers work.

* **[`reference/program-structure.md`](reference/program-structure.md)**  
  * **Content:** `indicator` / `strategy` / `library`, `import` / `export`, `enum` / `type`, user functions (`name() =>`, `f name() =>`, `method name() =>`), statements.  
  * **Keywords:** `indicator`, `strategy`, `library`, `import`, `export`, `enum`, `type`, `method`, `f`.

* **[`reference/directives.md`](reference/directives.md)**  
  * **Content:** Pine `//@version=` (only `5` and `6`), optional `// @agentscript=<n>`, duplicate rules.  
  * **Keywords:** version header, AgentScript header.

* **[`syntax/grammar.md`](syntax/grammar.md)**  
  * **Content:** Where the EBNF lives (§§1–13), what each major section covers, pointers to `spec/qas-v1-parser-status.md` for gaps.

---

## 2. Lexical and expression reference

Use for tokens, operators, types, and keyword semantics.

* **[`reference/keywords.md`](reference/keywords.md)**  
  * **Content:** Control flow, declarations, literals, `and` / `or` / `not`, function forms; notes where behavior is still partial vs TradingView.  
  * **Keywords:** `if`, `else`, `for`, `while`, `switch`, `var`, `varip`, `const`, `input`, `simple`, `series`, `break`, `continue`, `true`, `false`, `na`, and others listed there.

* **[`reference/types.md`](reference/types.md)**  
  * **Content:** Primitives, `float[]`-style arrays, `array<>` / `matrix<>` / `map<>`, object types, named UDT/enum types; known gaps (e.g. `footprint`).  
  * **Keywords:** `int`, `float`, `bool`, `string`, `color`, `label`, `line`, `box`, …

* **[`reference/operators.md`](reference/operators.md)**  
  * **Content:** Precedence sketch, arithmetic and comparison, `and` / `or`, `? :`, Pine `if`–`else` expression, assignment and compound assignment, `:=`.  
  * **Operators:** `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `and`, `or`, `not`, `? :`, `=`, `:=`, `+=`, …

---

## 3. Dialect and limitations

Use when the user assumes TradingView Pine behavior or indentation-based syntax.

* **[`concepts/dialect-and-limitations.md`](concepts/dialect-and-limitations.md)**  
  * **Content:** Braced blocks only, script-kind constraints at a high level, pointer to parity table and parser status.  
  * **Keywords:** braces, QAS, Pine parity.

* **[`concepts/tv-vs-agentscript-validation.md`](concepts/tv-vs-agentscript-validation.md)**  
  * **Content:** Tables of TV-accurate behaviors vs AgentScript: what the **static checker** typically accepts vs rejects, separate from WASM emit and TV runtime parity.  
  * **Keywords:** TradingView, validation, builtins, `strategy.*`, `request.*`.

---

## 4. Not covered here (see other paths)

* **Builtin libraries (`ta.*`, `strategy.*`, `request.*`, …):** not exhaustively documented in this tree; behavior and typing follow the checker and [`ROADMAP.md`](../../ROADMAP.md). The TV-shaped reference corpus under [`spec/pinescriptv6/reference/functions/`](../../spec/pinescriptv6/reference/functions/) is a **checklist**, not a guarantee of QAS semantics.
* **Execution model (bar state, `var` persistence):** runtime and IR; see [`ROADMAP.md`](../../ROADMAP.md) and Aether docs, not duplicated as TV-identical prose here.
* **WASM guest ABI:** [`docs/agentscript-guest-abi.md`](../agentscript-guest-abi.md).

---

## Routing logic (examples)

* **IF** the user asks how to declare a user function or `export` in a library → **`reference/program-structure.md`**.
* **IF** the user asks which `//@version=` values are valid → **`reference/directives.md`**.
* **IF** the user asks about `switch` with no scrutinee or `for … in` → **`reference/keywords.md`** (and EBNF in `agentscripts-v1.md` for exact productions).
* **IF** the user asks why a script fails inside `strategy.*` in an `indicator()` → **`concepts/dialect-and-limitations.md`** and resolver notes in [`ROADMAP.md`](../../ROADMAP.md).
* **IF** the user asks what TradingView allows that AgentScript does not, or whether a script “validates” vs TV → **`concepts/tv-vs-agentscript-validation.md`**.
