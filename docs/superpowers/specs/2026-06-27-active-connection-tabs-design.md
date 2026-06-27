# Active Connection Tabs Design

## Goal

Add a tabbed workbench to the right-hand side of the macOS app. Each active connection owns its own workspace tabs, so switching connections restores that connection's table explorer, SQL tabs, editor text, selected tab, and last results.

## Scope

This slice implements the tab workspace and a thin real query execution path:

- Every active connection has a pinned first tab named `Table Explorer`.
- Users can add any number of SQL tabs for the selected active connection.
- Each SQL tab owns its editor text, execution status, and last result grid or error.
- Running a SQL tab calls the native bridge instead of using placeholder UI-only results.
- The bridge returns structured JSON for query success or query failure.
- Connections that cannot yet be resolved to an executable engine session return a clear error in the tab that ran the query.

Full schema browsing, persistent query history, credential prompts, network connection setup, autocomplete, SQL formatting, and table editing remain outside this slice.

## User Experience

The right-hand workspace has a compact tab strip below the selected connection header. The pinned table explorer is always the first tab and cannot be closed. SQL tabs have editable query text, a run button, and a results panel below the editor.

The add-tab control creates a new SQL tab for the currently selected active connection and selects it immediately. Closing a SQL tab removes only that tab. If the active SQL tab is closed, selection moves to the nearest remaining tab, falling back to the table explorer.

Switching active connections swaps the entire tab workspace. Returning to a connection shows the same selected tab and result state it had before.

## State Model

SwiftUI owns a `ConnectionWorkspaceStore` keyed by active connection id. Each connection workspace contains:

- `tabs`: ordered list of workspace tabs.
- `selectedTabID`: current tab for that connection.
- `nextUntitledIndex`: counter for default SQL tab titles.

A workspace tab is either:

- `tableExplorer`: pinned, non-closeable, first in the list.
- `sql`: closeable, with title, SQL text, run state, and last result state.

The SQL tab result state is explicit:

- `empty`: the query has not run.
- `running`: an execution is in progress for this tab.
- `success`: columns, rows, elapsed time, rows affected, and truncation flag.
- `failure`: bridge or engine error message.

Execution results are applied only if the tab still exists and still matches the request id that started the run. This prevents stale async responses from overwriting a newer run on the same tab.

## Native Bridge Contract

Swift calls a new bridge function with JSON input:

```json
{
  "connectionId": "scratch",
  "sql": "select * from users limit 100",
  "maxRows": 100
}
```

The bridge returns one JSON envelope:

```json
{
  "ok": true,
  "columns": [{ "name": "id", "typeName": "INTEGER", "nullable": null }],
  "rows": [["1"]],
  "rowsAffected": 0,
  "elapsedMs": 3,
  "truncated": false
}
```

or:

```json
{
  "ok": false,
  "message": "Connection 'production' is not available for query execution yet."
}
```

The bridge is responsible for converting engine cell values into display strings for the Swift table. That keeps the Swift UI simple and avoids duplicating database type formatting in the app shell.

## Execution Strategy

The Rust engine already has `DatabaseSession::execute_query`. The native bridge adds a small execution service that resolves an active connection id into an executable profile/session.

For this slice:

- SQLite-backed active connections should execute real SQL through the engine.
- Mock or unresolved network active connections should return a structured failure.
- Errors should be reported in the initiating tab's result panel, not through a global alert.

This gives the UI a real end-to-end path without requiring the full connection manager and credential workflow in the same change.

## Error Handling

Empty SQL returns a validation error in the tab results area. Bridge decode failures, engine validation errors, SQL errors, timeouts, and unresolved connection ids all become `failure` result states on the initiating SQL tab.

The app should keep the editor text and previous tabs intact after failures. Running one tab must not clear or modify another tab's results.

## Testing

Swift tests cover the workspace model:

- new workspaces start with a pinned table explorer tab.
- adding SQL tabs scopes them to the selected connection.
- SQL tabs preserve editor text and last results independently.
- closing a selected SQL tab selects an adjacent tab or the table explorer.
- table explorer cannot be closed.

Native bridge tests cover query JSON shape:

- SQLite execution returns a successful result envelope.
- empty SQL returns a failure envelope.
- unresolved connection ids return a failure envelope.

Rust tests cover the bridge/service conversion from engine `QueryResult` to display JSON where practical.

## Verification

Run these checks before considering the implementation complete:

```bash
cargo fmt --check
cargo test --workspace
swift test --package-path apps/macos
swift build --package-path apps/macos
```
