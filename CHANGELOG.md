# Changelog

All notable changes to Turn are documented here. Turn uses [Semantic Versioning](https://semver.org/).

---

## [0.7.0] - 2026-02-27 (Ecosystem Mastery)

This release completes Turn's ecosystem bridge layer. Developers working with OpenAPI, GraphQL, gRPC, FHIR, legacy MCP servers, and CLI tools can now integrate all of them **natively in Turn with zero runtime bloat, zero bash injection risk, and zero LLM context pollution**.

### Language

- **`use schema::graphql("url")`** (GraphQL Compile-Time Adapter)
  Fetches a GraphQL introspection schema at compile time and generates native Turn `StructDef` and `Turn` closures for each `Query` and `Mutation`. The schema is resolved once; the LLM uses the generated closures at runtime with no awareness of the underlying GraphQL wire protocol.

- **`use schema::swagger("url")`** (Swagger/REST Compile-Time Adapter)
  Parses a Swagger v2 JSON definition and generates a Turn `tool` closure for every path+operation. REST endpoints become natively callable Turn functions.

- **`use schema::grpc("url")`** (gRPC/Protobuf Compile-Time Adapter)
  Parses `.proto` definition text and generates Turn `StructDef` for each message and a `turn` closure for each `rpc`. No Protobuf runtime or code generation step needed.

- **`use schema::fhir("url")`** (FHIR Conformance Compile-Time Adapter)
  Parses a FHIR Conformance Statement and generates Turn structs and CRUD closures for every resource type. Healthcare API integration without a FHIR SDK.

- **`mcp("stdio://...")`** (Native MCP Legacy Bridge)
  Spawns a stdio JSON-RPC MCP server process from within Turn's VM. The spawned subprocess is bound to the agent's lifetime and exposed as a `McpServer` struct containing the PID and status. Syntax: `let tools = mcp("stdio://npx @modelcontextprotocol/server-stripe");`

- **`call("sys_exec", { ... })`** (CLI Domestication Boundary)
  Strictly typed CLI execution: the OS binary and each argument must be individually declared as `Str` values in a Turn map. Shell operators (`&&`, `|`, `` ` ``) are structurally impossible. The LLM never touches bash; it only calls the Turn `tool` wrapper.

### Wasm Macro Architecture

All schema adapters are implemented as independent `wasm32-unknown-unknown` Rust crates compiled to `.wasm` files:

| Adapter | Wasm Module | Input | Output |
|---|---|---|---|
| GraphQL | `graphql_adapter.wasm` | Introspection JSON | Turn AST nodes |
| Swagger | `swagger_adapter.wasm` | Swagger v2 JSON | Turn AST nodes |
| gRPC | `grpc_adapter.wasm` | Proto text | Turn AST nodes |
| FHIR | `fhir_adapter.wasm` | Conformance JSON | Turn AST nodes |

Each adapter exposes `allocate_memory` / `deallocate_memory` / `expand_schema` FFI entry points. The Turn compiler calls these at compile time via Wasmtime, then inlines the resulting AST nodes as if they were hand-written Turn code. **Zero runtime overhead. Zero HTTP calls during execution.**

### Bytecode

| Instruction | Purpose |
|---|---|
| `McpStart` | Spawns a `stdio://` subprocess and pushes a `McpServer` struct to the stack |

### Tests

- `test_mcp.tn`: Validates MCP subprocess spawning and struct response
- `test_sys.tn`: Validates strictly bounded CLI execution via `sys_exec`
- `impl/macros/graphql_adapter/`: GraphQL Wasm macro source
- `impl/macros/swagger_adapter/`: Swagger Wasm macro source
- `impl/macros/grpc_adapter/`: gRPC Wasm macro source
- `impl/macros/fhir_adapter/`: FHIR Wasm macro source

---

## [0.6.0] - 2026-02-27 (The Post-Language Ecosystem)

This release eliminates Turn's remaining ecosystem boundaries. Instead of relying on centralized package registries, bulky language SDKs, or native CLI environments, Turn now connects to the outside world through three native language primitives backed by the compiler itself.

### Language

- **`use "https://..."`** (URL-Native Module Caching)
  Import a pure Turn module directly from any URL. The runtime fetches and cryptographically caches the AST locally so subsequent runs are fully offline. No `turn.toml`, no registry, no dependency resolution step required.

- **`use schema::openapi("url")`** (Compile-Time Schema Adapters)
  Download an OpenAPI JSON spec at compile time and generate native Turn closures for each endpoint automatically. Zero SDK installation. The generated methods are plain Turn functions that the LLM can call directly through `infer with [tools]`.

- **`use_wasm("path.wasm")`** (Wasm Component FFI)
  Mount a WebAssembly binary as a native map of Turn closures. Call Rust, Python-compiled, or any other Wasm-compiled function from inside a Turn agent. If the Wasm module panics, the fault is trapped at the boundary and returned as a Turn error value — the agent supervisor tree is unaffected.

### Observability (Phase 5 — now shipping)

- **`trace(pid)`**: Attach a transparent observer to any running agent. The VM routes a copy of every `TraceEvent` to the observer's mailbox in real time with no modifications to the observed agent's code.
- **`#[mock(infer Type = value)]`**: Replace any LLM call with a deterministic value during `--test` mode. Tests run instantly against the full agent logic without hitting a network.
- **Time-Travel Replay**: Every `infer` call and state mutation is written to a versioned Write-Ahead Log. Run `turn replay <agent-id>` to step forward and backward through an agent's execution history frame by frame.

### Bytecode

| Instruction | Purpose |
|---|---|
| `CallTool` | Dispatch to `sys_wasm_adapter`, `sys_wasm_call`, `sys_schema_adapter`, or a user tool |
| `MakeMap(n)` | Construct a Turn map from n key-value pairs on the stack |

### Tests

- `url_resolver_test.rs` — URL-native AST fetch and cache roundtrip
- `schema_adapter_test.rs` — OpenAPI schema expansion into callable Turn closures
- `wasm_sandbox_test.rs` — Wasm FFI roundtrip through `use_wasm`
- `trace_test.rs` — Glass VM tracing across spawned agents
- `mock_test.rs` — Stochastic mock compile-time testing

---

## [0.5.1] - 2026-02-25 (Agentic Physics — Phase 2)

This release embeds the 5 pillars of **Agentic Physics** directly into the Turn language runtime, making LLM resilience, tooling, token safety, context hygiene, and state persistence first-class language primitives.

### Language

- **`with budget(tokens: X, time: Y) { ... }` (Pillar 3 — Thermodynamic Vector)**  
  Enforces hard token and wall-clock time limits on any `infer` block. If the LLM burns more than `X` tokens or exceeds `Y` seconds, the VM raises a `Thermodynamic Constraint Exceeded` trap immediately. Keywords `budget`, `tokens`, and `time` are now reserved.

- **`compress(text, ratio)` (Pillar 4 — Context Entropy Pruning)**  
  Squeezes a context string to the target float ratio deterministically (V2 will invoke a cheap LLM pass). Keeps context window lean without manual substring logic.

- **`forget(label)` (Pillar 4 — Context Entropy Pruning)**  
  Physically deletes matching entries from the Semantic RAM (`.turn_store`). Rebalances the HNSW entry point automatically.

- **`persist let x = y;` (Pillar 5 — Persistence Vector)**  
  Variables declared with `persist` are automatically serialized to `.turn_store/persist_<name>.json` at the VM level via `Instr::StorePersist`. The next VM boot pre-loads them, achieving native state continuity across process restarts.

### Runtime

- **Pillar 1 — JSON Sanity Coercion & Recovery Loop**: `llm_tools.rs` now strips markdown artifacts (` ```json `, triple-backtick), trailing commas, and runs an automatic retry loop before crashing the VM on malformed LLM JSON.

- **Pillar 2 — Native Recursive AST Tool Execution**: When an `infer with [tools]` block receives a `ToolCallRequest`, the VM pauses inference, dispatches the native Turn closure (`Instr::InferResume`), executes it within the lexical scope, and resumes the LLM stream with the tool result.

- **`BudgetFrame` Tracking Stack**: `runtime::BudgetFrame` tracks `max_tokens`, `used_tokens`, `max_time_secs`, and `started_at_secs` per budget scope. Nested budget blocks are fully supported with independent exhaustion checks.

### Bytecode (New Instructions)

| Instruction | Pillar | Purpose |
|---|---|---|
| `InferResume(Type, usize, Name, Args)` | 2 | Resume LLM after native tool execution |
| `PushBudget` | 3 | Start a thermodynamic budget scope |
| `PopBudget` | 3 | End a thermodynamic budget scope |
| `Compress` | 4 | Squeeze context strings to a target ratio |
| `Forget` | 4 | Delete from Semantic RAM |
| `StorePersist(name)` | 5 | Write variable to disk persistence |

### Tests & Examples

- Fixed `server_test.rs`: Replaced broken `reqwest` HTTP test with a direct `Runner` integration test.
- Fixed `std_advanced_module_test.rs`: Renamed `time` variable to `time_mod` (now a reserved keyword).
- Added 3 new examples: `budget_guardrail.tn`, `context_pruning.tn`, `persistent_agent.tn`.

---

## [0.5.0-alpha] - 2026-02-23 (Distributed Sovereign Runtime)

This release elevates Turn from an experimental alpha interpreter into a production-grade, distributed, multi-threaded Sovereign Runtime. Every major architectural subsystem has been hardened for real agentic workloads.

### Major Features

- **Distributed Agent Swarms (`spawn_remote`)**: Agents can now be deployed across physical machines natively using `spawn_remote("node_ip", closure)`. A bare-metal TCP/gRPC Switchboard routes mailboxes and capability handles across nodes.
- **Async Work-Stealing Scheduler**: Replaced the single-threaded round-robin VM loop with a fully async `tokio`-based Work-Stealing executor. Turn processes now map 1:1 to `tokio::spawn` green threads, parallelizing execution across all CPU cores.
- **Object-Capability Security (OCap)**: Introduced `Value::Cap` — an unforgeable opaque integer handle to Host secrets (API keys, DB connections). Capabilities cannot be serialized, printed, or passed to `infer`. Any attempt to evaluate a capability outside trusted Rust host code raises a `PrivilegeViolation` trap.
- **Dynamic Memory Orbits (Ebbinghaus + Kepler)**: Semantic memory now uses a temporal retrieval model. Each memory entry tracks `created_at`, `last_accessed`, and `velocity`. Retrieval strength is computed as `Cosine(Q,M) * e^(-λt)`. A background GC prunes entries that decay below the noise threshold.
- **Structural Sharing (Arc)**: `Value` variants wrapping strings, lists, and maps are now backed by `Arc<T>`. Zero-copy message passing across actor mailboxes and context swaps — eliminating `O(N)` heap duplication on large LLM contexts.
- **Generic Type Boundaries**: `List<T>` and `Map<K, V>` are now fully type-parameterized in the AST, Semantic Analyzer, and `infer` schema generator. The VM enforces element-level type constraints at runtime.
- **Monadic Error Routing**: `Result<T, E>` and `Option<T>` are native VM types. The `try/catch` model is deprecated in favor of pattern-matched monadic branching — treating LLM hallucinations as expected stochastic values, not exceptions.
- **Confidence Execution Traps**: The VM tracks a `StrictnessThreshold`. If an `Uncertain` value falls below the configured confidence level during an `if` branch or arithmetic expression, the VM raises a native `ConfidenceViolation` trap before executing the sensitive path.
- **Provider Agnosticism**: The `infer` statement compiles to a generic `Instr::Infer` bytecode trap — not a hardcoded REST call. The Host adapter (`runner.rs`) intercepts the trap and maps the canonical Turn type to the target provider's format. LLM API changes require only a Host adapter update; Turn scripts are unaffected.
- **Supervisor Trees**: `link(pid)` and `monitor(pid)` instructions enable Erlang-style bi-directional process linking and uni-directional monitoring. Process exits emit typed signals to supervisors for controlled restart.

### Language
- **`tool turn(param: Type) -> Type { ... }`**: First-class tool declarations that auto-generate OpenAI-compatible JSON schemas.
- **`call(tool, args)`**: Native tool closure invocation syntax.
- **`secret` parameter modifier**: Marks parameters that are injected by the Host and never exposed to the LLM in the generated schema.
- **`spawn_remote("node", closure)`**: Deploy closures across networked Turn runtimes.
- **`link(pid)` / `monitor(pid)`**: Supervisor tree primitives.

### Runtime
- **Distributed `Value::Pid`**: Refactored to `Pid { node_id: String, local_id: usize }` for globally addressable multi-node actors.
- **HNSW Semantic Memory**: `O(log N)` approximate nearest-neighbor vector retrieval replacing `O(N)` linear scan.
- **Write-Ahead Log (WAL)**: Durable state persistence with `suspend` primitive for cross-restart continuations.
- **VS Code Extension**: Full syntax highlighting, LSP hover, go-to-definition, and live diagnostics via `turn lsp`.

---

## [0.4.0] - 2026-02-18

### Added
- **Orthogonal Persistence**: `suspend;` primitive for durable checkpoint boundaries.
- **Cognitive Type Safety**: `infer StructName { ... }` resolves named struct schemas for reliable, typed LLM output.
- **Documentation**: Added `VISION.md` and `WHITEPAPER.md`.

---

## [0.3.0] - 2026-02-18

### Added
- **Standard Library**: `std/fs`, `std/http`, `std/math`, `std/env`, `std/json`, `std/time`, `std/regex`.
- **Real LLM Providers**: `infer` connected to OpenAI, Anthropic, Gemini, Grok, Ollama via environment config.
- **Operators**: Comparison operators (`<`, `>`, `<=`, `>=`).
- **Method Calls**: Multi-argument methods (`math.max(10, 20)`).

---

## [0.2.0] - 2026-02-18

### Added
- `infer <Type> { ... }` for direct LLM calls.
- `confidence` operator and `Uncertain` value type with probabilistic propagation.
- Actor model: `spawn`, `send`, `receive`, `PID`.
- `vec[...]` literals and `~>` cosine similarity operator.
- Initial LSP implementation (`turn lsp`).
- Generics: `List<T>`, `Map<T>`.

---

## [0.1.0] - Initial Release

- Basic VM, Lexer, Parser.
- Functions, Structs, primitive types.
- HTTP and File I/O tools.
