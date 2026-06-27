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
    }

    private var contentViewURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources/CosmicDataExplorerMacCore/ContentView.swift")
    }
}
