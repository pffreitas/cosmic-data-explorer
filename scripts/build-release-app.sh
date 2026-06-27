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
# Produces: dist/Cosmic Data Explorer.app
# Bundle layout: Contents/MacOS, Contents/Frameworks, Contents/Resources
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
    "@executable_path/../Frameworks/libcosmic_native_bridge.dylib" \
    "$BUNDLED_EXECUTABLE"

log "Release app bundle ready"
printf '%s\n' "$APP_BUNDLE"
