# AgentScript compiler roadmap

## Primary goal

**Turn AgentScript / QAS (`.qas`) into validated, reproducible `wasm32` modules** that downstream runtimes—**Aether** (`aether-mwvm`, wasmtime / MWVM)—can load under a **shared strategy guest ABI**. The compiler is the language front end; execution policy and job orchestration live in Aether and **MWVM**.

## Semantics — development progress

**Legend:** **None** = not implemented. **AST only** = syntax is parsed into the AST; no typecheck, runtime, or codegen meaning. **Partial** = some real rules exist (validation, policy, or docs) but not a full semantic implementation.

| Semantic domain | Scope | Progress | Notes |
|-----------------|-------|----------|--------|
| **Lexical structure** | Whitespace, `//` / `/* */` comments, line structure | **Done** (parse) | Trivia handled in normal parse + leading scan. |
| **`//@version=` / `// @agentscript=`** | Which Pine / QAS header numbers are allowed | **Partial** | `//@version=` restricted to **5 and 6** (`version_policy.rs`, `leading_scan.rs`, `lex.rs`). `// @agentscript=` ≥ 1. No semantic *differences* between v5 vs v6 bodies yet. |
| **Script kind** | `indicator` / `strategy` / `library` declarations | **Partial** | Kind and args stored; [`resolve_script`](crates/agentscript-compiler/src/semantic/resolve.rs) rejects `strategy.*` outside `strategy()` scripts. |
| **Imports / exports** | `import … as`, `export` fn/var | **AST only** | No module graph, no symbol linking. |
| **Array literals** | `[a, b]`, `[]` | **AST only** | `Expr::Array`; element types not checked. |
| **Script-wide duplicate definitions** | Same function name twice, duplicate `import` alias, duplicate param in one `f` | **Partial** | [`analyze_script`](crates/agentscript-compiler/src/semantic/early.rs); no spans on AST yet. |
| **Scopes & name resolution** | Bindings, shadowing, qualified names | **Partial** | Dotted roots: [`resolve_script`](crates/agentscript-compiler/src/semantic/resolve.rs) + [`builtins`](crates/agentscript-compiler/src/semantic/builtins.rs); no lexical scope or user-type resolution yet. |
| **User functions** | `f name(params) => …` / block body, params, defaults | **AST only** | No recursion/arity/type checking. |
| **Variable qualifiers** | `var`, `varip`, `const`, `input`, `simple`, `series` | **AST only** | No Pine-style persistence or series/simple rules. |
| **Assignments** | `=` first assign vs `:=` reassignment | **AST only** | No “assign before declare” or single-assignment checks. |
| **Types (surface)** | `int` / `float` / `bool` / `string` / `color`, **`float[]`**-style arrays, `array<>` / `matrix<>` / `map<,>`, drawing types | **AST only** | Type syntax parsed; not checked or enforced. |
| **Type inference & checking** | `series` vs `simple`, call compatibility, generics | **None** | Planned Phase 1. |
| **Historical reference** | `expr[i]`, validity of offset, series history | **None** | Parsed as `Expr::Index`; no semantics. |
| **Operators** | Unary/binary, precedence, `==` vs `=` | **AST only** | No bool strictness or Na/na propagation rules. |
| **Ternary** | `cond ? a : b` | **AST only** | No lazy/short-circuit semantics in IR. |
| **Calls** | Positional / named args, `matrix.new<float>(…)` | **AST only** | No builtin resolution or signatures. |
| **Member / method syntax** | `a.b`, `close.sma(20)` | **AST only** | Desugared to `Member` / `Call`; no method tables. |
| **Control flow** | `if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i,v] in`**, `switch` (with or **without** scrutinee), `while`, `break`, `continue`, blocks | **Partial** | Parsed; `break`/`continue` inside `for`/`for…in`/`while` ([`loops`](crates/agentscript-compiler/src/semantic/loops.rs)); no reachability or Pine loop limits. |
| **Bar execution model** | Once per bar, `varip`, bar states | **None** | Requires IR + runtime + host. |
| **`ta.*` builtins** | Indicators, `crossover`, etc. | **None** | Registry + host/intrinsics (Phase 1–2). |
| **`strategy.*` builtins** | Orders, position, PnL, trade stats | **None** | Lowered to host imports; host implements semantics. |
| **`math.*` builtins** | Scalar math, rounding policy | **None** | |
| **`syminfo.*` / `timeframe.*`** | Symbol / timeframe metadata | **None** | Host-fed constants/imports. |
| **`request.security`** | MTF / foreign series, gaps, lookahead, dynamic symbol rules | **None** | Roadmap Phase 1–3; typing + ABI + host. |
| **`request.financial`** | Financial series by id/period | **None** | Roadmap Phase 1–3. |
| **Other `request.*`** | e.g. economic, dividend, … | **None** | Same pattern as security/financial when prioritized. |
| **`mcp.*` builtins** | `call`, `discover`, `emit`, quotas | **None** | QAS-specific; host MCP proxy. |
| **`plot.*` / drawing / `color.*`** | Visualization side effects | **None** | May be no-op in WASM or side-channel metadata. |
| **`input.*` factory fns** | `input.int`, `input.float`, … | **None** | `input` *qualifier* parsed; factories not special-cased. |
| **Side effects & order** | Order of `strategy.*` / `mcp.*` vs pure exprs | **None** | Needs effect typing + schedule in IR. |
| **Constant folding** | Compile-time evaluation of literals | **None** | Optional optimization after typecheck. |
| **IR & lowering** | Bar schedule, series nodes, calls → ops | **None** | Phase 2. |
| **WASM codegen** | `wasm32` module shape | **None** | Phase 2. |
| **Guest ABI** | Exports (`init`, `on_bar`, …), imports (data, strategy, request, mcp) | **None** | Spec + Aether/MWVM alignment; contract tests. |
| **Determinism** | FP rules, seeds, replay | **None** | Document + enforce in host for backtest. |
| **Runtime / host (Aether)** | Data feeds, fills, `request.*`, MCP | **None** | Outside this crate; semantics live here for execution. |
| **Diagnostics** | Errors beyond parse (types, builtins, ABI) | **Partial** | Parse: **miette**. Semantic: plain [`AnalyzeError`](crates/agentscript-compiler/src/semantic/mod.rs) until spans exist. |

