# Turn Architecture

This document is the entry point to understanding the Turn codebase. It describes the compilation pipeline, the VM execution model, the actor scheduler, and the inference sandbox.

---

## High-Level Architecture

```
                    ┌─────────────────────────────────────────┐
                    │             Turn Source (.tn)            │
                    └────────────────────┬────────────────────┘
                                         │
                              ┌──────────▼──────────┐
                              │    Lexer (lexer.rs)  │
                              │  Source → Token[]    │
                              └──────────┬──────────┘
                                         │
                              ┌──────────▼──────────┐
                              │   Parser (parser.rs) │
                              │  Token[] → AST       │
                              └──────────┬──────────┘
                                         │
                           ┌─────────────▼─────────────┐
                           │  Semantic Analyser          │
                           │  (analysis.rs)              │
                           │  Type check, scope check    │
                           └─────────────┬─────────────┘
                                         │
                           ┌─────────────▼─────────────┐
                           │  Compiler (compiler.rs)    │
                           │  AST → Bytecode (Instr[])  │
                           └─────────────┬─────────────┘
                                         │
                    ┌────────────────────▼────────────────────┐
                    │               Turn VM (vm.rs)            │
                    │                                          │
                    │  ┌──────────┐         ┌──────────────┐  │
                    │  │ Process  │  actor   │   Process    │  │
                    │  │  (Root)  │─────────▶│  (spawned)   │  │
                    │  └────┬─────┘ channels └──────────────┘  │
                    │       │                                   │
                    │       │ Suspend(tool_name, arg)           │
                    │       ▼                                   │
                    │  ┌──────────────────────────────────┐    │
                    │  │        Runner (runner.rs)         │    │
                    │  │  Tool dispatch / LLM bridge       │    │
                    │  └──────────┬───────────────────────┘    │
                    │             │                              │
                    └─────────────┼──────────────────────────--┘
                                  │
                    ┌─────────────▼─────────────────────────┐
                    │  Wasm Inference Host (wasm_host.rs)    │
                    │                                        │
                    │  ┌───────────────────────────────┐    │
                    │  │  Wasm Driver (.wasm)           │    │
                    │  │  transform_request(JSON) → u64 │    │
                    │  └───────────┬───────────────────┘    │
                    │              │ HTTP Config              │
                    │  ┌───────────▼───────────────────┐    │
                    │  │  reqwest (blocking)            │    │
                    │  │  $env:VAR substitution         │    │
                    │  │  HTTPS → LLM API               │    │
                    │  └───────────┬───────────────────┘    │
                    │              │ HTTP Response            │
                    │  ┌───────────▼───────────────────┐    │
                    │  │  Wasm Driver (.wasm)           │    │
                    │  │  transform_response(JSON) → u64│    │
                    │  └───────────────────────────────┘    │
                    └───────────────────────────────────────┘
```

---

## Module Reference

### `impl/src/lexer.rs` — Tokenizer

Converts a raw source string into a `Vec<Token>`. Handles:
- Keywords (`turn`, `let`, `infer`, `spawn`, `send`, `receive`, `remember`, `recall`, `suspend`, `return`, `if`, `else`, `struct`, `tool`, `import`)
- Operators and delimiters
- String and number literals
- Comments (`//`)

Entry point: `Lexer::new(source).tokenize() -> Result<Vec<Token>, LexError>`

---

### `impl/src/parser.rs` — Recursive Descent Parser

Converts `Vec<Token>` into a `Program` (a `Vec<Stmt>` AST). The parser is a handwritten recursive-descent parser.

Key AST nodes (defined in `ast.rs`):
- `Stmt::Let` — variable binding
- `Stmt::Return` — turn result
- `Stmt::If` / `Stmt::While` — control flow
- `Stmt::Infer` — LLM inference with schema
- `Stmt::Spawn` — actor creation
- `Stmt::Send` / `Stmt::Receive` — message passing
- `Stmt::Remember` / `Stmt::Recall` — memory access
- `Stmt::Suspend` — durable checkpoint

Entry point: `Parser::new(tokens).parse() -> Result<Program, ParseError>`

---

### `impl/src/analysis.rs` — Semantic Analyser

Performs type checking and scope analysis over the AST before compilation. Reports:
- Undefined variables
- Type mismatches on `infer` schemas
- Unresolvable module imports

Does not transform the AST — only validates it and emits diagnostics.

---

### `impl/src/compiler.rs` — AST → Bytecode

Walks the AST and emits a flat `Vec<Instr>` bytecode sequence. Key instructions (defined in `bytecode.rs`):

| Instruction | Effect |
|---|---|
| `PushStr(s)` | Push a string literal onto the stack |
| `PushNum(n)` | Push a number literal |
| `Store(name)` | Pop stack top → env binding |
| `Load(name)` | Push env value onto stack |
| `Infer(ty)` | Suspend: LLM inference trap |
| `Spawn` | Suspend: spawn new actor process |
| `Send` | Send top-of-stack value to PID |
| `Receive` | Suspend: wait for mailbox message |
| `Remember` | Persist key-value to semantic memory |
| `Recall` | Load from semantic memory |
| `Suspend` | Checkpoint full VM state to disk |
| `Call` | Suspend: invoke a registered host tool |
| `Return` | Exit current process with result |

---

### `impl/src/vm.rs` — Bytecode Executor

The VM runs `Vec<Instr>` across a set of `Process` actors managed by a shared `Registry`.

Key structures:
- `Process` — a single executing agent with its own `frames`, `stack`, `runtime`, `mailbox`, `token_budget`
- `Registry` — maps PIDs to their `UnboundedSender<Value>` channels
- `VmEvent` — the event emitted to the host runner: `Complete { result }`, `Error { error }`, `Suspend { tool_name, arg, resume_tx }`

