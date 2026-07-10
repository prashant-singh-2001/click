# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-07-11

Initial release. A Windows desktop app (Tauri v2 + React) that launches a whole
development environment from a single named workspace.

### Added

- **Workspaces** — create, edit, duplicate, and delete named bundles of actions,
  persisted to a human-readable `workspaces.json`.
- **App actions** — launch executables with command-line arguments and a working
  directory; correct handling of `.cmd`/`.bat` shims (e.g. `code`) and no console
  flash on launch.
- **URL actions** — open links in the system default browser.
- **Sequential launch** with a configurable global delay and per-action override;
  a single failing action is reported but never aborts the rest of the run.
- **Variables** — `${VAR}` placeholders resolved at launch time from the workspace
  map, falling back to process environment variables.
- **Per-action enable/disable and labels.**
- **Live launch progress** with per-action status and specific error messages.
- **System tray** with a per-workspace quick-launch menu (rebuilt on every config
  change).
- **Global hotkeys** — assign a shortcut per workspace.
- **CLI** — `click run --id <uuid>`, with **single-instance** forwarding so a second
  invocation runs in the existing process instead of starting a duplicate.
- **Desktop-shortcut generation** — creates a real `.lnk` that launches a workspace
  on double-click without the app needing to be open first.
- **Validation** of executable paths and URLs in the editor before launch.
- **Windows installers** — NSIS `.exe` and `.msi` via `tauri build`.

### Notes

- Developed under the working title **LaunchPad**; released as **Click**.
- The application bundle identifier remains `com.launchpad.app` so existing
  configuration keeps working.
- Builds are **not code-signed** yet; Windows SmartScreen warns on first run.

### Fixed (during initial development)

- Desktop-shortcut creation now resolves the real Desktop folder via the Windows
  known-folder API, fixing a failure on machines where OneDrive redirects Desktop
  to `%USERPROFILE%\OneDrive\Desktop`.
- Action fields now serialize as camelCase (`delayAfterMs`); `rename_all` on the
  `Action` enum previously left inner fields snake_case, silently dropping the value.

[Unreleased]: https://github.com/
[0.1.0]: https://github.com/
