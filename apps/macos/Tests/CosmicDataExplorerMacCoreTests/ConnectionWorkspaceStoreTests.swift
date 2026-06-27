import XCTest
@testable import CosmicDataExplorerMacCore

@MainActor
final class ConnectionWorkspaceStoreTests: XCTestCase {
    func testWorkspaceStartsWithPinnedTableExplorer() {
        let store = ConnectionWorkspaceStore()

        let workspace = store.workspace(for: "scratch")

        XCTAssertEqual(workspace.tabs.count, 1)
        XCTAssertEqual(workspace.tabs[0].kind, .tableExplorer)
        XCTAssertTrue(workspace.tabs[0].isPinned)
        XCTAssertEqual(workspace.selectedTabID, workspace.tabs[0].id)
    }

    func testSQLTabsAreScopedPerConnection() {
        let store = ConnectionWorkspaceStore()

        let scratchTab = store.addSQLTab(for: "scratch")
        store.updateSQL("select * from users", tabID: scratchTab, connectionID: "scratch")
        let analyticsTab = store.addSQLTab(for: "analytics")
        store.updateSQL("select * from events", tabID: analyticsTab, connectionID: "analytics")

        XCTAssertEqual(store.workspace(for: "scratch").tabs.count, 2)
        XCTAssertEqual(store.workspace(for: "analytics").tabs.count, 2)
        XCTAssertEqual(
            store.tab(id: scratchTab, connectionID: "scratch")?.sqlText,
            "select * from users"
        )
        XCTAssertEqual(
            store.tab(id: analyticsTab, connectionID: "analytics")?.sqlText,
            "select * from events"
        )
    }

    func testResultApplicationIsPerTabAndIgnoresStaleRequests() {
        let store = ConnectionWorkspaceStore()
        let firstTab = store.addSQLTab(for: "scratch")
        let secondTab = store.addSQLTab(for: "scratch")
        let firstRequest = UUID()
        let staleRequest = UUID()

        store.markRunning(tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.markRunning(tabID: secondTab, connectionID: "scratch", requestID: staleRequest)
        store.markRunning(tabID: secondTab, connectionID: "scratch", requestID: UUID())

        let result = QueryResultEnvelope.success(
            columns: [QueryResultColumn(name: "name", typeName: "TEXT", nullable: nil)],
            rows: [["Ada"]],
            rowsAffected: 0,
            elapsedMs: 4,
            truncated: false
        )
        store.applyResult(result, tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.applyResult(result, tabID: secondTab, connectionID: "scratch", requestID: staleRequest)

        XCTAssertEqual(store.tab(id: firstTab, connectionID: "scratch")?.resultState.rowCount, 1)
        XCTAssertEqual(store.tab(id: secondTab, connectionID: "scratch")?.resultState.rowCount, 0)
    }

    func testClosingSelectedSQLTabFallsBackToTableExplorer() {
        let store = ConnectionWorkspaceStore()
        let sqlTab = store.addSQLTab(for: "scratch")

        store.closeTab(sqlTab, connectionID: "scratch")

        let workspace = store.workspace(for: "scratch")
        XCTAssertEqual(workspace.tabs.count, 1)
        XCTAssertEqual(workspace.tabs[0].kind, .tableExplorer)
        XCTAssertEqual(workspace.selectedTabID, workspace.tabs[0].id)
    }

    func testTableExplorerCannotBeClosed() {
        let store = ConnectionWorkspaceStore()
        let tableExplorer = store.workspace(for: "scratch").tabs[0].id

        store.closeTab(tableExplorer, connectionID: "scratch")

        XCTAssertEqual(store.workspace(for: "scratch").tabs.count, 1)
    }
}
