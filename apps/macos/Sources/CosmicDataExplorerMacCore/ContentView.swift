import SwiftUI

public struct ContentView: View {
    @StateObject private var store: ConnectionStore
    @StateObject private var workspaceStore = ConnectionWorkspaceStore()
    @State private var showingSettings = false
    @State private var showingNewConnection = false
    @State private var openConnectionTasks: [ActiveConnection.ID: Task<ConnectionOpenEnvelope, Error>] = [:]
    private let bridge = NativeBridge()

    public init(store: ConnectionStore = ConnectionStore()) {
        _store = StateObject(wrappedValue: store)
    }

    public var body: some View {
        NavigationSplitView {
            sidebar
                .navigationTitle("Connections")
                .toolbar {
                    ToolbarItem(placement: .primaryAction) {
                        Button {
                            showingNewConnection = true
                        } label: {
                            Image(systemName: "plus")
                        }
                        .help("New Connection")
                    }
                }
        } detail: {
            queryWorkspace
        }
        .frame(minWidth: 980, minHeight: 640)
        .sheet(isPresented: $showingSettings) {
            ConnectionSettingsView(connections: store.activeConnections)
        }
        .sheet(isPresented: $showingNewConnection) {
            NewConnectionView { name, connectionString in
                try await store.createConnection(name: name, connectionString: connectionString)
            }
        }
        .task {
            store.load()
        }
        .task(id: store.selectedConnectionID) {
            await openSelectedConnection()
        }
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            List(selection: $store.selectedConnectionID) {
                Section("Active") {
                    ForEach(store.activeConnections) { connection in
                        ConnectionSidebarRow(connection: connection)
                            .tag(connection.id as String?)
                    }
                }
            }
            .listStyle(.sidebar)

            Divider()

            Button {
                showingSettings = true
            } label: {
                Label("Settings", systemImage: "gearshape")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .padding(10)
        }
    }

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

