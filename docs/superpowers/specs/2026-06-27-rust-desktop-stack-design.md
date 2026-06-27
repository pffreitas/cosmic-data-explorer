# Cosmic Data Explorer Rust Desktop Stack Design

## Scope

Cosmic Data Explorer v1 is a GPL-3.0-or-later macOS desktop database explorer. The bootstrap implements the browse-and-query core: connection profiles, secure credential references, local metadata storage, SQL highlighting, SQLite query execution, database-agnostic connector interfaces, and a Slint desktop shell.

The initial QA target is macOS current plus two previous releases: macOS Tahoe 26, Sequoia 15, and Sonoma 14. The architecture remains portable, but packaging and manual smoke testing are macOS-only for v1.

## Architecture

The repository is a Cargo workspace with two crates:

- `crates/engine`: domain types, validation, profile storage, credential storage abstraction, SQLx-backed database connectors, query result mapping, and syntect SQL highlighting.
- `crates/desktop`: Slint UI shell, `.slint` build integration, and minimal runtime wiring to the engine.

The engine owns behavior and keeps the UI thin. The desktop crate is responsible for presentation and invoking engine services; business rules stay in testable Rust modules under `engine`.

## Data And Secrets

Connection profiles never store raw passwords. Profile metadata and query history live in a local SQLite app database under OS-standard app data directories discovered with `directories`. Passwords are stored and retrieved through a `CredentialStore` trait backed by macOS Keychain via `keyring` on macOS.

Credential references are deterministic from profile identity and username so profile metadata can safely round-trip without exposing secrets.

## Database Layer

`DatabaseKind` supports `Postgres`, `MySql`, and `Sqlite`. `SqlxDatabaseConnector` uses concrete SQLx pool types per database kind instead of `AnyPool`, preserving database-specific schema introspection and dialect behavior.

SQLite is fully covered by local integration tests. PostgreSQL and MySQL/MariaDB connectors are implemented behind the same interface and intended for Docker-backed integration tests when Docker is available.

## SQL Editor And Highlighting

The Slint `TextEdit` is the editable source of truth. `HighlightService` uses `syntect` to produce line/span models containing text, byte ranges, and style information for a future Slint highlight overlay. V1 includes highlighting, execute callback plumbing, selected/full query request modeling, and query history persistence. Autocomplete, formatting, diagnostics, import/export, and multi-result workflows are out of scope.

## Packaging

The desktop crate includes `cargo-packager` metadata for macOS `.app`/DMG packaging. Development builds may be unsigned. Public downloads require Apple Developer ID signing and notarization outside this bootstrap.

## Verification

Required checks before considering the bootstrap ready on a machine with Rust installed:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p desktop` on macOS

This environment currently does not expose `cargo` or `rustc`, so verification must be rerun after installing or exposing a Rust toolchain.
