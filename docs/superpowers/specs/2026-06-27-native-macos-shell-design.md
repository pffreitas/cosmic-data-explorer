# Native macOS Shell Design

## Goal

Add a true native macOS shell for Cosmic Data Explorer while keeping the Rust database engine as the source of domain behavior. The native shell should use SwiftUI/AppKit conventions instead of painting macOS-like controls in Slint.

## Architecture

The repository keeps the existing Cargo workspace and adds:

- `crates/native-bridge`: a small Rust `cdylib` that exposes C ABI functions for Swift.
- `apps/macos`: a Swift Package executable using SwiftUI for the native macOS shell.

The first native slice exposes mock active connections through the bridge. The bridge is intentionally narrow so later work can replace the mock data with calls into `crates/engine` storage and database sessions without changing the SwiftUI layout.

## Native UI

The macOS shell uses `NavigationSplitView` for the active-connections sidebar and main workspace. The window relies on real macOS chrome, titlebar behavior, materials, native buttons, native sheets, focus, keyboard handling, and accessibility defaults. Connection configuration opens in a centered SwiftUI sheet from the main window.

The first slice includes:

- Sidebar with active connections.
- Main query workspace with editor, toolbar-style run action, and result placeholder.
- Settings sheet with native form layout for connection configuration.
- Bridge-backed connection list loaded from Rust.

## Build And Verification

Build order:

1. `cargo build -p cosmic-native-bridge`
2. `swift build --package-path apps/macos`

Verification:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `swift test --package-path apps/macos`
- `swift build --package-path apps/macos`

The existing Slint desktop shell remains available during migration.
