import XCTest

final class ContentViewContractTests: XCTestCase {
    func testContentViewContainsTabbedWorkbenchContract() throws {
        let source = try String(contentsOf: contentViewURL, encoding: .utf8)

        XCTAssertTrue(source.contains("ConnectionWorkspaceStore"))
        XCTAssertTrue(source.contains("tabStrip"))
        XCTAssertTrue(source.contains("addSQLTab"))
        XCTAssertTrue(source.contains("Table Explorer"))
        XCTAssertTrue(source.contains("QueryResultGrid"))
        XCTAssertTrue(source.contains("executeQuery"))
        XCTAssertTrue(source.contains("showingNewConnection"))
        XCTAssertTrue(source.contains("NewConnectionView"))
        XCTAssertTrue(source.contains("Image(systemName: \"plus\")"))
        XCTAssertTrue(source.contains("createConnection"))
        XCTAssertTrue(source.contains("Table(tables, selection:"))
        XCTAssertTrue(source.contains("loadTableSchema"))
        XCTAssertTrue(source.contains("previewSelectedTable"))
        XCTAssertTrue(source.contains("loadSchema(connectionID:"))
        XCTAssertTrue(source.contains("previewTable("))
        XCTAssertTrue(source.contains("maxRows: 50"))
        XCTAssertTrue(source.contains(".task(id: store.selectedConnectionID)"))
        XCTAssertTrue(source.contains("openSelectedConnection"))
        XCTAssertTrue(source.contains("openConnection(connectionID:"))
        XCTAssertTrue(source.contains("Table(resultRows)"))
        XCTAssertFalse(source.contains("Grid(alignment: .leading"))
    }

    private var contentViewURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources/CosmicDataExplorerMacCore/ContentView.swift")
    }
}
