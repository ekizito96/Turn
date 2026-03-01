# Changelog

## [0.4.0] - 2026-02-18 (Alpha: Control-Cycle + Cognitive Type Safety)

### Major Features
- **Orthogonal Persistence**: `suspend;` primitive added for durable checkpoint boundaries.
- **Cognitive Type Safety (Struct Inference)**: `infer StructName { ... }` now resolves named struct schemas at runtime for reliable, typed LLM output.
- **Documentation Overhaul**: Added public-facing `VISION.md` and a publishable `WHITEPAPER.md`. Consolidated specs for consistency and removed internal-only documents.

### Changed
- **Project Positioning**: Turn described as a systems language for agentic computation (capability objects + explicit effects + control-cycle semantics).

## [0.3.0] - 2026-02-18 (Alpha Release)

### Major Features
- **Standard Library**: Full suite of built-in modules accessible via `use "std/..."`:
  - `std/fs`: File system I/O.
  - `std/http`: Web client (GET, POST).
  - `std/math`: Math utilities (`max`, `min`, `abs`).
  - `std/env`: Environment variables.
  - `std/json`: Parse and stringify JSON.
  - `std/time`: System time and sleep.
  - `std/regex`: Pattern matching and replacement.
- **Real-World Intelligence**: `infer` keyword now connects to real LLM providers (OpenAI, Anthropic, Gemini, Grok, Ollama) via environment configuration.

### Language Improvements
- **Operators**: Added missing comparison operators (`<`, `>`, `<=`, `>=`).
- **Method Calls**: Support for multi-argument methods (e.g., `math.max(10, 20)`) and unified property/global function invocation.
- **Parser**: improved ambiguity resolution for blocks vs struct initializers.

### Breaking Changes
- **Persistence**: Updated `Closure` serialization format (store files from v0.2.0 are incompatible).

## [0.2.0] - 2026-02-18

### Added
- **Native Intelligence**: `infer <Type> { ... }` keyword for direct LLM calls from language syntax.
- **Probabilistic Logic**: `confidence` operator, `Uncertain` value type, and probabilistic propagation for `+`, `*`, `and`, `or`, `!`.
- **Concurrency**: Actor model primitives `spawn`, `send`, `receive`, `PID` type.
- **Vector Embeddings**: First-class `vec[...]` literals and `~>` cosine similarity operator.
- **Language Server**: Initial LSP implementation (`turn lsp`).
- **Package Manager**: `turn add <pkg>` command.
- **Runtime**: `llm_infer` mock provider for testing agentic logic.

### Changed
- **Type System**: Enhanced with Generics (`List<T>`, `Map<T>`) and Runtime Type Checking.
- **Performance**: Optimized VM instruction dispatch and memory model.

## [0.1.0] - Initial Release
- Basic VM, Lexer, Parser.
- Functions, Structs, primitive types.
- HTTP and File I/O tools.
