# Full Height Row Detail Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the row detail inspector into a floating trailing panel that spans the full selected connection workspace height.

**Architecture:** `QueryResultGrid` keeps row selection and row lookup, but emits a `RowDetailSelection` through callbacks instead of rendering the inspector inline. `connectionWorkspace` owns the floating panel presentation and overlays `RowDetailInspector` across the whole right workspace. A workspace-owned clear token lets the panel close action also reset the grid's selected row, so selecting the same row can reopen the panel.

**Tech Stack:** Swift 6, SwiftUI `Table`, SwiftUI overlay layout, XCTest source-contract tests, macOS 14 package target.

## Global Constraints

- The details panel opens when a result row is selected in SQL query results or table explorer previews.
- The panel spans the full height of the right workspace, including the connection header, tab strip, editor/table explorer area, and result area.
- The panel remains aligned to the trailing edge and floats above the workspace content.
- The existing close behavior, selected-row values, metadata display, and text selection remain intact.
- The panel continues to reset when a new accepted result identity replaces the current result.
- No row editing, detached windows, persistence, keyboard shortcuts, drag resizing, or backend changes are included.
- The current worktree is intentionally dirty on `main`; do not stage or commit implementation changes unless explicitly requested.

---

## File Structure

- Modify `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`: update source-contract assertions from split-pane inspector behavior to workspace-level floating overlay behavior.
- Modify `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`: add `RowDetailSelection`, move inspector rendering to `connectionWorkspace`, wire callbacks from `QueryResultGrid`, and remove `HSplitView`.

---

### Task 1: Contract Test For Workspace-Level Floating Panel

**Files:**
- Modify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`

**Interfaces:**
- Consumes: existing source-contract test.
- Produces: failing assertions for workspace overlay, callback-based row inspection, and no inline `HSplitView`.

- [ ] **Step 1: Update contract assertions**

In `ContentViewContractTests.swift`, replace the row-inspector assertions from `@State private var inspectedRow` through `row.value(at: index)` with:

```swift
        XCTAssertTrue(source.contains("@State private var rowDetailSelection: RowDetailSelection?"))
        XCTAssertTrue(source.contains("@State private var rowDetailClearID = UUID()"))
        XCTAssertTrue(source.contains(".overlay(alignment: .trailing)"))
        XCTAssertTrue(source.contains("workspaceRowDetailPanel"))
        XCTAssertTrue(source.contains("visibleRowDetailSelection"))
        XCTAssertTrue(source.contains("RowDetailSelection("))
        XCTAssertTrue(source.contains("let onInspectRow: (RowDetailSelection) -> Void"))
        XCTAssertTrue(source.contains("let onCloseInspector: () -> Void"))
        XCTAssertTrue(source.contains("let clearSelectionID: UUID"))
        XCTAssertTrue(source.contains("clearSelectionID: rowDetailClearID"))
        XCTAssertTrue(source.contains(".onChange(of: clearSelectionID) { _, _ in"))
        XCTAssertTrue(source.contains("onInspectRow: { selection in"))
        XCTAssertTrue(source.contains("onCloseInspector: clearRowDetailSelection"))
        XCTAssertTrue(source.contains("private func visibleResultID("))
        XCTAssertTrue(source.contains(".id(resultID)"))
        XCTAssertTrue(source.contains("clearSelection"))
        XCTAssertTrue(source.contains("selectedRowID = nil"))
        XCTAssertTrue(source.contains("rowDetailClearID = UUID()"))
        XCTAssertTrue(source.contains("Close Details"))
        XCTAssertTrue(source.contains("row.value(at: index)"))
        XCTAssertFalse(source.contains("HSplitView"))
```

- [ ] **Step 2: Run focused test and verify red**

Run:

```bash
swift test --package-path apps/macos --filter ContentViewContractTests
```

Expected: FAIL because `ContentView.swift` still renders the inspector through `HSplitView` inside `QueryResultGrid`.

---

### Task 2: Move Inspector To Workspace Overlay

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`

**Interfaces:**
- Consumes: `QueryResultGrid(resultID:columns:rows:rowsAffected:elapsedMs:truncated:)`
- Produces: `RowDetailSelection`
- Produces: `QueryResultGrid(..., onInspectRow:onCloseInspector:)`
- Produces: `workspaceRowDetailPanel(_:)`
- Produces: `visibleResultID(for:connectionID:)`

- [ ] **Step 1: Add workspace-level selection state**

Add this state next to the existing `@State` properties in `ContentView`:

```swift
    @State private var rowDetailSelection: RowDetailSelection?
```

- [ ] **Step 2: Wrap connection workspace in a trailing overlay**

