# Connection Session Reuse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reuse opened database sessions for one hour of inactivity and render query/table-preview results with the same native table component used by the table explorer.

**Architecture:** The Rust native bridge owns a process-wide session cache keyed by connection id. Swift opens the selected sidebar connection explicitly, while schema, preview, and query calls lazily reuse or recreate the same cached session. Query result rendering moves from SwiftUI `Grid` to SwiftUI `Table`.

**Tech Stack:** Rust 2021, SQLx, Tokio, Swift 6, SwiftUI, XCTest.

## Global Constraints

- Keep the session TTL at exactly `Duration::from_secs(3600)`.
- Do not add new crates or Swift package dependencies.
- Do not remove the existing JSON bridge entry points for schema, preview, or query.
- Keep existing table explorer, SQL tab, and result-state behavior intact.
- Work with the active uncommitted table-explorer changes already present in this checkout.

---

### Task 1: Rust Session Cache

**Files:**
- Modify: `crates/native-bridge/src/lib.rs`

**Interfaces:**
- Consumes: `ConnectionService<C, S>`, `DatabaseConnector::connect`, `CredentialStore::get_password`, `DatabaseSession`.
- Produces: `ConnectionSessionCache`, `session_for_connection`, `open_connection_async`, and `cosmic_open_connection_json`.

- [ ] **Step 1: Write the failing cache tests**

Add private unit tests in `crates/native-bridge/src/lib.rs`:

```rust
#[tokio::test]
async fn session_cache_reuses_open_session_within_ttl() {
    let fixture = SessionCacheFixture::new().await;
    let cache = std::sync::Mutex::new(ConnectionSessionCache::new(std::time::Duration::from_secs(3600)));
    let now = std::time::Instant::now();

    let first = session_for_connection(&fixture.profile.id, &fixture.service, &cache, now)
        .await
        .unwrap();
    let second = session_for_connection(
        &fixture.profile.id,
        &fixture.service,
        &cache,
        now + std::time::Duration::from_secs(30),
    )
    .await
    .unwrap();

    assert_eq!(first.profile_id, second.profile_id);
    assert_eq!(fixture.credentials.get_count(), 1);
}

#[tokio::test]
async fn session_cache_reopens_session_after_ttl() {
    let fixture = SessionCacheFixture::new().await;
    let cache = std::sync::Mutex::new(ConnectionSessionCache::new(std::time::Duration::from_secs(3600)));
    let now = std::time::Instant::now();

    let _ = session_for_connection(&fixture.profile.id, &fixture.service, &cache, now)
        .await
        .unwrap();
    let _ = session_for_connection(
        &fixture.profile.id,
        &fixture.service,
        &cache,
        now + std::time::Duration::from_secs(3601),
    )
    .await
    .unwrap();

    assert_eq!(fixture.credentials.get_count(), 2);
}
```

- [ ] **Step 2: Run the Rust red test**

Run: `cargo test -p cosmic-native-bridge session_cache --lib`

Expected: FAIL to compile because `ConnectionSessionCache` and `session_for_connection` do not exist yet.

- [ ] **Step 3: Implement the minimal cache**

Add:

```rust
const SESSION_IDLE_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug)]
struct CachedSession {
    session: DatabaseSession,
    last_activity: Instant,
}

#[derive(Debug)]
struct ConnectionSessionCache {
    ttl: Duration,
    sessions: HashMap<String, CachedSession>,
}
```

Implement `new`, `get`, `insert`, and `prune`, then add:

```rust
async fn session_for_connection<C, S>(
    connection_id: &str,
    service: &ConnectionService<C, S>,
    cache: &Mutex<ConnectionSessionCache>,
    now: Instant,
) -> cosmic_data_engine::Result<DatabaseSession>
where
    C: DatabaseConnector,
    S: CredentialStore,
{
    if let Some(session) = lock_session_cache(cache)?.get(connection_id, now) {
        return Ok(session);
    }

    let session = service.connect_session(connection_id).await?;
    Ok(lock_session_cache(cache)?.insert(connection_id.to_string(), session, now))
}
```

Add `ConnectionService::connect_session` so the cache can resolve the profile, read the password, bootstrap scratch data, and connect through the service connector.

- [ ] **Step 4: Run the Rust green test**

Run: `cargo test -p cosmic-native-bridge session_cache --lib`

Expected: PASS.

- [ ] **Step 5: Add the FFI open contract test**

Add to `crates/native-bridge/tests/ffi_contract.rs`:

```rust
#[test]
fn open_connection_json_returns_success_for_scratch() {
    let request = std::ffi::CString::new(r#"{"connectionId":"scratch"}"#).unwrap();

    let ptr = cosmic_native_bridge::cosmic_open_connection_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":true"#), "{json}");
}
```

- [ ] **Step 6: Run the FFI red test**

Run: `cargo test -p cosmic-native-bridge open_connection_json_returns_success_for_scratch`

Expected: FAIL because `cosmic_open_connection_json` is not exported.

- [ ] **Step 7: Implement the FFI open function**

Add `OpenConnectionInput`, `OpenConnectionEnvelope`, `OpenConnectionSuccess`, `OpenConnectionFailure`, `parse_open_connection_input`, `open_connection`, and `open_connection_async`. Export:

```rust
#[no_mangle]
pub extern "C" fn cosmic_open_connection_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_open_connection_input(input_json) {
        Ok(input) => open_connection(input),
        Err(message) => OpenConnectionEnvelope::Failure(OpenConnectionFailure { ok: false, message }),
    };

    json_to_c_string(&envelope)
}
```

