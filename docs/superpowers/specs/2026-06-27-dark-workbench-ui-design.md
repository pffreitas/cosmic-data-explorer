# Dark Workbench UI Design

## Goal

Rework the desktop shell to match the supplied dark macOS workbench references: a persistent left sidebar for active database connections, a query workspace on the right, and a centered settings modal for connection configuration.

## Visual System

The app uses a dark, low-contrast desktop palette:

- Window background: near-black graphite.
- Sidebar: darker translucent-feeling panel with subtle border and selected-row highlight.
- Workspace: charcoal surface with compact top/tab bar, editor panel, and result panel.
- Accent: blue for active selection, focus, and primary settings actions.
- Typography: compact desktop sizing, 11px-18px, with heavier labels for section headings.

Cards are only used for repeated connection rows and settings rows. Page sections are full-height workbench regions, not floating marketing-style panels.

## Layout

The root window keeps a fixed left sidebar around 234px wide. The sidebar contains a macOS traffic-light header, an Active section, grouped active connection rows, and a settings button pinned to the bottom. The main workspace contains a compact title/tab strip, a query editor, and a result grid/status area.

Settings open as a centered modal with a dimmed backdrop. The modal follows the reference screenshot: macOS traffic lights, left settings navigation, and right-side connection configuration content.

## Initial Behavior

This slice is a visual shell with mock connection data. Clicking the sidebar settings button toggles the centered settings modal. Closing the modal returns to the query workspace. Real connection persistence and edit forms remain an engine integration task after the shell is visually aligned.

## Phase 1 Native-Faithful Refinement

The first native-look pass keeps the pure Slint shell but avoids drawing fake macOS system chrome. Slint widgets are compiled with the explicit `native` style, the sidebar reserves space for the real macOS titlebar, and the main/sidebar/settings surfaces use Apple-like graphite values, compact spacing, and 6-8px control radii. Surfaces that should later become AppKit material-backed are marked in source for the Phase 2 native interop pass.

## Verification

The desktop crate includes a UI contract test that checks the Slint source for the approved primitives: active connection sidebar, settings button, settings modal state, and connection configuration content. Build verification remains `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build -p desktop`.
