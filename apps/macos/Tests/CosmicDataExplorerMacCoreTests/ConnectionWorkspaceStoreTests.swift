import Combine
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

    func testReadingMissingWorkspaceDoesNotPublishChanges() {
        let store = ConnectionWorkspaceStore()
        var publishCount = 0
        let cancellable = store.objectWillChange.sink {
            publishCount += 1
        }

        _ = store.workspace(for: "scratch")
        _ = store.tableExplorer(for: "analytics")

        XCTAssertEqual(publishCount, 0)
        cancellable.cancel()
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
        XCTAssertTrue(store.tab(id: firstTab, connectionID: "scratch")?.resultState.isRunning == true)
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

    func testAcceptedIdenticalResultsKeepDistinctSuccessIdentity() {
        let store = ConnectionWorkspaceStore()
        let firstTab = store.addSQLTab(for: "scratch")
        let secondTab = store.addSQLTab(for: "scratch")
        let firstRequest = UUID()
        let secondRequest = UUID()
        let result = QueryResultEnvelope.success(
            columns: [QueryResultColumn(name: "name", typeName: "TEXT", nullable: nil)],
            rows: [["Ada"]],
            rowsAffected: 0,
            elapsedMs: 4,
            truncated: false
        )

        store.markRunning(tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.markRunning(tabID: secondTab, connectionID: "scratch", requestID: secondRequest)

        store.applyResult(result, tabID: firstTab, connectionID: "scratch", requestID: firstRequest)
        store.applyResult(result, tabID: secondTab, connectionID: "scratch", requestID: secondRequest)

        guard
            case let .success(firstResultID, _, _, _, _, _)? = store.tab(
                id: firstTab,
                connectionID: "scratch"
            )?.resultState,
            case let .success(secondResultID, _, _, _, _, _)? = store.tab(
                id: secondTab,
                connectionID: "scratch"
            )?.resultState
        else {
            return XCTFail("Expected successful results for both tabs")
        }

        XCTAssertEqual(firstResultID, firstRequest)
        XCTAssertEqual(secondResultID, secondRequest)
        XCTAssertNotEqual(firstResultID, secondResultID)
    }

    func testAcceptedIdenticalPreviewResultsKeepDistinctSuccessIdentity() {
        let store = ConnectionWorkspaceStore()
        let users = SchemaTableSummary(schema: nil, name: "users", kind: "table", columnCount: 2)
        let firstRequest = UUID()
        let secondRequest = UUID()
        let result = QueryResultEnvelope.success(
            columns: [QueryResultColumn(name: "name", typeName: "TEXT", nullable: nil)],
            rows: [["Ada"]],
            rowsAffected: 0,
            elapsedMs: 4,
            truncated: false
        )

        XCTAssertTrue(store.selectTable(users, connectionID: "scratch"))

        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: firstRequest
        )
        store.applyPreviewResult(
            result,
            tableID: users.id,
            connectionID: "scratch",
            requestID: firstRequest
        )

        guard case let .success(firstResultID, _, _, _, _, _) = store.tableExplorer(for: "scratch")
            .previewState else {
            return XCTFail("Expected first preview result to succeed")
        }

        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: secondRequest
        )
        store.applyPreviewResult(
            result,
            tableID: users.id,
            connectionID: "scratch",
            requestID: secondRequest
        )

        guard case let .success(secondResultID, _, _, _, _, _) = store.tableExplorer(for: "scratch")
            .previewState else {
            return XCTFail("Expected second preview result to succeed")
        }

        XCTAssertEqual(firstResultID, firstRequest)
        XCTAssertEqual(secondResultID, secondRequest)
        XCTAssertNotEqual(firstResultID, secondResultID)
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

    func testTableExplorerSchemaStateIsScopedPerConnection() {
        let store = ConnectionWorkspaceStore()
        let scratchRequest = UUID()
        let analyticsRequest = UUID()
        let users = SchemaTableSummary(schema: nil, name: "users", kind: "table", columnCount: 2)
        let events = SchemaTableSummary(
            schema: "public",
            name: "events",
            kind: "table",
            columnCount: 4
        )

        store.markSchemaLoading(connectionID: "scratch", requestID: scratchRequest)
        store.markSchemaLoading(connectionID: "analytics", requestID: analyticsRequest)
        store.applySchemaResult(
            .success(tables: [users]),
            connectionID: "scratch",
            requestID: scratchRequest
        )
        store.applySchemaResult(
            .success(tables: [events]),
            connectionID: "analytics",
            requestID: analyticsRequest
        )

        XCTAssertEqual(store.tableExplorer(for: "scratch").tables, [users])
        XCTAssertEqual(store.tableExplorer(for: "analytics").tables, [events])
    }

    func testTableExplorerIgnoresStalePreviewResults() {
        let store = ConnectionWorkspaceStore()
        let users = SchemaTableSummary(schema: nil, name: "users", kind: "table", columnCount: 2)
        let firstRequest = UUID()
        let staleRequest = UUID()
        let activeRequest = UUID()
        let result = QueryResultEnvelope.success(
            columns: [QueryResultColumn(name: "name", typeName: "TEXT", nullable: nil)],
            rows: [["Ada"]],
            rowsAffected: 0,
            elapsedMs: 4,
            truncated: false
        )

        store.selectTable(users, connectionID: "scratch")
        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: firstRequest
        )
        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: staleRequest
        )
        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: activeRequest
        )

        store.applyPreviewResult(
            result,
            tableID: users.id,
            connectionID: "scratch",
            requestID: firstRequest
        )
        XCTAssertEqual(store.tableExplorer(for: "scratch").previewState.rowCount, 0)

        store.applyPreviewResult(
            result,
            tableID: users.id,
            connectionID: "scratch",
            requestID: activeRequest
        )
        XCTAssertEqual(store.tableExplorer(for: "scratch").previewState.rowCount, 1)
    }

    func testSelectingCurrentTableDoesNotRestartRunningPreview() {
        let store = ConnectionWorkspaceStore()
        let users = SchemaTableSummary(schema: nil, name: "users", kind: "table", columnCount: 2)
        let request = UUID()

        XCTAssertTrue(store.selectTable(users, connectionID: "scratch"))
        store.markPreviewRunning(
            tableID: users.id,
            connectionID: "scratch",
            requestID: request
        )

        XCTAssertFalse(store.selectTable(users, connectionID: "scratch"))
        XCTAssertEqual(
            store.tableExplorer(for: "scratch").previewState,
            .running(requestID: request)
        )
    }
}
