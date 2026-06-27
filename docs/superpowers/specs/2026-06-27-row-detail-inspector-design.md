# Row Detail Inspector Design

## Goal

Add an on-demand right-side details pane for query result rows. When a user clicks a row in either SQL query results or table explorer previews, the app shows that row's fields in a right-hand inspector similar to database clients such as DataGrip or TablePlus.

## Scope

This slice covers result inspection only:

- Row selection in successful result grids.
- A details pane that opens after a row is selected.
- A close action that hides the pane until another row is selected.
- Shared behavior for SQL query mode and table explorer previews.
- Text-selectable field values for copying.

Row editing, JSON formatting, binary previews, persisted selected rows, keyboard navigation, and detached inspector windows are outside this slice.

## User Experience

Successful result grids continue to show the status line and table. Initially, only the grid is visible. When a user selects a row, the result area becomes a horizontal split layout:

- Left: the existing result table.
- Right: a details pane for the selected row.

The details pane has a compact header with a title and close button. Its content is a vertical list of fields. Each field shows the column name, a small type/nullability label when available, and the selected row value. Values are selectable and scrollable so long text remains inspectable without expanding the table row height.

Closing the pane clears the current row selection. Selecting another row reopens or updates the pane. Running a new query or loading a different table preview creates a new `QueryResultGrid`, so no stale row details remain attached to old results.

## Architecture

The implementation should keep this behavior inside the shared result grid component.

`ContentView.resultView` already routes both SQL query results and table explorer previews through `QueryResultGrid`. Updating that component provides the feature in both places without adding duplicate state to `WorkspaceTab` or `TableExplorerState`.

`QueryResultGrid` owns local SwiftUI state:

- `selectedRowID: QueryResultTableRow.ID?`
- `isInspectorVisible: Bool`

The table binds its selection to `selectedRowID`. When selection becomes non-nil, `isInspectorVisible` becomes true. The close button clears both values.

The selected row is resolved from the existing `resultRows` computed property. The row detail model can be derived directly from `columns` and the selected row's cell values, so no backend or bridge changes are needed.

## Components

`QueryResultGrid` remains the public result display component and gains:

- a selectable result table.
- a split result content area.
- a private row details pane view.

`QueryResultTableRow` remains the row wrapper around result cells. It can continue using the row offset as its stable id because result rows are immutable for one query result render.

## Data Flow

1. SQL execution or table preview produces `QueryExecutionState.success`.
2. `resultView` constructs `QueryResultGrid(columns:rows:...)`.
3. `QueryResultGrid` maps raw rows to `QueryResultTableRow`.
4. User selects a table row.
5. `QueryResultGrid` stores the selected row id and shows the inspector.
6. Inspector renders each column/value pair from the selected row.
7. User closes the inspector, clearing local selection.

## Error Handling

The inspector only exists for successful result sets with columns. Empty, running, failure, and statement-completed states keep their current behavior.

If a selected row id cannot be resolved, the pane should not render. If a row has fewer values than columns, missing cells display as an empty string, matching the existing table behavior.

## Testing

Swift contract tests should assert the new result-grid behavior is present in source:

- `QueryResultGrid` includes table selection binding.
- `QueryResultGrid` includes a row details pane.
- The close action clears row selection.
- Existing dynamic/fallback table paths remain available.

This repository's current UI tests are source-contract style, so the implementation should extend those tests rather than adding brittle visual tests for this slice.

## Verification

Run:

```bash
swift test --package-path apps/macos
swift build --package-path apps/macos
```

