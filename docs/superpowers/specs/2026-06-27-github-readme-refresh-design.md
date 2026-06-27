# GitHub README Refresh Design

## Goal

Rewrite the repository README so GitHub visitors quickly understand Cosmic Data Explorer as a product: a native macOS database workbench powered by a Rust database engine.

## Audience

The README should serve three groups:

- Developers evaluating whether the project is useful or interesting.
- Contributors who need build, test, and architecture context.
- Users who want an honest sense of what works today and what is planned.

## Positioning

Lead with:

> Cosmic Data Explorer is a native macOS database workbench powered by a Rust engine.

The tone should be polished and product-first without overstating maturity. The project should feel ambitious, credible, and runnable.

## Structure

The README should include:

1. A GitHub-friendly hero section with the project name, concise pitch, and lightweight badges.
2. A feature-focused section explaining why the project matters:
   - native macOS SwiftUI shell.
   - Rust database core.
   - PostgreSQL, MySQL/MariaDB, and SQLite support through SQLx.
   - tabbed query workspaces per active connection.
   - table explorer and row detail inspector.
   - Keychain-backed credential storage.
3. A status section that clearly marks the project as early alpha and runnable.
4. A quick-start section with prerequisites and build/run commands for the native macOS app.
5. A checks section with the Rust and Swift verification commands.
6. An architecture section explaining `apps/macos`, `crates/native-bridge`, `crates/engine`, and the legacy Slint shell.
7. A roadmap section that separates implemented capabilities from planned work.
8. A contribution note and GPL-3.0-or-later license note.

## Constraints

- Do not claim the app is production-ready.
- Do not imply installers, packaged releases, ER diagrams, row editing, import/export, autocomplete, or plugin support exist yet.
- Keep build commands accurate for the current repository.
- Prefer concise, scannable Markdown optimized for GitHub rendering.

## Verification

After editing, review the README for:

- factual consistency with the current Rust and Swift code.
- working command snippets.
- no unsupported product claims.
- clear status and roadmap boundaries.
