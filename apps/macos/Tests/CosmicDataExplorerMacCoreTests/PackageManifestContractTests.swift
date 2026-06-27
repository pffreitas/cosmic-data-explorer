import XCTest

final class PackageManifestContractTests: XCTestCase {
    func testPackageManifestAllowsReleaseBridgeDirectoryOverride() throws {
        let source = try String(contentsOf: packageManifestURL, encoding: .utf8)

        XCTAssertTrue(
            source.contains("COSMIC_NATIVE_BRIDGE_DIR"),
            "Release scripts need to point SwiftPM at target/release for the Rust bridge."
        )
        XCTAssertTrue(
            source.contains("ProcessInfo.processInfo.environment"),
            "Package.swift reads the bridge override from the process environment."
        )
        XCTAssertTrue(
            source.contains("../../target/debug"),
            "Development builds keep the existing target/debug bridge fallback."
        )
    }

    private var packageManifestURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Package.swift")
    }
}