## Current status

**Done today**

- [x] **Parse → AST** (Chumsky): headers (`//@version=` **5 or 6**, optional `// @agentscript=`), `import` / `export`, script declarations (`indicator` / `strategy` / `library`), **control flow** (`if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i, v] in`**, **`switch` with optional scrutinee** `{ … }`, `while`, **`break` / `continue`**), **blocks** `{ … }`, **user functions** Pine-style `name(…) =>` / `{ … }` or QAS `f name(…)`, **`method name(…) =>`**, **export** of Pine-style or `f` functions, **qualified and typed vars** (`var` / `varip` / `const` / `input` / `simple` / `series`, optional types, **`float[]`**-style array types), assignments `=` / `:=` / **`+=` …**, **`[a, b] =` tuple destructuring**, **Pine `if` expression** `if cond a else b` (incl. `else if` via nested `IfExpr`), **ternary** `? :`, **indexing** `expr[i]`, **array literals** `[a, b]`, **dotted calls** and **method-style** `base.field(…)`, generics on calls (e.g. `matrix.new<float>`), numeric literals with **scientific notation**, optional trailing **`;`**, expressions and comments. See `crates/agentscript-compiler/src/parser/script.rs`, `expr.rs`, and `assign_type.rs`.
- [x] **Lightweight semantic passes**: [`check_script`](crates/agentscript-compiler/src/semantic/mod.rs) runs [`analyze_script`](crates/agentscript-compiler/src/semantic/early.rs) (duplicates), [`check_break_continue`](crates/agentscript-compiler/src/semantic/loops.rs), and [`resolve_script`](crates/agentscript-compiler/src/semantic/resolve.rs) (dotted roots + `strategy.*` vs script kind). [`parse_and_analyze`](crates/agentscript-compiler/src/lib.rs) chains parse + `check_script`.
- [x] **Diagnostics**: miette-backed `CompileError` with source spans (parse); analyze messages are textual until the AST carries spans.
- [x] **CLI** (`agentscriptc`): read a file path or stdin (`-`), run parse + analyze, print debug `Script` on success.
- [x] **Tests**: parser / error cases in `crates/agentscript-compiler/tests/`.

**Not started**

