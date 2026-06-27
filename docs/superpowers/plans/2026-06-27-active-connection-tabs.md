# Active Connection Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-active-connection workspace tabs with a pinned table explorer and SQL tabs whose editor text and last result grid are isolated per tab.

**Architecture:** SwiftUI owns a `ConnectionWorkspaceStore` keyed by active connection id. SQL execution flows through `NativeBridge.executeQuery`, which calls a new Rust FFI function returning a structured JSON envelope backed by the engine for executable SQLite connections.

**Tech Stack:** Swift 6, SwiftUI, XCTest, Rust 2021, SQLx, Tokio, serde JSON, C FFI.

## Global Constraints

- Every active connection has a pinned first tab named `Table Explorer`.
- Users can add any number of SQL tabs for the selected active connection.
- Each SQL tab owns its editor text, execution status, and last result grid or error.
- Running a SQL tab calls the native bridge instead of using placeholder UI-only results.
- SQLite-backed active connections execute real SQL through the engine.
- Mock or unresolved network active connections return a structured failure.
- Full schema browsing, persistent query history, credential prompts, network connection setup, autocomplete, SQL formatting, and table editing remain outside this slice.

---

## File Structure

- Modify `crates/native-bridge/Cargo.toml`: add the Tokio dependency used by synchronous FFI to block on engine futures.
- Modify `crates/native-bridge/src/lib.rs`: add query request/response envelopes, FFI entry point, SQLite scratch profile resolver, result conversion, and Rust unit tests.
- Modify `crates/native-bridge/tests/ffi_contract.rs`: cover the exported JSON contract through the public C ABI.
- Modify `apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift`: add Swift query envelope models and workspace tab models.
- Create `apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionWorkspaceStore.swift`: manage per-connection tab state and result application.
- Modify `apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift`: bind the new query FFI function and decode the response envelope.
- Modify `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`: replace the single-query workspace with a tabbed workbench and per-tab execution.
- Create `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ConnectionWorkspaceStoreTests.swift`: cover workspace state behavior.
- Modify `apps/macos/Tests/CosmicDataExplorerMacCoreTests/NativeBridgeTests.swift`: cover Swift bridge query decoding.

---

### Task 1: Rust Native Bridge Query Contract

**Files:**
- Modify: `crates/native-bridge/Cargo.toml`
- Modify: `crates/native-bridge/src/lib.rs`
- Modify: `crates/native-bridge/tests/ffi_contract.rs`

**Interfaces:**
- Consumes: `cosmic_data_engine::{CellValue, ConnectionConfig, ConnectionProfile, DatabaseConnector, QueryRequest, QueryResult, SqlxDatabaseConnector}`
- Produces: `pub extern "C" fn cosmic_execute_query_json(input_json: *const c_char) -> *mut c_char`
- Produces JSON success: `{ "ok": true, "columns": [...], "rows": [[...]], "rowsAffected": 0, "elapsedMs": 3, "truncated": false }`
- Produces JSON failure: `{ "ok": false, "message": "..." }`

- [ ] **Step 1: Write failing FFI tests**

Add these tests to `crates/native-bridge/tests/ffi_contract.rs`:

```rust
#[test]
fn execute_query_json_returns_sqlite_rows() {
    let request = std::ffi::CString::new(
        r#"{"connectionId":"scratch","sql":"select id, name from users order by id","maxRows":100}"#,
    )
    .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains("Ada"), "{json}");
    assert!(json.contains("Grace"), "{json}");
}

#[test]
fn execute_query_json_returns_failure_for_empty_sql() {
    let request = std::ffi::CString::new(
        r#"{"connectionId":"scratch","sql":"   ","maxRows":100}"#,
    )
    .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains("SQL text is required"), "{json}");
}

#[test]
fn execute_query_json_returns_failure_for_unresolved_connections() {
    let request = std::ffi::CString::new(
        r#"{"connectionId":"production","sql":"select 1","maxRows":100}"#,
    )
    .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains("not available for query execution yet"), "{json}");
}
```

- [ ] **Step 2: Run the Rust bridge test and verify failure**

