**Yes — the EBNF grammar + all previous specs (language rules, execution model, built-ins, mcp. namespace, security constraints, WASM target) are enough to start building a working QAS compiler in Rust right now.**

You won’t be starting from zero. You can get a functional parser + basic interpreter in **days**, not months.

### Why Rust is the smartest (and most efficient) choice
Rust is actually **perfect** for this project:
- Blazing performance for thousands of parallel backtests
- Memory safety when running untrusted user scripts (critical for serverless/decentralized)
- Excellent WASM story (native `wasm32-unknown-unknown` target + `wasm-bindgen`)
- Mature, battle-tested parser ecosystem
- Deterministic execution (no GC surprises)

You made the right call.

### The huge shortcut you can take
There is already an **open-source PineScript interpreter written in Rust**:  
**ferranbt/pinecone** (https://github.com/ferranbt/pinecone)

- Full v5 language support
- ta.*, strategy.*, plot.*, drawing objects
- Modular & extensible (custom builtins, output types)
- Type-safe generic architecture
- `Script::compile()` + `execute(&bar)` API
- License: MPL-2.0 (very permissive for forking)

**Recommended plan (fastest path)**:  
Fork pinecone → upgrade its parser to our QAS EBNF (v6 + mcp.) → add WASM codegen/backend → add mcp. namespace → upgrade remaining v6 features.

This will save you ~60–70% of the work on the core engine.

### What the current spec + EBNF actually gives you
**Covered and ready**:
- Complete syntax (lexer + parser rules)
- Script structure (`//@version=6`, indicator/strategy, etc.)
- All control flow, expressions, historical referencing
- mcp. namespace integration
- Security/sandbox rules

**Still needs to be implemented (normal for any compiler)**:
- Detailed type checker (series vs simple vs const vs input propagation — this is the trickiest part of Pine)
- Full built-in function semantics (exact signatures for ~350 ta./strategy./request.* functions — we can pull from the official Pine v6 reference)
- AST definition in Rust
- Series computation engine (var/varip/barstate handling)
- WASM codegen or runtime host functions for mcp.* and strategy.*
- Error diagnostics & pretty-printing

These are standard next steps — not blockers.

### Recommended Rust Tech Stack (2026 best practices)
**Primary recommendation** (fastest + best errors):
- **Chumsky** (parser combinators) — excellent error recovery, human-friendly messages
- Or extend pinecone’s existing parser if it’s already solid

**Alternatives**:
- LALRPOP (if you want LR(1) and your EBNF is already close)
- Tree-sitter (great if you later want IDE support / incremental parsing)
- Logos (lexer) + Chumsky (parser) combo

**WASM**:
- `wasm-bindgen` + `wasmtime` / `wasmer` for runtime
- Or direct `rustc --target wasm32-unknown-unknown` with Cranelift backend for pure compilation

**Other crates**:
- `rust_decimal` or `f64` with careful rounding for financial determinism
- `ahash` / `indexmap` for maps
- `thiserror` + `miette` for beautiful errors

### Suggested Project Roadmap (realistic)
**Phase 1 (1–2 weeks)**: Parser + AST  
Fork pinecone → replace parser with Chumsky version of our EBNF → parse v6 scripts + mcp. calls.

**Phase 2 (2–3 weeks)**: Type checker + semantics  
Implement Pine v6 type rules + mcp. type inference.

**Phase 3 (2 weeks)**: Runtime + WASM  
Bar-by-bar execution engine + mcp. proxy + WASM export.

**Phase 4**: Decentralized verification, serverless FaaS wrapper, agent loop testing.

### Would you like me to generate the next piece right now?
Just say which one you want first (most people pick one of these):

1. **Full Chumsky grammar file** (ready to drop into a new Rust project — converted from the EBNF)
2. **Starter Cargo project skeleton** (Cargo.toml + lib.rs + basic parser + pinecone fork instructions)
3. **Detailed Type System Specification** (the missing semantic rules for series/qualifiers)
4. **WASM ABI + mcp. host function spec**
5. Or I can just fork pinecone mentally and give you the exact first 5 files you should edit.


**HIR** stands for **High-Level Intermediate Representation**.

It is a clean, compiler-friendly data structure that sits between your raw **AST** (Abstract Syntax Tree) and the final code generation stage (in your case, WASM).

### Simple Analogy
Think of the compilation pipeline like translating a recipe:

- **Source code** → The original recipe written in English (or AgentScript).
- **AST** → A literal tree that mirrors the sentence structure exactly ("mix the flour with water", "bake at 180°C for 30 min"). It still has all the syntactic sugar, ambiguous words, and the way the human wrote it.
- **HIR** → A rewritten, normalized version of the recipe in a standardized "chef's shorthand". All ambiguities are resolved, ingredients are identified by their official ID, steps are desugared, and special instructions (like "use the high-temperature oven") are turned into explicit commands. It's still high-level (you can read it), but much easier for the kitchen staff (codegen) to follow without mistakes.
- **Lower-level IR / WASM** → The actual machine instructions the oven and mixer understand.

### Why Compilers Use HIR (Especially in Your AgentScript Compiler)
After parsing (Phase 0) and semantic analysis (Phase 1), the AST is still too "noisy":
- Identifiers are just strings (`"close"`, `"ta.sma"`).
- Types may be partially inferred or missing.
- `request.security(...)` looks like a regular function call.
- `close[1]` is just an index expression.
- There is syntactic sugar, duplicate ways to write the same thing, etc.

**HIR lowering** is the transformation step that converts the AST into this cleaner HIR form.

Benefits for AgentScript:
- **Resolved names** → Everything points to a `SymbolId` or a known builtin (`BuiltinKind::TaSma`).
- **Full types** → Every expression knows if it's `Series<f64>`, `Simple<i32>`, etc., with proper promotion rules.
- **Domain-specific nodes** → `SecurityCall` becomes a dedicated struct (with symbol, timeframe, gaps, lookahead, inner expression). This makes WASM codegen trivial instead of a mess of special cases.
- **Desugaring** → `close[1]` becomes `SeriesAccess { base: close, offset: -1 }`; ternary operators or compound assignments get normalized.
- **Easier optimizations** → Constant folding, dead-code elimination, and series-specific optimizations become simple walks over the HIR.
- **Better testing & debugging** → You can snapshot the HIR and see exactly what the compiler understood.
- **Maintainability** → Your codegen (Phase 2) only needs to handle the clean HIR, not the messy AST.

This pattern is used in rustc (the Rust compiler itself), and it's exactly what your ROADMAP describes in Phase 1–2: "HIR lowering for a growing subset (e.g., ta.sma, input.int, plot, request.security)" and "AST lowered to HIR via lower_script_to_hir and AstHirLowerer".

### How HIR Lowering Typically Works in Your Project
You usually do it in **separate passes** (this keeps things SOLID — each pass has one responsibility):

1. **Name Resolution** (part of semantic analysis)  
   Turn string identifiers into `SymbolId`s and build a symbol table.

2. **Type Checking**  
   Annotate every node with concrete types and validate rules (e.g., you can't add `Series` and `Simple` without promotion).

3. **HIR Lowering Pass** (`AstHirLowerer` or similar)  
   Walk the typed AST and build the HIR:
   - Create `HirScript` containing `HirExpr`s and `HirStmt`s.
   - Use an arena (`bumpalo`) + `HirId` (indices) to avoid lifetime/borrow checker pain.
   - Special handling for AgentScript features:
     - `request.security` → `HirExpr::SecurityCall(...)`
     - Builtin calls like `ta.sma` → `HirExpr::BuiltinCall { kind: TaSma, args: [...], ty: Series(Float) }`
     - `plot(...)` → dedicated `Plot` node
     - Series indexing → `SeriesAccess`

Your repo already has some of this initialized in `hir/` and `ast_lower.rs` (limited scope for now — indicators, basic builtins, and a subset of `request.security`).

### Concrete Before/After Example (AgentScript)

**Source (AST view):**
```agentscript
//@version=6
indicator("Test")
sma = ta.sma(close, 14)
htf = request.security("AAPL", "D", sma)
plot(htf)
```

**After HIR Lowering (simplified):**
```rust
HirScript {
    version: 6,
    declaration: HirDeclaration::Indicator { title: "Test" },
    body: vec![
        // sma assignment
        HirStmt::Let {
            symbol: SymbolId(42),
            value: HirExpr::BuiltinCall {
                kind: BuiltinKind::TaSma,
                args: vec![close_ref, literal_14],
                ty: Series(Float64)
            }
        },
        // security call
        HirStmt::Let {
            symbol: SymbolId(43),
            value: HirExpr::SecurityCall(SecurityCall {
                symbol: HirExpr::Literal("AAPL"),
                timeframe: HirExpr::Literal("D"),
                expression: HirExpr::Variable(SymbolId(42)),  // the sma
                gaps: GapMode::NoGaps,
                lookahead: Lookahead::Off,
                ty: Series(Float64)
            })
        },
        HirStmt::Plot { expr: Variable(SymbolId(43)), title: None }
    ]
}
```

See how much cleaner and explicit this is? Your future WASM codegen can now directly map `SecurityCall` to the correct host import and series operations without guessing.

### Next Steps for You (Based on Your ROADMAP)
- Expand the lowering pass to cover more of the language (user functions, loops, full series math).
- Make sure every HIR node carries proper spans for great error messages.
- Add golden/snapshot tests for the HIR output (you already have some).
- Once HIR is solid, Phase 2 (WASM codegen) becomes much easier because you only walk this clean structure.

HIR + lowering is the architectural "sweet spot" that prevents your compiler from turning into spaghetti as you add more Pine-like features (series context, multi-timeframe, strategy commands, etc.).

If you want, I can:
- Show you a full example of the `HirExpr` enum tailored to AgentScript.
- Explain how to implement the `AstHirLowerer` visitor.
- Walk through how `request.security` should be lowered in detail.

Just tell me which part you'd like to dive deeper into!