- [ ] Typechecker (scopes, builtins, strategy vs library rules).
- [ ] **`request.security` / `request.financial`** end-to-end: Pine v6-aligned typing and lowering, guest ABI imports, and Aether/MWVM host that serves aligned foreign series (and financial fields where applicable).
- [ ] IR and lowering.
- [ ] Codegen to **`wasm32-unknown-unknown`** (or agreed target triple).
- [ ] **Guest ABI** alignment with Aether (`aether-common` / ABI doc): exports, calling convention, host imports for data and backtest hooks.

## Downstream alignment

| Consumer | What we owe them |
|----------|------------------|
| **Aether** | Stable ABI + `.wasm` bytes + deterministic build story so jobs can pin `wasm_sha256`. |
| **MWVM** | WASM that matches the same ABI and host expectations as other agent guests, where applicable. |

Spec and economics context: **`vaulted-knowledge-protocol/backtesting-infra`**.

## Phase 0 — Parser & AST (current)

- [x] Chumsky grammar for a **core subset** of QAS (expressions, calls, indexing, **array literals**, `indicator` / `strategy` / `library`, `=` / `:=`, `//@version` 5 or 6, comments, **`break` / `continue`**). See `spec/agentscripts-v1.md` for a **compiler-oriented EBNF** (plus product context). **Implementation vs that EBNF** is tracked in [`spec/qas-v1-parser-status.md`](spec/qas-v1-parser-status.md) (divergences include version tokens and many QAS/Pine constructs not duplicated in the short EBNF block).
- [x] AST types for what the parser accepts today; more variants will follow as syntax grows.
- [ ] **Close remaining gaps vs `spec/agentscripts-v1.md`:** align the spec’s lexical `VERSION_DECL` and program-structure EBNF with what we parse (`5`/`6`, `import`/`export`, `enum`/`type`, extended control flow, etc.); Pine-indent bodies vs QAS braces; finalize **`map.from`** (spec line is a stub) if required. **`enum` / `type`:** braced forms + `export` are implemented; unbraced TV-style bodies still out of scope.
- [ ] Expand tests: edge cases, larger fixtures, fuzz or corpus vs real `.qas` / Pine v6 samples, sharper errors for common mistakes.

### Pine v6 parity vs bundled docs (`pinescriptv6/`)

The folder **`pinescriptv6/`** mirrors TradingView’s Pine Script® v6 manual (keywords, types, operators, namespaces, visuals). Use it as the **checklist** below; the compiler today is **QAS-shaped** (`f` functions, braced blocks) and only **partially** overlaps TV v6 **syntax**.

