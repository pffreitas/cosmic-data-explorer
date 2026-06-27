# Native macOS Release App Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build repo-local scripts that assemble the primary native SwiftUI macOS app into an unsigned local `.app` bundle ready to install from `dist/`.

**Architecture:** Keep development linking unchanged by default, but allow the Swift package to read a `COSMIC_NATIVE_BRIDGE_DIR` environment override for release builds. Add a contract test for the release script, then implement a strict shell builder that compiles the Rust bridge and Swift executable in release mode, assembles the macOS bundle, embeds the bridge dylib, and rewrites the executable's dylib load path.

**Tech Stack:** Bash, Cargo, SwiftPM, macOS app bundle layout, `install_name_tool`, XCTest source-contract tests.

## Global Constraints

- The release target is the native SwiftUI macOS app in `apps/macos`.
- The generated artifact is `dist/Cosmic Data Explorer.app`.
- The app bundle is unsigned and intended for local developer or alpha tester installation.
- The legacy Slint `desktop` shell is not included in the release script.
- Code signing, notarization, `.zip`, `.dmg`, `.pkg`, app icon generation, CI publishing, and update feeds are out of scope.
- The minimum macOS version is `14.0`.
- The bundle identifier is `dev.cosmic-data-explorer.mac`.
- The executable name is `CosmicDataExplorerMac`.
- The embedded Rust bridge library name is `libcosmic_native_bridge.dylib`.
- The script requires macOS, Xcode command line tools with Swift 6 support, a stable Rust toolchain, and `install_name_tool`.

---

## File Structure

- `apps/macos/Package.swift`: keep `target/debug` as the development default bridge path and add `COSMIC_NATIVE_BRIDGE_DIR` as an override used by release scripts.
- `apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift`: source-level test that locks the bridge path override and debug fallback into the Swift package manifest.
- `scripts/test-release-app-script.sh`: shell contract test for the release builder. It validates script structure without performing a full release build.
- `scripts/build-release-app.sh`: executable release builder that creates `dist/Cosmic Data Explorer.app`.
- `.gitignore`: ignore generated `dist/` release artifacts.

---

### Task 1: Swift Package Release Bridge Directory Override

**Files:**
- Modify: `apps/macos/Package.swift`
- Create: `apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift`

**Interfaces:**
- Consumes: `COSMIC_NATIVE_BRIDGE_DIR` environment variable when present.
- Produces: `bridgeLibraryDirectory: String` that resolves to `COSMIC_NATIVE_BRIDGE_DIR` for release scripts or `../../target/debug` for normal development builds.

- [ ] **Step 1: Add the failing manifest contract test**

Create `apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift` with this content:

```swift
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
```

- [ ] **Step 2: Build the debug bridge needed by the current Swift package**

Run:

```bash
cargo build -p cosmic-native-bridge
```

Expected: exit `0` and `target/debug/libcosmic_native_bridge.dylib` exists.

- [ ] **Step 3: Run the red test**

Run:

```bash
swift test --package-path apps/macos --filter PackageManifestContractTests/testPackageManifestAllowsReleaseBridgeDirectoryOverride
```

Expected: FAIL because `Package.swift` does not contain `COSMIC_NATIVE_BRIDGE_DIR`.

- [ ] **Step 4: Add the environment override to Package.swift**

Replace the current `bridgeLibraryDirectory` definition in `apps/macos/Package.swift`:

```swift
let bridgeLibraryDirectory = packageDirectory
    .appendingPathComponent("../../target/debug")
    .standardizedFileURL
    .path
```

with:

```swift
let defaultBridgeLibraryDirectory = packageDirectory
    .appendingPathComponent("../../target/debug")
    .standardizedFileURL
    .path

let bridgeLibraryDirectory = ProcessInfo.processInfo.environment["COSMIC_NATIVE_BRIDGE_DIR"]
    ?? defaultBridgeLibraryDirectory
```

- [ ] **Step 5: Run the manifest contract test**

Run:

```bash
swift test --package-path apps/macos --filter PackageManifestContractTests/testPackageManifestAllowsReleaseBridgeDirectoryOverride
```

Expected: PASS.

- [ ] **Step 6: Commit the bridge directory override**

Run:

```bash
git add apps/macos/Package.swift apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift
git commit -m "build(macos): allow release bridge link directory override"
```

Expected: commit succeeds.

---

### Task 2: Release App Script Contract Test

**Files:**
- Create: `scripts/test-release-app-script.sh`

