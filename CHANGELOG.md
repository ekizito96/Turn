# Changelog

All notable changes to the Turn Language will be documented in this file.

## [1.0.0] - 2026-03-02

### Added
- **Turn Language Core**: First stable release of the Turn programming language and its native Rust Bytecode VM.
- **Cognitive Type Safety (`infer Struct`)**: Natively intercept struct definitions to ensure exact JSON schema outputs from language models.
- **Probabilistic Routing (`confidence`)**: First-class `confidence` operator allowing conditional branching logic based on the certainty of probabilistic model outputs (`if confidence x < 0.85 { ... }`).
- **Actor-Model Concurrency (`spawn_link`, `receive`)**: Multi-agent orchestration through isolated VM execution trees (Actors) and deterministic, zero-shared-state mailboxes.
- **Semantic Memory Integration (`remember`, `recall`)**: Built-in keywords for managing persistent vector-based institutional memory across long-running autonomous processes.
- **Provider Agnostic Infrastructure**: Core dispatcher natively routes inference requests to Anthropic, Azure OpenAI, standard OpenAI, Google Gemini, xAI Grok, and Ollama using standard `TURN_LLM_PROVIDER` and `*_API_KEY` environment variables.
- **Standard Library Core Tools**: Out-of-the-box support for `http_get`, `http_post`, `json_parse`, `json_stringify`, `regex_replace`, `fs_read`, `fs_write`, and `time.sleep`.
- **Advanced Examples**: Added production-grade autonomous templates including `quant_syndicate.tn`, `investment_committee.tn`, and `marketing_agency.tn` which fully demonstrate the type-safety, concurrency, and intelligence routing capabilities of the framework.

### Changed
- Transitioned the default open-source license from Apache 2.0 to the MIT License.
- Revamped the main `README.md` to properly communicate Turn's paradigm shift as a compiled systems language for non-deterministic compute.
- Improved the compiler Lexer and Parser, introducing more robust syntax error spans.

### Fixed
- Fixed a major concurrency bug in the VM where `spawn_link` child processes failed to inherit the parent's runtime `struct` definitions, ensuring exact type conformity during parallel execution.
- Added strict `User-Agent` headers to the native `http_get` and `http_post` tools to prevent failures on restrictive public endpoints (like Wikipedia REST API).
- Resolved unused mutability warnings enforcing a clean `cargo clippy` and stricter CI pass rates.

## [0.4.0] - Prior Work
- Initial prototyping of the lexical analyzer, parser, compiler, and stack-based virtual machine.
- Implementation of base AST components (`Expr::Infer`, `Expr::Confidence`, `Expr::Spawn`).
- Foundational scaffolding for standard library and context windows.
