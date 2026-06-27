# GitHub README Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the repository README so GitHub visitors understand Cosmic Data Explorer as a native macOS database workbench powered by Rust.

**Architecture:** This is a documentation-only change. `README.md` remains the public entry point and should combine product positioning, honest project status, build instructions, architecture notes, and roadmap in one GitHub-friendly document.

**Tech Stack:** Markdown, Rust workspace, Swift Package Manager, SwiftUI macOS app, Rust native bridge, SQLx engine.

## Global Constraints

- Do not claim the app is production-ready.
- Do not imply installers, packaged releases, ER diagrams, row editing, import/export, autocomplete, or plugin support exist yet.
- Keep build commands accurate for the current repository.
- Prefer concise, scannable Markdown optimized for GitHub rendering.

---

### Task 1: Rewrite the GitHub README

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: current repository structure and commands from `README.md`, `Cargo.toml`, and `apps/macos/Package.swift`.
- Produces: a product-first README with accurate install, build, architecture, status, roadmap, contribution, and license sections.

- [ ] **Step 1: Replace the README content**

Write `README.md` with this structure and content:

```markdown
# Cosmic Data Explorer

> A native macOS database workbench powered by a Rust engine.

[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)
![Built with Rust](https://img.shields.io/badge/Rust-engine-orange.svg)
![SwiftUI](https://img.shields.io/badge/SwiftUI-native%20shell-blue.svg)

Cosmic Data Explorer is an open-source desktop database explorer for people who want a fast, native macOS workflow without giving up a serious Rust core. It pairs a SwiftUI interface with a SQLx-powered engine for browsing connections, exploring tables, running SQL, and inspecting result rows.

The project is early, but already runnable. The current focus is a clean browse-and-query experience with a native macOS shell, reusable Rust database primitives, and a narrow bridge between the two.

## Why Cosmic Data Explorer

- **Native macOS experience:** the main app lives in SwiftUI and uses real macOS windowing, navigation, sheets, controls, keyboard behavior, and accessibility defaults.
- **Rust where it matters:** connection profiles, credential references, persistence, SQL highlighting, database sessions, schema loading, table previews, and query execution live in a shared Rust engine.
- **Multi-database foundation:** SQLx-backed support targets PostgreSQL, MySQL/MariaDB, and SQLite.
- **Connection-focused workspaces:** each active connection owns its own table explorer, SQL tabs, editor state, and last query results.
- **Real query loop:** SQL tabs execute through the native bridge into the Rust engine and render structured result grids.
- **Table exploration:** load schema metadata, browse tables, and preview rows without leaving the workspace.
- **Row detail inspector:** select a result row and inspect its fields in a dedicated right-side panel.
- **macOS credential storage:** saved connection passwords are stored through Keychain via the Rust `keyring` integration.

## Status

Cosmic Data Explorer is in early alpha. It is useful as a working development build and as a foundation for a native database client, but it is not packaged as a stable end-user release yet.

Current capabilities include:

- native macOS SwiftUI shell in `apps/macos`.
- Rust database engine in `crates/engine`.
- C ABI bridge for the macOS app in `crates/native-bridge`.
- PostgreSQL connection-string creation through the macOS UI.
- built-in SQLite scratch connection.
- active connection sidebar.
- per-connection table explorer and SQL tabs.
- query execution, result grids, table previews, and row detail inspection.
- legacy Slint desktop shell kept available during the migration.

Planned work includes:

- broader connection setup for MySQL/MariaDB and SQLite from the native UI.
- richer schema browsing.
- query history.
- safer credential and connection-management flows.
- autocomplete and formatting.
- import/export.
- table editing.
- packaged macOS releases.

## Quick Start

### Prerequisites

- macOS 14 or newer for the native SwiftUI app.
- Xcode command line tools with Swift 6 support.
- A recent stable Rust toolchain.

### Run the native macOS app

From the repository root:

```bash
cargo build -p cosmic-native-bridge
swift run --package-path apps/macos CosmicDataExplorerMac
```

The Swift app links against the debug Rust bridge library in `target/debug`, so build the bridge first whenever the Rust FFI layer changes.

### Run the legacy Slint shell

```bash
cargo run -p desktop
```

The Slint shell remains available while the native macOS app becomes the primary interface.

## Development Checks

Run the Rust checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run the macOS package checks:

```bash
cargo build -p cosmic-native-bridge
swift test --package-path apps/macos
swift build --package-path apps/macos
```

## Architecture

```text
apps/macos
  Native SwiftUI macOS app, app state, workspaces, settings, result grids.

crates/native-bridge
  Rust cdylib exposing a narrow C ABI and JSON envelopes for Swift.

crates/engine
  Shared Rust domain model, storage, credentials, SQL highlighting, and SQLx database sessions.

crates/desktop
  Legacy Slint desktop shell retained during the native macOS migration.
```

The intended direction is simple: SwiftUI owns the macOS experience, Rust owns the database behavior, and the native bridge keeps the contract between them small enough to test.

## Repository Layout

```text
.
├── apps/macos              # Swift Package for the native macOS app
├── crates/desktop          # Slint desktop shell
├── crates/engine           # Rust database engine and domain model
├── crates/native-bridge    # C ABI bridge used by Swift
└── docs/superpowers        # Design and implementation notes
```

## Contributing

This project is still taking shape, so small, focused changes are easiest to review. Good contribution areas include tests, connection workflows, schema browsing, query ergonomics, result-grid polish, and documentation.

Before opening a pull request, run the relevant checks from the development section and keep product claims aligned with what the app can do today.

## License

Cosmic Data Explorer is licensed under GPL-3.0-or-later. See [LICENSE](LICENSE).
```

- [ ] **Step 2: Review the Markdown locally**

Run:

```bash
sed -n '1,260p' README.md
```

Expected: the README renders as a complete GitHub-facing document with no broken code fences, no unsupported claims, and no missing major sections.

- [ ] **Step 3: Check for unsupported product claims**

Run:

```bash
rg -n "production-ready|stable end-user release|installer|packaged release|ER diagram|row editing|autocomplete|plugin" README.md
```

Expected: matches only appear in status, planned-work, or constraint-compatible language. No match should claim these capabilities are currently implemented.

- [ ] **Step 4: Inspect the final diff**

Run:

```bash
git diff -- README.md
```

Expected: `README.md` is the only implementation file changed by this task, and the diff replaces the old technical stub with the product-first README.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: refresh GitHub README"
```
