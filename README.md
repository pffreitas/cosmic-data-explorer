# Cosmic Data Explorer

Cosmic Data Explorer is a GPL-3.0-or-later desktop database explorer for macOS, built with a pure Rust stack.

## Stack

- `crates/desktop`: Slint desktop shell.
- `crates/engine`: connection profiles, credentials, persistence, SQL highlighting, and SQLx database connectors.
- SQLx support targets PostgreSQL, MySQL/MariaDB, and SQLite.
- macOS credential storage uses Keychain through `keyring`.

## Scope

V1 is browse + query: connection manager primitives, schema/table browsing primitives, table previews, a SQL editor model, result grids, and query history. Row editing, table design, SSH tunnels, import/export, ER diagrams, plugins, formatting, diagnostics, and autocomplete are outside the bootstrap.

## Local Checks

This repository requires a Rust toolchain. Run these before considering a bootstrap ready:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p desktop
```

`cargo run -p desktop` is the macOS Slint smoke test.

## Native macOS Shell

The native SwiftUI shell lives in `apps/macos` and links to the Rust bridge dylib from `crates/native-bridge`.

Build and test it from the repo root:

```bash
cargo build -p cosmic-native-bridge
swift test --package-path apps/macos
swift build --package-path apps/macos
```

Run it from the repo root:

```bash
cargo build -p cosmic-native-bridge
swift run --package-path apps/macos CosmicDataExplorerMac
```

The `swift run` command launches the native macOS window and keeps the terminal attached until the app window is closed or the process is stopped.
