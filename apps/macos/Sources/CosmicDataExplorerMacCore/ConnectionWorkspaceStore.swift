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

    public func updateSQL(
        _ sql: String,
        tabID: WorkspaceTab.ID,
        connectionID: ActiveConnection.ID
    ) {
        updateTab(tabID, connectionID: connectionID) { tab in
            guard tab.kind == .sql else {
                return
            }
            tab.sqlText = sql
        }
    }

    public func markRunning(
        tabID: WorkspaceTab.ID,
        connectionID: ActiveConnection.ID,
        requestID: UUID
    ) {
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
