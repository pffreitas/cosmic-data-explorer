import SwiftUI

@MainActor
public final class ConnectionStore: ObservableObject {
    @Published public private(set) var activeConnections: [ActiveConnection] = []
    @Published public var selectedConnectionID: ActiveConnection.ID?
    @Published public private(set) var lastError: String?

    private let bridge: NativeBridge

    public init(bridge: NativeBridge = NativeBridge()) {
        self.bridge = bridge
        load()
    }

    public var selectedConnection: ActiveConnection? {
        activeConnections.first { $0.id == selectedConnectionID } ?? activeConnections.first
    }

    public func load() {
        do {
            activeConnections = try bridge.activeConnections()
            selectedConnectionID = selectedConnectionID ?? activeConnections.first?.id
            lastError = nil
        } catch {
            activeConnections = []
            selectedConnectionID = nil
            lastError = error.localizedDescription
        }
    }

    @discardableResult
    public func createConnection(name: String, connectionString: String) async throws
        -> ActiveConnection
    {
        let bridge = bridge
        do {
            let created = try await Task.detached {
                try bridge.createConnection(name: name, connectionString: connectionString)
            }.value

            load()
            selectedConnectionID = created.id
            lastError = nil
            return created
        } catch {
            lastError = error.localizedDescription
            throw error
        }
    }
}
