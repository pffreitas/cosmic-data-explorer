import Foundation

public struct ActiveConnection: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let kind: String
    public let detail: String
    public let status: String
}

public enum ConnectionOpenEnvelope: Equatable, Sendable {
    case success
    case failure(message: String)
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
        resultID: UUID,
        columns: [QueryResultColumn],
        rows: [[String]],
        rowsAffected: UInt64,
        elapsedMs: UInt64,
        truncated: Bool
    )
    case failure(message: String)

    public var rowCount: Int {
        guard case let .success(_, _, rows, _, _, _) = self else {
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

public struct SchemaTableSummary: Codable, Equatable, Identifiable, Sendable {
    public var id: String {
        [schema, name].compactMap { $0 }.joined(separator: ".")
    }

    public let schema: String?
    public let name: String
    public let kind: String
    public let columnCount: Int

    public init(schema: String?, name: String, kind: String, columnCount: Int) {
        self.schema = schema
        self.name = name
        self.kind = kind
        self.columnCount = columnCount
    }
}

public enum SchemaLoadEnvelope: Equatable, Sendable {
    case success(tables: [SchemaTableSummary])
    case failure(message: String)
}

public enum SchemaLoadState: Equatable, Sendable {
    case empty
    case running(requestID: UUID)
    case success(tables: [SchemaTableSummary])
    case failure(message: String)

    public var tables: [SchemaTableSummary] {
        guard case let .success(tables) = self else {
            return []
        }
        return tables
    }

    public var isRunning: Bool {
        if case .running = self {
            return true
        }
        return false
    }
}

public struct TableExplorerState: Equatable, Sendable {
    public var schemaState: SchemaLoadState
    public var selectedTableID: SchemaTableSummary.ID?
    public var previewState: QueryExecutionState

    public var tables: [SchemaTableSummary] {
        schemaState.tables
    }

    public static func initial() -> TableExplorerState {
        TableExplorerState(schemaState: .empty, selectedTableID: nil, previewState: .empty)
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
    public var tableExplorer: TableExplorerState

    public static func initial() -> ConnectionWorkspace {
        let tableExplorer = WorkspaceTab.tableExplorer()
        return ConnectionWorkspace(
            tabs: [tableExplorer],
            selectedTabID: tableExplorer.id,
            nextUntitledIndex: 1,
            tableExplorer: .initial()
        )
    }
}