| Area | TV v6 (`pinescriptv6/` paths) | In roadmap semantics table | Parser / AST gap (compile path) |
|------|------------------------------|----------------------------|--------------------------------|
| **Function declaration shape** | `name(params) =>` / block; `export name(...) =>`; optional QAS `f name(...)` ([`reference/keywords.md`](pinescriptv6/reference/keywords.md) `export`) | — | **Parser:** Pine form `name(...) =>` / `{` and `export` + same; **`f` still supported.** Semantics / UDT `this` still missing. |
| **`method` declarations** | `method foo(type id, ...) =>` ([`keywords.md`](pinescriptv6/reference/keywords.md) `method`) | — | **Parser:** `method` + name + params + body; [`FnDecl.is_method`](crates/agentscript-compiler/src/ast.rs). No typecheck for first-param dispatch yet. |
| **`type` (UDT)** | Composite types, `Type.new()`, field defaults ([`keywords.md`](pinescriptv6/reference/keywords.md) `type`, [`reference/types.md`](pinescriptv6/reference/types.md)) | Types (surface) partial | **Parser:** braced `type name { qual? ty field = expr; ... }`, `export type` ([`Item::TypeDef`](crates/agentscript-compiler/src/ast.rs)); no `Type.new` / method semantics yet. |
| **`enum`** | `enum name` / fields / `export enum` ([`keywords.md`](pinescriptv6/reference/keywords.md) `enum`) | — | **Parser:** braced `enum name { id = expr; ... }`, `export enum`; **`Type::Named`** for `map<symbols, float>` ([`ast.rs`](crates/agentscript-compiler/src/ast.rs)). |
| **`if` as expression** | `x = if cond a else b`, chained `else if` ([`keywords.md`](pinescriptv6/reference/keywords.md) `if`) | Ternary + **IfExpr** | **Parser:** [`Expr::IfExpr`](crates/agentscript-compiler/src/ast.rs); no type/lazy semantics yet. |
| **`switch` forms** | Expression switch; **no-scrutinee** `switch` + `cond =>` arms ([`keywords.md`](pinescriptv6/reference/keywords.md) `switch`) | Control flow (partial) | **Parser:** [`Stmt::Switch`](crates/agentscript-compiler/src/ast.rs) with `scrutinee: Option<Expr>`; braced body only (no indent-only TV style). |
| **`for … in` / `for [i, v] in`** | Arrays, matrices as iterables ([`keywords.md`](pinescriptv6/reference/keywords.md) `for...in`) | — | **Parser:** [`Stmt::ForIn`](crates/agentscript-compiler/src/ast.rs) + [`ForInPattern`](crates/agentscript-compiler/src/ast.rs). |
| **Compound assignments** | `+=`, `-=`, `*=`, `/=`, `%=` ([`reference/operators.md`](pinescriptv6/reference/operators.md)) | Assignments AST only | **Parser:** all five compound ops + `=` / `:=` ([`AssignOp`](crates/agentscript-compiler/src/ast.rs)). No lowering to `x = x + y` yet. |
| **Tuple / multi-assign** | `[a, b, c] = expr` ([`reference/types.md`](pinescriptv6/reference/types.md) `simple` example) | — | **Parser:** [`Stmt::TupleAssign`](crates/agentscript-compiler/src/ast.rs). |
| **Type syntax variants** | `float[]` style vs `array<float>` ([`keywords.md`](pinescriptv6/reference/keywords.md) `for...in` examples) | Types (surface) | **Parser:** `int[]` / `float[]` / … in [`assign_type.rs`](crates/agentscript-compiler/src/parser/assign_type.rs). |
| **`footprint` type** | `request.footprint()` ([`reference/types.md`](pinescriptv6/reference/types.md) `footprint`) | — | **Missing:** type keyword + later `request.*` wiring. |
| **Compiler annotations** | `//@description`, `//@function`, `//@param`, `//@field`, `//@enum`, `//@strategy_alert_message`, etc. ([`reference/annotations.md`](pinescriptv6/reference/annotations.md)) | — | **Parse:** treat as comments (ok today) or preserve for library docs / tooling. |
| **Indentation-based blocks** | TV allows indent bodies for `while`/`if` in some styles; we use **`{ … }`** only | — | **Dialect:** many TV examples use braces in v6 docs; confirm against `limitations.md` / style. |
| **`break` / `continue`** | Loop control ([`keywords.md`](pinescriptv6/reference/keywords.md) `while` remarks) | Control flow (partial) | **Parser:** `break` / `continue`; **semantic:** must appear inside `for` / `while` ([`loops.rs`](crates/agentscript-compiler/src/semantic/loops.rs)). |
| **Built-in namespaces** | `ta`, `strategy`, `request` (+ `seed`, `currency_rate`, `footprint`, …), `math`, `str`, `array`, `matrix`, `map`, drawing APIs ([`LLM_MANIFEST.md`](pinescriptv6/LLM_MANIFEST.md), `reference/functions/*`) | Per-namespace rows (None) | **Semantics + ABI**, not parser-only; signatures live in `reference/functions/*.md`. |
| **Visual / plot API** | `plot*`, `line`, `label`, `box`, `table`, fills, etc. ([`visuals/*.md`](pinescriptv6/visuals)) | plot.* / drawing row | Same: mostly **stdlib + host**, not syntax. |
| **Execution model** | `barstate`, `var`, `varip`, history ([`concepts/execution_model.md`](pinescriptv6/concepts/execution_model.md), [`pine_script_execution_model.md`](pinescriptv6/pine_script_execution_model.md)) | Bar execution model | **IR + runtime**, Phase 2+. |

**Summary:** ROADMAP Phase 0 already tracks **matrix/map literals** and spec EBNF audit; the table above adds **TV-specific syntax** documented under `pinescriptv6/` but not listed explicitly before (especially **`f`-less functions**, **`method`**, **`type`/`enum`**, **`for…in`**, **compound assigns**, **`if` expression**, **tuple assign**, and **`switch` without scrutinee**). Phase 1+ rows still cover builtins (`reference/functions/*`) and semantics.

### Phases 1–3 vs parsing

