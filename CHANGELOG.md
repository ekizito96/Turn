# Changelog

All notable changes to Turn will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CI workflow (GitHub Actions): format check, clippy, tests, build on ubuntu/macos/windows
- CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md
- GitHub issue templates (bug report, feature request)
- Cargo.toml metadata for crates.io (license, repository, keywords)

## [0.1.0] - 2026-02-17

### Added

- Rust bytecode VM implementation
- Lexer, parser, AST, compiler, VM, runtime
- Multi-turn support with nested turns
- True suspension/resume on tool calls (VM pauses, serializable state)
- `turn run <file>` CLI
- `run_with_tools()` for custom tool registries
- Tests: hello_turn, example_agent, suspension
- Documentation: DOCUMENTATION_INDEX.md, PROJECT_SUMMARY.md
- Empirical analysis: research/07-empirical-analysis.md

### Spec (locked for v1)

- Design mandate, minimal core, grammar, runtime model
- Type-friendly design, implementation strategy
- Reference programs: hello_turn, example_agent