**Interfaces:**
- Consumes: `scripts/build-release-app.sh` as source text.
- Produces: executable test command `bash scripts/test-release-app-script.sh`.

- [ ] **Step 1: Write the failing shell contract test**

Create `scripts/test-release-app-script.sh` with this content:

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_SCRIPT="$SCRIPT_DIR/build-release-app.sh"

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

assert_executable_file() {
    local path="$1"
    [[ -f "$path" ]] || fail "missing file: $path"
    [[ -x "$path" ]] || fail "file is not executable: $path"
}

assert_contains() {
    local needle="$1"
    grep -F -- "$needle" "$BUILD_SCRIPT" >/dev/null || fail "missing script content: $needle"
}

assert_executable_file "$BUILD_SCRIPT"

assert_contains "set -euo pipefail"
assert_contains "cargo build --release -p cosmic-native-bridge"
assert_contains "COSMIC_NATIVE_BRIDGE_DIR="
assert_contains "swift build --package-path"
assert_contains "-c release"
assert_contains "dist"
assert_contains "Cosmic Data Explorer.app"
assert_contains "Contents/MacOS"
assert_contains "Contents/Frameworks"
assert_contains "Contents/Resources"
assert_contains "CosmicDataExplorerMac"
assert_contains "libcosmic_native_bridge.dylib"
assert_contains "Info.plist"
assert_contains "CFBundleIdentifier"
assert_contains "dev.cosmic-data-explorer.mac"
assert_contains "LSMinimumSystemVersion"
assert_contains "14.0"
assert_contains "LSApplicationCategoryType"
assert_contains "public.app-category.developer-tools"
assert_contains "install_name_tool"
assert_contains "@executable_path/../Frameworks/libcosmic_native_bridge.dylib"

printf 'Release app script contract passed.\n'
```

- [ ] **Step 2: Mark the contract test executable**

Run:

```bash
chmod +x scripts/test-release-app-script.sh
```

Expected: exit `0`.

- [ ] **Step 3: Run the red contract test**

Run:

```bash
bash scripts/test-release-app-script.sh
```

Expected: FAIL with `missing file: .../scripts/build-release-app.sh`.

---

### Task 3: Native macOS Release App Builder

**Files:**
- Create: `scripts/build-release-app.sh`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: Cargo package `cosmic-native-bridge`.
- Consumes: Swift package executable `CosmicDataExplorerMac`.
- Consumes: `COSMIC_NATIVE_BRIDGE_DIR` support from Task 1.
- Produces: `dist/Cosmic Data Explorer.app`.

- [ ] **Step 1: Add the release builder script**

Create `scripts/build-release-app.sh` with this content:

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

APP_NAME="Cosmic Data Explorer"
EXECUTABLE_NAME="CosmicDataExplorerMac"
BRIDGE_LIBRARY_NAME="libcosmic_native_bridge.dylib"
BUNDLE_IDENTIFIER="dev.cosmic-data-explorer.mac"
MINIMUM_MACOS_VERSION="14.0"

DIST_DIR="$REPO_ROOT/dist"
APP_BUNDLE="$DIST_DIR/$APP_NAME.app"
CONTENTS_DIR="$APP_BUNDLE/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
FRAMEWORKS_DIR="$CONTENTS_DIR/Frameworks"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

MACOS_PACKAGE_DIR="$REPO_ROOT/apps/macos"
BRIDGE_BUILD_DIR="$REPO_ROOT/target/release"
BRIDGE_LIBRARY="$BRIDGE_BUILD_DIR/$BRIDGE_LIBRARY_NAME"
SWIFT_EXECUTABLE="$MACOS_PACKAGE_DIR/.build/release/$EXECUTABLE_NAME"
BUNDLED_EXECUTABLE="$MACOS_DIR/$EXECUTABLE_NAME"
BUNDLED_BRIDGE="$FRAMEWORKS_DIR/$BRIDGE_LIBRARY_NAME"

log() {
    printf '\n==> %s\n' "$1"
}

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null || fail "required command not found: $1"
}

require_file() {
    local path="$1"
    local description="$2"
    [[ -f "$path" ]] || fail "missing $description: $path"
}

cd "$REPO_ROOT"

require_command cargo
require_command swift
require_command install_name_tool
require_command otool

log "Cleaning release output"
rm -rf "$DIST_DIR"
mkdir -p "$MACOS_DIR" "$FRAMEWORKS_DIR" "$RESOURCES_DIR"

log "Building Rust native bridge"
cargo build --release -p cosmic-native-bridge
require_file "$BRIDGE_LIBRARY" "release native bridge dylib"

log "Building Swift macOS app"
COSMIC_NATIVE_BRIDGE_DIR="$BRIDGE_BUILD_DIR" swift build --package-path "$MACOS_PACKAGE_DIR" -c release
require_file "$SWIFT_EXECUTABLE" "Swift release executable"

log "Assembling app bundle"
cp "$SWIFT_EXECUTABLE" "$BUNDLED_EXECUTABLE"
cp "$BRIDGE_LIBRARY" "$BUNDLED_BRIDGE"
chmod +x "$BUNDLED_EXECUTABLE"

cat > "$CONTENTS_DIR/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleExecutable</key>
    <string>$EXECUTABLE_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_IDENTIFIER</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MINIMUM_MACOS_VERSION</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
PLIST

log "Rewriting native bridge load path"
current_bridge_load_path="$(
    otool -L "$BUNDLED_EXECUTABLE" \
        | awk -v library="$BRIDGE_LIBRARY_NAME" '$1 ~ library { print $1; exit }'
)"

[[ -n "$current_bridge_load_path" ]] || fail "could not find $BRIDGE_LIBRARY_NAME load command in $BUNDLED_EXECUTABLE"

install_name_tool \
    -change "$current_bridge_load_path" \
    "@executable_path/../Frameworks/$BRIDGE_LIBRARY_NAME" \
    "$BUNDLED_EXECUTABLE"

log "Release app bundle ready"
printf '%s\n' "$APP_BUNDLE"
```