Update `connectionWorkspace(_:)` so the existing `VStack` is wrapped in a `ZStack(alignment: .trailing)` with this overlay:

```swift
        return ZStack(alignment: .trailing) {
            VStack(spacing: 0) {
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
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .overlay(alignment: .trailing) {
            if let selection = visibleRowDetailSelection(
                for: selectedTab,
                connectionID: connection.id
            ) {
                workspaceRowDetailPanel(selection)
            }
        }
```

- [ ] **Step 3: Pass callbacks into `QueryResultGrid`**

Update the `QueryResultGrid` construction in `resultView`:

```swift
            QueryResultGrid(
                resultID: resultID,
                columns: columns,
                rows: rows,
                rowsAffected: rowsAffected,
                elapsedMs: elapsedMs,
                truncated: truncated,
                clearSelectionID: rowDetailClearID,
                onInspectRow: { selection in
                    rowDetailSelection = selection
                },
                onCloseInspector: clearRowDetailSelection
            )
            .id(resultID)
```

- [ ] **Step 4: Add workspace helper methods**

Add these methods inside `ContentView`:

```swift
    private func visibleRowDetailSelection(
        for selectedTab: WorkspaceTab,
        connectionID: ActiveConnection.ID
    ) -> RowDetailSelection? {
        guard let rowDetailSelection,
            rowDetailSelection.resultID == visibleResultID(
                for: selectedTab,
                connectionID: connectionID
            )
        else {
            return nil
        }
        return rowDetailSelection
    }

    private func visibleResultID(
        for selectedTab: WorkspaceTab,
        connectionID: ActiveConnection.ID
    ) -> UUID? {
        if selectedTab.kind == .tableExplorer {
            guard case let .success(resultID, _, _, _, _, _) = workspaceStore
                .tableExplorer(for: connectionID)
                .previewState
            else {
                return nil
            }
            return resultID
        }

        guard case let .success(resultID, _, _, _, _, _) = selectedTab.resultState else {
            return nil
        }
        return resultID
    }

    @ViewBuilder
    private func workspaceRowDetailPanel(_ selection: RowDetailSelection) -> some View {
        RowDetailInspector(
            columns: selection.columns,
            row: selection.row,
            onClose: clearRowDetailSelection
        )
        .frame(width: 340)
        .frame(maxHeight: .infinity)
        .background(.regularMaterial)
        .overlay(alignment: .leading) {
            Divider()
        }
        .shadow(color: Color.black.opacity(0.18), radius: 14, x: -4, y: 0)
        .zIndex(1)
    }

    private func clearRowDetailSelection() {
        rowDetailSelection = nil
        rowDetailClearID = UUID()
    }
```

- [ ] **Step 5: Update `QueryResultGrid` API**

Add callback and clear-token properties:

```swift
    let clearSelectionID: UUID
    let onInspectRow: (RowDetailSelection) -> Void
    let onCloseInspector: () -> Void
```

Remove `inspectedRow`, `isInspectorVisible`, and `resultContent`. In `body`, render `resultTable` directly for non-empty result sets.

In `showInspector(for:)`, replace local inspector state mutation with:

```swift
        onInspectRow(RowDetailSelection(resultID: resultID, columns: columns, row: selectedRow))
```

In the unresolved-row guard, call `clearSelection()`.

Attach this reset to the grid body so workspace-level close actions clear table selection too:

```swift
        .onChange(of: clearSelectionID) { _, _ in
            selectedRowID = nil
        }
```

- [ ] **Step 6: Add `RowDetailSelection` value**

Add this private struct above `RowDetailInspector`:

```swift
private struct RowDetailSelection {
    let resultID: UUID
    let columns: [QueryResultColumn]
    let row: QueryResultTableRow
}
```

- [ ] **Step 7: Update `clearSelection`**

Update `QueryResultGrid.clearSelection()`:

```swift
    private func clearSelection() {
        selectedRowID = nil
        onCloseInspector()
    }
```

- [ ] **Step 8: Run focused test and verify green**

Run:

```bash
swift test --package-path apps/macos --filter ContentViewContractTests
```

Expected: PASS.

---

### Task 3: Verify Full Swift Package

**Files:**
- Verify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`
- Verify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`

**Interfaces:**
- Consumes: workspace-level floating panel from Task 2.
- Produces: verified test and build results.

- [ ] **Step 1: Run full Swift tests**

Run:

```bash
swift test --package-path apps/macos
```

Expected: all tests pass.

- [ ] **Step 2: Run Swift build**

Run:

```bash
swift build --package-path apps/macos
```

Expected: build succeeds.