Run: `cargo test -p cosmic-native-bridge --test ffi_contract`

Expected: FAIL because `cosmic_execute_query_json` is not defined.

- [ ] **Step 3: Add Tokio dependency**

In `crates/native-bridge/Cargo.toml`, add:

```toml
tokio.workspace = true
```

- [ ] **Step 4: Implement the bridge contract**

In `crates/native-bridge/src/lib.rs`, add:

```rust
use std::{
    ffi::{c_char, CStr, CString},
    path::PathBuf,
    ptr,
};

use cosmic_data_engine::{
    CellValue, ConnectionProfile, DatabaseConnector, QueryRequest, QueryResult,
    SqlxDatabaseConnector,
};
use serde::{Deserialize, Serialize};
```

Define these request and response types:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteQueryInput {
    connection_id: String,
    sql: String,
    max_rows: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ExecuteQueryEnvelope {
    Success(ExecuteQuerySuccess),
    Failure(ExecuteQueryFailure),
}

#[derive(Debug, Serialize)]
struct ExecuteQuerySuccess {
    ok: bool,
    columns: Vec<QueryColumnOutput>,
    rows: Vec<Vec<String>>,
    rows_affected: u64,
    elapsed_ms: u128,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct QueryColumnOutput {
    name: String,
    #[serde(rename = "typeName")]
    type_name: String,
    nullable: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ExecuteQueryFailure {
    ok: bool,
    message: String,
}
```

Add the FFI entry point:

```rust
#[no_mangle]
pub extern "C" fn cosmic_execute_query_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_execute_query_input(input_json) {
        Ok(input) => execute_query(input),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    };

    json_to_c_string(&envelope)
}
```

Add these helpers:

```rust
fn parse_execute_query_input(input_json: *const c_char) -> std::result::Result<ExecuteQueryInput, String> {
    if input_json.is_null() {
        return Err("Query request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Query request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json).map_err(|error| format!("Invalid query request JSON: {error}"))
}

fn execute_query(input: ExecuteQueryInput) -> ExecuteQueryEnvelope {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
        .and_then(|runtime| runtime.block_on(execute_query_async(input)).map_err(|error| error.to_string()));

    match result {
        Ok(result) => ExecuteQueryEnvelope::Success(result_to_output(result)),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    }
}

async fn execute_query_async(input: ExecuteQueryInput) -> cosmic_data_engine::Result<QueryResult> {
    let profile = executable_profile(&input.connection_id)?;
    bootstrap_profile(&profile).await?;

    let connector = SqlxDatabaseConnector;
    let session = connector.connect(&profile, None).await?;
    session
        .execute_query(QueryRequest::new(
            input.connection_id,
            input.sql,
            input.max_rows.unwrap_or(100),
        ))
        .await
}

fn executable_profile(connection_id: &str) -> cosmic_data_engine::Result<ConnectionProfile> {
    match connection_id {
        "scratch" => Ok(ConnectionProfile::new_sqlite("Scratch", scratch_database_path())),
        other => Err(cosmic_data_engine::EngineError::Validation(format!(
            "Connection '{other}' is not available for query execution yet."
        ))),
    }
}

async fn bootstrap_profile(profile: &ConnectionProfile) -> cosmic_data_engine::Result<()> {
    if profile.display_name != "Scratch" {
        return Ok(());
    }

    let connector = SqlxDatabaseConnector;
    let session = connector.connect(profile, None).await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "create table if not exists users (id integer primary key, name text not null)",
            100,
        ))
        .await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "insert into users (id, name) values (1, 'Ada') on conflict(id) do nothing",
            100,
        ))
        .await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "insert into users (id, name) values (2, 'Grace') on conflict(id) do nothing",
            100,
        ))
        .await?;
    Ok(())
}

fn scratch_database_path() -> PathBuf {
    std::env::temp_dir()
        .join("cosmic-data-explorer")
        .join("scratch.sqlite")
}

