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

    func testBridgeDecodesCreateConnectionSuccess() throws {
        let bridge = NativeBridge(
            createConnectionJson: { requestJSON in
                XCTAssertTrue(requestJSON.contains("\"name\":\"Hackathon\""))
                XCTAssertTrue(requestJSON.contains("postgres"))
                XCTAssertTrue(requestJSON.contains("localhost"))
                XCTAssertTrue(requestJSON.contains("hackathon"))
                return """
                {"ok":true,"connection":{"id":"profile_123","name":"Hackathon","kind":"PostgreSQL","detail":"hackathon / admin","status":"Saved"}}
                """
            }
        )

        let connection = try bridge.createConnection(
            name: "Hackathon",
            connectionString: "postgres://admin:secret@localhost/hackathon"
        )

        XCTAssertEqual(connection.id, "profile_123")
        XCTAssertEqual(connection.name, "Hackathon")
        XCTAssertEqual(connection.status, "Saved")
    }

    func testBridgeThrowsCreateConnectionFailure() throws {
        let bridge = NativeBridge(
            createConnectionJson: { _ in
                #"{"ok":false,"message":"connection display name is required"}"#
            }
        )

        XCTAssertThrowsError(
            try bridge.createConnection(name: "", connectionString: "not a url")
        ) { error in
            XCTAssertEqual(
                error as? NativeBridgeError,
                .operationFailed("connection display name is required")
            )
        }
    }
}
