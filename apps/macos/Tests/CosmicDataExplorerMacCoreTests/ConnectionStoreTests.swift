import Foundation
import XCTest
@testable import CosmicDataExplorerMacCore

@MainActor
final class ConnectionStoreTests: XCTestCase {
    func testInitializationDoesNotLoadConnections() {
        let loadCounter = LoadCounter()
        let bridge = NativeBridge(
            activeConnectionsJson: {
                loadCounter.increment()
                return "[]"
            }
        )

        let store = ConnectionStore(bridge: bridge)

        XCTAssertEqual(loadCounter.value, 0)
        XCTAssertTrue(store.activeConnections.isEmpty)
        XCTAssertNil(store.selectedConnectionID)
    }

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

private final class LoadCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    var value: Int {
        lock.withLock {
            count
        }
    }

    func increment() {
        lock.withLock {
            count += 1
        }
    }
}