fn result_to_output(result: QueryResult) -> ExecuteQuerySuccess {
    ExecuteQuerySuccess {
        ok: true,
        columns: result
            .columns
            .into_iter()
            .map(|column| QueryColumnOutput {
                name: column.name,
                type_name: column.type_name,
                nullable: column.nullable,
            })
            .collect(),
        rows: result
            .rows
            .into_iter()
            .map(|row| row.cells.into_iter().map(display_cell).collect())
            .collect(),
        rows_affected: result.rows_affected,
        elapsed_ms: result.elapsed_ms,
        truncated: result.truncated,
    }
}

fn display_cell(cell: CellValue) -> String {
    match cell {
        CellValue::Null => "NULL".to_string(),
        CellValue::Text(value) => value,
        CellValue::Integer(value) => value.to_string(),
        CellValue::Float(value) => value.to_string(),
        CellValue::Boolean(value) => value.to_string(),
        CellValue::Bytes(value) => format!("<{} bytes>", value.len()),
        CellValue::Date(value) => value.to_string(),
        CellValue::Time(value) => value.to_string(),
        CellValue::DateTime(value) => value.to_string(),
        CellValue::Timestamp(value) => value.to_rfc3339(),
        CellValue::Json(value) => value.to_string(),
    }
}

fn json_to_c_string<T: Serialize>(value: &T) -> *mut c_char {
    let Ok(json) = serde_json::to_string(value) else {
        return ptr::null_mut();
    };
    let Ok(c_string) = CString::new(json) else {
        return ptr::null_mut();
    };
    c_string.into_raw()
}
```

- [ ] **Step 5: Run Rust tests**

Run: `cargo test -p cosmic-native-bridge --test ffi_contract`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/native-bridge/Cargo.toml crates/native-bridge/src/lib.rs crates/native-bridge/tests/ffi_contract.rs
git commit -m "feat: add native query execution bridge"
```

---

### Task 2: Swift Workspace Tab State

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift`
- Create: `apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionWorkspaceStore.swift`
- Create: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ConnectionWorkspaceStoreTests.swift`

**Interfaces:**
- Produces: `WorkspaceTab`, `WorkspaceTabKind`, `QueryExecutionState`, `QueryResultEnvelope`, `ConnectionWorkspaceStore`
- Produces methods: `workspace(for:)`, `selectedTab(for:)`, `addSQLTab(for:)`, `closeTab(_:connectionID:)`, `updateSQL(_:tabID:connectionID:)`, `markRunning(tabID:connectionID:requestID:)`, `applyResult(_:tabID:connectionID:requestID:)`

- [ ] **Step 1: Write failing Swift state tests**

Create `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ConnectionWorkspaceStoreTests.swift`:

