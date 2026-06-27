import XCTest
@testable import CosmicDataExplorerMacCore

@MainActor
final class ConnectionStoreTests: XCTestCase {
    func testCreateConnectionReloadsAndSelectsCreatedConnection() async throws {
        let created = ActiveConnection(
            id: "profile_123",
            name: "Hackathon",
            kind: "PostgreSQL",
            detail: "hackathon / admin",
            status: "Saved"
        )
        let bridge = NativeBridge(
            activeConnectionsJson: {
                """
                [{"id":"profile_123","name":"Hackathon","kind":"PostgreSQL","detail":"hackathon / admin","status":"Saved"}]
                """
            },
            createConnectionJson: { requestJSON in
                XCTAssertTrue(requestJSON.contains("\"name\":\"Hackathon\""))
                return """
                {"ok":true,"connection":{"id":"profile_123","name":"Hackathon","kind":"PostgreSQL","detail":"hackathon / admin","status":"Saved"}}
                """
            }
        )
        let store = ConnectionStore(bridge: bridge)

        let returned = try await store.createConnection(
            name: "Hackathon",
            connectionString: "postgres://admin:secret@localhost/hackathon"
        )

        XCTAssertEqual(returned, created)
        XCTAssertEqual(store.activeConnections, [created])
        XCTAssertEqual(store.selectedConnectionID, created.id)
    }
}
