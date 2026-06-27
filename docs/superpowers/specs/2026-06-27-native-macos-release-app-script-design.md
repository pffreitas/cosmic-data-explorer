# Native macOS Release App Script Design

## Goal

Add repository scripts that build the primary native SwiftUI macOS app into a local unsigned `.app` bundle ready to install from the checkout. The output is intended for developer and alpha tester installation by copying or opening the app bundle locally. Public distribution, code signing, notarization, DMG creation, and update feeds are out of scope.

## Target Artifact

The script produces:

```text
dist/Cosmic Data Explorer.app
```

The app bundle contains:

- `Contents/MacOS/CosmicDataExplorerMac`: the SwiftPM release executable.
- `Contents/Frameworks/libcosmic_native_bridge.dylib`: the Rust release native bridge.
- `Contents/Info.plist`: bundle metadata for macOS launch services.
- `Contents/Resources`: an empty resources directory reserved for future icons and assets.

The legacy Slint desktop shell is not included in this release script.

## Script Interface

Add a repo-local shell script:

```text
scripts/build-release-app.sh
```

The script is run from any working directory and resolves paths relative to the repository root. It uses strict shell behavior and fails on the first build, copy, or bundle assembly error.

The script performs these steps:

1. Clean and recreate `dist/`.
2. Build the Rust bridge with `cargo build --release -p cosmic-native-bridge`.
3. Build the Swift app with `swift build --package-path apps/macos -c release`.
4. Create the macOS bundle directory structure.
5. Copy the Swift release executable into `Contents/MacOS/`.
6. Copy the Rust release dylib into `Contents/Frameworks/`.
7. Generate `Contents/Info.plist` with:
   - bundle display name `Cosmic Data Explorer`
   - bundle identifier `dev.cosmic-data-explorer.mac`
   - executable name `CosmicDataExplorerMac`
   - package type `APPL`
   - minimum macOS version `14.0`
   - category `public.app-category.developer-tools`
8. Rewrite the executable's `libcosmic_native_bridge.dylib` load command to `@executable_path/../Frameworks/libcosmic_native_bridge.dylib` using `install_name_tool`.
9. Print the final app bundle path.

## Build Assumptions

The existing Swift package currently links the bridge from `target/debug` for development. The release script intentionally builds and embeds the release bridge, then rewrites the executable load path so the assembled app is self-contained for local installation.

The script requires:

- macOS.
- Xcode command line tools with Swift 6 support.
- a stable Rust toolchain.
- `install_name_tool`, available with Apple developer tools.

## Testing

Add a lightweight shell contract test:

```text
scripts/test-release-app-script.sh
```

The test does not perform a full release build. It verifies the release script contract by checking that:

- `scripts/build-release-app.sh` exists and is executable.
- the script uses strict shell error handling.
- the script invokes `cargo build --release -p cosmic-native-bridge`.
- the script invokes `swift build --package-path apps/macos -c release`.
- the script creates `dist/Cosmic Data Explorer.app`.
- the script copies `CosmicDataExplorerMac` into `Contents/MacOS`.
- the script copies `libcosmic_native_bridge.dylib` into `Contents/Frameworks`.
- the script generates `Contents/Info.plist`.
- the script rewrites the bridge load path with `install_name_tool`.

Full verification after implementation runs the contract test and, where local toolchains allow, runs the release builder itself.

## Error Handling

The script exits non-zero on missing build outputs, failed tool commands, or copy failures. Error messages name the missing artifact or failed phase so a developer can distinguish Rust build failures, Swift build failures, and bundle assembly failures.

## Out Of Scope

- Code signing.
- Notarization.
- `.zip`, `.dmg`, or `.pkg` packaging.
- App icon generation.
- CI release publishing.
- Changes to the legacy Slint shell packaging metadata.