```swift
import XCTest
@testable import CosmicDataExplorerMacCore

@MainActor
final class ConnectionWorkspaceStoreTests: XCTestCase {
    func testWorkspaceStartsWithPinnedTableExplorer() {
        let store = ConnectionWorkspaceStore()

        let workspace = store.workspace(for: "scratch")

        XCTAssertEqual(workspace.tabs.count, 1)
        XCTAssertEqual(workspace.tabs[0].kind, .tableExplorer)
        XCTAssertTrue(workspace.tabs[0].isPinned)
        XCTAssertEqual(workspace.selectedTabID, workspace.tabs[0].id)
    }

    func testSQLTabsAreScopedPerConnection() {
        let store = ConnectionWorkspaceStore()

        let scratchTab = store.addSQLTab(for: "scratch")
        store.updateSQL("select * from users", tabID: scratchTab, connectionID: "scratch")
        let analyticsTab = store.addSQLTab(for: "analytics")
        store.updateSQL("select * from events", tabID: analyticsTab, connectionID: "analytics")

        XCTAssertEqual(store.workspace(for: "scratch").tabs.count, 2)
        XCTAssertEqual(store.workspace(for: "analytics").tabs.count, 2)
        XCTAssertEqual(store.tab(id: scratchTab, connectionID: "scratch")?.sqlText, "select * from users")
        XCTAssertEqual(store.tab(id: analyticsTab, connectionID: "analytics")?.sqlText, "select * from events")
    }

    func testResultApplicationIsPerTabAndIgnoresStaleRequests() {
        let store = ConnectionWorkspaceStore()
        let firstTab = store.addSQLTab(for: "scratch")
        let secondTab = store.addSQLTab(for: "scratch")
        let firstRequest = UUID()
        let staleRequest = UUID()

        store.markRunning(tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.markRunning(tabID: secondTab, connectionID: "scratch", requestID: staleRequest)
        store.markRunning(tabID: secondTab, connectionID: "scratch", requestID: UUID())

        let result = QueryResultEnvelope.success(
            columns: [QueryResultColumn(name: "name", typeName: "TEXT", nullable: nil)],
            rows: [["Ada"]],
            rowsAffected: 0,
            elapsedMs: 4,
            truncated: false
        )
        store.applyResult(result, tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.applyResult(result, tabID: secondTab, connectionID: "scratch", requestID: staleRequest)

        XCTAssertEqual(store.tab(id: firstTab, connectionID: "scratch")?.resultState.rowCount, 1)
        XCTAssertEqual(store.tab(id: secondTab, connectionID: "scratch")?.resultState.rowCount, 0)
    }

    func testClosingSelectedSQLTabFallsBackToTableExplorer() {
        let store = ConnectionWorkspaceStore()
        let sqlTab = store.addSQLTab(for: "scratch")

        store.closeTab(sqlTab, connectionID: "scratch")

        let workspace = store.workspace(for: "scratch")
        XCTAssertEqual(workspace.tabs.count, 1)
        XCTAssertEqual(workspace.tabs[0].kind, .tableExplorer)
        XCTAssertEqual(workspace.selectedTabID, workspace.tabs[0].id)
    }

    func testTableExplorerCannotBeClosed() {
        let store = ConnectionWorkspaceStore()
        let tableExplorer = store.workspace(for: "scratch").tabs[0].id

        store.closeTab(tableExplorer, connectionID: "scratch")

        XCTAssertEqual(store.workspace(for: "scratch").tabs.count, 1)
    }
}
```

- [ ] **Step 2: Run Swift state tests and verify failure**

Run: `swift test --package-path apps/macos --filter ConnectionWorkspaceStoreTests`

Expected: FAIL because `ConnectionWorkspaceStore` and workspace models do not exist.

- [ ] **Step 3: Add workspace and query models**

Append to `apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift`:

```swift
public struct QueryResultColumn: Codable, Equatable, Sendable, Identifiable {
    public var id: String { name }
    public let name: String
    public let typeName: String
    public let nullable: Bool?

    public init(name: String, typeName: String, nullable: Bool?) {
        self.name = name
        self.typeName = typeName
        self.nullable = nullable
    }
}

public enum QueryResultEnvelope: Equatable, Sendable {
    case success(columns: [QueryResultColumn], rows: [[String]], rowsAffected: UInt64, elapsedMs: UInt64, truncated: Bool)
    case failure(message: String)
}

public enum QueryExecutionState: Equatable, Sendable {
    case empty
    case running(requestID: UUID)
    case success(columns: [QueryResultColumn], rows: [[String]], rowsAffected: UInt64, elapsedMs: UInt64, truncated: Bool)
    case failure(message: String)

    public var rowCount: Int {
        guard case let .success(_, rows, _, _, _) = self else {
            return 0
        }
        return rows.count
    }
}

public enum WorkspaceTabKind: String, Equatable, Sendable {
    case tableExplorer
    case sql
}

public struct WorkspaceTab: Identifiable, Equatable, Sendable {
    public let id: UUID
    public let kind: WorkspaceTabKind
    public var title: String
    public var sqlText: String
    public var resultState: QueryExecutionState

    public var isPinned: Bool {
        kind == .tableExplorer
    }

    public static func tableExplorer() -> WorkspaceTab {
        WorkspaceTab(
            id: UUID(),
            kind: .tableExplorer,
            title: "Table Explorer",
            sqlText: "",
            resultState: .empty
        )
    }

    public static func sql(title: String, sqlText: String = "select * from users limit 100;") -> WorkspaceTab {
        WorkspaceTab(
            id: UUID(),
            kind: .sql,
            title: title,
            sqlText: sqlText,
            resultState: .empty
        )
    }
}

public struct ConnectionWorkspace: Equatable, Sendable {
    public var tabs: [WorkspaceTab]
    public var selectedTabID: UUID
    public var nextUntitledIndex: Int

    public static func initial() -> ConnectionWorkspace {
        let tableExplorer = WorkspaceTab.tableExplorer()
        return ConnectionWorkspace(tabs: [tableExplorer], selectedTabID: tableExplorer.id, nextUntitledIndex: 1)
    }
}
```

