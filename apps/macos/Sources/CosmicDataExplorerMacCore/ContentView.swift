import SwiftUI

public struct ContentView: View {
    @StateObject private var store: ConnectionStore
    @StateObject private var workspaceStore = ConnectionWorkspaceStore()
    @State private var showingSettings = false
    private let bridge = NativeBridge()

    public init(store: ConnectionStore = ConnectionStore()) {
        _store = StateObject(wrappedValue: store)
    }

    public var body: some View {
        NavigationSplitView {
            sidebar
                .navigationTitle("Connections")
        } detail: {
            queryWorkspace
        }
        .frame(minWidth: 980, minHeight: 640)
        .sheet(isPresented: $showingSettings) {
            ConnectionSettingsView(connections: store.activeConnections)
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

            ScrollView([.horizontal, .vertical]) {
                if columns.isEmpty {
                    Text("Statement completed.")
                        .foregroundStyle(.secondary)
                        .padding()
                } else {
                    Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                        GridRow {
                            ForEach(columns) { column in
                                Text(column.name)
                                    .fontWeight(.semibold)
                            }
                        }

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
        }
        .padding()
    }

    private var statusText: String {
        "\(rows.count) rows, \(rowsAffected) affected, \(elapsedMs) ms\(truncated ? ", truncated" : "")"
    }
}
