# Turn Documentation Index

**Complete navigation guide for the Turn programming language project.**

---

## Quick Start

| Document | Purpose | Audience |
|----------|---------|----------|
| [README.md](README.md) | Project overview, status, running instructions | Everyone |
| [spec/04-hello-turn.md](spec/04-hello-turn.md) | First program example | New developers |
| [spec/00-design-mandate.md](spec/00-design-mandate.md) | Core mission and design principles | Decision makers, contributors |

---

## 1. Specification (`spec/`)

**Locked for v1.** The authoritative definition of the Turn language.

| # | Document | Purpose | Status |
|---|----------|---------|--------|
| 00 | [design-mandate.md](spec/00-design-mandate.md) | Mission, design goals, optimization targets | ✅ Locked |
| 01 | [minimal-core.md](spec/01-minimal-core.md) | v1 primitives: turn, context, memory, tool | ✅ Locked |
| 02 | [grammar.md](spec/02-grammar.md) | BNF, lexer, operators, precedence | ✅ Locked |
| 03 | [runtime-model.md](spec/03-runtime-model.md) | Configuration, transitions, semantics | ✅ Locked |
| 04 | [hello-turn.md](spec/04-hello-turn.md) | Reference program with trace | ✅ Locked |
| 05 | [types-and-errors.md](spec/05-types-and-errors.md) | Type system design, error model | ✅ Locked |
| 06 | [example-agent.md](spec/06-example-agent.md) | Realistic agent example | ✅ Locked |
| 07 | [implementation-strategy.md](spec/07-implementation-strategy.md) | Rust VM architecture, rationale | ✅ Locked |

**Reading order for implementers:** 00 → 01 → 02 → 03 → 04 → 07

---

## 2. Research (`research/`)

**Deep science and first-principles justification.** Why Turn exists and what problems it solves.

| # | Document | Purpose | Status |
|---|----------|---------|--------|
| 00 | [problems-we-solve.md](research/00-problems-we-solve.md) | 10 deep pains in building agents with Python/TS/C | ✅ Complete |
| 01 | [syntax.md](research/01-syntax.md) | Grammar design lessons from Wirth, McCarthy, Iverson | 📝 Seed |
| 02 | [structure.md](research/02-structure.md) | Modules, scoping, types from Scheme, Smalltalk | 📝 Seed |
| 03 | [algorithms.md](research/03-algorithms.md) | Parsing, compilation pipeline | 📝 Seed |
| 04 | [foundations.md](research/04-foundations.md) | Formal semantics, operational/denotational | 📝 Seed |
| 05 | [turn-primitives.md](research/05-turn-primitives.md) | Mapping agentic concepts to language primitives | 📝 Placeholder |
| 06 | [critique-and-gaps.md](research/06-critique-and-gaps.md) | Critical review, consolidated gaps | ✅ Complete |
| 07 | [empirical-analysis.md](research/07-empirical-analysis.md) | Production codebase analysis validating design | ✅ Complete |

**Reading order for understanding "why Turn":** 00 → 07 → 06

---

## 3. Implementation (`impl/`)

**Rust bytecode VM.** The reference implementation of Turn.

| Component | File | Purpose |
|-----------|------|---------|
| Lexer | `src/lexer.rs` | Tokenization |
| Parser | `src/parser.rs` | AST construction |
| AST | `src/ast.rs` | Abstract syntax tree definitions |
| Compiler | `src/compiler.rs` | AST → Bytecode |
| Bytecode | `src/bytecode.rs` | Instruction set |
| VM | `src/vm.rs` | Bytecode execution, suspension |
| Runtime | `src/runtime.rs` | Agent state (context, memory, env) |
| Tools | `src/tools.rs` | Tool registry and handlers |
| Value | `src/value.rs` | Runtime value representation |
| Server | `src/server.rs` | HTTP API for deployment |
| Store | `src/store.rs` | Persistence layer |
| Runner | `src/runner.rs` | Execution engine (VM + Store + Tools) |
| CLI | `src/main.rs` | `turn run` and `turn serve` commands |

**Key tests:**
- `tests/integration_test.rs` — End-to-end hello_turn
- `tests/suspension_test.rs` — Suspension/resume cycle
- `tests/example_agent_test.rs` — Realistic agent