- [ ] **Step 4: Implement workspace store**

Create `apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionWorkspaceStore.swift`:

```swift
import Foundation
import SwiftUI

@MainActor
public final class ConnectionWorkspaceStore: ObservableObject {
    @Published private var workspaces: [ActiveConnection.ID: ConnectionWorkspace] = [:]

    public init() {}

    public func workspace(for connectionID: ActiveConnection.ID) -> ConnectionWorkspace {
        ensureWorkspace(for: connectionID)
    }

    public func selectedTab(for connectionID: ActiveConnection.ID) -> WorkspaceTab? {
        let workspace = ensureWorkspace(for: connectionID)
        return workspace.tabs.first { $0.id == workspace.selectedTabID }
    }

    public func tab(id tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID) -> WorkspaceTab? {
        let workspace = ensureWorkspace(for: connectionID)
        return workspace.tabs.first { $0.id == tabID }
    }

    @discardableResult
    public func addSQLTab(for connectionID: ActiveConnection.ID) -> WorkspaceTab.ID {
        var workspace = ensureWorkspace(for: connectionID)
        let tab = WorkspaceTab.sql(title: "Query \(workspace.nextUntitledIndex)")
        workspace.nextUntitledIndex += 1
        workspace.tabs.append(tab)
        workspace.selectedTabID = tab.id
        workspaces[connectionID] = workspace
        return tab.id
    }

    public func selectTab(_ tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID) {
        var workspace = ensureWorkspace(for: connectionID)
        guard workspace.tabs.contains(where: { $0.id == tabID }) else {
            return
        }
        workspace.selectedTabID = tabID
        workspaces[connectionID] = workspace
    }

    public func closeTab(_ tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID) {
        var workspace = ensureWorkspace(for: connectionID)
        guard let index = workspace.tabs.firstIndex(where: { $0.id == tabID }),
              !workspace.tabs[index].isPinned
        else {
            return
        }

        workspace.tabs.remove(at: index)
        if workspace.selectedTabID == tabID {
            let fallbackIndex = min(index, workspace.tabs.count - 1)
            workspace.selectedTabID = workspace.tabs[fallbackIndex].id
        }
        workspaces[connectionID] = workspace
    }

    public func updateSQL(_ sql: String, tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID) {
        updateTab(tabID, connectionID: connectionID) { tab in
            guard tab.kind == .sql else {
                return
            }
            tab.sqlText = sql
        }
    }

    public func markRunning(tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID, requestID: UUID) {
        updateTab(tabID, connectionID: connectionID) { tab in
            guard tab.kind == .sql else {
                return
            }
            tab.resultState = .running(requestID: requestID)
        }
    }

    public func applyResult(
        _ result: QueryResultEnvelope,
        tabID: WorkspaceTab.ID,
        connectionID: ActiveConnection.ID,
        requestID: UUID
    ) {
        updateTab(tabID, connectionID: connectionID) { tab in
            guard case let .running(activeRequestID) = tab.resultState,
                  activeRequestID == requestID
            else {
                return
            }

            switch result {
            case let .success(columns, rows, rowsAffected, elapsedMs, truncated):
                tab.resultState = .success(
                    columns: columns,
                    rows: rows,
                    rowsAffected: rowsAffected,
                    elapsedMs: elapsedMs,
                    truncated: truncated
                )
            case let .failure(message):
                tab.resultState = .failure(message: message)
            }
        }
    }

    private func ensureWorkspace(for connectionID: ActiveConnection.ID) -> ConnectionWorkspace {
        if let workspace = workspaces[connectionID] {
            return workspace
        }
        let workspace = ConnectionWorkspace.initial()
        workspaces[connectionID] = workspace
        return workspace
    }

    private func updateTab(
        _ tabID: WorkspaceTab.ID,
        connectionID: ActiveConnection.ID,
        update: (inout WorkspaceTab) -> Void
    ) {
        var workspace = ensureWorkspace(for: connectionID)
        guard let index = workspace.tabs.firstIndex(where: { $0.id == tabID }) else {
            return
        }
        update(&workspace.tabs[index])
        workspaces[connectionID] = workspace
    }
}
```

