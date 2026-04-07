# Research: EVM-style opcodes vs WebAssembly as the shipped artifact

This note compares two architectural patterns for **deterministic, portable execution** and **on-chain (or job-level) reproducibility**, in the context of languages like AgentScript that compile through typed IR to a binary artifact.

It is **not** a product decision record: it frames tradeoffs so future work (second codegen target, pinning strategy, or custom VM) can be scoped deliberately.

**Related in this repo:** [`docs/aether-integration-gap.md`](../aether-integration-gap.md) (WASM + guest ABI integration), [`docs/agentscript-guest-abi.md`](../agentscript-guest-abi.md), [`spec/hir.md`](../../spec/hir.md).

---

## 1. The EVM-style pattern (custom opcode + on-chain bytecode)

The Ethereum Virtual Machine is a reference architecture, not a single implementation detail, but its **design shape** is widely copied:

1. **Instruction set (ISA)** — A fixed, versioned set of opcodes with explicit operand layouts and control flow rules.
2. **Bytecode** — Programs are encoded as a compact byte sequence. Deployed contracts store this bytecode (or a hash + factory pattern); validators and nodes agree on decoding and execution.
3. **Deterministic semantics** — Given the same bytecode, same block context, and same state transition rules, all honest nodes compute the same result. Nondeterminism is either forbidden or confined to well-defined host hooks (precompiles, environmental reads with consensus rules).
4. **Metering** — Each opcode has a **cost** (gas). That bounds work and ties economic incentives to execution.

For **non-Ethereum** systems, the useful abstraction is: **you own the VM**, the **bytecode format is yours**, and **anything stored on a ledger is usually a commitment** (hash + version + optional metadata) to that artifact.

---

## 2. The WebAssembly pattern (standard binary + host-defined imports)

WebAssembly (Wasm) is a **standardized** stack machine with a binary encoding. A typical embedding looks like:

1. **Module** — Functions, memory, tables, globals; validated before instantiation.
2. **Imports** — The module declares **host functions** (I/O, crypto, syscalls). Semantics of the whole program = Wasm semantics **plus** those import contracts.
3. **Runtime** — Production engines (e.g. Wasmtime, V8) implement validation, instantiation, and execution.

In **this** workspace, the compiler lowers AgentScript through HIR toward **`wasm32`** modules and a **narrow guest ABI** (exports such as strategy `init` / `step`, imports such as `request.*`). Reproducibility is expressed as **pinned WASM bytes** (e.g. `wasm_sha256`) and aligned stub linkers on the host—not as storing a custom opcode stream on chain.

---

## 3. Comparison (condensed)

| Topic | Custom opcode VM (EVM-like) | Wasm export |
|--------|------------------------------|-------------|
| **Artifact size** | Can be **very small** if the ISA is minimal and encoding is tight. | Modules are often **larger**; still practical if the chain or job spec stores **only a hash** and fetches bytes elsewhere. |
| **Implementation burden** | **High**: interpreter or JIT, security patches, test matrix, formal spec if you care about multi-impl consensus. | **Lower** for execution: reuse a mature engine; **cost moves** to defining and freezing **import ABI** + determinism (FP, host calls). |
| **Portability** | Portable only where **your** VM is deployed and trusted. | Portable across many **Wasm** hosts; **semantic** portability requires pinning engine + ABI. |
| **Auditability** | Small ISAs can be **easier to review** than full Wasm. | Wasm spec is large; **your** effective surface is “validated module + allowed imports + disabled features,” which can be documented and capped. |
| **Feature growth** | Every new language feature tends to need **new opcodes** or lowering patterns. | Rich baseline (calls, memory, arithmetic); language features map to **existing** Wasm patterns + ABI. |
| **On-chain story** | Natural fit for “**code is data**” on L1: bytecode or CREATE2-style hashes. | Natural fit for “**content-addressed binary**” + off-chain or L2 storage; same as long as the **hash + toolchain metadata** are part of the contract. |

Neither row is universally “better”; they optimize for different **ownership** and **ecosystem** assumptions.

---

## 4. When each pattern tends to win

**Custom opcodes** tend to make sense when:

- You need **minimum bytes** on a consensus-critical layer and can afford a **long-term VM team**.
- You want **full control** over determinism and metering down to individual instructions.
- Your language is **narrow** (e.g. expression templates, fixed financial primitives) and will not grow toward a general-purpose runtime.

**Wasm export** tends to make sense when:

- You want to **ship execution soon** and lean on existing validation/sandbox tooling.
- Your host already standardizes **imports** (oracle, strategy hooks, plotting side effects).
- You are okay defining **determinism** as: pinned compiler version + target + **guest ABI** + engine version (plus FP policy where relevant).

---

## 5. Hybrids (often the pragmatic path)

Many systems combine both **ideas** without choosing a single physical format:

1. **Canonical IR inside the compiler** (AST → typed IR → HIR) for analysis, optimization, and replay tooling.
2. **Wasm (or JVM, or LLVM) as the execution artifact** for production.
3. **On-chain or job manifest** carries **`artifact_hash`**, **`compiler_id`**, **`language_version`**, and **`abi_version`**—EVM-style *commitment* without EVM-style *opcodes on L1*.

A **second backend** that emits a compact opcode stream remains an option if a future requirement (e.g. zk-proving a custom ISA, or ultra-small on-chain blobs) outweighs the maintenance cost.

---

## 6. Mapping to AgentScript / Aether (informal)

- **Today’s direction** in this repository is **Wasm + guest ABI**, not a public custom opcode VM.
- **Gaps** that apply to **both** targets if you care about strict replay: floating-point rules, import semantics, and pinning the **entire** toolchain graph (see [`docs/aether-integration-gap.md`](../aether-integration-gap.md)).
- Introducing **EVM-like opcodes** would be a **new major subsystem** (spec, VM, tests, possibly second codegen from HIR), not a small tweak to `--emit=wasm`.

---

## 7. References (external)

- [Ethereum Yellow Paper](https://ethereum.github.io/yellowpaper/paper.pdf) — canonical EVM opcode and gas model (dense; historical versions vary by hard fork).
- [WebAssembly Core Specification](https://webassembly.github.io/spec/core/) — Wasm validation and execution rules.
- [WebAssembly Design Documents](https://github.com/WebAssembly/design) — rationale for portability and security model.

---

## 8. Open questions (for a future ADR)

- Should a **formal job spec** always pin **Wasm bytes**, or only a **hash** with bytes fetched from a registry?
- If a **compact bytecode** is ever added, is it a **proving-friendly** ISA, a **size-optimized** ISA, or both—and can one lowering target serve both?
- How much of **determinism** is guaranteed by the **compiler** vs **documented host stubs** (imports) vs **engine** configuration?

This folder can accumulate follow-up notes (e.g. zk-friendly ISA sketches) without changing the primary Wasm codegen path unless explicitly scheduled.
