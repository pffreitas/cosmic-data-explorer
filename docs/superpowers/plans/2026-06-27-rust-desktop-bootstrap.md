# Cosmic Data Explorer Rust Desktop Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bootstrap a GPL Rust desktop database explorer with a Slint shell and a tested engine for profiles, credentials, persistence, SQL highlighting, and SQLx-backed database access.

**Architecture:** Use a Cargo workspace with a thin `desktop` crate and a behavior-heavy `engine` crate. Keep connector implementations concrete by database kind while sharing typed request/result/domain models.

**Tech Stack:** Rust 2021, Slint, SQLx, Tokio, keyring, directories, syntect, serde, chrono, uuid, cargo-packager.

## Global Constraints

- License expression is `GPL-3.0-or-later`.
- V1 platform QA is macOS Tahoe 26, Sequoia 15, and Sonoma 14.
- V1 scope is browse + query only.
- Passwords must never be persisted in connection profile metadata.
- SQLite integration tests must run locally without Docker.
- PostgreSQL and MySQL/MariaDB tests are Docker-backed and optional when Docker is unavailable.
- Required local checks are `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo run -p desktop`.

---

### Task 1: Workspace And Engine Domain

**Files:**
- Create: `Cargo.toml`
- Create: `crates/engine/Cargo.toml`
- Create: `crates/engine/src/lib.rs`
- Create: `crates/engine/src/domain.rs`
- Create: `crates/engine/tests/domain.rs`

**Interfaces:**
- Produces: `DatabaseKind`, `ConnectionProfile`, `ConnectionConfig`, `CredentialRef`, `QueryRequest`, `QueryResult`, `CellValue`, and validation helpers.

- [ ] **Step 1: Write failing tests for profile validation and credential references**

```rust
use cosmic_data_engine::{ConnectionConfig, ConnectionProfile, DatabaseKind, SslMode};

#[test]
fn sqlite_profile_requires_a_file_path() {
    let profile = ConnectionProfile::new_sqlite("Local", "");
    assert!(profile.validate().is_err());
}

#[test]
fn network_profile_generates_a_stable_credential_reference_without_password() {
    let profile = ConnectionProfile::new_network(
        "Warehouse",
        DatabaseKind::Postgres,
        "localhost",
        5432,
        "analytics",
        "paulo",
        SslMode::Preferred,
    );

    let credential = profile.credential_ref();

    assert_eq!(credential.service, "cosmic-data-explorer");
    assert!(credential.account.contains("warehouse"));
    assert!(credential.account.contains("paulo"));
    assert!(!format!("{profile:?}").contains("password"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cosmic-data-engine --test domain`
Expected: failure because `cosmic_data_engine` does not yet expose the requested types.

- [ ] **Step 3: Implement minimal domain types**

Create the workspace manifests and domain module with deterministic profile IDs, typed configs, validation errors, and credential reference generation.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cosmic-data-engine --test domain`
Expected: both domain tests pass.

### Task 2: Credentials, Storage, And Highlighting

**Files:**
- Create: `crates/engine/src/credentials.rs`
- Create: `crates/engine/src/storage.rs`
- Create: `crates/engine/src/highlight.rs`
- Create: `crates/engine/tests/credentials_storage_highlight.rs`

**Interfaces:**
- Consumes: `ConnectionProfile`, `CredentialRef`, `DatabaseKind`.
- Produces: `CredentialStore`, `InMemoryCredentialStore`, `KeyringCredentialStore`, `AppStorage`, `HighlightService`, and `HighlightedDocument`.

- [ ] **Step 1: Write failing tests for secrets, storage round-trip, and highlighting**

```rust
use cosmic_data_engine::{AppStorage, HighlightService, InMemoryCredentialStore};

#[tokio::test]
async fn profile_metadata_round_trips_without_password() {
    let temp = tempfile::tempdir().unwrap();
    let storage = AppStorage::connect(temp.path().join("app.sqlite")).await.unwrap();
    storage.initialize().await.unwrap();

    let profile = cosmic_data_engine::ConnectionProfile::new_sqlite("Local", "data.sqlite");
    storage.save_profile(&profile).await.unwrap();

    let profiles = storage.list_profiles().await.unwrap();
    assert_eq!(profiles, vec![profile]);
}

