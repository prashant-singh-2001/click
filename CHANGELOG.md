# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **The launch engine now has unit test coverage.** `launch.rs` is split into a pure
  planning step (variable resolution and `.cmd`/`.bat` routing, with no `AppHandle`) and a
  pure loop driver (skip-disabled, continue-on-failure, delay sequencing), with the actual
  process spawn / URL open reduced to a thin executor. No behavior change — this closes
  the coverage gap on load-bearing invariants (NFR-3, `.cmd`/`.bat` routing) that
  previously had no tests. ([#7])
- **The frontend is now linted** (type-aware ESLint, gated in CI alongside `cargo fmt` /
  `clippy`). Fixing what it found turned up a handful of `.then()` chains whose rejection
  went to an unhandled-promise-rejection warning instead of anywhere useful — those now
  log to the console explicitly. No user-visible behavior change. ([#13])

### Fixed

- **The UI is properly keyboard-navigable now** (NFR-7). There was no focus styling at
  all, so tabbing through the app was effectively invisible; every control now shows a
  focus ring. The hotkey field no longer traps focus — it swallowed *every* keystroke
  while capturing, Tab included, which left Escape as the only way out and nothing on
  screen saying so. Tab now exits capture, and a "Press Esc to cancel" hint appears while
  it's armed. The app picker keeps focus inside itself while open and hands it back to the
  button that opened it, which is what its `aria-modal` already claimed. Icon-only buttons
  (move up/down, remove action, remove variable) now announce what they do instead of
  reading out as "up arrow" or "multiplication x". ([#22])
- **A panic no longer permanently bricks the running app.** Every shared-state lock was
  `std::sync::Mutex`, which poisons on panic — one panic on any thread (the launch worker,
  a hotkey press, a tray click) would have made every later `.lock()` on that mutex panic
  too, forever, while the process itself kept running. Switched to `parking_lot::Mutex`
  (no poisoning, no `Result` to unwrap), and a `clippy.toml` now bans `std::sync::Mutex`
  so the pattern can't come back. Also: saving no longer holds the config lock across the
  disk write, so a save can no longer block the tray, hotkeys, or an in-flight launch for
  the duration of a `fs::write`. No user-visible behavior change. ([#4])
- **Deleting a workspace now asks first, and can be undone.** Delete used to remove a
  workspace immediately on click, with no confirmation and no way back. It now opens a
  confirmation dialog, and a successful delete leaves an "Undo" banner that restores the
  workspace to its exact original position in the list (not appended to the end) — actions,
  variables, hotkey, and all. The dialog's focus trap and focus restore reuse a new shared
  `Modal` component, extracted from the app picker (#22) rather than duplicated. ([#10])

### Security

- **The webview now runs under a Content-Security-Policy.** It was disabled outright
  (`"csp": null`); it is now `default-src 'self'`, with the Tauri IPC endpoint allowed
  through `connect-src` and `object-src` / `base-uri` / `frame-ancestors` / `form-action`
  locked down. The app loads no remote resources and executes no inline or dynamic script,
  so nothing changes in use — this is defense-in-depth, so that a workspace name, tag, or
  path that ever reached the DOM as markup couldn't pull in outside code. ([#18])

## [0.2.3] — 2026-08-04

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
[0.2.3]: https://github.com/prashant-singh-2001/click/releases/tag/v0.2.3
[0.2.2]: https://github.com/prashant-singh-2001/click/releases/tag/v0.2.2
[0.2.0]: https://github.com/prashant-singh-2001/click/releases/tag/v0.2.0
[0.1.0]: https://github.com/prashant-singh-2001/click/releases/tag/v0.1.0
[#3]: https://github.com/prashant-singh-2001/click/issues/3
[#4]: https://github.com/prashant-singh-2001/click/issues/4
[#10]: https://github.com/prashant-singh-2001/click/issues/10
[#7]: https://github.com/prashant-singh-2001/click/issues/7
[#13]: https://github.com/prashant-singh-2001/click/issues/13
[#18]: https://github.com/prashant-singh-2001/click/issues/18
[#22]: https://github.com/prashant-singh-2001/click/issues/22
[#23]: https://github.com/prashant-singh-2001/click/issues/23
[#5]: https://github.com/prashant-singh-2001/click/issues/5
[#14]: https://github.com/prashant-singh-2001/click/issues/14
[#19]: https://github.com/prashant-singh-2001/click/issues/19
[#32]: https://github.com/prashant-singh-2001/click/issues/32
