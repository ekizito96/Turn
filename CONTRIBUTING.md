# Contributing to Turn

Thank you for your interest in contributing to Turn! This document explains how to get started, how the codebase is structured, and what kinds of contributions are most needed.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.75 or later)
- `wasm32-unknown-unknown` target (for building providers):
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

### Build the VM

```bash
git clone https://github.com/ekizito96/Turn.git
cd Turn/impl
cargo build --release

# Verify
./target/release/turn --version
```

### Build a Wasm Provider

```bash
cd Turn/providers/turn-provider-openai
cargo build --target wasm32-unknown-unknown --release
# Output: target/wasm32-unknown-unknown/release/turn_provider_openai.wasm
```

### Run Tests

```bash
cd Turn/impl
cargo test
```

### Run a Script

```bash
export TURN_INFER_PROVIDER=../providers/turn-provider-openai/target/wasm32-unknown-unknown/release/turn_provider_openai.wasm
export OPENAI_API_KEY=sk-...
cd impl
./target/release/turn run examples/hello.tn
```

---

## Areas for Contribution

### High Priority

| Area | Description |
|---|---|
| **HNSW embeddings** | Implement the real embedding pipeline in `impl/src/runtime.rs` (currently a stub) |
| **AWS Bedrock provider** | Complete `providers/turn-provider-aws-anthropic` — needs SigV4 timestamp host-call |
| **Streaming inference** | Design a `transform_chunk` FFI for streaming tokens from provider to VM |
| **Tool call loop** | Implement the `tool_call` round-trip in `impl/src/llm_tools.rs` |
| **Standard library** | Expand `std/fs`, `std/http`, `std/time`, `std/json` in `impl/src/tools.rs` |
| **Test coverage** | Integration tests for the VM, actor model, and Wasm provider pipeline |

### Good First Issues

- Add new math operations to the `std/math` standard library
- Write example `.tn` files in `impl/examples/`
- Improve LSP diagnostics with source location and suggestions
- Fix typos or improve clarity in documentation
- Improve VS Code syntax highlighting for new keywords

---

## Code Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full internal guide. Summary:

```
impl/src/
├── lexer.rs          ← Tokenizer
├── parser.rs         ← Recursive descent parser
├── compiler.rs       ← AST → Bytecode
├── vm.rs             ← Async bytecode executor (Tokio actors)
├── wasm_host.rs      ← Wasmtime sandbox for use_wasm FFI
├── schema_compiler.rs← Compile-time OpenAPI schema adapter
├── llm_tools.rs      ← infer instruction handler
├── runtime.rs        ← HNSW semantic memory + WAL
├── runner.rs         ← Host: tool dispatch, agent lifecycle
├── store.rs          ← Write-ahead log for Time-Travel Replay
└── tools.rs          ← Standard tool registry (http_get, fs_read, etc.)
impl/tests/
├── suspension_test.rs    ← suspend + resume across restart
├── persistence_test.rs   ← persist let across boots
├── trace_test.rs         ← trace(pid) Glass VM telemetry
├── mock_test.rs          ← #[mock(...)] compile-time testing
├── url_resolver_test.rs  ← use "https://..." fetch + cache
├── schema_adapter_test.rs← use schema::openapi(...) expansion
└── wasm_sandbox_test.rs  ← use_wasm(...) FFI roundtrip
```

---

## Submitting Changes

1. **Fork** and create a branch: `git checkout -b feat/my-feature`
2. **Write tests** for any new behaviour
3. **Run**: `cargo test && cargo clippy --all-targets`
4. **Open a Pull Request** against `main`

### Commit Style

```
feat(vm): add confidence() trap for uncertain values
fix(parser): handle trailing comma in struct fields
docs(providers): document transform_chunk streaming API
test(vm): add actor mailbox overflow test
```

---

## Writing a New Wasm Provider

See [PROVIDERS.md](PROVIDERS.md) for the full protocol specification. The canonical reference is `providers/turn-provider-openai/src/lib.rs`.

Required exports: `alloc`, `transform_request`, `transform_response`.

---

## Code of Conduct

Be kind, constructive, and assume good faith. Contributions of all sizes are welcome.
