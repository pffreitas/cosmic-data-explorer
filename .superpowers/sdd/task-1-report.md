# Task 1 Report: Swift Package Release Bridge Directory Override

## What I implemented

- Added a manifest contract test in `apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift` that checks `Package.swift` contains:
  - `COSMIC_NATIVE_BRIDGE_DIR`
  - `ProcessInfo.processInfo.environment`
  - `../../target/debug`
- Updated `apps/macos/Package.swift` so the bridge link directory now resolves from `COSMIC_NATIVE_BRIDGE_DIR` when present, and falls back to the existing debug path otherwise.
- Kept the development default aligned with `../../target/debug` while allowing release scripts to override the directory without changing the manifest.

## TDD RED/GREEN evidence

### RED

Command:

```bash
swift test --package-path apps/macos --filter PackageManifestContractTests/testPackageManifestAllowsReleaseBridgeDirectoryOverride
```

Result:

- Failed as expected.
- Failure showed the manifest did not yet contain `COSMIC_NATIVE_BRIDGE_DIR` or `ProcessInfo.processInfo.environment`.

### GREEN

Command:

```bash
swift test --package-path apps/macos --filter PackageManifestContractTests/testPackageManifestAllowsReleaseBridgeDirectoryOverride
```

Result:

- Passed after the manifest update.

## Test results

- `cargo build -p cosmic-native-bridge` succeeded.
- `swift test --package-path apps/macos --filter PackageManifestContractTests/testPackageManifestAllowsReleaseBridgeDirectoryOverride` passed.
- `swift test --package-path apps/macos` passed with 26 tests total.

## Files changed

- `apps/macos/Package.swift`
- `apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift`

## Self-review

- The change is tightly scoped to the manifest bridge directory resolution.
- The test verifies the contract from the manifest source, which is appropriate for this release-bridge override behavior.
- The fallback remains unchanged for development builds.

## Concerns

- None at this time.
