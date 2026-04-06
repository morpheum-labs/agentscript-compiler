# AgentScript compiler

Compiler front end for **AgentScript / QAS** (`.qas`, `.pine`): parse → AST → semantic passes → [HIR](spec/hir.md) → experimental `wasm32` guest modules aligned with the Aether / MWVM strategy ABI.

## Quick start

From the workspace root:

```bash
cargo build -p agentscript-compiler
cargo test -p agentscript-compiler
```

CLI (`agentscriptc`): read a file path or stdin (`-`), emit AST, HIR, or WASM:

```bash
cargo run -p agentscript-compiler -- path/to/script.qas --emit=hir
```

## Documentation

| Resource | Purpose |
|----------|---------|
| [ROADMAP.md](ROADMAP.md) | Phases, semantics progress table, integration notes |
| [spec/](spec/) | Language + HIR design |
| [docs/aether-integration-gap.md](docs/aether-integration-gap.md) | Compiler ↔ Aether checklist |

## Library API (crate `agentscript-compiler`)

- `parse_script` — Chumsky parse + node ids  
- `check_script` — default semantic pipeline (no HIR)  
- `analyze_to_hir_compiler` — passes + [`HirLowerPass`](crates/agentscript-compiler/src/semantic/passes/mod.rs); read HIR with [`session_hir`](crates/agentscript-compiler/src/lib.rs)  
- `compile_script_to_wasm_v0` — HIR subset only (see ROADMAP)

## Layout

| Path | Role |
|------|------|
| `crates/agentscript-compiler/` | Rust crate (parser, semantic, HIR, codegen) |
| `pinescriptv6/` | Pine v6 manual mirror (reference checklist) |
| `PinescriptV6-docs-crawler/` | Optional doc tooling for builtin metadata |
