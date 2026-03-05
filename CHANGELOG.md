# Changelog

All notable changes to the Turn Language will be documented in this file.

## [Unreleased]

### Added
- **Native List Primitives (`map`, `filter`)**: `map(list, closure)` and `filter(list, closure)` are now native compiler keywords, expanded directly into inline bytecode `while` loops. No imports, no recursive stdlib patterns required.
- **Struct Spread Syntax (`..base`)**: Concise immutable struct evolution. `let next = State { field: new_value, ..current }` creates a new epoch without copying every field manually.
- **Expression `if` / Block Yielding**: `if/else` can now return values, enabling pure functional expressions without helper functions.
- **`spawn_each` Primitive**: Native concurrent iteration over a list — delegates each item to a micro-actor and gathers results. Replaces imperative `for` loops with the Actor model's scatter/gather pattern.
- **`context.append(value)` Primitive**: Dynamically inject contextual data into the LLM's context window at runtime.
- **N-ary Function Arguments**: Function and method calls now support multiple arguments (`call(func, arg1, arg2)`).
- **WASM Drivers — Anthropic**: New `anthropic_provider.wasm` driver for direct `api.anthropic.com` inference.
- **WASM Drivers — xAI Grok**: New `grok_provider.wasm` driver for `api.x.ai` inference.
- **WASM Drivers — Ollama**: New `ollama_provider.wasm` driver for local Ollama inference (no API key required).

### Changed
- **WASM Driver Model hardened**: `llm_infer` and `llm_generate` now strictly route through WASM drivers. The Turn compiler and VM contain zero knowledge of any LLM vendor API.
- **AWS Anthropic provider removed**: The `turn-provider-aws-anthropic` crate and its `.wasm` binary have been removed. Use `turn-provider-anthropic` for direct Anthropic access.
- **Removed hardcoded API parameters**: Removed `temperature` and `max_output_tokens` from all provider payloads for compatibility with O1/O3-series and Responses API models.

### Fixed
- Resolved `generics_test.rs` conflict where a local variable named `map` clashed with the new native `map` keyword.
- Fixed clippy warnings (`collapsible_else_if`, `needless_return`) across `parser.rs`, `runner.rs`, and `tools.rs`.

---

## [1.0.0] - 2026-03-02

### Added
- **Turn Language Core**: First stable release of the Turn programming language and its native Rust Bytecode VM.
- **Cognitive Type Safety (`infer Struct`)**: Natively intercept struct definitions to ensure exact JSON schema outputs from language models.
- **Probabilistic Routing (`confidence`)**: First-class `confidence` operator allowing conditional branching logic based on the certainty of probabilistic model outputs (`if confidence x < 0.85 { ... }`).
- **Actor-Model Concurrency (`spawn_link`, `receive`)**: Multi-agent orchestration through isolated VM execution trees (Actors) and deterministic, zero-shared-state mailboxes.
- **Working Memory (`remember`, `recall`)**: Built-in keywords for managing persistent key-value memory across long-running autonomous processes. Each actor has its own isolated memory namespace.
- **WASM Driver Infrastructure**: Core WASM host architecture for provider-agnostic LLM routing. Ships with drivers for OpenAI, Azure OpenAI, Azure Anthropic, and Google Gemini.
- **Standard Library Core Tools**: Out-of-the-box support for `http_get`, `http_post`, `json_parse`, `json_stringify`, `regex_replace`, `fs_read`, `fs_write`, `sleep`, `len`, `list_push`, `list_contains`, `time_now`, `regex_match`, `env_get`, `env_set`.
- **Advanced Examples**: Production-grade autonomous templates: `quant_syndicate.tn`, `investment_committee.tn`, and `marketing_agency.tn`.

### Changed
- Transitioned the default open-source license from Apache 2.0 to the MIT License.
- Revamped the main `README.md` to properly communicate Turn's paradigm shift as a compiled systems language for non-deterministic compute.
- Improved the compiler Lexer and Parser, introducing more robust syntax error spans.

### Fixed
- Fixed a major concurrency bug in the VM where `spawn_link` child processes failed to inherit the parent's runtime `struct` definitions.
- Added strict `User-Agent` headers to `http_get` and `http_post` to prevent failures on restrictive public endpoints.
- Resolved unused mutability warnings for a clean `cargo clippy` pass.

## [0.4.0] - Prior Work
- Initial prototyping of the lexical analyzer, parser, compiler, and stack-based virtual machine.
- Implementation of base AST components (`Expr::Infer`, `Expr::Confidence`, `Expr::Spawn`).
- Foundational scaffolding for standard library and context windows.
