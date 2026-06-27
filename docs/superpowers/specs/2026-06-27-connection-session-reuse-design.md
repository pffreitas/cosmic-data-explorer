# Connection Session Reuse Design

## Goal

Keep an opened connection alive for one hour of inactivity, reuse it for schema loads, table previews, and SQL execution, and render preview/query results with the same native grid component used by the table explorer.

## Scope

This slice changes the native macOS shell and Rust native bridge only:

- Selecting an active connection in the sidebar opens or reuses that connection.
- Repeated table clicks and query runs reuse the same backend database session.
- Keychain password lookup happens only when a session is first opened or after it expires.
- Sessions expire after one hour with no schema, preview, query, or explicit open activity.
- Result panes use SwiftUI `Table`, matching the top table explorer list.

Manual disconnect controls, user-configurable TTL, connection-health badges, background keepalive pings, and row editing remain outside this slice.

## User Experience

When the app selects a connection, the backend establishes the connection behind the selected workspace. The existing table explorer auto-load then runs through that cached session. Selecting tables in the top pane updates the bottom pane without triggering another Keychain prompt while the session is active.

The bottom pane should look and behave like a native grid, not a text grid. Query results and table previews keep the current status line, empty-state behavior, selectable cell text, and horizontal/vertical scrolling, but the rows and columns are displayed using SwiftUI `Table`.

If the user returns after more than one hour of inactivity, the next schema load, table preview, SQL query, or sidebar selection may reopen the session and can prompt for Keychain access again.

## Backend Session Model

The native bridge owns a process-wide session cache keyed by connection id. Each cached entry stores:

- `DatabaseSession`: the SQLx-backed engine session.
- `last_activity`: the latest successful cache touch time.

All operations that need a database call first prune entries idle for more than `Duration::from_secs(3600)`. They then resolve the target connection id:

- If an unexpired session exists, update its `last_activity` and reuse it.
- If no session exists, resolve the profile, read its password from the credential store, bootstrap scratch data if needed, connect through `SqlxDatabaseConnector`, cache the session, and return it.

The existing schema, preview, and query bridge functions call this shared session resolver. A new explicit open bridge function uses the same resolver and returns a small success or failure envelope. Explicit open is not the only way to connect; operations still lazily reconnect after expiry so the UI remains resilient.

## Swift Integration

`NativeBridge` adds `openConnection(connectionID:)`. `ContentView` calls it when the selected sidebar connection changes, including the initially selected connection after app launch. The call is asynchronous and non-blocking; any failure is stored as the connection store's last error or otherwise surfaced without clearing the workspace.

Existing calls remain:

- `loadSchema(connectionID:)`
- `previewTable(connectionID:schema:table:maxRows:)`
- `executeQuery(connectionID:sql:maxRows:)`

Their Swift signatures do not need to change because session reuse is hidden behind the Rust bridge.

## Result Grid

Introduce a row model for query results, with a stable row id and a dictionary from column name to cell value. `QueryResultGrid` renders those rows with SwiftUI `Table`, creating one `TableColumn` per result column.

The component keeps:

- The status text above the grid.
- The empty "Statement completed." view for statements without columns.
- Text selection inside cells.
- Existing elapsed, affected-row, row-count, and truncated display.

## Error Handling

Open failures do not destroy tabs or table explorer state. Schema, preview, and query failures continue to render in the pane that initiated the operation. Cache poisoning is avoided by caching only successfully connected sessions.

If a cached session later fails because the server closed it, that failure is returned to the initiating operation. Automatic retry is outside this slice; the next operation can reconnect after the failed session is removed by error handling if needed.

## Testing

Rust tests cover the session cache with an in-memory credential store and test connector:

- Two operations on the same connection within the TTL read the password and connect once.
- A second operation after one hour of inactivity reconnects and reads the password again.
- Explicit open shares the same cache used by schema, preview, and query operations.

Swift tests cover contracts:

- `NativeBridge.openConnection(connectionID:)` encodes the expected JSON and decodes success/failure.
- `ContentView` contains the selected-connection task that calls `openConnection`.
- `QueryResultGrid` uses `Table` rather than SwiftUI `Grid`.

## Verification

Run:

```bash
cargo fmt --check
cargo test --workspace
swift test --package-path apps/macos
swift build --package-path apps/macos
```
