# Row Detail Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an on-demand right-side inspector that shows the selected result row's column values in both SQL query results and table explorer previews.

**Architecture:** Keep the behavior inside `QueryResultGrid`, the shared SwiftUI result component already used by both query mode and table explorer preview mode. The grid owns local row-selection state, opens an `HSplitView` inspector after row selection, and clears selection when the user closes the inspector.

**Tech Stack:** Swift 6, SwiftUI `Table`, SwiftUI `HSplitView`, XCTest source-contract tests, macOS 14 package target.

## Global Constraints

- The inspector opens only after a user selects a row.
- The inspector remains visible until the close button clears the selected row.
- The implementation is shared by SQL query results and table explorer previews through `QueryResultGrid`.
- The inspector renders only for successful result sets with columns.
- Field values are text-selectable for copying.
- No backend, bridge, persistence, row editing, JSON formatting, binary preview, keyboard navigation, or detached-window work is included.
- The current worktree already has unrelated uncommitted changes in `ContentView.swift` and `ContentViewContractTests.swift`; do not revert them and do not create a commit that bundles them with this feature.

---

## File Structure

- Modify `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`: extend the existing source-contract test so it fails until row selection, inspector rendering, and close behavior are present.
- Modify `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`: update `QueryResultGrid`, add the result split layout, and add private row-detail views next to the existing `QueryResultTableRow`.

---

### Task 1: Add Row Inspector Contract Assertions

**Files:**
- Modify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`

**Interfaces:**
- Consumes: existing `ContentViewContractTests.testContentViewContainsTabbedWorkbenchContract()`
- Produces: source-contract assertions for `selectedRowID`, `isInspectorVisible`, `RowDetailInspector`, `HSplitView`, table selection binding, and the close action

- [ ] **Step 1: Add failing assertions to the content-view contract test**

Insert these assertions after the existing `XCTAssertTrue(source.contains("QueryResultGrid"))` line:

```swift
        XCTAssertTrue(source.contains("@State private var selectedRowID: QueryResultTableRow.ID?"))
        XCTAssertTrue(source.contains("@State private var isInspectorVisible = false"))
        XCTAssertTrue(source.contains("Table(resultRows, selection: $selectedRowID)"))
        XCTAssertTrue(source.contains("HSplitView"))
        XCTAssertTrue(source.contains("RowDetailInspector"))
        XCTAssertTrue(source.contains("clearSelection"))
        XCTAssertTrue(source.contains("selectedRowID = nil"))
        XCTAssertTrue(source.contains("isInspectorVisible = false"))
        XCTAssertTrue(source.contains("Close Details"))
        XCTAssertTrue(source.contains("row.value(at: index)"))
```

- [ ] **Step 2: Run the focused Swift test and verify failure**

Run:

```bash
swift test --package-path apps/macos --filter ContentViewContractTests
```

Expected result: the test fails because the new row inspector strings are not present in `ContentView.swift`.

---

### Task 2: Implement Selectable Result Grid And Inspector

**Files:**
- Modify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`

**Interfaces:**
- Consumes: `QueryResultGrid(columns:rows:rowsAffected:elapsedMs:truncated:)`
- Consumes: `QueryResultColumn.name`, `QueryResultColumn.typeName`, `QueryResultColumn.nullable`
- Consumes: `QueryResultTableRow.value(at:)`
- Produces: `RowDetailInspector`
- Produces: `RowDetailField`
- Produces: `QueryResultGrid.clearSelection()`

- [ ] **Step 1: Replace the existing `QueryResultGrid` and following row helper block**

In `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`, replace the file content from `private struct QueryResultGrid: View {` through the closing brace of `private struct QueryResultTableRow: Identifiable` with this code:

