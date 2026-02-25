# Changelog

All notable changes to Turn are documented here. Turn uses [Semantic Versioning](https://semver.org/).

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
