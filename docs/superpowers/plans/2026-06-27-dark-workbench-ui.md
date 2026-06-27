# Dark Workbench UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the starter Slint shell with a dark macOS-style workbench UI containing an active-connections sidebar and centered settings modal.

**Architecture:** Keep the UI in `crates/desktop/ui/app.slint` and keep Rust wiring in `crates/desktop/src/main.rs`. Add a small source-level UI contract test in `crates/desktop/tests/ui_contract.rs` to pin the requested shell primitives.

**Tech Stack:** Slint 1.17, Rust 2021, Cargo workspace.

## Global Constraints

- Match the supplied dark desktop screenshots as the design-system reference.
- Sidebar lists all active connections.
- Sidebar has a bottom settings button.
- All connection configuration happens in a centered settings modal.
- This slice uses mock connection rows; persistence wiring is out of scope.
- Phase 1 native-look refinement uses Slint's explicit `native` style and removes fake traffic-light system chrome.

---

### Task 1: UI Contract

**Files:**
- Create: `crates/desktop/tests/ui_contract.rs`

**Interfaces:**
- Consumes: `crates/desktop/ui/app.slint`.
- Produces: tests that fail until the Slint shell exposes the approved sidebar/settings primitives.

- [ ] **Step 1: Write failing contract test**

```rust
const APP_SLINT: &str = include_str!("../ui/app.slint");

#[test]
fn workbench_shell_contains_active_connections_sidebar_and_settings_modal() {
    for expected in [
        "active-connections",
        "settings-button",
        "settings-open",
        "Connection Settings",
        "Add Connection",
        "PostgreSQL",
        "SQLite",
    ] {
        assert!(
            APP_SLINT.contains(expected),
            "missing UI contract marker: {expected}"
        );
    }
}
```

- [ ] **Step 2: Run red test**

Run: `cargo test -p desktop --test ui_contract`
Expected: failure because the current Slint shell does not contain the new workbench markers.

### Task 2: Slint Workbench Shell

**Files:**
- Modify: `crates/desktop/ui/app.slint`
- Modify: `crates/desktop/src/main.rs`

**Interfaces:**
- Consumes: `query-text`, `status-text`, and `execute-query(string)`.
- Produces: `settings-open` state and a visual shell with sidebar, settings button, and centered modal.

- [ ] **Step 1: Replace the light shell with the dark workbench**

Implement fixed sidebar, active connection rows, bottom settings button, main query workspace, dimmed overlay, and centered settings modal.

- [ ] **Step 2: Run green test**

Run: `cargo test -p desktop --test ui_contract`
Expected: pass.

### Task 3: Verification

**Files:**
- Modify only files required by verification failures.

**Interfaces:**
- Consumes: complete workspace.
- Produces: formatted, lint-clean, tested desktop shell.

- [ ] **Step 1: Format**

Run: `cargo fmt --check`
Expected: pass.

- [ ] **Step 2: Lint**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: pass.

- [ ] **Step 3: Test**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 4: Build Desktop**

Run: `cargo build -p desktop`
Expected: pass.
