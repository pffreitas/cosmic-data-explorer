import XCTest

final class AppLaunchContractTests: XCTestCase {
    func testSwiftPackageExecutableCreatesAndActivatesNativeWindow() throws {
        let source = try String(contentsOf: executableEntryPointURL, encoding: .utf8)

        XCTAssertTrue(source.contains("NSWindow"), "The SwiftPM executable should create the main native window explicitly.")
        XCTAssertTrue(source.contains("setActivationPolicy(.regular)"), "Terminal-launched apps must opt into regular app activation.")
        XCTAssertTrue(source.contains("makeKeyAndOrderFront"), "The main window should be ordered to the front on launch.")
        XCTAssertTrue(source.contains("activate(ignoringOtherApps: true)"), "The app should activate itself after launch.")
    }

    private var executableEntryPointURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources/CosmicDataExplorerMac/CosmicDataExplorerApp.swift")
    }
}
