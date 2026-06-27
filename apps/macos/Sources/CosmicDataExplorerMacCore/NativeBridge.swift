import Foundation

@_silgen_name("cosmic_active_connections_json")
private func cosmicActiveConnectionsJson() -> UnsafeMutablePointer<CChar>?

@_silgen_name("cosmic_execute_query_json")
private func cosmicExecuteQueryJson(_ inputJson: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("cosmic_string_free")
private func cosmicStringFree(_ pointer: UnsafeMutablePointer<CChar>?)

public enum NativeBridgeError: Error, Equatable {
    case emptyResponse
    case invalidUtf8
    case decodeFailed(String)
}

public struct NativeBridge: Sendable {
    public init() {}

    public func activeConnections() throws -> [ActiveConnection] {
        guard let pointer = cosmicActiveConnectionsJson() else {
            throw NativeBridgeError.emptyResponse
        }
        defer {
            cosmicStringFree(pointer)
        }

        guard let json = String(validatingCString: pointer) else {
            throw NativeBridgeError.invalidUtf8
        }

        do {
            return try JSONDecoder().decode([ActiveConnection].self, from: Data(json.utf8))
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

        let pointer = inputJson.withCString { inputPointer in
            cosmicExecuteQueryJson(inputPointer)
        }

        guard let pointer else {
            throw NativeBridgeError.emptyResponse
        }
        defer {
            cosmicStringFree(pointer)
        }

        guard let json = String(validatingCString: pointer) else {
            throw NativeBridgeError.invalidUtf8
        }

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
