# Changelog

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