    private func tabStrip(
        workspace: ConnectionWorkspace,
        connectionID: ActiveConnection.ID
    ) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(workspace.tabs) { tab in
                    HStack(spacing: 4) {
                        Button {
                            workspaceStore.selectTab(tab.id, connectionID: connectionID)
                        } label: {
                            Label(
                                tab.title,
                                systemImage: tab.kind == .tableExplorer ? "tablecells" : "doc.text"
                            )
                            .labelStyle(.titleAndIcon)
                        }
                        .buttonStyle(.plain)

                        if !tab.isPinned {
                            Button {
                                workspaceStore.closeTab(tab.id, connectionID: connectionID)
                            } label: {
                                Image(systemName: "xmark")
                                    .imageScale(.small)
                            }
                            .buttonStyle(.plain)
                            .help("Close Tab")
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(
                        tab.id == workspace.selectedTabID
                            ? Color.accentColor.opacity(0.18)
                            : Color.clear
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 6))
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
        let explorer = workspaceStore.tableExplorer(for: connection.id)

        return VStack(spacing: 0) {
            HStack {
                Text("Table Explorer")
                    .font(.headline)
                Spacer()
                Button {
                    loadTableSchema(connectionID: connection.id)
                } label: {
                    Label("Reload", systemImage: "arrow.clockwise")
                }
                .disabled(explorer.schemaState.isRunning)
            }
            .padding()

            Divider()

            VSplitView {
                tableListView(explorer: explorer, connectionID: connection.id)
                    .frame(minHeight: 180)

                tablePreviewView(explorer.previewState)
                    .frame(minHeight: 220)
            }
        }
        .task(id: connection.id) {
            _ = await openConnectionIfNeeded(connectionID: connection.id)
            if case .empty = workspaceStore.tableExplorer(for: connection.id).schemaState {
                loadTableSchema(connectionID: connection.id)
            }
        }
    }

    @ViewBuilder
    private func tableListView(
        explorer: TableExplorerState,
        connectionID: ActiveConnection.ID
    ) -> some View {
        switch explorer.schemaState {
        case .empty:
            Text("Load schema to browse tables.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .running:
            ProgressView("Loading tables...")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case let .failure(message):
            Text(message)
                .foregroundStyle(.red)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
        case let .success(tables):
            if tables.isEmpty {
                Text("No tables found.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(tables, selection: Binding<SchemaTableSummary.ID?>(
                        get: {
                            workspaceStore.tableExplorer(for: connectionID).selectedTableID
                        },
                        set: { selectedTableID in
                            guard let selectedTableID,
                                let table = workspaceStore
                                    .tableExplorer(for: connectionID)
                                    .tables
                                    .first(where: { $0.id == selectedTableID })
                            else {
                                return
                            }
                            previewSelectedTable(table, connectionID: connectionID)
                        }
                    )
                ) {
                    TableColumn("name", value: \.name)
                    TableColumn("schema") { table in
                        Text(table.schema ?? "--")
                    }
                    TableColumn("kind", value: \.kind)
                    TableColumn("columns") { table in
                        Text("\(table.columnCount)")
                            .monospacedDigit()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func tablePreviewView(_ state: QueryExecutionState) -> some View {
        switch state {
        case .empty:
            Text("Select a table to preview the first 50 rows.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        default:
            resultView(state)
        }
    }

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
                    get: {
                        workspaceStore.tab(id: tab.id, connectionID: connection.id)?.sqlText ?? ""
                    },
                    set: { sql in
                        workspaceStore.updateSQL(sql, tabID: tab.id, connectionID: connection.id)
                    }
                )
            )
            .font(.system(.body, design: .monospaced))
            .padding(12)
            .frame(minHeight: 220)

            Divider()

            resultView(tab.resultState)
        }
    }

    @ViewBuilder
    private func resultView(_ state: QueryExecutionState) -> some View {
        switch state {
        case .empty:
            Text("Run a SQL statement to see results.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .running:
            ProgressView("Running...")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case let .failure(message):
            Text(message)
                .foregroundStyle(.red)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
        case let .success(columns, rows, rowsAffected, elapsedMs, truncated):
            QueryResultGrid(
                columns: columns,
                rows: rows,
                rowsAffected: rowsAffected,
                elapsedMs: elapsedMs,
                truncated: truncated
            )
        }
    }

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

            await MainActor.run {
                workspaceStore.applyResult(
                    result,
                    tabID: tabID,
                    connectionID: connectionID,
                    requestID: requestID
                )
            }
        }
    }

    @MainActor
    private func openSelectedConnection() async {
        guard let connectionID = store.selectedConnectionID else {
            return
        }

        _ = await openConnectionIfNeeded(connectionID: connectionID)
    }

    @MainActor
    @discardableResult
    private func openConnectionIfNeeded(
        connectionID: ActiveConnection.ID
    ) async -> ConnectionOpenEnvelope? {
        if let task = openConnectionTasks[connectionID] {
            return await applyOpenConnectionResult(task)
        }

        let bridge = bridge
        let task = Task.detached {
            try bridge.openConnection(connectionID: connectionID)
        }
        openConnectionTasks[connectionID] = task
        defer {
            openConnectionTasks[connectionID] = nil
        }

        return await applyOpenConnectionResult(task)
    }

    @MainActor
    private func applyOpenConnectionResult(
        _ task: Task<ConnectionOpenEnvelope, Error>
    ) async -> ConnectionOpenEnvelope? {
        do {
            let result = try await task.value
            switch result {
            case .success:
                store.recordError(nil)
            case let .failure(message):
                store.recordError(message)
            }
            return result
        } catch {
            store.recordError(error.localizedDescription)
            return nil
        }
    }

    private func loadTableSchema(connectionID: ActiveConnection.ID) {
        let requestID = UUID()
        let hadSelection = workspaceStore.tableExplorer(for: connectionID).selectedTableID != nil
        workspaceStore.markSchemaLoading(connectionID: connectionID, requestID: requestID)

        Task {
            let result: SchemaLoadEnvelope
            do {
                result = try await Task.detached {
                    try bridge.loadSchema(connectionID: connectionID)
                }.value
            } catch {
                result = .failure(message: error.localizedDescription)
            }

            await MainActor.run {
                workspaceStore.applySchemaResult(
                    result,
                    connectionID: connectionID,
                    requestID: requestID
                )

                if case let .success(tables) = result, !hadSelection, let firstTable = tables.first {
                    previewSelectedTable(firstTable, connectionID: connectionID)
                }
            }
        }
    }

    private func previewSelectedTable(
        _ table: SchemaTableSummary,
        connectionID: ActiveConnection.ID
    ) {
        let requestID = UUID()
        guard workspaceStore.selectTable(table, connectionID: connectionID) else {
            return
        }
        workspaceStore.markPreviewRunning(
            tableID: table.id,
            connectionID: connectionID,
            requestID: requestID
        )

        Task {
            let result: QueryResultEnvelope
            do {
                result = try await Task.detached {
                    try bridge.previewTable(
                        connectionID: connectionID,
                        schema: table.schema,
                        table: table.name,
                        maxRows: 50
                    )
                }.value
            } catch {
                result = .failure(message: error.localizedDescription)
            }

            await MainActor.run {
                workspaceStore.applyPreviewResult(
                    result,
                    tableID: table.id,
                    connectionID: connectionID,
                    requestID: requestID
                )
            }
        }
    }
}

private struct NewConnectionView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var connectionString = ""
    @State private var errorMessage: String?
    @State private var isCreating = false

    let onCreate: @MainActor (String, String) async throws -> ActiveConnection

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("New Connection")
                    .font(.title2.bold())

                Spacer()

                Button("Cancel") {
                    dismiss()
                }
                .disabled(isCreating)

                Button {
                    createConnection()
                } label: {
                    if isCreating {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Text("Create")
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canCreate)
            }
            .padding()

            Divider()

            Form {
                TextField("Name", text: $name)
                Section("Connection String") {
                    TextEditor(text: $connectionString)
                        .font(.system(.body, design: .monospaced))
                        .frame(minHeight: 120)
                }

                if let errorMessage {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }
            }
            .formStyle(.grouped)
            .padding()
        }
        .frame(minWidth: 560, minHeight: 360)
    }

    private var canCreate: Bool {
        !isCreating
            && !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !connectionString.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func createConnection() {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedConnectionString = connectionString.trimmingCharacters(in: .whitespacesAndNewlines)

        Task { @MainActor in
            isCreating = true
            errorMessage = nil
            defer {
                isCreating = false
            }

            do {
                _ = try await onCreate(trimmedName, trimmedConnectionString)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

private struct ConnectionSidebarRow: View {
    let connection: ActiveConnection

    var body: some View {
        Label {
            VStack(alignment: .leading, spacing: 2) {
                Text(connection.name)
                Text("\(connection.kind) / \(connection.detail)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        } icon: {
            Image(systemName: "externaldrive.connected.to.line.below")
        }
        .help("\(connection.kind) - \(connection.status)")
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
            Text(statusText)
                .font(.caption)
                .foregroundStyle(.secondary)

            if columns.isEmpty {
                Text("Statement completed.")
                    .foregroundStyle(.secondary)
                    .padding()
            } else {
                resultTable
                    .textSelection(.enabled)
            }
        }
        .padding()
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
        Table(resultRows) {
            TableColumnForEach(Array(columns.enumerated()), id: \.offset) { index, column in
                TableColumn(column.name) { row in
                    Text(row.value(at: index))
                }
            }
        }
    }

    @ViewBuilder
    private var fallbackResultTable: some View {
        switch columns.count {
        case 1:
            Table(resultRows) {
                resultColumn(0)
            }
        case 2:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
            }
        case 3:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
            }
        case 4:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
            }
        case 5:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
            }
        case 6:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
            }
        case 7:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
            }
        case 8:
            Table(resultRows) {
                resultColumn(0)
                resultColumn(1)
                resultColumn(2)
                resultColumn(3)
                resultColumn(4)
                resultColumn(5)
                resultColumn(6)
                resultColumn(7)
            }
        case 9:
            Table(resultRows) {
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
        case 10:
            Table(resultRows) {
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
        default:
            Table(resultRows) {
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

    private var statusText: String {
        "\(rows.count) rows, \(rowsAffected) affected, \(elapsedMs) ms\(truncated ? ", truncated" : "")"
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
