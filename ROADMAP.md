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
| **Array literals** | `[a, b]`, `[]` | **Partial** | [`ExprKind::Array`](crates/agentscript-compiler/src/frontend/ast/expr.rs); homogeneous typing + promotion in [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs). HIR: [`HirExpr::Array`](crates/agentscript-compiler/src/hir/expr.rs) in [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs). **WASM v0** still rejects array values (no guest array ABI yet). |
| **Script-wide duplicate definitions** | Same function name twice, duplicate `import` alias, duplicate param in one `f` | **Partial** | [`analyze_script`](crates/agentscript-compiler/src/semantic/passes/early.rs). Many AST nodes carry [`Span`](crates/agentscript-compiler/src/frontend/ast/node.rs); semantic errors are still mostly strings ([`AnalyzeError`](crates/agentscript-compiler/src/semantic/mod.rs)). |
| **Scopes & name resolution** | Bindings, shadowing, qualified names | **Partial** | Dotted roots: [`resolve_script`](crates/agentscript-compiler/src/semantic/passes/resolver.rs) + [`builtins`](crates/agentscript-compiler/src/semantic/builtins.rs). Minimal typecheck maintains scopes for a growing subset ([`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs)); full lexical + UDT resolution still open. |
| **User functions** | `f name(params) => …` / block body, params, defaults | **Partial** | Typecheck: arity + arg types vs params. HIR: [`HirExpr::UserCall`](crates/agentscript-compiler/src/hir/expr.rs) + block/`=>` bodies via [`lower_user_function`](crates/agentscript-compiler/src/hir/ast_lower.rs); function symbol type uses [`AstType::Named`](crates/agentscript-compiler/src/frontend/ast/types.rs)(fn name). |
| **Variable qualifiers** | `var`, `varip`, `const`, `input`, `simple`, `series` | **Partial** | Binding kinds from qualifiers in typecheck; `simple`/`const`/`input` cannot take a series initializer (clear diagnostic). HIR/WASM slice: `var` / `varip` map to [`HirScript::persist_symbols`](crates/agentscript-compiler/src/hir/script.rs) and **wasm globals** in [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs); full Pine bar/`varip` semantics vs host still open. |
| **Assignments** | `=` first assign vs `:=` reassignment | **Partial** | Pine-style first `=` introduces binding (lexical + typecheck + HIR). Compound `+=` … `/=` lowered to `Let` + binary in [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs). `:=` reassignment lowers to `HirStmt::Let` when the target is already bound (subset); tuple assign: typecheck partial; full tuple `:=` HIR still limited. |
| **Types (surface)** | `int` / `float` / `bool` / `string` / `color`, **`float[]`**-style arrays, `array<>` / `matrix<>` / `map<,>`, drawing types | **AST only** | Type syntax parsed; not checked or enforced. |
| **Type inference & checking** | `series` vs `simple`, call compatibility, generics | **Partial** | [`typecheck_script`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs) + default pipeline [`TypecheckPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs). Adds enum / UDT **field and variant** typing for `E.a` / `T.field` (incl. dotted [`IdentPath`](crates/agentscript-compiler/src/frontend/ast/expr.rs)); JSON-driven [`builtin_registry`](crates/agentscript-compiler/src/semantic/builtin_registry.rs). Generics and full Pine parity still open. |
| **Historical reference** | `expr[i]`, validity of offset, series history | **Partial** | Typecheck: integral index + element type. HIR: [`HirExpr::SeriesAccess`](crates/agentscript-compiler/src/hir/expr.rs) with literal offset; WASM: `close[k]` via host `series_hist`. |
| **Operators** | Unary/binary, precedence, `==` vs `=` | **Partial** | Numeric/bool promotion in [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs). Na/na propagation still open. |
| **Ternary** | `cond ? a : b` | **Partial** | Condition must be bool-like; branch types unified. Lowers to [`HirExpr::Select`](crates/agentscript-compiler/src/hir/expr.rs); guest WASM uses `select` (f64) in [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs). Lazy TV semantics still open. |
| **Calls** | Positional / named args, `matrix.new<float>(…)` | **Partial** | JSON [`builtin_registry`](crates/agentscript-compiler/src/semantic/builtin_registry.rs) + minimal resolver/typecheck for a growing set; not full Pine overload / generic instantiation parity. |
| **Member / method syntax** | `a.b`, `close.sma(20)` | **Partial** | Enum variants and UDT static fields typed (`Side.buy`, `Bar.o`); [`Member`](crates/agentscript-compiler/src/frontend/ast/expr.rs) + two-segment [`IdentPath`](crates/agentscript-compiler/src/frontend/ast/expr.rs). **HIR:** `close.sma` / `close.ema` desugar to `ta.*` in [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs); UDT instance methods / arbitrary `base.method` still limited. |
| **Control flow** | `if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i,v] in`**, `switch` (with or **without** scrutinee), `while`, `break`, `continue`, blocks | **Partial** | Parsed; `break`/`continue` ([`loops.rs`](crates/agentscript-compiler/src/semantic/passes/loops.rs)); `if` / `while` / `switch` condition typing in [`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs); top-level `if` lowers to [`HirStmt::If`](crates/agentscript-compiler/src/hir/stmt.rs). Reachability / Pine loop limits still open. |
| **Bar execution model** | Once per bar, `varip`, bar states | **Partial** | Compiler slice: persist locals via wasm globals + host `on_bar` loop (see [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs)); **`barstate.*`**, tick replay, and full TV execution rules still require host + semantics work. |
| **`ta.*` builtins** | Indicators, `crossover`, etc. | **Partial** | `ta.sma`, `ta.ema`, `ta.crossover`, `ta.crossunder` in typecheck + HIR + WASM imports ([`BuiltinKind`](crates/agentscript-compiler/src/hir/builtin.rs), [`builtin_wasm_emit.rs`](crates/agentscript-compiler/src/codegen/builtin_wasm_emit.rs)). Rest of `ta.*` still **None** until registry + host. |
| **`strategy.*` builtins** | Orders, position, PnL, trade stats | **None** | Lowered to host imports; host implements semantics. |
| **`math.*` builtins** | Scalar math, rounding policy | **Partial** | `max`/`min`/`abs`/`sqrt`/`round`/`ceil`/`floor`/`trunc` (native `f64.*`) + `log`/`exp`/`pow` (HIR + WASM; `log`/`exp`/`pow` via `aether` imports). Remaining `math.*` still open. |
| **`syminfo.*` / `timeframe.*`** | Symbol / timeframe metadata | **Partial** | Typecheck: limited two-segment `syminfo.*` paths (e.g. `ticker`) as series string ([`builtin_global`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs)); no HIR/WASM surface beyond generic typing. `timeframe.*` and full syminfo parity still **None**. |
| **`request.security`** | MTF / foreign series, gaps, lookahead, dynamic symbol rules | **Partial** | HIR [`SecurityCall`](crates/agentscript-compiler/src/hir/security.rs) with `gaps` / `lookahead` from `barmerge.*` or booleans; typecheck validates optional args. WASM v0: `aether.request_security` (string literals + inner expr); `aether-mwvm` stub passes through inner `f64`. Dynamic symbol / full MTF merge / real host feed still open. |
| **`request.financial`** | Financial series by id/period | **Partial** | Typecheck + HIR [`FinancialCall`](crates/agentscript-compiler/src/hir/financial.rs) (`gaps`, `currency`, `ignore`); WASM v0: `aether.request_financial` **`(i32×10)->f64`** (symbol/id/period/currency string literals; `gaps` / `ignore` flags; default currency `-1`,`0`). Dynamic args / full host still open. |
| **Other `request.*`** | e.g. economic, dividend, … | **None** | Same pattern as security/financial when prioritized. |
| **`mcp.*` builtins** | `call`, `discover`, `emit`, quotas | **None** | QAS-specific; host MCP proxy. |
| **`plot.*` / drawing / `color.*`** | Visualization side effects | **Partial** | Top-level `plot(expr)` lowered to [`HirStmt::Plot`](crates/agentscript-compiler/src/hir/stmt.rs). `color.new` / `color.rgb` parse as ordinary member calls (no separate `color.*` atom). Statement-level `plotshape` / `fill` / `alertcondition` are accepted by typecheck but **skipped** in HIR (no drawing IR or WASM yet). |
| **`input.*` factory fns** | `input.int`, `input.float`, … | **Partial** | `input.int` / `input.float` (positional or Pine-style `defval=` kwargs) in assign / decls → [`HirInputDecl`](crates/agentscript-compiler/src/hir/script.rs) + WASM imports. Bare Pine **`input(title=…, defval=…)`** and first-arg series (e.g. `hl2`) typecheck; bool `defval` stored as int input for WASM. Other `input.*` factories not modeled. |
| **Side effects & order** | Order of `strategy.*` / `mcp.*` vs pure exprs | **None** | Needs effect typing + schedule in IR. |
| **Constant folding** | Compile-time evaluation of literals | **None** | Optional optimization after typecheck. |
| **IR & lowering** | Bar schedule, series nodes, calls → ops | **Partial** | **HIR** + spec [`spec/hir.md`](spec/hir.md). **AST → `HirScript`:** [`lower_script_to_hir_in_bump_with_session`](crates/agentscript-compiler/src/hir/ast_lower.rs) / [`HirLowerPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs) uses [`CompilerSession`](crates/agentscript-compiler/src/session.rs) (`def_semantic_ids`, `name_bindings`, `expr_types`) + lexical scoping for blocks/`if`. Surface: **`HirExpr::UserCall`** (non-`method` user fns), **`HirExpr::Select`**, **`HirExpr::Array`**, unary `-` (desugared), full **numeric/compare** binaries (typed from session), compound assign, **`HirStmt::If`**, `input.int` / `input.float` / plain `input` defvals, `close` (pre-interned; other OHLC series names not in WASM path yet), `ta.sma` / `ta.ema` / `ta.crossover` / `ta.crossunder`, `math.*` (`max`…`pow`, `ceil`/`floor`/`trunc`), `request.security`, **`request.financial`**, `plot`, `close[k]`, **`var` / `varip`** persist symbols. No explicit bar scheduler IR yet. |
| **WASM codegen** | `wasm32` module shape | **Partial** | [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) (`wasm-encoder`): `aether` imports + dual exports (`init`/`on_bar` + `aether_strategy_*`). Includes **`request_financial`** (v0 literal strings). Compare / logic / `select` on **f64 0/1** bool encoding; **`%`** via trunc/div/mul remainder pattern. User-function bodies and top-level **`if`** emit when in the supported HIR subset; **nested `plot`**, non-`close` series history, and **UDT `method`** defs still error or skip. Not full language. |
| **Guest ABI** | Exports (`init`, `on_bar`, …), imports (data, strategy, request, mcp) | **Partial** | **v1 exports:** `() -> i32` init, `(i32 bar_index) -> i32` step + documented `aether` import table; [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md), [`docs/agentscript-guest-abi.md`](docs/agentscript-guest-abi.md). `guest_abi::VERSION` **2**. Host still stubs imports / production engine does not drive guest `step` yet. **Compiler tests:** [`tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) links imports, **instantiates**, calls **`init` + `step` loop**; guard on [`GUEST_ABI_V0_IMPORTS`](crates/agentscript-compiler/src/codegen/wasm/abi.rs). |
| **Determinism** | FP rules, seeds, replay | **None** | Document + enforce in host for backtest. |
| **Runtime / host (Aether)** | Data feeds, fills, `request.*`, MCP | **None** | Outside this crate; semantics live here for execution. |
| **Diagnostics** | Errors beyond parse (types, builtins, ABI) | **Partial** | Parse: **miette** with spans. Semantic: [`AnalyzeError`](crates/agentscript-compiler/src/semantic/mod.rs) + [`SemanticDiagnostic`](crates/agentscript-compiler/src/semantic/mod.rs) carry `Span`; CLI maps analysis failures through [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) for miette output. **Codegen:** [`HirScript::source_span`](crates/agentscript-compiler/src/hir/script.rs) (script header) backs [`expr_span`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) fallbacks instead of `Span::DUMMY` when possible. |

## Current status

**Done today**

- [x] **Parse → AST** (Chumsky): headers (`//@version=` **5 or 6**, optional `// @agentscript=`), `import` / `export`, script declarations (`indicator` / `strategy` / `library`), **control flow** (`if` / `else`, `for` … `to` [`by`], **`for … in`** / **`for [i, v] in`**, **`switch` with optional scrutinee** `{ … }`, `while`, **`break` / `continue`**), **blocks** `{ … }`, **user functions** Pine-style `name(…) =>` / `{ … }` or QAS `f name(…)`, **`method name(…) =>`**, **export** of Pine-style or `f` functions, **qualified and typed vars** (`var` / `varip` / `const` / `input` / `simple` / `series`, optional types, **`float[]`**-style array types), assignments `=` / `:=` / **`+=` …**, **`[a, b] =` tuple destructuring**, **Pine `if` expression** `if cond a else b` (incl. `else if` via nested `IfExpr`), **ternary** `? :`, **indexing** `expr[i]`, **array literals** `[a, b]`, **dotted calls** and **method-style** `base.field(…)`, generics on calls (e.g. `matrix.new<float>`), numeric literals with **scientific notation**, optional trailing **`;`**, expressions and comments. See `crates/agentscript-compiler/src/frontend/parser/script.rs`, `expr.rs`, and `assign_type.rs`.
- [x] **Semantic passes** ([`default_passes`](crates/agentscript-compiler/src/semantic/passes/mod.rs)): early analyze (duplicates), `break`/`continue` placement, resolver (dotted roots + `strategy.*` vs script kind), **minimal typecheck** ([`typecheck.rs`](crates/agentscript-compiler/src/semantic/passes/typecheck.rs)). [`check_script`](crates/agentscript-compiler/src/semantic/mod.rs) / [`parse_and_analyze`](crates/agentscript-compiler/src/lib.rs) run this pipeline.
- [x] **HIR crate** ([`hir/mod.rs`](crates/agentscript-compiler/src/hir/mod.rs)): `HirScript`, `HirExpr` arena (`exprs` + `HirId`), `SymbolTable`, `SecurityCall`, `FinancialCall`, etc.; design in [`spec/hir.md`](spec/hir.md).
- [x] **AST → HIR (first slice):** [`lower_script_to_hir`](crates/agentscript-compiler/src/hir/ast_lower.rs), [`AstHirLowerer`](crates/agentscript-compiler/src/hir/ast_lower.rs) + [`LowerToHir`](crates/agentscript-compiler/src/hir/lowering.rs); golden snapshot `crates/agentscript-compiler/src/hir/snapshots/`.
- [x] **Session hook**: [`CompilerSession`](crates/agentscript-compiler/src/session.rs) with `bumpalo::Bump` (ready for arena-backed IR later).
- [x] **Diagnostics**: miette-backed `CompileError` with spans (parse); semantic failures use [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) in the CLI when spans are available.
- [x] **CLI** (`agentscriptc`): read a file path or stdin (`-`), parse + analyze; `--emit=ast` | `hir` | `wasm` (see [`main.rs`](crates/agentscript-compiler/src/main.rs)).
- [x] **WASM (HIR subset):** [`compile_script_to_wasm_v0`](crates/agentscript-compiler/src/lib.rs), [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs); `cargo test` validates module + import/export names. Includes **`%`**, **`ta.crossover` / `ta.crossunder`**, **`input.float`**, user-fn bodies where lowered, and **`var` / `varip`** persist globals for supported scripts.
- [x] **Tests**: parser / error cases in `crates/agentscript-compiler/tests/`; HIR golden in-crate; **wasmtime link + instantiate** for guest ABI v0 in [`tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) (stubs must stay in sync with `aether-mwvm` `aether_guest_stubs.rs`). An `aether-mwvm` dev-dependency on this crate is **not** used: some toolchains fail the combined build (`ar_archive_writer` / rustc `let`-chain features); see [`docs/aether-integration-gap.md`](docs/aether-integration-gap.md).
- [x] **Example script coverage:** [`examples/uptrend.pine`](examples/uptrend.pine) (Pine v6–style source) is exercised by **`parse_and_analyze`** in [`parse_smoke.rs`](crates/agentscript-compiler/tests/parse_smoke.rs) (`examples_uptrend_pine_parse_and_analyze`). [`examples/weighted_strategy.pine`](examples/weighted_strategy.pine) is a larger manual fixture (not required to parse/analyze in CI yet). WASM unit tests still use a **minimal indicator** string because full real-world scripts need additional `ta.*` / series host surface (e.g. `ta.tr`, `ta.atr`, non-`close` series in guest ABI) before `compile_script_to_wasm_v0` can succeed end-to-end.

**Outstanding work (near term)**

**Latest prioritized backlog** *(refreshed 2026-04-06)*

1. **Full span coverage** — replace `Span::DUMMY` on semantic/codegen error paths where the AST or HIR has a real range (progress: [`hir_ty_to_val`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) / wasm type errors use the expression span; internal let-dedupe in [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) no longer reports `DUMMY` for an impossible map miss; **`expr_span`** uses [`HirScript::source_span`](crates/agentscript-compiler/src/hir/script.rs) as last-resort fallback; grep `Span::DUMMY` for remaining sites — notably [`Expr::synthetic`](crates/agentscript-compiler/src/frontend/ast/expr.rs)). Optional issue stubs: [`docs/github-backlog.md`](docs/github-backlog.md).
2. **Full type system + symbol tables** — imports/exports remain **AST-only** (no module graph); surface `array<>` / `matrix<>` / `map<>` types parsed but not enforced (semantics table).
3. **`request.security` / `request.financial` end-to-end** — security: typecheck + HIR + WASM `request_security` + MWVM stub; financial: typecheck + HIR + WASM `request_financial` (**`i32×10`**, `gaps` / `currency` / `ignore` literals) + MWVM stub. Remaining: dynamic symbols/args, real host data + golden tests.
4. **Codegen** — current path is [`GuestWasmV0`](crates/agentscript-compiler/src/codegen/backend.rs) → [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) (`wasm-encoder` bytes). **Not** yet a rustc **`wasm32-unknown-unknown`** (or agreed) artifact unified with **`aether-common`** beyond the written guest ABI doc.

Parallel larger items: **full-language** HIR+WASM, **guest ABI** finalization + Aether invocation + cross-repo contract tests (see “Not started” below and Phase 1–3).

- [x] **Widen HIR lowering** (incremental): `if` / `else if`, user `=>` calls, bool literals, `request.security` gaps/lookahead + typed inner result; still grow builtins / block user bodies / WASM in step.
- [x] **Wire HIR into the driver**: [`HirLowerPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs) + [`analyze_to_hir_compiler`](crates/agentscript-compiler/src/lib.rs) / [`session_hir`](crates/agentscript-compiler/src/lib.rs); [`lower_script_to_hir`](crates/agentscript-compiler/src/hir/ast_lower.rs) remains for tests and direct tooling.
- [x] **CLI semantic diagnostics** — [`AnalyzeCompileError`](crates/agentscript-compiler/src/error.rs) + miette for analysis failures with spans.
- [ ] **Full span coverage** — ensure every semantic error path attaches a non-`DUMMY` [`Span`](crates/agentscript-compiler/src/frontend/ast/node.rs) where the AST has one.
- [ ] **Full** type system + symbol tables (Pine/QAS parity, generics, library linking).
- [ ] **`request.security` / `request.financial`** end-to-end: full v6-aligned typing and params, real host oracle (beyond MWVM stubs), integration / golden tests. *(Progress: both map to WASM imports + `aether-mwvm` stubs today.)*
- [ ] **Codegen** to **`wasm32-unknown-unknown`** (or agreed triple) + **guest ABI** (`aether-common` / ABI doc).

**Not started / still open**

- [ ] **Full-language** WASM + HIR (strategy bodies, full `request.*`, library/module linking, UDT methods, etc.).
- [ ] **Guest ABI** production-hardened: optional **`step` linear-memory OHLCV layout**, **`strategy.*` imports**, and **Aether production** invocation + cross-repo contract tests. *(Compiler progress: **`init`/`step` v1 signatures**, wasmtime **`init`+`step` loop** + import-name guard in [`tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs).)*

## Downstream alignment

| Consumer | What we owe them |
|----------|------------------|
| **Aether** | Stable ABI + `.wasm` bytes + deterministic build story so jobs can pin `wasm_sha256`. |
| **MWVM** | WASM that matches the same ABI and host expectations as other agent guests, where applicable. |

**Integration gap (compiler ↔ Aether):** tracked in [`docs/aether-integration-gap.md`](docs/aether-integration-gap.md) (checklist + references).

Spec and economics context: **`vaulted-knowledge-protocol/backtesting-infra`**.

## Next chapter — Language & ABI surface extension

**Purpose:** Living tracker for the active parallel track: **what scripts may express** (parse → semantics → HIR → WASM) and **what the guest may call** (import/export contract, versioning, docs). Check items off as slices land; the big inventory stays in the **Semantics** table and **Phases 0–3** above and below.

### Language surface (compiler)

- [ ] **HIR + WASM:** widen lowering/codegen for the next real scripts (e.g. more OHLC series on the guest path, additional `ta.*` from the registry, nested `plot` / let-values where the HIR subset still errors).
- [ ] **Builtins:** grow [`builtin_registry`](crates/agentscript-compiler/src/semantic/builtin_registry.rs) (`BUILTIN_ENTRIES`) + typecheck + emit for prioritized namespaces (`ta.*`, `syminfo.*` / `timeframe.*`, …) as product priorities dictate.
- [ ] **Types & modules:** enforce surface `array<>` / `matrix<>` / `map<>`; real **import/export linking** when library graphs matter.
- [ ] **`strategy.*` / side effects:** when strategies are in scope, specify host imports and ordering before deep codegen (see Semantics table — **None** today).

### Guest ABI (compiler ↔ Aether)

- [x] **ABI v1 (2026-04):** `aether_strategy_init` **`() -> i32`**; `aether_strategy_step` **`(i32 bar_index) -> i32`**; `guest_abi::VERSION` **2**; docs + [`validate_guest_abi_v1`](crates/agentscript-compiler/src/codegen/wasm/abi.rs); wasmtime **`init`/`step`** smoke.
- [ ] **Next ABI bump (optional):** `step` gains pointer/length or struct for OHLCV/context in linear memory per [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md); increment `guest_abi::VERSION` when shipped.
- [ ] **Import table discipline:** any new/changed `aether` import updates [`codegen/wasm/abi.rs`](crates/agentscript-compiler/src/codegen/wasm/abi.rs) (`GUEST_ABI_V0_IMPORTS`), **Aether/MWVM** linker stubs, [`tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs), and the guest ABI doc in lockstep.
- [x] **`request.security` (static string slice):** symbol/timeframe may be string literals **or** `let`-bound chains resolving to literals (merged script + user-fn `let` map); same Wasm import. **Still open:** runtime / series-string symbol & timeframe, prefetch/determinism.
- [ ] **`request.financial` / other `request.*`:** dynamic args and production host semantics as needed.

### Slice definition of done

A slice counts as **done** when: emitted WASM validates; the import table matches stubs + written ABI; there is at least one **compiler** test for the new surface, and a **cross-repo** smoke test when the host can run it.

**Last reviewed:** 2026-04-07

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
- [ ] **Test and UX polish:** larger fixtures, corpus samples, fuzz, sharper errors for common mistakes. *(In progress: one real-style script [`examples/uptrend.pine`](examples/uptrend.pine) covered by [`examples_uptrend_pine_parse_and_analyze`](crates/agentscript-compiler/tests/parse_smoke.rs).)*

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
| **Compound assignments** | `+=`, `-=`, `*=`, `/=`, `%=` ([`reference/operators.md`](pinescriptv6/reference/operators.md)) | Assignments (partial) | **Parser:** all five compound ops + `=` / `:=` ([`AssignOp`](crates/agentscript-compiler/src/frontend/ast/stmt.rs)). **HIR:** compound ops lowered to `Let` + binary in [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs) where the lowering pass supports the surrounding stmt. |
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
- [x] **First HIR lowering slice:** [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs) + golden snapshots (indicator + inputs + `ta.sma` + `request.security` + `plot`; additional cases for EMA, user fn + `if`, series access + security options). `request.financial` covered in typecheck + WASM tests, not the main HIR golden fixture.
- [x] **Lexical ↔ HIR symbol alignment:** [`CompilerSession::def_semantic_ids`](crates/agentscript-compiler/src/session.rs) recorded in [`lexical.rs`](crates/agentscript-compiler/src/semantic/passes/lexical.rs); [`ast_lower`](crates/agentscript-compiler/src/hir/ast_lower.rs) consumes it with scoped locals when lowering with a session. **`:=`** reassignment lowers to `Let` for bound names (subset). Further polish (LSP-grade def/use, tuple `:=`, full Pine reassignment rules) still open.
- [ ] Symbol tables and lexical name resolution **to full Pine/QAS parity** (imports, methods, `:=` HIR, …).
- [ ] Type system for **all** core expressions (numbers, series, calls) and script kinds. *(Progress: bool conditions for `if` / ternary / `while`; `switch` scrutinee vs arm types; `request.security` optional merge args; simple/const/input vs series initializers.)*
- [ ] Further script-kind rules (`library` exports-only, etc., as you align with Pine/QAS).
- [ ] **`request.security`:** Pine v6-aligned signatures and parameter typing (symbol, timeframe, expression, `gaps`, `lookahead`, `ignore_invalid_symbol`, related overloads); result type as **series** aligned with the expression’s type; **dynamic** first-argument rules (where TV allows `request.*` inside loops/conditionals—match or document QAS deltas); errors for invalid combinations.
- [ ] **`request.financial`:** Pine v6-aligned signatures and field typing (symbol, financial id, period, `ignore_invalid_symbol`, related forms); result typing consistent with TV’s financial series rules; same **dynamic** / scope constraints as other `request.*` where QAS aligns.
- [ ] Rich diagnostics (second pass after typecheck).

## Phase 2 — IR & codegen

- [x] **HIR as internal IR (v0):** typed/normalized shapes under [`src/hir/`](crates/agentscript-compiler/src/hir/); AST lowering for a **small** language slice (see [`ast_lower.rs`](crates/agentscript-compiler/src/hir/ast_lower.rs)).
- [ ] **HIR completeness:** cover the rest of the typed surface, optimizations, bar/series schedule if needed.
- [x] **`request.security` lowering (v0):** WASM `request_security` + MWVM stub (identity on inner `f64`). **Still open:** real MTF merge / feed policy, dynamic symbol rules, determinism, optional static request graph for prefetch.
- [x] **`request.financial` lowering (v0):** HIR + WASM import `request_financial` + MWVM stub; string pool + 10×`i32` args (`gaps`, `ignore`, optional `currency`). **Still open:** non-literal / dynamic args, determinism / prefetch aligned with `request.security`.
- [x] **WASM emission (HIR subset v0):** [`emit_hir_guest_wasm`](crates/agentscript-compiler/src/codegen/hir_wasm.rs) + [`compile_script_to_wasm_v0`](crates/agentscript-compiler/src/lib.rs) using `wasm-encoder` / `wasmparser` in tests. Emits `aether` imports, `memory`, and dual exports (`init`/`on_bar` + `aether_strategy_*`). **Still not** full-language codegen: e.g. non-`close` series history, nested `plot` as let-values, UDT **`method`** defs in HIR, and scripts outside the lowered subset still fail with [`HirWasmError`](crates/agentscript-compiler/src/codegen/hir_wasm.rs).
- [x] **Guest module ABI v1 (partial):** import indices and export names/types in [`wasm/abi.rs`](crates/agentscript-compiler/src/codegen/wasm/abi.rs) / [`hir_wasm.rs`](crates/agentscript-compiler/src/codegen/hir_wasm.rs); [`validate_guest_abi_v1`](crates/agentscript-compiler/src/codegen/wasm/abi.rs); docs [`aether/docs/agentscript-guest-abi.md`](../../aether/docs/agentscript-guest-abi.md) + [`docs/agentscript-guest-abi.md`](docs/agentscript-guest-abi.md). **Compiler wasmtime smoke:** [`tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) calls **`init` + `step`**. **Still open for Phase 2 “done”:** full `request.*` / strategy surfaces in HIR+WASM, complete user-fn + library coverage, optional linear-memory `step` context + **Aether production** contract tests (MWVM↔compiler dev-dep still blocked on some toolchains; duplicated stub table in the integration test until resolved).

## Phase 3 — Tooling & integration

- [x] **CLI emit modes (partial):** `--emit=ast` | `hir` | `wasm` on file path or stdin (`-`) — see [`main.rs`](crates/agentscript-compiler/src/main.rs).
- [ ] CLI polish: `-o` (write wasm to file instead of stdout), quiet / JSON diagnostics (as needed); optional long-form aliases (`--emit-ast` as synonym) if desired.
- [ ] **Documented loop**: `.qas` → `agentscript-compiler` → `.wasm` → `aether` run (when Aether’s WASM path is ready).
- [ ] **`request.security`:** integration / golden tests with multi-timeframe fixture data (compiler + host), including at least one dynamic-symbol case if QAS supports it.
- [ ] **`request.financial`:** integration / golden tests with fixture financial data (compiler + host), including invalid-symbol / missing-field cases as needed.
- [ ] Optional: `cargo` integration or `build.rs` helper for strategy crates.

## Success criteria by phase

| Phase | Done when |
|-------|-----------|
| **0** | `cargo test` green; braced QAS / Pine-shaped sources in scope parse with clear errors on invalid input; intentional gaps documented in [`spec/qas-v1-parser-status.md`](spec/qas-v1-parser-status.md). *(Corpus / fuzz is a stretch, not a gate.)* |
| **1** | Ill-typed scripts fail fast with actionable diagnostics; well-typed scripts have a stable semantic model; **`request.security` and `request.financial` are typed** (signatures + series rules) or rejected explicitly. *(Progress: minimal typecheck + first HIR slice; full Phase 1 criteria not met yet.)* |
| **2** | Valid strategies compile to **loadable** WASM that satisfies the **written guest ABI** (verified against Aether/MWVM smoke tests); **`request.security` / `request.financial` map to imports** and a stub host can run a minimal MTF + financial example. *(Progress: imports + MWVM stubs; minimal financial example compiles in compiler tests; full MTF + financial **fixture** run still Phase 3.)* |
| **3** | Builders can compile and run end-to-end without reading compiler internals. |

## Repository layout

| Piece | Location |
|-------|----------|
| Library API | `crates/agentscript-compiler` (`parse_script`, AST, `check_script`, `lower_script_to_hir`, `HirScript`, errors) |
| CLI | `crates/agentscript-compiler/src/main.rs` |
| Guest ABI constants + validation | [`codegen/wasm/abi.rs`](crates/agentscript-compiler/src/codegen/wasm/abi.rs) (`GUEST_ABI_V0_IMPORTS`, `validate_guest_abi_v1` / `validate_guest_abi_v0` alias) |
| wasmtime instantiate smoke (v0 imports) | [`crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs`](crates/agentscript-compiler/tests/wasmtime_guest_instantiate.rs) |
| GitHub issue / PR stubs from ROADMAP | [`docs/github-backlog.md`](docs/github-backlog.md) |
| Example `.pine` sources (manual / integration-test fixtures) | `examples/` (e.g. [`examples/uptrend.pine`](examples/uptrend.pine)) |
| Pine v6 manual (reference corpus) | `pinescriptv6/` (`LLM_MANIFEST.md`, `reference/`, `concepts/`, `visuals/`) |
