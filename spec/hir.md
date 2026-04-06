**HIR Lowering for AgentScript – Complete, Practical Explanation**

This is the exact step you need right now to keep your compiler maintainable as you move into semantic analysis, type checking, and WASM codegen.

### What is HIR Lowering?

**HIR** = **High-Level Intermediate Representation**  
**Lowering** = the transformation pass that turns your raw **AST** into HIR.

| Aspect              | AST (what you have today)                          | HIR (what you want next)                                      |
|---------------------|----------------------------------------------------|---------------------------------------------------------------|
| Identifiers         | Raw strings (`"ta.sma"`, `"close"`)               | Resolved `SymbolId` or `Builtin::TaSma`                       |
| Types               | None or partial                                    | Fully annotated (`Series<f64>`, `Simple<i32>`, etc.)          |
| `request.security`  | Generic `Call` node                                | Dedicated `HirSecurityCall` node (explicit context switch)    |
| `close[1]`          | Index expression                                   | Explicit `SeriesAccess { base, offset }`                      |
| Structure           | Close to source code (syntactic sugar)             | Normalized, desugared, ready for optimization & codegen       |
| Validation          | Minimal                                            | Full semantic rules enforced here                             |

In short: AST = “what the programmer wrote”.  
HIR = “what the code *actually means*” after the compiler understands it.

### Why AgentScript *Specifically* Needs a Strong HIR Layer

Your language is Pine Script-inspired for trading agents, so it has several domain-specific features that explode complexity if you try to handle them directly in codegen:

1. **request.security(symbol, timeframe, expression)**  
   → This is not a normal function call. It switches the entire execution context (different symbol + timeframe + series history). In HIR it becomes a first-class node so the WASM backend can emit the correct guest calls to `aether-mwvm`.

2. **series<T> semantics**  
   → `close + open`, `ta.sma(close, 14)`, `close[1]`, `nz()`, `na()` handling, bar-state awareness. All of these need explicit representation.

#### Typing notes: equality and `na`

Semantic typing for **`==` / `!=`** (before HIR lowering) uses `type_compatible_eq` in [`crates/agentscript-compiler/src/hir/ty.rs`](../crates/agentscript-compiler/src/hir/ty.rs) (invoked from the typechecker). Two operands type-check together if either side is assignable to the other **or** both are **numeric** (so comparisons like `close == na` work when `na` is modeled as a float special and `close` is series float). This is an explicit **QAS / compiler policy**; TradingView Pine has additional dynamic rules—if we tighten behavior later, update this paragraph and `hir/ty.rs` together.

#### User function bodies (`HirUserFunction`)

User-defined functions lower to [`HirUserFunction`](../crates/agentscript-compiler/src/hir/script.rs): `symbol` matches [`HirExpr::UserCall`](../crates/agentscript-compiler/src/hir/expr.rs)’s `callee`, `params` are fresh [`SymbolId`]s per parameter (no cross-function interning), `body_stmts` holds the block prefix, and `result` is the final expression (`=>` bodies use an empty `body_stmts` and lower the tail into `result`). Block bodies without a trailing expression statement use a `0.0` float placeholder aligned with the typechecker’s default return inference.

3. **Builtins** (`ta.*`, `strategy.*`, `plot`, `alert`, `input.*`)  
   → Many should become intrinsics or special nodes so codegen can map them directly to optimized WASM instructions.

4. **Version policy + context rules** (`//@version=6`, `indicator()`, `strategy()`)

Without HIR, your WASM codegen will become a nightmare of special cases. With HIR, everything becomes clean, optimizable, and testable.

### SOLID mapping (how the crate lays out `src/hir/`)

| Principle | How it shows up |
|-----------|-----------------|
| **S**ingle responsibility | One Rust module per concern: `ids`, `ty`, `literal`, `builtin`, `security`, `expr`, `stmt`, `symbols`, `script`, `lowering`. Each file owns one kind of shape or seam. |
| **O**pen / closed | New builtins extend `BuiltinKind` / `HirExpr` without touching security or symbol-table code. New pipeline stages implement `CompilerPass` and/or `hir::LowerToHir`. |
| **L**iskov substitution | Any `LowerToHir` implementation must produce a coherent `HirScript` for the same typed input; tests lock this with golden HIR snapshots. |
| **I**nterface segregation | Callers import only the submodule they need (`use agentscript_compiler::hir::expr::HirExpr` vs the whole tree). |
| **D**ependency inversion | Lowering and future codegen depend on `SymbolTable`, `HirId`, and `LowerToHir`, not on resolver internals or parser-private types. |

### Recommended HIR structure (implemented under `crates/.../src/hir/`)

The monolithic sketch below is split into the modules above. `HirId` / `SymbolId` are newtypes in `hir/ids.rs` (not raw `u32` aliases) for type safety.

