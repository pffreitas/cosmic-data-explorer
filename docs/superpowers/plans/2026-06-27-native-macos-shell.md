# Native macOS Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a SwiftUI macOS shell linked to a Rust native bridge, preserving the existing Rust engine and Slint shell during migration.

**Architecture:** Add `crates/native-bridge` as a Rust `cdylib` with C ABI functions. Add `apps/macos` as a Swift Package with a core library target for SwiftUI views/bridge code, an executable target for the app entry point, and tests for bridge decoding.

**Tech Stack:** Rust 2021, C ABI FFI, Swift 6, SwiftUI, macOS 14+ deployment target.

## Global Constraints

- The native shell uses real macOS SwiftUI/AppKit controls rather than Slint-drawn macOS chrome.
- The first slice lists active connections from Rust bridge mock data.
- Connection settings open in a centered native SwiftUI sheet.
- The existing Slint desktop shell remains in the workspace during migration.
- Verification commands are `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `swift test --package-path apps/macos`, and `swift build --package-path apps/macos`.

---

### Task 1: Rust Native Bridge

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/native-bridge/Cargo.toml`
- Create: `crates/native-bridge/src/lib.rs`
- Create: `crates/native-bridge/tests/ffi_contract.rs`

**Interfaces:**
- Produces: `cosmic_active_connections_json() -> *mut c_char`
- Produces: `cosmic_string_free(ptr: *mut c_char)`

- [ ] **Step 1: Write failing bridge contract test**

```rust
#[test]
fn active_connections_json_returns_mock_connections() {
    let ptr = cosmic_native_bridge::cosmic_active_connections_json();
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    cosmic_native_bridge::cosmic_string_free(ptr);

    assert!(json.contains("Production"));
    assert!(json.contains("PostgreSQL"));
}
```

- [ ] **Step 2: Run red test**

Run: `cargo test -p cosmic-native-bridge --test ffi_contract`
Expected: fail before implementation exists.

### Task 2: Swift Package Shell

**Files:**
- Create: `apps/macos/Package.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMac/CosmicDataExplorerApp.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionStore.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/SettingsView.swift`
- Create: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/NativeBridgeTests.swift`

**Interfaces:**
- Consumes: Rust `libcosmic_native_bridge.dylib`.
- Produces: `ConnectionStore`, `NativeBridge`, `ContentView`, and `ConnectionSettingsView`.

- [ ] **Step 1: Build bridge dylib**

Run: `cargo build -p cosmic-native-bridge`
Expected: `target/debug/libcosmic_native_bridge.dylib` exists.

- [ ] **Step 2: Add Swift package and test bridge decoding**

Run: `swift test --package-path apps/macos`
Expected: Swift test decodes at least three bridge-backed active connections.

### Task 3: Full Verification

**Files:**
- Modify only files required by failures.

- [ ] **Step 1: Cargo checks**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: pass.

- [ ] **Step 2: Swift checks**

Run: `swift test --package-path apps/macos && swift build --package-path apps/macos`
Expected: pass.