#[test]
fn in_memory_credentials_store_and_delete_passwords() {
    let profile = cosmic_data_engine::ConnectionProfile::new_sqlite("Local", "data.sqlite");
    let store = InMemoryCredentialStore::default();
    let credential = profile.credential_ref();

    store.set_password(&credential, "secret").unwrap();
    assert_eq!(store.get_password(&credential).unwrap(), Some("secret".to_string()));
    store.delete_password(&credential).unwrap();
    assert_eq!(store.get_password(&credential).unwrap(), None);
}

#[test]
fn sql_highlighter_preserves_source_text() {
    let doc = HighlightService::default().highlight_sql("select * from users", cosmic_data_engine::DatabaseKind::Postgres).unwrap();
    assert_eq!(doc.plain_text(), "select * from users");
    assert!(!doc.lines[0].spans.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cosmic-data-engine --test credentials_storage_highlight`
Expected: failure because the storage, credential, and highlight services do not yet exist.

- [ ] **Step 3: Implement minimal services**

Implement `CredentialStore` with an in-memory test implementation and a macOS keyring implementation, `AppStorage` tables for profiles and query history, and `HighlightService` backed by syntect defaults.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cosmic-data-engine --test credentials_storage_highlight`
Expected: all tests pass.

### Task 3: SQLx Connector

**Files:**
- Create: `crates/engine/src/database.rs`
- Create: `crates/engine/tests/sqlite_connector.rs`

**Interfaces:**
- Consumes: `ConnectionProfile`, `QueryRequest`, and `QueryResult`.
- Produces: `DatabaseConnector`, `SqlxDatabaseConnector`, `DatabaseSession`, `DatabaseSchema`, `SchemaTable`, `ColumnInfo`.

- [ ] **Step 1: Write failing SQLite integration tests**

```rust
use cosmic_data_engine::{ConnectionProfile, DatabaseConnector, QueryRequest, SqlxDatabaseConnector};

#[tokio::test]
async fn sqlite_connector_executes_queries_and_previews_tables() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("sample.sqlite");
    let profile = ConnectionProfile::new_sqlite("Local", db.to_string_lossy());
    let connector = SqlxDatabaseConnector::default();
    let session = connector.connect(&profile, None).await.unwrap();

    session.execute_query(QueryRequest::new(profile.id.clone(), "create table users (id integer primary key, name text)", 100)).await.unwrap();
    session.execute_query(QueryRequest::new(profile.id.clone(), "insert into users (name) values ('Ada')", 100)).await.unwrap();

    let result = session.execute_query(QueryRequest::new(profile.id.clone(), "select id, name from users", 100)).await.unwrap();
    assert_eq!(result.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["id", "name"]);
    assert_eq!(result.rows.len(), 1);

    let preview = session.preview_table(None, "users", 50).await.unwrap();
    assert_eq!(preview.rows.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cosmic-data-engine --test sqlite_connector`
Expected: failure because `SqlxDatabaseConnector` does not exist.

- [ ] **Step 3: Implement SQLx connector**

Implement concrete SQLx pools for SQLite, PostgreSQL, and MySQL/MariaDB. Add SQLite schema introspection and table preview with quoted identifiers and max-row caps.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cosmic-data-engine --test sqlite_connector`
Expected: SQLite integration test passes.

### Task 4: Slint Desktop Shell And Packaging

**Files:**
- Create: `crates/desktop/Cargo.toml`
- Create: `crates/desktop/build.rs`
- Create: `crates/desktop/src/main.rs`
- Create: `crates/desktop/ui/app.slint`
- Create: `.gitignore`
- Create: `README.md`

**Interfaces:**
- Consumes: `cosmic_data_engine` public API.
- Produces: runnable `desktop` binary with Slint shell and cargo-packager metadata.

- [ ] **Step 1: Add a minimal Slint shell**

Create a main window with a connection sidebar, query editor, execute button callback, result area, and status text. Keep behavior thin and route state through Rust callbacks.

- [ ] **Step 2: Run desktop build**

Run: `cargo check -p desktop`
Expected: desktop crate compiles.

### Task 5: Full Verification

**Files:**
- Modify only files required by failures found during verification.

**Interfaces:**
- Consumes: complete workspace.
- Produces: formatted, lint-clean, tested bootstrap.

- [ ] **Step 1: Format check**

Run: `cargo fmt --check`
Expected: no formatting diffs.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings or errors.

- [ ] **Step 3: Tests**

Run: `cargo test --workspace`
Expected: all workspace tests pass.

- [ ] **Step 4: macOS smoke test**

Run: `cargo run -p desktop`
Expected: Slint window opens and execute button updates status text.