---

## 4. Documentation Flow

### For Language Designers
1. Read `spec/00-design-mandate.md` — Understand the mission
2. Read `research/00-problems-we-solve.md` — Understand the pain points
3. Read `research/07-empirical-analysis.md` — See real-world validation
4. Read `spec/01-minimal-core.md` → `spec/02-grammar.md` → `spec/03-runtime-model.md` — Learn the language
5. Read `research/06-critique-and-gaps.md` — Understand trade-offs

### For Implementers
1. Read `spec/00-design-mandate.md` — Mission
2. Read `spec/01-minimal-core.md` — Primitives
3. Read `spec/02-grammar.md` — Syntax
4. Read `spec/03-runtime-model.md` — Runtime model (critical!)
5. Read `spec/07-implementation-strategy.md` — Why Rust, architecture
6. Study `impl/src/` — Reference implementation

### For Users (Developers Writing Turn Code)
1. Read `README.md` — Project overview
2. Read `spec/04-hello-turn.md` — First program
3. Read `spec/06-example-agent.md` — Realistic example
4. Read `spec/02-grammar.md` — Full syntax reference
5. Read `spec/05-types-and-errors.md` — Error handling

### For Researchers/Academics
1. Read `research/00-problems-we-solve.md` — Problem space with citations
2. Read `research/07-empirical-analysis.md` — Empirical validation
3. Read `spec/03-runtime-model.md` — Formal operational semantics
4. Read `research/04-foundations.md` — Theoretical foundations
5. Read `research/06-critique-and-gaps.md` — Critical analysis

---

## 5. Key Concepts Cross-Reference

| Concept | Spec | Research | Implementation |
|---------|------|----------|----------------|
| **Turn** | 01, 02, 03 | 00 (Problem 3), 05 | `vm.rs`, `compiler.rs` |
| **Context** | 01, 03 | 00 (Problem 1), 07 | `runtime.rs` (Context object) |
| **Memory** | 01, 03 | 00 (Problem 2) | `runtime.rs` (Memory object) |
| **Tool Call** | 01, 02, 03 | 00 (Problem 3) | `vm.rs` (Suspension), `tools.rs` |
| **Suspension** | 03, 07 | 00 (Effect handlers) | `vm.rs` (VmResult::Suspended) |
| **Agent** | 00, 01, 03 | 00, 07 | `runtime.rs` (Runtime struct) |
| **Bounded Context** | 00, 01, 03 | 00 (Problem 1, 8), 07 | `runtime.rs` (MAX_CONTEXT_SIZE) |
| **Deployment** | - | - | `server.rs` (HTTP API) |
| **Persistence** | - | - | `store.rs` (FileStore) |
| **Modules & Imports** | 08 | - | `compiler.rs`, `runner.rs` |
| **Error Handling** | 09 | - | `compiler.rs`, `vm.rs` |
| **LSP** | 10 | - | `lsp.rs` |
| **Standard Library** | - | - | `tools.rs` (`fs`, `env`, `http`, `json`) |

---

## 6. Status Legend

- ✅ **Complete/Locked** — Stable, authoritative
- 📝 **Seed** — Outline with key ideas, needs expansion
- 🚧 **Placeholder** — Stub, needs content
- 🔄 **In Progress** — Actively being written

---

## 7. Contributing

When adding or modifying documentation:

1. **Spec changes require justification** — Link to research or empirical evidence
2. **Research should cite sources** — Academic papers, production postmortems, etc.
3. **Keep cross-references updated** — If you change a concept, update all related docs
4. **Maintain the index** — Update this file when adding new documents
5. **Follow the flow** — Spec → Research → Implementation consistency

---

## 8. External References

- **Physics of AI Engineering:** [ai-dojo.io/papers/the-physics-of-ai-engineering](https://ai-dojo.io/papers/the-physics-of-ai-engineering)
- **Nexus Protocol:** [zenodo.org/records/18315572](https://zenodo.org/records/18315572)
- **Production Agent Codebases:** See `research/07-empirical-analysis.md` for analyzed systems

---

**Last Updated:** 2026-02-17
**Maintainer:** Turn Research Group
