# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Icon, color, and tags are now editable**, and workspaces can be searched by name or
  tag. The editor gets an Appearance section (an emoji icon picker, a color swatch, and
  tag chips); the workspace list shows the icon, a color accent, and tag chips on each
  card, plus a search box that filters by name or tag. These fields were persisted since
  v0.1.0 but had no UI. ([#23])

### Fixed

- **Launching a workspace no longer blocks the app.** Every launch trigger (the
  Launch button, tray menu, global hotkeys, and `click run`) now runs on a
  dedicated blocking thread instead of the async runtime, so the UI, tray, and
  hotkeys all stay responsive for the full duration of a multi-action launch
  with delays between actions. Also: the delay after the last action in a
  workspace is no longer applied, so a launch returns immediately once its
  final action starts instead of waiting out one more unnecessary delay.
  ([#3])

## [0.2.2] — 2026-07-28

### Added

- **Frontend test suite** — Vitest + React Testing Library, gated in CI. Includes
  known-failing regression tests for two open bugs (variables-editor focus loss, #2;
  editor Launch using the saved record instead of the current draft, #9) so a future
  fix can't silently leave them broken. ([#14])
- **App picker** — adding an app action now offers **Choose app…** alongside the
  existing path field and file-browse dialog: a searchable list of installed
  applications (scanned from the Start Menu), so adding VS Code or Docker no longer
  means typing or hunting for its install path. ([#32])

## [0.2.0] — 2026-07-26

### Added

- **Click-to-capture hotkey input** — the global-hotkey field is now a capture widget
  instead of free text: click it, press the combo, and it's translated to the
  accelerator syntax automatically. ([#19])

### Fixed

- Hotkey registration failures are no longer silent. The editor now shows whether a
  workspace's hotkey is active, invalid, already used by another workspace, or
  conflicting with another app's binding — both live, while capturing a new combo,
  and for the hotkey that's actually saved. ([#5])

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

[Unreleased]: https://github.com/prashant-singh-2001/click/commits/main
[0.2.2]: https://github.com/prashant-singh-2001/click/releases/tag/v0.2.2
[0.2.0]: https://github.com/prashant-singh-2001/click/releases/tag/v0.2.0
[0.1.0]: https://github.com/prashant-singh-2001/click/releases/tag/v0.1.0
[#3]: https://github.com/prashant-singh-2001/click/issues/3
[#23]: https://github.com/prashant-singh-2001/click/issues/23
[#5]: https://github.com/prashant-singh-2001/click/issues/5
[#14]: https://github.com/prashant-singh-2001/click/issues/14
[#19]: https://github.com/prashant-singh-2001/click/issues/19
[#32]: https://github.com/prashant-singh-2001/click/issues/32
