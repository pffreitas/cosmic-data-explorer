# Full Height Row Detail Panel Design

## Goal

Change the row detail inspector from a result-grid split pane into a floating right-side panel that spans the full height of the selected connection workspace.

## Scope

This slice changes layout and ownership only:

- The details panel opens when a result row is selected in SQL query results or table explorer previews.
- The panel spans the full height of the right workspace, including the connection header, tab strip, editor/table explorer area, and result area.
- The panel remains aligned to the trailing edge and floats above the workspace content.
- The existing close behavior, selected-row values, metadata display, and text selection remain intact.
- The panel continues to reset when a new accepted result identity replaces the current result.

No row editing, detached windows, persistence, keyboard shortcuts, drag resizing, or backend changes are included.

## User Experience

When no row is selected, the workspace looks unchanged. Selecting a result row opens a fixed-width right-side floating panel that overlays the selected connection workspace from top to bottom.

The panel keeps the current details header, close button, field list, metadata labels, selectable values, and scrolling field content. The underlying result grid remains visible behind or beside the panel, depending on available width, but does not shrink into a split view.

Closing the panel clears the current detail selection. Selecting another row opens or updates the same panel. Switching result identities, such as rerunning a query or loading another table preview, clears stale panel state.

## Architecture

Move the inspector rendering out of `QueryResultGrid` and up to the selected connection workspace level.

`QueryResultGrid` keeps the table selection mechanics because it owns the result rows and knows how to map row ids to values. Instead of rendering `RowDetailInspector` directly, it reports selection changes through callbacks:

- `onInspectRow(RowDetailSelection)`
- `onCloseInspector()`

The selected connection workspace owns local inspector presentation state:

- selected row details, including columns and row values.
- panel visibility.
- close behavior.

The workspace uses an overlay aligned to `.trailing` so the floating panel spans the entire workspace height. The overlay should not be implemented as a sheet or popover, because it needs to feel like a database-client inspector attached to the workbench.

## Components

`RowDetailSelection` is a lightweight value containing:

- `resultID`
- `columns`
- selected `QueryResultTableRow`

`QueryResultGrid`:

- remains the shared result table for SQL results and table previews.
- binds SwiftUI table selection.
- calls `onInspectRow` when a row is selected.
- calls `onCloseInspector` when a selected row cannot be resolved for the current result.

`RowDetailInspector`:

- remains a private SwiftUI view for the details content.
- is rendered by the workspace-level floating panel, not inside `QueryResultGrid`.

## Error Handling

The panel only opens for successful result sets with columns. Empty, running, failure, and statement-completed states keep their current behavior.

If a selected row id cannot be resolved, the grid clears the inspector instead of preserving stale details.

## Testing

Extend the existing Swift source-contract tests to assert:

- `connectionWorkspace` uses an overlay aligned to the trailing edge.
- `QueryResultGrid` exposes row-inspection callbacks instead of rendering `HSplitView`.
- `RowDetailInspector` is rendered at workspace level.
- stale details are cleared through result identity and unresolved-row handling.

Keep the existing workspace-store identity tests because they protect stale result resets.

## Verification

Run:

```bash
swift test --package-path apps/macos
swift build --package-path apps/macos
```

