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
   ./run.sh hello
   ```

## Development Workflow

1. Create a branch from `main`
2. Make your changes
3. Run `cargo fmt` and `cargo clippy` before committing
4. Ensure all tests pass: `cargo test`
5. Open a pull request

## Code Style

- Follow Rust standard style (`cargo fmt`)
- Address all Clippy warnings
- Use `thiserror` for error types, `anyhow` for application error handling
- Prefer `Result` over `panic!` in library code

## Areas of Contribution

- **Spec**: Changes to `spec/` require justification and consensus
- **Implementation**: Bug fixes, performance, new features
- **Documentation**: Clarifications, examples, tutorials
- **Tests**: Coverage for edge cases, error paths

## Pull Request Process

1. Ensure CI passes (format, clippy, tests)
2. Update CHANGELOG.md if your change is user-facing
3. Keep PRs focused; split large changes into smaller ones
4. Request review from maintainers

## Questions?

Open an issue for discussion. We welcome contributions of all kinds.
