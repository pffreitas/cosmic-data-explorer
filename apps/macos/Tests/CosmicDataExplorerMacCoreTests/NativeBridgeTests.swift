import XCTest
@testable import CosmicDataExplorerMacCore

final class NativeBridgeTests: XCTestCase {
    func testBridgeDecodesActiveConnections() throws {
        let connections = try NativeBridge().activeConnections()

        XCTAssertGreaterThanOrEqual(connections.count, 3)
        XCTAssertTrue(connections.contains { $0.name == "Production" })
        XCTAssertTrue(connections.contains { $0.kind == "PostgreSQL" })
        XCTAssertTrue(connections.contains { $0.kind == "SQLite" })
    }

    func testBridgeExecutesSQLiteQuery() throws {
        let result = try NativeBridge().executeQuery(
            connectionID: "scratch",
            sql: "select id, name from users order by id",
            maxRows: 100
        )

        guard case let .success(columns, rows, _, _, _) = result else {
            return XCTFail("Expected successful query result")
        }

        XCTAssertEqual(columns.map(\.name), ["id", "name"])
        XCTAssertTrue(rows.contains(["1", "Ada"]))
    }

    func testBridgeDecodesQueryFailure() throws {
        let result = try NativeBridge().executeQuery(
            connectionID: "production",
            sql: "select 1",
            maxRows: 100
        )

        guard case let .failure(message) = result else {
            return XCTFail("Expected failure query result")
        }

        XCTAssertTrue(message.contains("not available for query execution yet"))
    }
}
