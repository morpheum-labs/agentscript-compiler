# AgentScript compiler roadmap

## Primary goal

**Turn AgentScript / QAS (`.qas`) into validated, reproducible `wasm32` modules** that downstream runtimes—**Aether** (`aether-mwvm`, wasmtime / MWVM)—can load under a **shared strategy guest ABI**. The compiler is the language front end; execution policy and job orchestration live in Aether and **MWVM**.

## Semantics — development progress

**Legend:** **None** = not implemented. **AST only** = syntax is parsed into the AST; no typecheck, runtime, or codegen meaning. **Partial** = some real rules exist (validation, policy, or docs) but not a full semantic implementation.

| Semantic domain | Scope | Progress | Notes |
|-----------------|-------|----------|--------|
| **Lexical structure** | Whitespace, `//` / `/* */` comments, line structure | **Done** (parse) | Trivia handled in normal parse + leading scan. |
| **`//@version=` / `// @agentscript=`** | Which Pine / QAS header numbers are allowed | **Partial** | `//@version=` restricted to **5 and 6** ([`version_policy.rs`](crates/agentscript-compiler/src/frontend/parser/version_policy.rs), [`leading_scan.rs`](crates/agentscript-compiler/src/frontend/parser/leading_scan.rs), [`lex.rs`](crates/agentscript-compiler/src/frontend/parser/lex.rs)). `// @agentscript=` ≥ 1. No semantic *differences* between v5 vs v6 bodies yet. |
| **Script kind** | `indicator` / `strategy` / `library` declarations | **Partial** | Kind and args stored; [`resolve_script`](crates/agentscript-compiler/src/semantic/passes/resolver.rs) rejects `strategy.*` outside `strategy()` scripts. |
| **Imports / exports** | `import … as`, `export` fn/var | **AST only** | No module graph, no symbol linking. |
| **Array literals** | `[a, b]`, `[]` | **AST only** | [`ExprKind::Array`](crates/agentscript-compiler/src/frontend/ast/expr.rs); element types not checked. |
| **Script-wide duplicate definitions** | Same function name twice, duplicate `import` alias, duplicate param in one `f` | **Partial** | [`analyze_script`](crates/agentscript-compiler/src/semantic/passes/early.rs). Many AST nodes carry [`Span`](crates/agentscript-compiler/src/frontend/ast/node.rs); semantic errors are still mostly strings ([`AnalyzeError`](crates/agentscript-compiler/src/semantic/mod.rs)). |
| **Scopes & name resolution** | Bindings, shadowing, qualified names | **Partial** | Dotted roots: [`resolve_script`](crates/agentscript-compiler/src/semantic/passes/resolver.rs) + [`builtins`](crates/agentscript-compiler/src/semantic/builtins.rs). Minimal typecheck maintains scopes for a growing subset ([`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs)); full lexical + UDT resolution still open. |
| **User functions** | `f name(params) => …` / block body, params, defaults | **Partial** | Typecheck: arity + arg types vs params. HIR: [`HirExpr::UserCall`](crates/agentscript-compiler/src/hir/expr.rs) for expression-body `=>` only; block bodies rejected at lowering. |
| **Variable qualifiers** | `var`, `varip`, `const`, `input`, `simple`, `series` | **Partial** | Binding kinds from qualifiers in typecheck; `simple`/`const`/`input` cannot take a series initializer (clear diagnostic). No `var`/`varip` bar-state yet. |
| **Assignments** | `=` first assign vs `:=` reassignment | **AST only** | No “assign before declare” or single-assignment checks. |
| **Types (surface)** | `int` / `float` / `bool` / `string` / `color`, **`float[]`**-style arrays, `array<>` / `matrix<>` / `map<,>`, drawing types | **AST only** | Type syntax parsed; not checked or enforced. |
| **Type inference & checking** | `series` vs `simple`, call compatibility, generics | **Partial** | [`typecheck_script`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs) + default pipeline [`TypecheckPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs). Covers a **minimal** surface (builtins like `close` / `ta.sma` / `plot` / `request.security`, inputs, top-level flow); not a full Pine type system. |
| **Historical reference** | `expr[i]`, validity of offset, series history | **Partial** | Typecheck: integral index + element type. HIR: [`HirExpr::SeriesAccess`](crates/agentscript-compiler/src/hir/expr.rs) with literal offset; WASM: `close[k]` via host `series_hist`. |
| **Operators** | Unary/binary, precedence, `==` vs `=` | **Partial** | Numeric/bool promotion in [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs). Na/na propagation still open. |
| **Ternary** | `cond ? a : b` | **Partial** | Condition must be bool-like; branch types unified. Lazy IR semantics still open. |
| **Calls** | Positional / named args, `matrix.new<float>(…)` | **AST only** | No builtin resolution or signatures. |
| **Member / method syntax** | `a.b`, `close.sma(20)` | **AST only** | Desugared to `Member` / `Call`; no method tables. |
| **Control flow** | `if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i,v] in`**, `switch` (with or **without** scrutinee), `while`, `break`, `continue`, blocks | **Partial** | Parsed; `break`/`continue` ([`loops.rs`](crates/agentscript-compiler/src/semantic/passes/loops.rs)); `if` / `while` / `switch` condition typing in [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs); top-level `if` lowers to [`HirStmt::If`](crates/agentscript-compiler/src/hir/stmt.rs). Reachability / Pine loop limits still open. |
| **Bar execution model** | Once per bar, `varip`, bar states | **None** | Requires IR + runtime + host. |
| **`ta.*` builtins** | Indicators, `crossover`, etc. | **Partial** | `ta.sma` recognized in minimal typecheck and lowered to HIR ([`BuiltinKind::TaSma`](crates/agentscript-compiler/src/hir/builtin.rs)). Rest of `ta.*` still **None** until registry + host. |
| **`strategy.*` builtins** | Orders, position, PnL, trade stats | **None** | Lowered to host imports; host implements semantics. |
| **`math.*` builtins** | Scalar math, rounding policy | **None** | |
| **`syminfo.*` / `timeframe.*`** | Symbol / timeframe metadata | **None** | Host-fed constants/imports. |
| **`request.security`** | MTF / foreign series, gaps, lookahead, dynamic symbol rules | **Partial** | HIR [`SecurityCall`](crates/agentscript-compiler/src/hir/security.rs) with `gaps` / `lookahead` from `barmerge.*` or booleans; typecheck validates optional args. Dynamic symbol / full overloads / host ABI still Phase 1–3. |
| **`request.financial`** | Financial series by id/period | **None** | Roadmap Phase 1–3. |
| **Other `request.*`** | e.g. economic, dividend, … | **None** | Same pattern as security/financial when prioritized. |
| **`mcp.*` builtins** | `call`, `discover`, `emit`, quotas | **None** | QAS-specific; host MCP proxy. |
| **`plot.*` / drawing / `color.*`** | Visualization side effects | **Partial** | Top-level `plot(expr)` lowered to [`HirStmt::Plot`](crates/agentscript-compiler/src/hir/stmt.rs) in the same tiny slice as `ast_lower`. Drawing / `plotshape` / `color.*` still **None** for semantics. |
| **`input.*` factory fns** | `input.int`, `input.float`, … | **Partial** | `input.int` literal default in assign / `input int` decl handled in HIR lowering + typecheck subset ([`BuiltinKind::InputInt`](crates/agentscript-compiler/src/hir/builtin.rs)). Other `input.*` factories not modeled. |
| **Side effects & order** | Order of `strategy.*` / `mcp.*` vs pure exprs | **None** | Needs effect typing + schedule in IR. |
| **Constant folding** | Compile-time evaluation of literals | **None** | Optional optimization after typecheck. |
| **IR & lowering** | Bar schedule, series nodes, calls → ops | **Partial** | **HIR** + spec [`spec/hir.md`](spec/hir.md). **AST → `HirScript`:** [`lower_script_to_hir`](crates/agentscript-compiler/src/hir/ast_lower.rs) / [`HirLowerPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs): indicator slice + **`HirExpr::UserCall`** + **`HirStmt::If`** + bool literals + `input.int` + `close` + `ta.sma` / `ta.ema` + `request.security` + `plot` + `close[k]`. No bar scheduler yet. |
| **WASM codegen** | `wasm32` module shape | **Partial** | [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) (`wasm-encoder`): `aether` imports + dual exports (`init`/`on_bar` + `aether_strategy_*`). Coverage tracks the HIR subset; not full language. |
| **Guest ABI** | Exports (`init`, `on_bar`, …), imports (data, strategy, request, mcp) | **Partial** | v0 preview: `() -> ()` exports + documented `aether` import table; [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md). Host still stubs imports / does not call `step` in production paths. |
| **Determinism** | FP rules, seeds, replay | **None** | Document + enforce in host for backtest. |
| **Runtime / host (Aether)** | Data feeds, fills, `request.*`, MCP | **None** | Outside this crate; semantics live here for execution. |
| **Diagnostics** | Errors beyond parse (types, builtins, ABI) | **Partial** | Parse: **miette** with spans. Semantic: [`AnalyzeError`](crates/agentscript-compiler/src/semantic/mod.rs) + [`SemanticDiagnostic`](crates/agentscript-compiler/src/semantic/mod.rs) carry `Span`; CLI maps analysis failures through [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) for miette output. |

## Current status

**Done today**

- [x] **Parse → AST** (Chumsky): headers (`//@version=` **5 or 6**, optional `// @agentscript=`), `import` / `export`, script declarations (`indicator` / `strategy` / `library`), **control flow** (`if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i, v] in`**, **`switch` with optional scrutinee** `{ … }`, `while`, **`break` / `continue`**), **blocks** `{ … }`, **user functions** Pine-style `name(…) =>` / `{ … }` or QAS `f name(…)`, **`method name(…) =>`**, **export** of Pine-style or `f` functions, **qualified and typed vars** (`var` / `varip` / `const` / `input` / `simple` / `series`, optional types, **`float[]`**-style array types), assignments `=` / `:=` / **`+=` …**, **`[a, b] =` tuple destructuring**, **Pine `if` expression** `if cond a else b` (incl. `else if` via nested `IfExpr`), **ternary** `? :`, **indexing** `expr[i]`, **array literals** `[a, b]`, **dotted calls** and **method-style** `base.field(…)`, generics on calls (e.g. `matrix.new<float>`), numeric literals with **scientific notation**, optional trailing **`;`**, expressions and comments. See `crates/agentscript-compiler/src/frontend/parser/script.rs`, `expr.rs`, and `assign_type.rs`.
- [x] **Semantic passes** ([`default_passes`](crates/agentscript-compiler/src/semantic/passes/mod.rs)): early analyze (duplicates), `break`/`continue` placement, resolver (dotted roots + `strategy.*` vs script kind), **minimal typecheck** ([`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs)). [`check_script`](crates/agentscript-compiler/src/semantic/mod.rs) / [`parse_and_analyze`](crates/agentscript-compiler/src/lib.rs) run this pipeline.
- [x] **HIR crate** ([`hir/mod.rs`](crates/agentscript-compiler/src/hir/mod.rs)): `HirScript`, `HirExpr` arena (`exprs` + `HirId`), `SymbolTable`, `SecurityCall`, etc.; design in [`spec/hir.md`](spec/hir.md).
- [x] **AST → HIR (first slice):** [`lower_script_to_hir`](crates/agentscript-compiler/src/hir/ast_lower.rs), [`AstHirLowerer`](crates/agentscript-compiler/src/hir/ast_lower.rs) + [`LowerToHir`](crates/agentscript-compiler/src/hir/lowering.rs); golden snapshot `crates/agentscript-compiler/src/hir/snapshots/`.
- [x] **Session hook**: [`CompilerSession`](crates/agentscript-compiler/src/session.rs) with `bumpalo::Bump` (ready for arena-backed IR later).
- [x] **Diagnostics**: miette-backed `CompileError` with spans (parse); semantic failures use [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) in the CLI when spans are available.
- [x] **CLI** (`agentscriptc`): read a file path or stdin (`-`), parse + analyze; `--emit=ast` | `hir` | `wasm` (see [`main.rs`](crates/agentscript-compiler/src/main.rs)).
- [x] **WASM (HIR subset):** [`compile_script_to_wasm_v0`](crates/agentscript-compiler/src/lib.rs), [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs); `cargo test` validates module + import/export names.
- [x] **Tests**: parser / error cases in `crates/agentscript-compiler/tests/`; HIR golden in-crate.

**Outstanding work (near term)**

- [x] **Widen HIR lowering** (incremental): `if` / `else if`, user `=>` calls, bool literals, `request.security` gaps/lookahead + typed inner result; still grow builtins / block user bodies / WASM in step.
- [x] **Wire HIR into the driver**: [`HirLowerPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs) + [`analyze_to_hir_compiler`](crates/agentscript-compiler/src/lib.rs) / [`session_hir`](crates/agentscript-compiler/src/lib.rs); [`lower_script_to_hir`](crates/agentscript-compiler/src/hir/ast_lower.rs) remains for tests and direct tooling.
- [x] **CLI semantic diagnostics** — [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) + miette for analysis failures with spans.
- [ ] **Full span coverage** — ensure every semantic error path attaches a non-`DUMMY` [`Span`](crates/agentscript-compiler/src/frontend/ast/node.rs) where the AST has one.
- [ ] **Full** type system + symbol tables (Pine/QAS parity, generics, library linking).
- [ ] **`request.security` / `request.financial`** end-to-end: v6-aligned typing, WASM/host imports, Aether/MWVM fixtures.
- [ ] **Codegen** to **`wasm32-unknown-unknown`** (or agreed triple) + **guest ABI** (`aether-common` / ABI doc).

**Not started / still open**

- [ ] **Full-language** WASM + HIR (strategy bodies, full `request.*`, user functions in IR, etc.).
- [ ] **Guest ABI** finalized (`init` → `i32`, `step` memory layout) and **invoked** by Aether with contract tests across repos.

## Downstream alignment

| Consumer | What we owe them |
|----------|------------------|
| **Aether** | Stable ABI + `.wasm` bytes + deterministic build story so jobs can pin `wasm_sha256`. |
| **MWVM** | WASM that matches the same ABI and host expectations as other agent guests, where applicable. |

**Integration gap (compiler ↔ Aether):** tracked in [`docs/aether-integration-gap.md`](docs/aether-integration-gap.md) (checklist + references).

Spec and economics context: **`vaulted-knowledge-protocol/backtesting-infra`**.

## Phase 0 — Parser & AST

**Dialect in scope:** **braced** blocks (`{ … }`), QAS `f` / Pine-style `name(…) =>`, and the constructs listed below. **Not** required for Phase 0 “done”: TradingView **indent-only** bodies, unbraced `enum`/`type`, or finalized **`map.from`** (see [`spec/qas-v1-parser-status.md`](spec/qas-v1-parser-status.md)).

### Exit criteria (aligned with success table below)

- [x] Parser + AST cover the **§§1–13 EBNF** in [`spec/agentscripts-v1.md`](spec/agentscripts-v1.md) for the **braced** grammar, with known gaps documented in [`spec/qas-v1-parser-status.md`](spec/qas-v1-parser-status.md).
- [x] **`cargo test -p agentscript-compiler`** green; regression coverage in [`crates/agentscript-compiler/tests/parse_smoke.rs`](crates/agentscript-compiler/tests/parse_smoke.rs) (and related tests).
- [x] Parse failures use **miette**-style diagnostics where the pipeline reports [`CompileError`](crates/agentscript-compiler/src/error.rs).
- [ ] **Stretch (still Phase 0–friendly):** corpus vs real `.pine` / `.qas`, fuzz, or extra edge-case tests; optional grammar export (Tree-sitter / ANTLR).

### Checklist

- [x] Chumsky grammar for the **core QAS surface**: expressions, calls, indexing, **array literals**, `indicator` / `strategy` / `library`, `=` / `:=`, `//@version` **5 or 6**, comments, **`break` / `continue`**, imports/exports, `enum` / `type`, user functions (Pine + QAS shapes), extended **`for` / `while` / `switch`**, compound and tuple assign, **`array.from`** / **`matrix.new` / `map.new`** call forms. Implementation: [`crates/agentscript-compiler/src/frontend/parser/`](crates/agentscript-compiler/src/frontend/parser/).
- [x] AST types for what the parser accepts: [`crates/agentscript-compiler/src/frontend/ast/`](crates/agentscript-compiler/src/frontend/ast/).
- [x] **Spec EBNF alignment (§§1–13)** with intentional exclusions: unbraced TV **`enum` / `type`** bodies out of scope; **`map.from`** TBD in §11 until reference + tests lock it.
- [ ] **Remaining grammar/spec work (optional tracks):** Pine-indent bodies vs braces; finalize **`map.from`** in §11; grammar export for external tooling.
- [ ] **Test and UX polish:** larger fixtures, corpus samples, fuzz, sharper errors for common mistakes.

### Pine v6 parity vs bundled docs (`pinescriptv6/`)

The folder **`pinescriptv6/`** mirrors TradingView’s Pine Script® v6 manual (keywords, types, operators, namespaces, visuals). Use it as the **checklist** below; the compiler today is **QAS-shaped** (`f` functions, braced blocks) and only **partially** overlaps TV v6 **syntax**.

| Area | TV v6 (`pinescriptv6/` paths) | In roadmap semantics table | Parser / AST gap (compile path) |
|------|------------------------------|----------------------------|--------------------------------|
| **Function declaration shape** | `name(params) =>` / block; `export name(...) =>`; optional QAS `f name(...)` ([`reference/keywords.md`](pinescriptv6/reference/keywords.md) `export`) | — | **Parser:** Pine form `name(...) =>` / `{` and `export` + same; **`f` still supported.** Semantics / UDT `this` still missing. |
| **`method` declarations** | `method foo(type id, ...) =>` ([`keywords.md`](pinescriptv6/reference/keywords.md) `method`) | — | **Parser:** `method` + name + params + body; [`FnDecl.is_method`](crates/agentscript-compiler/src/frontend/ast/decl.rs). No typecheck for first-param dispatch yet. |
| **`type` (UDT)** | Composite types, `Type.new()`, field defaults ([`keywords.md`](pinescriptv6/reference/keywords.md) `type`, [`reference/types.md`](pinescriptv6/reference/types.md)) | Types (surface) partial | **Parser:** braced `type name { qual? ty field = expr; ... }`, `export type` ([`Item::TypeDef`](crates/agentscript-compiler/src/frontend/ast/decl.rs)); no `Type.new` / method semantics yet. |
| **`enum`** | `enum name` / fields / `export enum` ([`keywords.md`](pinescriptv6/reference/keywords.md) `enum`) | — | **Parser:** braced `enum name { id = expr; ... }`, `export enum`; **`Type::Named`** for `map<symbols, float>` ([`types.rs`](crates/agentscript-compiler/src/frontend/ast/types.rs)). |
| **`if` as expression** | `x = if cond a else b`, chained `else if` ([`keywords.md`](pinescriptv6/reference/keywords.md) `if`) | Ternary + **IfExpr** | **Parser:** [`ExprKind::IfExpr`](crates/agentscript-compiler/src/frontend/ast/expr.rs); no type/lazy semantics yet. |
| **`switch` forms** | Expression switch; **no-scrutinee** `switch` + `cond =>` arms ([`keywords.md`](pinescriptv6/reference/keywords.md) `switch`) | Control flow (partial) | **Parser:** [`StmtKind::Switch`](crates/agentscript-compiler/src/frontend/ast/stmt.rs) with `scrutinee: Option<Expr>`; braced body only (no indent-only TV style). |
| **`for … in` / `for [i, v] in`** | Arrays, matrices as iterables ([`keywords.md`](pinescriptv6/reference/keywords.md) `for...in`) | — | **Parser:** [`StmtKind::ForIn`](crates/agentscript-compiler/src/frontend/ast/stmt.rs) + [`ForInPattern`](crates/agentscript-compiler/src/frontend/ast/stmt.rs). |
| **Compound assignments** | `+=`, `-=`, `*=`, `/=`, `%=` ([`reference/operators.md`](pinescriptv6/reference/operators.md)) | Assignments AST only | **Parser:** all five compound ops + `=` / `:=` ([`AssignOp`](crates/agentscript-compiler/src/frontend/ast/stmt.rs)). No lowering to `x = x + y` yet. |
| **Tuple / multi-assign** | `[a, b, c] = expr` ([`reference/types.md`](pinescriptv6/reference/types.md) `simple` example) | — | **Parser:** [`StmtKind::TupleAssign`](crates/agentscript-compiler/src/frontend/ast/stmt.rs). |
| **Type syntax variants** | `float[]` style vs `array<float>` ([`keywords.md`](pinescriptv6/reference/keywords.md) `for...in` examples) | Types (surface) | **Parser:** `int[]` / `float[]` / … in [`assign_type.rs`](crates/agentscript-compiler/src/frontend/parser/assign_type.rs). |
| **`footprint` type** | `request.footprint()` ([`reference/types.md`](pinescriptv6/reference/types.md) `footprint`) | — | **Missing:** type keyword + later `request.*` wiring. |
| **Compiler annotations** | `//@description`, `//@function`, `//@param`, `//@field`, `//@enum`, `//@strategy_alert_message`, etc. ([`reference/annotations.md`](pinescriptv6/reference/annotations.md)) | — | **Parse:** treat as comments (ok today) or preserve for library docs / tooling. |
| **Indentation-based blocks** | TV allows indent bodies for `while`/`if` in some styles; we use **`{ … }`** only | — | **Dialect:** many TV examples use braces in v6 docs; confirm against `limitations.md` / style. |
| **`break` / `continue`** | Loop control ([`keywords.md`](pinescriptv6/reference/keywords.md) `while` remarks) | Control flow (partial) | **Parser:** `break` / `continue`; **semantic:** must appear inside `for` / `while` ([`loops.rs`](crates/agentscript-compiler/src/semantic/passes/loops.rs)). |
| **Built-in namespaces** | `ta`, `strategy`, `request` (+ `seed`, `currency_rate`, `footprint`, …), `math`, `str`, `array`, `matrix`, `map`, drawing APIs ([`LLM_MANIFEST.md`](pinescriptv6/LLM_MANIFEST.md), `reference/functions/*`) | Per-namespace rows (None) | **Semantics + ABI**, not parser-only; signatures live in `reference/functions/*.md`. |
| **Visual / plot API** | `plot*`, `line`, `label`, `box`, `table`, fills, etc. ([`visuals/*.md`](pinescriptv6/visuals)) | plot.* / drawing row | Same: mostly **stdlib + host**, not syntax. |
| **Execution model** | `barstate`, `var`, `varip`, history ([`concepts/execution_model.md`](pinescriptv6/concepts/execution_model.md), [`pine_script_execution_model.md`](pinescriptv6/pine_script_execution_model.md)) | Bar execution model | **IR + runtime**, Phase 2+. |

**Summary:** ROADMAP Phase 0 already tracks **matrix/map literals** and spec EBNF audit; the table above adds **TV-specific syntax** documented under `pinescriptv6/` but not listed explicitly before (especially **`f`-less functions**, **`method`**, **`type`/`enum`**, **`for…in`**, **compound assigns**, **`if` expression**, **tuple assign**, and **`switch` without scrutinee**). Phase 1+ rows still cover builtins (`reference/functions/*`) and semantics.

### Phases 1–3 vs parsing

**Phases 1–3 in this roadmap are not “finish the parser.”** Phase 1 is semantics, Phase 2 is IR/codegen, Phase 3 is CLI and integration. Parser work that remains for **full** QAS syntax belongs under **Phase 0** (and can proceed in parallel with early Phase 1 on the subset).

## Phase 1 — Semantic analysis

- [x] **Early checks (no types yet):** duplicate top-level function names, duplicate `import` aliases, duplicate parameters per `f` — [`early.rs`](crates/agentscript-compiler/src/semantic/passes/early.rs).
- [x] **Path glue (no full symbol table):** known builtin namespace roots + import aliases; unknown dotted roots rejected; `strategy.*` only in `strategy()` — [`resolver.rs`](crates/agentscript-compiler/src/semantic/passes/resolver.rs).
- [x] **Loop control placement:** `break` / `continue` only inside `for` / `while` — [`loops.rs`](crates/agentscript-compiler/src/semantic/passes/loops.rs).
- [x] **Minimal typecheck (subset):** [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs) + [`TypecheckPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs) (not full Pine typing).
- [x] **First HIR lowering slice:** [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs) + golden test (indicator + inputs + `ta.sma` + `request.security` + `plot`).
- [ ] Symbol tables and lexical name resolution (locals, params, shadowing) **to completion**.
- [ ] Type system for **all** core expressions (numbers, series, calls) and script kinds. *(Progress: bool conditions for `if` / ternary / `while`; `switch` scrutinee vs arm types; `request.security` optional merge args; simple/const/input vs series initializers.)*
- [ ] Further script-kind rules (`library` exports-only, etc., as you align with Pine/QAS).
- [ ] **`request.security`:** Pine v6-aligned signatures and parameter typing (symbol, timeframe, expression, `gaps`, `lookahead`, `ignore_invalid_symbol`, related overloads); result type as **series** aligned with the expression’s type; **dynamic** first-argument rules (where TV allows `request.*` inside loops/conditionals—match or document QAS deltas); errors for invalid combinations.
- [ ] **`request.financial`:** Pine v6-aligned signatures and field typing (symbol, financial id, period, `ignore_invalid_symbol`, related forms); result typing consistent with TV’s financial series rules; same **dynamic** / scope constraints as other `request.*` where QAS aligns.
- [ ] Rich diagnostics (second pass after typecheck).

## Phase 2 — IR & codegen

- [x] **HIR as internal IR (v0):** typed/normalized shapes under [`src/hir/`](crates/agentscript-compiler/src/hir/); AST lowering for a **small** language slice (see [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs)).
- [ ] **HIR completeness:** cover the rest of the typed surface, optimizations, bar/series schedule if needed.
- [ ] **`request.security` lowering to host:** beyond the HIR node — documented **host imports** (resolve symbol/timeframe, merge bars, return OHLC/series slice or per-bar samples per ABI); **determinism** (feed + merge policy ⇒ stable results); optional **static request graph** in metadata for host prefetch.
- [ ] **`request.financial` lowering:** lower to **host imports** that resolve symbol + financial id + period and return series aligned with the ABI; **determinism** and prefetch/metadata story consistent with `request.security`.
- [x] **WASM emission (HIR subset v0):** [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) + [`compile_script_to_wasm_v0`](crates/agentscript-compiler/src/lib.rs) using `wasm-encoder` / `wasmparser` in tests. Emits `aether` imports, `memory`, and dual exports (`init`/`on_bar` + `aether_strategy_*`). **Still not** full-language codegen: [`HirWasmError`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) rejects `UserCall`, body `if`, non-`close` series history, nested `plot`, etc.
- [x] **Guest module ABI v0 (partial):** import indices and export names are defined in [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) and described in [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md). **Still open for Phase 2 “done”:** full `request.*` / user-fn lowering, richer `init`/`step` signatures, and Aether/MWVM contract tests beyond the current smoke checks.

## Phase 3 — Tooling & integration

- [ ] CLI flags: `--emit-ast`, `--emit-wasm`, `-o`, quiet / JSON diagnostics (as needed).
- [ ] **Documented loop**: `.qas` → `agentscript-compiler` → `.wasm` → `aether` run (when Aether’s WASM path is ready).
- [ ] **`request.security`:** integration / golden tests with multi-timeframe fixture data (compiler + host), including at least one dynamic-symbol case if QAS supports it.
- [ ] **`request.financial`:** integration / golden tests with fixture financial data (compiler + host), including invalid-symbol / missing-field cases as needed.
- [ ] Optional: `cargo` integration or `build.rs` helper for strategy crates.

## Success criteria by phase

| Phase | Done when |
|-------|-----------|
| **0** | `cargo test` green; braced QAS / Pine-shaped sources in scope parse with clear errors on invalid input; intentional gaps documented in [`spec/qas-v1-parser-status.md`](spec/qas-v1-parser-status.md). *(Corpus / fuzz is a stretch, not a gate.)* |
| **1** | Ill-typed scripts fail fast with actionable diagnostics; well-typed scripts have a stable semantic model; **`request.security` and `request.financial` are typed** (signatures + series rules) or rejected explicitly. *(Progress: minimal typecheck + first HIR slice; full Phase 1 criteria not met yet.)* |
| **2** | Valid strategies compile to **loadable** WASM that satisfies the **written guest ABI** (verified against Aether/MWVM smoke tests); **`request.security` / `request.financial` map to imports** and a stub host can run a minimal MTF + financial example. |
| **3** | Builders can compile and run end-to-end without reading compiler internals. |

## Repository layout

| Piece | Location |
|-------|----------|
| Library API | `crates/agentscript-compiler` (`parse_script`, AST, `check_script`, `lower_script_to_hir`, `HirScript`, errors) |
| CLI | `crates/agentscript-compiler/src/main.rs` |
| Pine v6 manual (reference corpus) | `pinescriptv6/` (`LLM_MANIFEST.md`, `reference/`, `concepts/`, `visuals/`) |
