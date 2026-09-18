# Versioning policy

Turn follows Semantic Versioning 2.0.0 using `MAJOR.MINOR.PATCH` releases.

The Turn release version covers the language, compiler, VM, CLI, standard library, serialized runtime state, and bundled `turn-provider-*` WASM adapters. Those provider crates use the same release version as Turn.

## Major versions

Increment `MAJOR` for incompatible changes, including:

- Removing or changing accepted Turn syntax in a way that breaks valid programs.
- Changing public Rust APIs without a compatibility path.
- Incompatible bytecode, checkpoint, or serialized `VmState` changes without migration support.
- Removing CLI commands, flags, environment variables, or provider contracts.
- Changing runtime behavior when existing programs cannot retain their prior behavior through configuration.

## Minor versions

Increment `MINOR` for backward-compatible capabilities, including:

- New language expressions, types, effects, or standard-library functions.
- New CLI commands and optional flags.
- New bundled providers or provider capabilities.
- Backward-compatible fields in runtime state with serialization defaults.
- Deprecations that do not yet remove behavior.

## Patch versions

Increment `PATCH` for backward-compatible fixes, security corrections, documentation updates, and performance improvements that do not intentionally add public behavior.

## Pre-releases

Use SemVer pre-release identifiers for functionality that is not ready for a stable contract, such as `1.2.0-alpha.1`, `1.2.0-beta.1`, or `2.0.0-rc.1`. Pre-release versions have lower precedence than the associated stable release.

Feature status labels such as "preview" do not replace SemVer. A preview feature added compatibly may ship in a minor release, but its documented stability boundary must be explicit.

## Release process

1. Keep unreleased changes under `CHANGELOG.md`'s `Unreleased` heading.
2. Select the next version from the compatibility impact above.
3. Set the same version in `impl/Cargo.toml` and every bundled `turn-provider-*` manifest.
4. Update `Cargo.lock` files and create `.github/releases/vMAJOR.MINOR.PATCH.md`.
5. Pass formatting, Clippy, tests, provider WASM builds, package installation, and launch smoke tests.
6. Merge to `main`, then create the signed or annotated `vMAJOR.MINOR.PATCH` tag.

## Turn 2.0.0 decision

Turn `2.0.0` is a major release. Existing ordinary Turn programs remain source-compatible, but strict SemVer also covers the public Rust API and language grammar. The release adds variants to the public `Token`, `Expr`, and `Instr` enums; reserves `decide` as a keyword; extends serialized runtime state; and changes uncaught tool failures from successful string values to errors. Each can break a downstream consumer that relied on the 1.x contract.