// swift-tools-version: 6.0

import Foundation
import PackageDescription

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bridgeLibraryDirectory = packageDirectory
    .appendingPathComponent("../../target/debug")
    .standardizedFileURL
    .path

let package = Package(
    name: "CosmicDataExplorerMac",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "CosmicDataExplorerMac", targets: ["CosmicDataExplorerMac"])
    ],
    targets: [
        .executableTarget(
            name: "CosmicDataExplorerMac",
            dependencies: ["CosmicDataExplorerMacCore"]
        ),
        .target(
            name: "CosmicDataExplorerMacCore",
            linkerSettings: [
                .unsafeFlags([
                    "-L", bridgeLibraryDirectory,
                    "-lcosmic_native_bridge",
                    "-Xlinker", "-rpath",
                    "-Xlinker", bridgeLibraryDirectory,
                ])
            ]
        ),
        .testTarget(
            name: "CosmicDataExplorerMacCoreTests",
            dependencies: ["CosmicDataExplorerMacCore"]
        ),
    ]
)