The VM is entirely `async` using `tokio::spawn` for actor concurrency. When a `Suspend` event is emitted, the runner handles the tool call and sends the result back via `resume_tx`.

**Token budget**: Each instruction consumes 1 budget unit. HNSW searches cost 50 units. Budget exhaustion terminates the process cleanly.

---

### `impl/src/runner.rs` — Host: Tool Dispatch & Agent Lifecycle

The `Runner` owns:
- The `ToolRegistry` — a map of registered host tools
- The `FileStore` — durable agent state persistence
- A module cache for resolving `import` statements

When the VM emits a `Suspend` event, the Runner:
1. Matches on `tool_name`
2. Dispatches to the appropriate tool handler
3. Sends the result back via `resume_tx`

Special tool names handled internally:
- `"sys_import"` — loads and evaluates a `.tn` module file
- `"sys_suspend"` — checkpoints the VM state to the `FileStore`
- `"llm_infer"` — delegates to `wasm_host.rs` for Wasm-sandboxed inference

---

### `impl/src/wasm_host.rs` — Wasm Inference Sandbox

Manages the `wasmtime` engine and executes the dual-pass inference pipeline.

**API:**
```rust
let provider = WasmProvider::new(path_to_wasm)?;
let result = provider.execute_inference(turn_request_json)?;
```

**Pipeline:**
1. Allocates memory in the Wasm module via the exported `alloc` function
2. Writes the Turn request JSON into Wasm memory
3. Calls `transform_request` → reads the HTTP Config JSON out of Wasm memory
4. Resolves `$env:VAR_NAME` placeholders from the host environment
5. Executes the HTTPS call via `reqwest::blocking::Client`
6. Writes the HTTP response JSON into Wasm memory
7. Calls `transform_response` → reads the Turn result JSON

The Wasm module has **no host imports** — it is a pure computational sandbox.

---

### `impl/src/llm_tools.rs` — `infer` Instruction Handler

Registered as the `"llm_infer"` tool in the Runner. When the VM emits a `Suspend` for `llm_infer`, this module:

1. Reads `TURN_INFER_PROVIDER` from the environment
2. Lazily initializes a `WasmProvider` (singleton per process)
3. Prepares the JSON-RPC request payload from the VM's `infer` arguments
4. Calls `provider.execute_inference(request_json)`
5. Parses the JSON-RPC response and converts it to a Turn `Value`

Environment variables:
- `TURN_INFER_PROVIDER` — absolute path to a `.wasm` driver file

---

### `impl/src/runtime.rs` — Semantic Memory + WAL

Implements the per-process `Runtime`:
- `env` — the lexical scope (a stack of `HashMap<String, Value>`)
- `context` — the `ContextWindow` (priority-stacked, token-budgeted)
- `memory` — the `SemanticMemory` (HNSW vector index + key-value store)
- `mailbox` — a `VecDeque<Value>` message queue

**HNSW Memory:**
- Memories are stored as (key, value, embedding_vector) tuples
- Nearest-neighbor search returns semantically similar memories
- The Ebbinghaus decay function de-weights infrequently accessed memories over time

---

### `impl/src/tools.rs` — Standard Tool Registry

Provides built-in host tools registered at VM startup:

| Tool name | Behavior |
|---|---|
| `echo` | Prints a value to stdout |
| `http_get` | Makes a GET request and returns the body |
| `http_post` | Makes a POST request |
| `fs_read` | Reads a file from disk |
| `fs_write` | Writes a file to disk |
| `json_parse` | Parses a JSON string into a Turn Map |
| `json_stringify` | Serializes a Turn value to JSON |
| `math.floor`, `math.sqrt`, etc. | Math utilities |

---

### `impl/src/server.rs` — HTTP Server Mode

`turn serve` starts an HTTP API server using `axum`. It exposes:
- `POST /run` — execute a Turn script body, return the result as JSON
- `GET /health` — liveness probe

---

### `impl/src/lsp.rs` — Language Server

Implements the Language Server Protocol using `tower-lsp`. Provides:
- `textDocument/hover` — type information and documentation
- `textDocument/publishDiagnostics` — parse and analysis errors
- `textDocument/definition` — go-to-definition for functions and variables
- `textDocument/completion` — keyword and symbol completions

---

## Providers (`providers/`)

Each provider is a Rust `cdylib` crate compiled to `wasm32-unknown-unknown`. They have **zero external dependencies** beyond `serde` and `serde_json`. No `tokio`, no `reqwest`, no `std::env`.

**Required exports:**
- `alloc(len: u32) -> u32` — memory allocator
- `transform_request(ptr: u32, len: u32) -> u64` — Turn Request → HTTP Config
- `transform_response(ptr: u32, len: u32) -> u64` — HTTP Response → Turn Result

See [PROVIDERS.md](PROVIDERS.md) for the full driver protocol specification.

---

## Key Design Decisions

**Why Rust?**
Zero-cost abstractions, single static binary, native async via Tokio, no GC pauses, true memory safety. The design goals (performance, portability, correctness) required a systems language.

**Why Wasm for providers?**
A single `.wasm` file runs on any platform the Turn VM supports. The sandbox prevents malicious drivers from accessing the host OS. API keys are never in driver code.

**Why a flat bytecode VM instead of a tree-walking interpreter?**
Bytecode is serializable — which is how `suspend`/resume works. The full VM state (`frames`, `stack`, `runtime`) can be serialized to JSON and restored exactly, enabling durable agent execution across process restarts.

**Why Tokio for actors?**
Turn actors map 1:1 to `tokio::spawn` green threads. The work-stealing scheduler uses all CPU cores. Suspension/resume is `async` — no OS threads block during tool calls or inference.