- [ ] **Step 5: Run Swift state tests**

Run: `swift test --package-path apps/macos --filter ConnectionWorkspaceStoreTests`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift apps/macos/Sources/CosmicDataExplorerMacCore/ConnectionWorkspaceStore.swift apps/macos/Tests/CosmicDataExplorerMacCoreTests/ConnectionWorkspaceStoreTests.swift
git commit -m "feat: add per-connection workspace tab state"
```

---

### Task 3: Swift Bridge Decoding

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift`
- Modify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/NativeBridgeTests.swift`

**Interfaces:**
- Consumes: `cosmic_execute_query_json`
- Produces: `NativeBridge.executeQuery(connectionID:sql:maxRows:) throws -> QueryResultEnvelope`

- [ ] **Step 1: Write failing Swift bridge tests**

Append to `NativeBridgeTests`:

```swift
func testBridgeExecutesSQLiteQuery() throws {
    let result = try NativeBridge().executeQuery(
        connectionID: "scratch",
        sql: "select id, name from users order by id",
        maxRows: 100
    )

    guard case let .success(columns, rows, _, _, _) = result else {
        return XCTFail("Expected successful query result")
    }

    XCTAssertEqual(columns.map(\.name), ["id", "name"])
    XCTAssertTrue(rows.contains(["1", "Ada"]))
}

