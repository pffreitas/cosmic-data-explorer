import Foundation

public struct ActiveConnection: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let kind: String
    public let detail: String
    public let status: String
}

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
    case success(
        columns: [QueryResultColumn],
        rows: [[String]],
        rowsAffected: UInt64,
        elapsedMs: UInt64,
        truncated: Bool
    )
    case failure(message: String)
}

public enum QueryExecutionState: Equatable, Sendable {
    case empty
    case running(requestID: UUID)
    case success(
        columns: [QueryResultColumn],
        rows: [[String]],
        rowsAffected: UInt64,
        elapsedMs: UInt64,
        truncated: Bool
    )
    case failure(message: String)

    public var rowCount: Int {
        guard case let .success(_, rows, _, _, _) = self else {
            return 0
        }
        return rows.count
    }

    public var isRunning: Bool {
        if case .running = self {
            return true
        }
        return false
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

    public static func sql(title: String, sqlText: String = "select * from users limit 100;")
        -> WorkspaceTab
    {
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
        return ConnectionWorkspace(
            tabs: [tableExplorer],
            selectedTabID: tableExplorer.id,
            nextUntitledIndex: 1
        )
    }
}