**Phases 1–3 in this roadmap are not “finish the parser.”** Phase 1 is semantics, Phase 2 is IR/codegen, Phase 3 is CLI and integration. Parser work that remains for **full** QAS syntax belongs under **Phase 0** (and can proceed in parallel with early Phase 1 on the subset).

## Phase 1 — Semantic analysis

- [x] **Early checks (no types yet):** duplicate top-level function names, duplicate `import` aliases, duplicate parameters per `f` — [`early.rs`](crates/agentscript-compiler/src/semantic/early.rs).
- [x] **Path glue (no full symbol table):** known builtin namespace roots + import aliases; unknown dotted roots rejected; `strategy.*` only in `strategy()` — [`resolve.rs`](crates/agentscript-compiler/src/semantic/resolve.rs).
- [x] **Loop control placement:** `break` / `continue` only inside `for` / `while` — [`loops.rs`](crates/agentscript-compiler/src/semantic/loops.rs).
- [ ] Symbol tables and lexical name resolution (locals, params, shadowing).
- [ ] Type system for core expressions (numbers, series, calls).
- [ ] Further script-kind rules (`library` exports-only, etc., as you align with Pine/QAS).
- [ ] **`request.security`:** Pine v6-aligned signatures and parameter typing (symbol, timeframe, expression, `gaps`, `lookahead`, `ignore_invalid_symbol`, related overloads); result type as **series** aligned with the expression’s type; **dynamic** first-argument rules (where TV allows `request.*` inside loops/conditionals—match or document QAS deltas); errors for invalid combinations.
- [ ] **`request.financial`:** Pine v6-aligned signatures and field typing (symbol, financial id, period, `ignore_invalid_symbol`, related forms); result typing consistent with TV’s financial series rules; same **dynamic** / scope constraints as other `request.*` where QAS aligns.
- [ ] Rich diagnostics (second pass after typecheck).

## Phase 2 — IR & codegen

- [ ] Internal IR suited for lowering and optimization passes.
- [ ] **`request.security` lowering:** lower to documented **host imports** (resolve symbol/timeframe, merge bars, return OHLC/series slice or per-bar samples per ABI); specify **determinism** (feed + merge policy ⇒ stable results); optional **static request graph** in metadata for host prefetch.
- [ ] **`request.financial` lowering:** lower to **host imports** that resolve symbol + financial id + period and return series aligned with the ABI; **determinism** and prefetch/metadata story consistent with `request.security`.
- [ ] WASM emission (likely `wasm-encoder` / `wasmparser` validation, or another chosen stack).
- [ ] **ABI contract** implemented in codegen (documented in-repo + mirrored types in Aether where useful).

## Phase 3 — Tooling & integration

- [ ] CLI flags: `--emit-ast`, `--emit-wasm`, `-o`, quiet / JSON diagnostics (as needed).
- [ ] **Documented loop**: `.qas` → `agentscript-compiler` → `.wasm` → `aether` run (when Aether’s WASM path is ready).
- [ ] **`request.security`:** integration / golden tests with multi-timeframe fixture data (compiler + host), including at least one dynamic-symbol case if QAS supports it.
- [ ] **`request.financial`:** integration / golden tests with fixture financial data (compiler + host), including invalid-symbol / missing-field cases as needed.
- [ ] Optional: `cargo` integration or `build.rs` helper for strategy crates.

## Success criteria by phase

| Phase | Done when |
|-------|-----------|
| **0** | `cargo test` green; real-world-ish `.qas` samples parse with clear errors on invalid input. |
| **1** | Ill-typed scripts fail fast with actionable diagnostics; well-typed scripts have a stable semantic model; **`request.security` and `request.financial` are typed** (signatures + series rules) or rejected explicitly. |
| **2** | Valid strategies compile to **loadable** WASM that satisfies the **written guest ABI** (verified against Aether/MWVM smoke tests); **`request.security` / `request.financial` map to imports** and a stub host can run a minimal MTF + financial example. |
| **3** | Builders can compile and run end-to-end without reading compiler internals. |

## Repository layout

| Piece | Location |
|-------|----------|
| Library API | `crates/agentscript-compiler` (`parse_script`, AST, errors) |
| CLI | `crates/agentscript-compiler/src/main.rs` |
| Pine v6 manual (reference corpus) | `pinescriptv6/` (`LLM_MANIFEST.md`, `reference/`, `concepts/`, `visuals/`) |