```swift
private struct QueryResultGrid: View {
    let columns: [QueryResultColumn]
    let rows: [[String]]
    let rowsAffected: UInt64
    let elapsedMs: UInt64
    let truncated: Bool

    @State private var selectedRowID: QueryResultTableRow.ID?
    @State private var isInspectorVisible = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(statusText)
                .font(.caption)
                .foregroundStyle(.secondary)

            if columns.isEmpty {
                Text("Statement completed.")
                    .foregroundStyle(.secondary)
                    .padding()
            } else {
                resultContent
                    .textSelection(.enabled)
            }
        }
        .padding()
    }

    @ViewBuilder
    private var resultContent: some View {
        if isInspectorVisible, let selectedRow = selectedRow {
            HSplitView {
                resultTable
                    .frame(minWidth: 420)

                RowDetailInspector(
                    columns: columns,
                    row: selectedRow,
                    onClose: clearSelection
                )
                .frame(minWidth: 260, idealWidth: 320, maxWidth: 460)
            }
        } else {
            resultTable
        }
    }

    @ViewBuilder
    private var resultTable: some View {
        if #available(macOS 14.4, *) {
            dynamicResultTable
        } else {
            fallbackResultTable
        }
    }

    @available(macOS 14.4, *)
    private var dynamicResultTable: some View {
        Table(resultRows, selection: $selectedRowID) {
            TableColumnForEach(Array(columns.enumerated()), id: \.offset) { index, column in
                TableColumn(column.name) { row in
                    Text(row.value(at: index))
                }
            }
        }
        .onChange(of: selectedRowID) { _, newValue in
            if newValue != nil {
                isInspectorVisible = true
            }
        }
    }

    @ViewBuilder
    private var fallbackResultTable: some View {
        switch columns.count {
        case 1:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 2:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 3:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 4:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 5:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 6:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 7:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 8:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
                resultColumn(7)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 9:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
                resultColumn(7)
                resultColumn(8)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        case 10:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
                resultColumn(7)
                resultColumn(8)
                resultColumn(9)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        default:
            Table(resultRows, selection: $selectedRowID) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
                resultColumn(7)
                resultColumn(8)
                overflowColumn(from: 9)
            }
            .onChange(of: selectedRowID) { _, newValue in
                if newValue != nil {
                    isInspectorVisible = true
                }
            }
        }
    }

    private func resultColumn(_ index: Int) -> TableColumn<QueryResultTableRow, Never, Text, Text> {
        TableColumn(columns[index].name) { row in
            Text(row.value(at: index))
        }
    }

    private func overflowColumn(from index: Int) -> TableColumn<QueryResultTableRow, Never, Text, Text> {
        TableColumn("more") { row in
            Text(row.values(from: index).joined(separator: " | "))
        }
    }

    private var resultRows: [QueryResultTableRow] {
        rows.enumerated().map { offset, cells in
            QueryResultTableRow(id: offset, cells: cells)
        }
    }

    private var selectedRow: QueryResultTableRow? {
        guard let selectedRowID else {
            return nil
        }
        return resultRows.first { $0.id == selectedRowID }
    }

    private var statusText: String {
        "\(rows.count) rows, \(rowsAffected) affected, \(elapsedMs) ms\(truncated ? ", truncated" : "")"
    }

    private func clearSelection() {
        selectedRowID = nil
        isInspectorVisible = false
    }
}

private struct RowDetailInspector: View {
    let columns: [QueryResultColumn]
    let row: QueryResultTableRow
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Details")
                    .font(.headline)

                Spacer()

                Button {
                    onClose()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.borderless)
                .help("Close Details")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)

            Divider()

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(Array(columns.enumerated()), id: \.offset) { index, column in
                        RowDetailField(
                            column: column,
                            value: row.value(at: index)
                        )
                    }
                }
                .padding(12)
            }
        }
    }
}

private struct RowDetailField: View {
    let column: QueryResultColumn
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(column.name)
                    .font(.caption)
                    .fontWeight(.semibold)

                if !metadataText.isEmpty {
                    Text(metadataText)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }

            Text(value.isEmpty ? " " : value)
                .font(.system(.body, design: .monospaced))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(Color.secondary.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .textSelection(.enabled)
        }
    }

    private var metadataText: String {
        let nullableText = column.nullable.map { $0 ? "nullable" : "not null" }
        return [column.typeName, nullableText]
            .compactMap { $0 }
            .filter { !$0.isEmpty }
            .joined(separator: " / ")
    }
}

private struct QueryResultTableRow: Identifiable {
    let id: Int
    let cells: [String]

    func value(at index: Int) -> String {
        index < cells.count ? cells[index] : ""
    }

    func values(from index: Int) -> [String] {
        guard index < cells.count else {
            return []
        }
        return Array(cells[index...])
    }
}
```

- [ ] **Step 2: Run the focused Swift test and verify pass**

Run:

```bash
swift test --package-path apps/macos --filter ContentViewContractTests
```

Expected result: `ContentViewContractTests` passes.

---

### Task 3: Verify Build And Full Swift Tests

**Files:**
- Verify: `apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift`
- Verify: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift`

**Interfaces:**
- Consumes: `QueryResultGrid` row-selection implementation from Task 2
- Produces: verified Swift package test and build results

- [ ] **Step 1: Run the full Swift test suite**

Run:

```bash
swift test --package-path apps/macos
```

Expected result: all Swift tests pass.

- [ ] **Step 2: Run the Swift build**

Run:

```bash
swift build --package-path apps/macos
```

Expected result: the macOS package builds successfully.

- [ ] **Step 3: Inspect the final diff**

Run:

```bash
git diff -- apps/macos/Sources/CosmicDataExplorerMacCore/ContentView.swift apps/macos/Tests/CosmicDataExplorerMacCoreTests/ContentViewContractTests.swift
```

Expected result: the diff contains the row-detail inspector implementation and test assertions, while preserving the pre-existing navigation split-view width changes already present in the worktree.

- [ ] **Step 4: Leave commit creation to the user or a later clean staging pass**

Do not run `git add` or `git commit` for this implementation while unrelated pre-existing changes remain in the same modified files. Report the verified files and test results instead.
