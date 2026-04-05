**The best way to develop a compiler in Rust for a new programming language is iterative, crate-heavy, and focused on a clean pipeline: lexer → parser → AST → semantic analysis → IR → codegen.** Rust is an *excellent* choice here because its enums, pattern matching, and zero-cost abstractions make ASTs and IRs feel natural, while the ecosystem gives you battle-tested tools that avoid reinventing wheels.

### Recommended Modern Rust Compiler Stack (2025–2026)
| Stage              | Recommended Crate          | Why it's best for new languages                                                                 | Alternatives                  |
|--------------------|----------------------------|-------------------------------------------------------------------------------------------------|-------------------------------|
| **Lexer**          | **logos**                  | Derive-based, blazing fast, zero-cost, handles keywords/identifiers perfectly                  | Hand-written or nom          |
| **Parser**         | **chumsky**                | Parser combinators with *excellent* error recovery & diagnostics; very ergonomic              | Hand-written recursive descent, lalrpop (faster but more boilerplate) |
| **Diagnostics**    | **ariadne** or **miette**  | Beautiful, colored, source-mapped errors (like rustc)                                          | codespan-reporting           |
| **AST / Analysis** | Enums + arenas             | Use `bumpalo` or `typed-arena` for lifetimes; avoid `Box` everywhere                           | —                            |
| **Codegen**        | **Cranelift** (preferred) or **inkwell** (LLVM) | Cranelift = pure Rust, fast compilation, great JIT. LLVM = mature optimizations               | QBE (simpler but less mature) |

This stack is what most serious hobby/production Rust language projects converge on (you'll see it in cranelift-jit-demo, many rlox ports, and recent blog series).

### Step-by-Step Technique (The Practical Pipeline)
1. **Design the language first**  
   Write a tiny grammar (EBNF) and a language spec. Start *tiny*: literals, variables, arithmetic, `if`, functions. Add features one by one. This prevents scope creep.

2. **Lexer + Parser → AST** (Frontend)  
   - Use `logos` for tokens + spans.  
   - Feed tokens into `chumsky` (or write a recursive-descent parser if you want maximum control and learning).  
   - AST should be `enum`-heavy (Rust shines here). Store spans everywhere for errors.  
   - **Pro tip**: Use arena allocation early (`bumpalo`) so you don't fight lifetimes.

3. **Semantic Analysis** (Name resolution + Type checking)  
   - Build a symbol table / scope resolver.  
   - For types: either simple Hindley-Milner inference or constraint generation + solver (like Rust does).  
   - Add borrow checking / ownership rules *only* if your language needs it.

4. **Lower to IR**  
   - You can skip a custom IR at first and go straight to Cranelift/LLVM, but a simple custom IR (or Cranelift's own IR) makes optimizations easier later.

5. **Code Generation & Optimization**  
   - **Cranelift route** (recommended for most new languages): Use `cranelift` + `cranelift-jit` or `cranelift-object`. Pure Rust, fast compile times, good enough for most use cases.  
   - **LLVM route**: `inkwell` crate. More powerful optimizations, but pulls in a C++ dependency and slower builds.  
   - Add simple passes (constant folding, dead-code elimination) yourself first.

6. **Driver & Tooling**  
   - `clap` or `argh` for CLI (`myLang build file.myl --emit=llvm --target=x86_64`).  
   - Use `insta` for snapshot testing of parser/AST output.  
   - Add a REPL early (`rustyline` or `reedline`).

7. **Testing & Iteration**  
   - Unit tests for every stage.  
   - Golden-file / snapshot tests for full programs.  
   - Fuzzing the parser once it's stable.

**Start with an interpreter first** (many people follow the "Crafting Interpreters" book and port Lox to Rust → "rlox"). Once semantics are solid, add the compiler backend. This is the fastest way to a working language.

### Essential Skills & Techniques You Need to Develop
- **Rust mastery** (non-negotiable):  
  - Enums + pattern matching (your #1 superpower).  
  - Lifetimes & ownership (especially with spans and arenas).  
  - Error handling (`thiserror` + `miette`/`ariadne`).  
  - Iterators and functional-style code for visitors/walkers.

- **Compiler theory basics** (you don't need a PhD):  
  - Lexical analysis, recursive descent / LL parsing, AST construction.  
  - Name resolution, type checking, simple optimization.  
  - (Optional later) SSA, register allocation, etc. if you go deep on optimizations.

- **Practical techniques that make Rust compilers great**:  
  - **Arena allocation** for AST nodes (avoids `Rc<RefCell<...>>` hell).  
  - **Visitor pattern** or `fold`/`map` over the AST with enums.  
  - **Span tracking everywhere** → beautiful errors.  
  - **Incremental compilation** later (via `salsa` crate, like rust-analyzer does).  
  - Good error recovery in the parser (chumsky gives this almost for free).

### Best Learning Resources (in order of usefulness)
- **createlang.rs** – "Create Your Own Programming Language with Rust" → exactly what you asked for (incremental from toy interpreter to full LLVM compiler).
- **Crafting Interpreters** (free online) + port it to Rust ("rlox" on GitHub) → best conceptual foundation.
- **Cranelift JIT demo** repo → real working example of a toy language → Cranelift.
- Dave Beazley's "Write a Compiler in Rust" (Wabbit language to LLVM).
- Tristan Hume's classic post "Writing a Compiler in Rust" (still excellent).
- YouTube series on logos + chumsky (search "chumsky compiler Rust").

### Final Advice
- **Start stupidly small** — your first working compiler should compile `print(1 + 2 * 3);` in < 1 week.
- Expect the borrow checker to fight you on the AST at first — that's normal. Arenas + indices fix 90% of it.
- Make error messages *beautiful* from day one. Users will thank you.
- Have fun. Writing a language in Rust is one of the most satisfying projects you'll ever do.