```rust
// Conceptual shape (see `hir/script.rs`, `hir/expr.rs`, …)
pub struct HirScript {
    pub version: u32,
    /// Script header span for diagnostics when a per-expr span is missing (`hir/script.rs`).
    pub source_span: Span,
    pub declaration: HirDeclaration,
    pub inputs: Vec<HirInputDecl>,
    pub body: Vec<HirStmt>,
    pub user_functions: Vec<HirUserFunction>,
    pub symbols: SymbolTable,
}

pub enum HirExpr {
    Literal(HirLiteral, HirType),
    Variable(SymbolId, HirType),
    Binary { op: BinOp, lhs: HirId, rhs: HirId, ty: HirType },
    BuiltinCall { kind: BuiltinKind, args: Vec<HirId>, ty: HirType },
    UserCall { callee: SymbolId, args: Vec<HirId>, ty: HirType },
    SeriesAccess { base: HirId, offset: i32, ty: HirType },
    Security(Box<SecurityCall>),
    Plot { expr: HirId, title: Option<String> },
}

pub struct SecurityCall {
    pub symbol: HirId,
    pub timeframe: HirId,
    pub expression: HirId,
    pub gaps: GapMode,
    pub lookahead: Lookahead,
    /// Element type follows the inner expression (e.g. `bar_index` → `series int`).
    pub ty: HirType,
}

pub enum HirStmt {
    /* Let, Plot, Block, */
    If { cond: HirId, then_branch: Vec<HirStmt>, else_branch: Option<Vec<HirStmt>> },
    /* … */
}
```

**Lowering notes:** `SeriesAccess` requires a non-negative integer literal index in the current pass. WASM v0 rejects `HirStmt::If` and `HirExpr::UserCall` until codegen catches up; the HIR still records them for snapshots and future backends. See `hir/stmt.rs` / `hir/expr.rs` for the full `enum` definitions.

Use the `bumpalo::Bump` arena in `CompilerSession` plus dense `Vec` storage keyed by `HirId` (rustc / rust-analyzer style) to avoid self-referential structs.

### How the Lowering Pipeline Works (Step-by-Step)

You should have **three separate passes** (each a `CompilerPass` trait impl — keeps it SOLID):

1. **Name Resolver** → turns string identifiers into `SymbolId` + builds symbol table  
2. **Type Checker** → infers `Series<f64>` vs `Simple<i32>` everywhere  
3. **HIR Lowerer** (the one we’re talking about) → does the final desugaring

```rust
// Future: semantic/lowering.rs — implements both CompilerPass and hir::LowerToHir
pub struct HirLowerer<'sess> {
    session: &'sess mut CompilerSession,
}

impl<'sess> hir::LowerToHir for HirLowerer<'sess> {
    type Err = AnalyzeError;
    fn lower(&mut self, script: &Script) -> Result<hir::HirScript, Self::Err> {
        let mut builder = HirBuilder::new(self.session);
        builder.build(script)
    }
}
```

`CompilerPass` today takes `&Script`; when a typed IR exists, either thread `HirScript` through `CompilerSession` or add a sibling trait that accepts typed input — keep the **lowering trait** (`LowerToHir`) as the stable abstraction for “AST → HIR”.

### Real Example: Before → After

**AgentScript source:**
```agentscript
//@version=6
indicator("Test Agent")

len = input.int(14)
sma = ta.sma(close, len)

htf = request.security("AAPL", "D", sma)
plot(htf)
```

**After lowering (simplified HIR):**
```rust
HirScript {
    version: 6,
    declaration: Indicator { title: "Test Agent" },
    inputs: [InputDecl::Int { name: "len", default: 14 }],
    body: [
        HirStmt::Let {
            symbol: SymbolId(5),           // resolved "sma"
            value: HirExpr::BuiltinCall {
                kind: BuiltinKind::TaSma,
                args: [close_ref, len_ref],
                ty: Series(Float)
            }
        },
        HirStmt::Let {
            symbol: SymbolId(7),           // resolved "htf"
            value: HirExpr::SecurityCall(SecurityCall {
                symbol: Literal("AAPL"),
                timeframe: Literal("D"),
                expression: Variable(SymbolId(5)),   // the sma
                gaps: GapMode::NoGaps,
                lookahead: Lookahead::Off,
                ty: Series(Float)
            })
        },
        HirStmt::Plot { expr: Variable(SymbolId(7)), title: None }
    ],
    user_functions: [],
}
```

See how clean that is? Your WASM codegen can now walk the HIR and emit direct calls to the runtime for security context switching and series math.

### Immediate Action Plan for You

1. Add `bumpalo` to Cargo.toml  
2. Create `src/hir/` with the structure above  
3. Implement the three passes (resolver → typeck → lowerer) in `semantic/`  
4. Add snapshot tests with `insta` for the HIR output (gold-standard for compilers)

Do this **before** you start writing the full type checker or any WASM code. It will save you weeks of pain later.

This architecture is exactly what keeps big Rust compilers (rustc, swc, deno) maintainable.

Want me to drop right now:
- The full `hir/mod.rs` + arena setup code?
- A complete `HirLowerer` + `HirBuilder` skeleton (visitor-based)?
- Or the exact rules for how `request.security` and `series` promotion should work in the lowering pass?
