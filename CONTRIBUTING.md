# Contributing to Turn

Thank you for your interest in contributing to Turn. This document describes how to get started.

## Quick Start

1. **Fork and clone** the repository
2. **Install Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
3. **Build and test**:
   ```bash
   cd impl
   cargo build
   cargo test
   cargo run -- run tests/test_actor.tn
   ```

## Project Structure

| Directory | Contents |
|---|---|
| `impl/src/` | Core compiler and VM (lexer, parser, AST, compiler, vm, runtime, runner) |
| `impl/tests/` | Integration and unit tests |
| `spec/` | Formal language specification (locked; changes require consensus) |
| `editors/vscode/` | VS Code extension (TypeScript) |

## Development Workflow

1. Create a branch from `main`
2. Make your changes
3. Run the full CI check locally before opening a PR:
   ```bash
   cd impl
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```
4. Open a pull request targeting `main`

## Code Style

- Follow Rust standard style (`cargo fmt`)
- Address **all** Clippy warnings — CI runs with `-D warnings`
- Use `thiserror` for error types, `anyhow` for application error handling
- Prefer `Result` over `panic!` in library code
- Prefer `Arc<T>` over `clone()` for large values

## Writing Tests

- Integration tests live in `impl/tests/*.rs`
- Tests must use `#[tokio::test]` (not `#[test]`) since the VM is async
- Turn snippets under test should use `turn::run(source)` or `turn::run_with_tools`
- Avoid `assert!(true)` — write meaningful assertions

## Areas of Contribution

- **Spec**: Changes to `spec/` require justification and community consensus
- **VM / Compiler**: Bug fixes, performance improvements, new bytecode instructions
- **Standard Library**: New built-in tools in `impl/src/tools.rs`
- **LSP**: Improvements to hover, completion, and diagnostics in `impl/src/lsp.rs`
- **Documentation**: Examples, tutorials, spec clarifications
- **Tests**: Coverage for edge cases, error paths, and new language features

## Pull Request Process

1. Ensure CI passes (`fmt`, `clippy`, `test`)
2. Update `CHANGELOG.md` if your change is user-facing
3. Keep PRs focused — split large changes into smaller ones
4. Request review from a maintainer

## Questions?

Open a GitHub issue for discussion. We welcome contributions of all kinds.
