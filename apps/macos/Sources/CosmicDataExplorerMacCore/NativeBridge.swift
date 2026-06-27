import Foundation

@_silgen_name("cosmic_active_connections_json")
private func cosmicActiveConnectionsJson() -> UnsafeMutablePointer<CChar>?

@_silgen_name("cosmic_create_connection_json")
private func cosmicCreateConnectionJson(_ inputJson: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cosmic_execute_query_json")
private func cosmicExecuteQueryJson(_ inputJson: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cosmic_string_free")
private func cosmicStringFree(_ pointer: UnsafeMutablePointer<CChar>?)

public enum NativeBridgeError: Error, Equatable {
    case emptyResponse
    case invalidUtf8
    case decodeFailed(String)
    case operationFailed(String)
}

extension NativeBridgeError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .emptyResponse:
            "Native bridge returned an empty response"
        case .invalidUtf8:
            "Native bridge returned invalid UTF-8"
        case let .decodeFailed(message):
            "Native bridge response could not be decoded: \(message)"
        case let .operationFailed(message):
            message
        }
    }
}

public struct NativeBridge: Sendable {
    private let activeConnectionsJson: @Sendable () throws -> String
    private let createConnectionJson: @Sendable (String) throws -> String
    private let executeQueryJson: @Sendable (String) throws -> String

    public init() {
        self.init(
            activeConnectionsJson: NativeBridge.defaultActiveConnectionsJson,
            createConnectionJson: NativeBridge.defaultCreateConnectionJson,
            executeQueryJson: NativeBridge.defaultExecuteQueryJson
        )
    }

    init(
        activeConnectionsJson: @escaping @Sendable () throws -> String = NativeBridge
            .defaultActiveConnectionsJson,
        createConnectionJson: @escaping @Sendable (String) throws -> String = NativeBridge
            .defaultCreateConnectionJson,
        executeQueryJson: @escaping @Sendable (String) throws -> String = NativeBridge
            .defaultExecuteQueryJson
    ) {
        self.activeConnectionsJson = activeConnectionsJson
        self.createConnectionJson = createConnectionJson
        self.executeQueryJson = executeQueryJson
    }

    public func activeConnections() throws -> [ActiveConnection] {
        let json = try activeConnectionsJson()

        do {
            return try JSONDecoder().decode([ActiveConnection].self, from: Data(json.utf8))
        } catch {
            throw NativeBridgeError.decodeFailed(error.localizedDescription)
        }
    }

    public func createConnection(name: String, connectionString: String) throws -> ActiveConnection {
        let request = CreateConnectionRequest(name: name, connectionString: connectionString)
        let inputData = try JSONEncoder().encode(request)
        guard let inputJson = String(data: inputData, encoding: .utf8) else {
            throw NativeBridgeError.invalidUtf8
        }

        let json = try createConnectionJson(inputJson)

        do {
            let response = try JSONDecoder().decode(
                CreateConnectionResponse.self,
                from: Data(json.utf8)
            )
            if response.ok, let connection = response.connection {
                return connection
            }
            throw NativeBridgeError.operationFailed(
                response.message ?? "Connection could not be created"
            )
        } catch let error as NativeBridgeError {
            throw error
        } catch {
            throw NativeBridgeError.decodeFailed(error.localizedDescription)
        }
    }

    public func executeQuery(
        connectionID: String,
        sql: String,
        maxRows: UInt32 = 100
    ) throws -> QueryResultEnvelope {
        let request = ExecuteQueryRequest(connectionId: connectionID, sql: sql, maxRows: maxRows)
        let inputData = try JSONEncoder().encode(request)
        guard let inputJson = String(data: inputData, encoding: .utf8) else {
            throw NativeBridgeError.invalidUtf8
        }

        let json = try executeQueryJson(inputJson)

        do {
            let response = try JSONDecoder().decode(
                ExecuteQueryResponse.self,
                from: Data(json.utf8)
            )
            if response.ok {
                return .success(
                    columns: response.columns ?? [],
                    rows: response.rows ?? [],
                    rowsAffected: response.rowsAffected ?? 0,
                    elapsedMs: response.elapsedMs ?? 0,
                    truncated: response.truncated ?? false
                )
            }
            return .failure(message: response.message ?? "Query execution failed")
        } catch {
            throw NativeBridgeError.decodeFailed(error.localizedDescription)
        }
    }

    private static func defaultActiveConnectionsJson() throws -> String {
        try stringFromOwnedCString(cosmicActiveConnectionsJson())
    }

    private static func defaultCreateConnectionJson(_ inputJson: String) throws -> String {
        let pointer = inputJson.withCString { inputPointer in
            cosmicCreateConnectionJson(inputPointer)
        }
        return try stringFromOwnedCString(pointer)
    }

    private static func defaultExecuteQueryJson(_ inputJson: String) throws -> String {
        let pointer = inputJson.withCString { inputPointer in
            cosmicExecuteQueryJson(inputPointer)
        }
        return try stringFromOwnedCString(pointer)
    }

    private static func stringFromOwnedCString(_ pointer: UnsafeMutablePointer<CChar>?) throws
        -> String
    {
        guard let pointer else {
            throw NativeBridgeError.emptyResponse
        }
        defer {
            cosmicStringFree(pointer)
        }

        guard let json = String(validatingCString: pointer) else {
            throw NativeBridgeError.invalidUtf8
        }
        return json
    }
}

private struct CreateConnectionRequest: Encodable {
    let name: String
    let connectionString: String
}

private struct CreateConnectionResponse: Decodable {
    let ok: Bool
    let connection: ActiveConnection?
    let message: String?
}

private struct ExecuteQueryRequest: Encodable {
    let connectionId: String
    let sql: String
    let maxRows: UInt32
}

private struct ExecuteQueryResponse: Decodable {
    let ok: Bool
    let columns: [QueryResultColumn]?
    let rows: [[String]]?
    let rowsAffected: UInt64?
    let elapsedMs: UInt64?
    let truncated: Bool?
    let message: String?
}