func testBridgeDecodesQueryFailure() throws {
    let result = try NativeBridge().executeQuery(
        connectionID: "production",
        sql: "select 1",
        maxRows: 100
    )

    guard case let .failure(message) = result else {
        return XCTFail("Expected failure query result")
    }

    XCTAssertTrue(message.contains("not available for query execution yet"))
}
```

- [ ] **Step 2: Build bridge then run Swift bridge tests and verify failure**

Run: `cargo build -p cosmic-native-bridge`

Run: `swift test --package-path apps/macos --filter NativeBridgeTests`

Expected: FAIL because `NativeBridge.executeQuery` is not implemented.

- [ ] **Step 3: Implement Swift FFI binding and decoding**

In `apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift`, add:

```swift
@_silgen_name("cosmic_execute_query_json")
private func cosmicExecuteQueryJson(_ inputJson: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
```

Add private Codable DTOs:

```swift
private struct ExecuteQueryRequest: Encodable {
    let connectionId: String
    let sql: String
    let maxRows: UInt32
}

private struct ExecuteQueryResponse: Decodable {
    let ok: Bool
    let columns: [QueryResultColumn]?
    let rows: [[String]]?
    let rowsAffected: UInt64?
    let elapsedMs: UInt64?
    let truncated: Bool?
    let message: String?
}
```

Add this method:

```swift
public func executeQuery(connectionID: String, sql: String, maxRows: UInt32 = 100) throws -> QueryResultEnvelope {
    let request = ExecuteQueryRequest(connectionId: connectionID, sql: sql, maxRows: maxRows)
    let inputData = try JSONEncoder().encode(request)
    guard let inputJson = String(data: inputData, encoding: .utf8) else {
        throw NativeBridgeError.invalidUtf8
    }

    let pointer = inputJson.withCString { inputPointer in
        cosmicExecuteQueryJson(inputPointer)
    }

    guard let pointer else {
        throw NativeBridgeError.emptyResponse
    }
    defer {
        cosmicStringFree(pointer)
    }

    guard let json = String(validatingCString: pointer) else {
        throw NativeBridgeError.invalidUtf8
    }

    do {
        let response = try JSONDecoder().decode(ExecuteQueryResponse.self, from: Data(json.utf8))
        if response.ok {
            return .success(
                columns: response.columns ?? [],
                rows: response.rows ?? [],
                rowsAffected: response.rowsAffected ?? 0,
                elapsedMs: response.elapsedMs ?? 0,
                truncated: response.truncated ?? false
            )
        }
        return .failure(message: response.message ?? "Query execution failed")
    } catch {
        throw NativeBridgeError.decodeFailed(error.localizedDescription)
    }
}
```

- [ ] **Step 4: Run Swift bridge tests**

Run: `cargo build -p cosmic-native-bridge`

Run: `swift test --package-path apps/macos --filter NativeBridgeTests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/macos/Sources/CosmicDataExplorerMacCore/NativeBridge.swift apps/macos/Tests/CosmicDataExplorerMacCoreTests/NativeBridgeTests.swift
git commit -m "feat: decode query results in mac bridge"
```

---

### Task 4: SwiftUI Tabbed Workbench

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`

**Interfaces:**
- Consumes: `ConnectionWorkspaceStore`, `NativeBridge.executeQuery(connectionID:sql:maxRows:)`, `QueryExecutionState`
- Produces: right-hand UI with tab strip, pinned table explorer tab, SQL editor tabs, run button, and per-tab results.

- [ ] **Step 1: Add workspace store to ContentView**

In `ContentView`, add:

```swift
@StateObject private var workspaceStore = ConnectionWorkspaceStore()
private let bridge = NativeBridge()
```

- [ ] **Step 2: Replace `queryWorkspace` with a tab-aware workspace**

Implement the right-hand workspace around the selected connection id:

```swift
private var queryWorkspace: some View {
    Group {
        if let connection = store.selectedConnection {
            connectionWorkspace(connection)
        } else {
            ContentUnavailableView(
                "No Connection",
                systemImage: "externaldrive.badge.questionmark",
                description: Text("Open Settings to add a connection")
            )
        }
    }
}
```

- [ ] **Step 3: Add tab strip and scoped table explorer tab**

Add helper views:

```swift
private func connectionWorkspace(_ connection: ActiveConnection) -> some View {
    let workspace = workspaceStore.workspace(for: connection.id)
    let selectedTab = workspace.tabs.first { $0.id == workspace.selectedTabID } ?? workspace.tabs[0]

    return VStack(spacing: 0) {
        workspaceHeader(connection)
        Divider()
        tabStrip(workspace: workspace, connectionID: connection.id)
        Divider()
        if selectedTab.kind == .tableExplorer {
            tableExplorerView(connection)
        } else {
            sqlTabView(selectedTab, connection: connection)
        }
    }
}

private func workspaceHeader(_ connection: ActiveConnection) -> some View {
    HStack(spacing: 12) {
        VStack(alignment: .leading, spacing: 2) {
            Text(connection.name)
                .font(.headline)
            Text(connection.detail)
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        Spacer()
    }
    .padding()
}

private func tabStrip(workspace: ConnectionWorkspace, connectionID: ActiveConnection.ID) -> some View {
    ScrollView(.horizontal, showsIndicators: false) {
        HStack(spacing: 6) {
            ForEach(workspace.tabs) { tab in
                Button {
                    workspaceStore.selectTab(tab.id, connectionID: connectionID)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: tab.kind == .tableExplorer ? "tablecells" : "doc.text")
                        Text(tab.title)
                        if !tab.isPinned {
                            Image(systemName: "xmark")
                                .imageScale(.small)
                                .onTapGesture {
                                    workspaceStore.closeTab(tab.id, connectionID: connectionID)
                                }
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(tab.id == workspace.selectedTabID ? Color.accentColor.opacity(0.18) : Color.clear)
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
            }

            Button {
                workspaceStore.addSQLTab(for: connectionID)
            } label: {
                Image(systemName: "plus")
                    .padding(6)
            }
            .buttonStyle(.plain)
            .help("New SQL Tab")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }
}

private func tableExplorerView(_ connection: ActiveConnection) -> some View {
    VStack(alignment: .leading, spacing: 8) {
        Text("Table Explorer")
            .font(.headline)
        Text("\(connection.name) schema browsing is outside this slice.")
            .foregroundStyle(.secondary)
        Spacer()
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    .padding()
}
```

- [ ] **Step 4: Add SQL editor and per-tab results**

Add helper views:

```swift
private func sqlTabView(_ tab: WorkspaceTab, connection: ActiveConnection) -> some View {
    VStack(spacing: 0) {
        HStack {
            Text(tab.title)
                .font(.headline)
            Spacer()
            Button {
                runQuery(tabID: tab.id, connectionID: connection.id)
            } label: {
                Label("Run", systemImage: "play.fill")
            }
            .keyboardShortcut(.return, modifiers: .command)
            .disabled(tab.resultState.isRunning)
        }
        .padding()

        TextEditor(
            text: Binding(
                get: { workspaceStore.tab(id: tab.id, connectionID: connection.id)?.sqlText ?? "" },
                set: { workspaceStore.updateSQL($0, tabID: tab.id, connectionID: connection.id) }
            )
        )
        .font(.system(.body, design: .monospaced))
        .padding(12)
        .frame(minHeight: 220)

        Divider()

        resultView(tab.resultState)
    }
}
```

Add `isRunning` to `QueryExecutionState` in `Models.swift`:

```swift
public var isRunning: Bool {
    if case .running = self {
        return true
    }
    return false
}
```

Add result grid rendering:

```swift
private func resultView(_ state: QueryExecutionState) -> some View {
    switch state {
    case .empty:
        return AnyView(Text("Run a SQL statement to see results.").foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity))
    case .running:
        return AnyView(ProgressView("Running...").frame(maxWidth: .infinity, maxHeight: .infinity))
    case let .failure(message):
        return AnyView(Text(message).foregroundStyle(.red).frame(maxWidth: .infinity, maxHeight: .infinity))
    case let .success(columns, rows, rowsAffected, elapsedMs, truncated):
        return AnyView(QueryResultGrid(columns: columns, rows: rows, rowsAffected: rowsAffected, elapsedMs: elapsedMs, truncated: truncated))
    }
}

private struct QueryResultGrid: View {
    let columns: [QueryResultColumn]
    let rows: [[String]]
    let rowsAffected: UInt64
    let elapsedMs: UInt64
    let truncated: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\(rows.count) rows, \(rowsAffected) affected, \(elapsedMs) ms\(truncated ? ", truncated" : "")")
                .font(.caption)
                .foregroundStyle(.secondary)

            ScrollView([.horizontal, .vertical]) {
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                    GridRow {
                        ForEach(columns) { column in
                            Text(column.name)
                                .fontWeight(.semibold)
                        }
                    }
                    Divider()
                    ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                        GridRow {
                            ForEach(Array(columns.enumerated()), id: \.offset) { index, _ in
                                Text(index < row.count ? row[index] : "")
                                    .textSelection(.enabled)
                            }
                        }
                    }
                }
                .padding()
            }
        }
        .padding()
    }
}
```

- [ ] **Step 5: Implement async run**

Add:

```swift
private func runQuery(tabID: WorkspaceTab.ID, connectionID: ActiveConnection.ID) {
    guard let tab = workspaceStore.tab(id: tabID, connectionID: connectionID),
          tab.kind == .sql
    else {
        return
    }

    let sql = tab.sqlText
    let requestID = UUID()
    workspaceStore.markRunning(tabID: tabID, connectionID: connectionID, requestID: requestID)

    Task {
        let result: QueryResultEnvelope
        do {
            result = try await Task.detached {
                try bridge.executeQuery(connectionID: connectionID, sql: sql, maxRows: 100)
            }.value
        } catch {
            result = .failure(message: error.localizedDescription)
        }

        workspaceStore.applyResult(result, tabID: tabID, connectionID: connectionID, requestID: requestID)
    }
}
```

- [ ] **Step 6: Build Swift app**

Run: `cargo build -p cosmic-native-bridge`

Run: `swift build --package-path apps/macos`

Expected: PASS.

- [ ] **Step 7: Run full local checks**

Run:

```bash
cargo fmt --check
cargo test --workspace
swift test --package-path apps/macos
swift build --package-path apps/macos
```

Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift apps/macos/Sources/CosmicDataExplorerMacCore/Models.swift
git commit -m "feat: add tabbed SQL workbench"
```