- [ ] **Step 2: Mark the release builder executable**

Run:

```bash
chmod +x scripts/build-release-app.sh
```

Expected: exit `0`.

- [ ] **Step 3: Ignore generated release artifacts**

Add this line to `.gitignore`:

```gitignore
/dist/
```

- [ ] **Step 4: Run the shell contract test**

Run:

```bash
bash scripts/test-release-app-script.sh
```

Expected: PASS with `Release app script contract passed.`

- [ ] **Step 5: Commit the release scripts**

Run:

```bash
git add .gitignore scripts/build-release-app.sh scripts/test-release-app-script.sh
git commit -m "build(macos): add release app bundle scripts"
```

Expected: commit succeeds.

---

### Task 4: Full Verification

**Files:**
- Modify only files required by verification failures.

**Interfaces:**
- Consumes: `scripts/test-release-app-script.sh`.
- Consumes: `scripts/build-release-app.sh`.
- Produces: verified `dist/Cosmic Data Explorer.app` when local macOS toolchains allow the release build to complete.

- [ ] **Step 1: Run the focused script contract test**

Run:

```bash
bash scripts/test-release-app-script.sh
```

Expected: PASS with `Release app script contract passed.`

- [ ] **Step 2: Run the Swift package tests**

Run:

```bash
cargo build -p cosmic-native-bridge
swift test --package-path apps/macos
```

Expected: both commands exit `0`.

- [ ] **Step 3: Run the release app builder**

Run:

```bash
scripts/build-release-app.sh
```

Expected: exit `0` and final output path is `dist/Cosmic Data Explorer.app`.

- [ ] **Step 4: Verify the assembled bundle files**

Run:

```bash
test -x "dist/Cosmic Data Explorer.app/Contents/MacOS/CosmicDataExplorerMac"
test -f "dist/Cosmic Data Explorer.app/Contents/Frameworks/libcosmic_native_bridge.dylib"
test -f "dist/Cosmic Data Explorer.app/Contents/Info.plist"
```

Expected: all commands exit `0`.

- [ ] **Step 5: Verify the executable uses the bundled bridge**

Run:

```bash
otool -L "dist/Cosmic Data Explorer.app/Contents/MacOS/CosmicDataExplorerMac" | grep -F "@executable_path/../Frameworks/libcosmic_native_bridge.dylib"
```

Expected: output contains `@executable_path/../Frameworks/libcosmic_native_bridge.dylib`.

- [ ] **Step 6: Commit any verification fixes**

If verification required fixes, run:

```bash
git add apps/macos/Package.swift apps/macos/Tests/CosmicDataExplorerMacCoreTests/PackageManifestContractTests.swift .gitignore scripts/build-release-app.sh scripts/test-release-app-script.sh
git commit -m "build(macos): fix release app bundle verification"
```

Expected: commit succeeds only when fixes were necessary.
