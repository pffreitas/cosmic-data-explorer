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
assert_contains "-id \"@executable_path/../Frameworks/libcosmic_native_bridge.dylib\""
assert_contains "\"\$BUNDLED_BRIDGE\""
assert_contains "@executable_path/../Frameworks/libcosmic_native_bridge.dylib"

printf 'Release app script contract passed.\n'