Update schema, preview, and query async functions to call `session_for_connection` instead of resolving the profile, reading the password, and connecting independently.

- [ ] **Step 8: Run Rust tests**

Run: `cargo test -p cosmic-native-bridge`

Expected: PASS.

### Task 2: Swift Native Bridge Open API

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift`
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift`
- Modify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/NativeBridgeTests.swift`

**Interfaces:**
- Consumes: `cosmic_open_connection_json`.
- Produces: `ConnectionOpenEnvelope` and `NativeBridge.openConnection(connectionID:)`.

- [ ] **Step 1: Write the failing Swift bridge tests**

Add tests:

```swift
func testBridgeOpensConnection() throws {
    let bridge = NativeBridge(
        openConnectionJson: { requestJSON in
            XCTAssertTrue(requestJSON.contains(#""connectionId":"scratch""#))
            return #"{"ok":true}"#
        }
    )

    XCTAssertEqual(try bridge.openConnection(connectionID: "scratch"), .success)
}

func testBridgeDecodesOpenConnectionFailure() throws {
    let bridge = NativeBridge(
        openConnectionJson: { _ in
            #"{"ok":false,"message":"denied"}"#
        }
    )

    XCTAssertEqual(try bridge.openConnection(connectionID: "scratch"), .failure(message: "denied"))
}
```

- [ ] **Step 2: Run the Swift red tests**

Run: `swift test --package-path apps/macos --filter NativeBridgeTests/testBridgeOpensConnection`

Expected: FAIL to compile because `openConnectionJson`, `openConnection`, and `ConnectionOpenEnvelope` do not exist.

- [ ] **Step 3: Implement the Swift bridge API**

Add to `Models.swift`:

```swift
public enum ConnectionOpenEnvelope: Equatable, Sendable {
    case success
    case failure(message: String)
}
```

Add to `NativeBridge.swift`:

```swift
@_silgen_name("cosmic_open_connection_json")
private func cosmicOpenConnectionJson(_ inputJson: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
```

Wire an injectable `openConnectionJson` closure, encode `OpenConnectionRequest(connectionId:)`, decode `OpenConnectionResponse(ok:message:)`, and return `ConnectionOpenEnvelope`.

- [ ] **Step 4: Run the Swift green tests**

Run: `swift test --package-path apps/macos --filter NativeBridgeTests`

Expected: PASS.

### Task 3: Sidebar Open and Native Result Table

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionStore.swift`
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`
- Modify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`

**Interfaces:**
- Consumes: `NativeBridge.openConnection(connectionID:)`, `QueryResultColumn`, `QueryExecutionState`.
- Produces: selected-connection open task and `QueryResultGrid` backed by SwiftUI `Table`.

- [ ] **Step 1: Write failing ContentView contract assertions**

Add assertions:

```swift
XCTAssertTrue(source.contains(".task(id: store.selectedConnectionID)"))
XCTAssertTrue(source.contains("openSelectedConnection"))
XCTAssertTrue(source.contains("openConnection(connectionID:"))
XCTAssertTrue(source.contains("Table(resultRows)"))
XCTAssertFalse(source.contains("Grid(alignment: .leading"))
```

- [ ] **Step 2: Run the ContentView red test**

Run: `swift test --package-path apps/macos --filter ContentViewContractTests`

Expected: FAIL because the selected-connection task and `Table(resultRows)` are not present.

- [ ] **Step 3: Implement sidebar open**

Add a public error setter in `ConnectionStore`:

```swift
public func recordError(_ message: String?) {
    lastError = message
}
```

Attach a selected-connection task to `ContentView.body`:

```swift
.task(id: store.selectedConnectionID) {
    await openSelectedConnection()
}
```

Implement:

```swift
private func openSelectedConnection() async {
    guard let connectionID = store.selectedConnectionID else {
        return
    }

    do {
        let result = try await Task.detached {
            try bridge.openConnection(connectionID: connectionID)
        }.value
        if case let .failure(message) = result {
            store.recordError(message)
        }
    } catch {
        store.recordError(error.localizedDescription)
    }
}
```

- [ ] **Step 4: Implement `QueryResultGrid` with `Table`**

Add:

```swift
private struct QueryResultTableRow: Identifiable {
    let id: Int
    let cells: [String]

    func value(at index: Int) -> String {
        index < cells.count ? cells[index] : ""
    }
}
```

Replace the inner SwiftUI `Grid` with:

```swift
Table(resultRows) {
    ForEach(Array(columns.enumerated()), id: \.offset) { index, column in
        TableColumn(column.name) { row in
            Text(row.value(at: index))
                .textSelection(.enabled)
        }
    }
}
```

Keep the existing status and empty statement views.

- [ ] **Step 5: Run the Swift green tests**

Run: `swift test --package-path apps/macos --filter ContentViewContractTests`

Expected: PASS.

### Task 4: Full Verification

**Files:**
- Modify only files touched by Tasks 1-3 if verification exposes compile or formatting failures.

**Interfaces:**
- Consumes: all previous task outputs.
- Produces: formatted, passing Rust and Swift worktree.

- [ ] **Step 1: Format Rust**

Run: `cargo fmt`

Expected: command exits 0.

- [ ] **Step 2: Check Rust formatting**

Run: `cargo fmt --check`

Expected: PASS.

- [ ] **Step 3: Run Rust tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 4: Run Swift tests**

Run: `swift test --package-path apps/macos`

Expected: PASS.

- [ ] **Step 5: Build Swift app**

Run: `swift build --package-path apps/macos`

Expected: PASS.
