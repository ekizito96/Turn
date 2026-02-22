# Turn

**A systems language for agentic computation.** Turn is a compiled language whose primitives encode the physical constraints of agentic software: finite context capacity, stochastic inference, durable state, and suspension/resumption across effects.

> **v0.5.0-alpha** — Distributed Sovereign Runtime. This release elevates Turn from an experimental interpreter into a multi-threaded, distributed, provider-agnostic agentic OS.

**Created by [Muyukani Ephraim Kizito](https://github.com/ekizito96), Founder of [AI Dojo](https://ai-dojo.io) and Prescott Data.** Built with research and resources developed at Prescott Data.

**Quick Links:**
- [Vision](VISION.md) — Language vision, philosophy, and roadmap
- [Whitepaper](WHITEPAPER.md) — Publishable model and runtime semantics
- [Spec](spec/) — Formal grammar and runtime model

---

## Status

**v0.5.0-alpha is the current release.** The Turn compiler and Virtual Machine are implemented in **Rust**, chosen for its zero-cost abstractions, native async support, and memory safety guarantees. Turn scripts themselves can call any external service or tool regardless of what language that backend is written in.

---

## Getting Started

**Prerequisites:** [Rust toolchain](https://rustup.rs/)

```bash
# Clone and build
git clone https://github.com/ekizito96/Turn.git
cd Turn/impl
cargo build --release

# Run a Turn script
cargo run -- run tests/test_actor.turn

# Run the full test suite
cargo test

# Start the Language Server (for VS Code)
cargo run -- lsp
```

---

## Features (v0.5.0-alpha)

### Core Language
- **`infer Type { prompt }`** — Stochastic inference as a first-class statement. Compiles to a generic bytecode trap, completely decoupled from any specific LLM provider.
- **`tool turn(arg: Type) -> Type { ... }`** — First-class cognitive tools that generate OpenAI-compatible JSON schemas automatically. Called via `call(tool, args)`.
- **`suspend`** — Durable checkpoint boundary. Serializes the full VM heap to disk and safely resumes across restarts.
- **`spawn`, `send`, `receive`** — Erlang-style actor model with isolated process mailboxes.
- **`spawn_remote("node_ip", closure)`** — Deploy agents across physical machines natively over TCP.
- **`confidence(val)`** — Probabilistic execution traps that protect boolean branching on low-certainty LLM outputs.
- **`remember(key, val)`** / **`recall(key)`** — Semantic memory backed by an HNSW vector index with Ebbinghaus Exponential Decay.
- **`link(pid)`** / **`monitor(pid)`** — Erlang-style supervisor trees for fault-tolerant agent orchestration.

### Type System
- Scalar primitives: `Str`, `Num`, `Bool`, `Null`
- Collections: `List<T>`, `Map<K, V>` with full generic type boundaries
- Cognitive types: `Uncertain(T, confidence)`, `Result<T, E>`, `Option<T>`
- Capability type: `Cap` — an unforgeable opaque handle for secrets (API keys, DB connections); cannot be serialized or printed

### Runtime Architecture
- **Multi-threaded Tokio Scheduler** — Agents map 1:1 with `tokio::spawn` green threads; work-stealing across all CPU cores
- **Object-Capability Security (OCap)** — API secrets never enter the guest VM heap
- **HNSW Semantic Memory** — `O(log N)` approximate nearest-neighbor vector retrieval with temporal decay
- **Durable WAL** — Write-ahead log ensures agent state survives crashes and restarts
- **Distributed Switchboard** — TCP-based message router for cross-node agent swarms
- **Provider Agnostic** — `infer` is a VM trap, not an HTTP call; swap LLM backends without changing Turn code

### Standard Library
| Module | Functions |
|---|---|
| `std/fs` | `read`, `write`, `exists` |
| `std/http` | `http_get`, `http_post` |
| `std/math` | `math.max`, `math.min`, `math.floor`, `math.sqrt` |
| `std/json` | `json.parse`, `json.stringify` |
| `std/time` | `time.now` |
| `std/regex` | `regex.match`, `regex.replace` |

### Tooling
- **VS Code Extension** (`editors/vscode/`) — Syntax highlighting, hover docs, go-to-definition, and live diagnostics via the built-in LSP
- **Language Server** — `cargo run -- lsp` (stdio transport)
- **`turn serve`** — Built-in HTTP server mode for exposing Turn agents as REST endpoints

---

## Project Layout

```
Turn/
├── README.md
├── VISION.md                    # Vision and engineering philosophy
├── WHITEPAPER.md                # Publishable model + semantics
├── spec/                        # Formal language specification
│   ├── 00-design-mandate.md
│   ├── 01-minimal-core.md
│   ├── 02-grammar.md
│   ├── 03-runtime-model.md
│   ├── 04-hello-turn.md
│   └── 07-implementation-strategy.md
├── impl/                        # Rust bytecode VM (reference implementation)
│   ├── src/
│   │   ├── lexer.rs             # Tokenizer
│   │   ├── parser.rs            # Recursive descent parser
│   │   ├── ast.rs               # Abstract syntax tree
│   │   ├── analysis.rs          # Semantic analyzer / type checker
│   │   ├── compiler.rs          # AST → Bytecode compiler
│   │   ├── vm.rs                # Async bytecode executor (Tokio)
│   │   ├── runtime.rs           # HNSW semantic memory + WAL
│   │   ├── runner.rs            # Host: tool dispatch, LLM adapters
│   │   ├── tools.rs             # Standard tool registry (http, fs, json...)
│   │   ├── llm_tools.rs         # Provider-agnostic LLM bridge
│   │   ├── lsp.rs               # Language server (tower-lsp)
│   │   └── server.rs            # HTTP server mode
│   └── tests/                   # Integration tests
└── editors/
    └── vscode/                  # VS Code extension (TypeScript)
```

---

## Reading Order

1. [`VISION.md`](VISION.md) — Start here for the philosophy
2. [`WHITEPAPER.md`](WHITEPAPER.md) — Technical model and runtime semantics
3. [`spec/00-design-mandate.md`](spec/00-design-mandate.md) → [`spec/03-runtime-model.md`](spec/03-runtime-model.md)
4. [`impl/src/`](impl/src/) — Reference implementation in Rust

---

## License

Apache 2.0 — see [LICENSE](LICENSE).
