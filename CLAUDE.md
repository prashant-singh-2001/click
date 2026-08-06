# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**Click** — a Windows tray app (Tauri v2: Rust core + React 19/TypeScript UI) that launches a whole "workspace" (a bundle of app and URL actions) in one click, hotkey, desktop shortcut, or CLI command. Config is a single JSON file at `%APPDATA%\com.launchpad.app\workspaces.json`.

`docs/REQUIREMENTS.md` is the original design spec; code comments reference it by requirement id (`FR-4.4`, `NFR-3`) and by GitHub issue number (`issue #3`, `#32`). Both are useful when a piece of code looks arbitrary.

## Commands

```bash
npm install
npm run tauri dev             # full app with hot reload (Vite on :1420, fixed port)
npm run tauri build           # NSIS .exe + .msi under src-tauri/target/release/bundle/

# Frontend
npm run build                 # tsc && vite build
npx tsc --noEmit              # type-check only
npm run lint                  # eslint . (type-aware; see eslint.config.js)
npm test                      # vitest run
npx vitest run src/components/ActionEditor.test.tsx    # one file
npx vitest run -t "keeps focus"                        # one test by name

# Rust (all from src-tauri/)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test store::tests::round_trips_workspaces        # one test
```

CI (`.github/workflows/ci.yml`, windows-latest) runs exactly: `npm run lint`, `npm run build`, `npm test`, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. All six must pass.

`.github/workflows/smoke.yml` is separate and not a PR gate — nightly, on `workflow_dispatch`, and on pushes to `main`. It runs `npm run tauri build`, then `scripts/smoke-test.ps1`, which silent-installs the NSIS bundle and runs `click.exe run --id <uuid>` against a seeded throwaway workspace to prove the packaged app actually boots and launches (issue #24) — none of the `ci.yml` gates ever call `tauri::Builder::run`.

## Architecture

**The launch engine lives in Rust, never in the webview.** A desktop shortcut or `click run --id <uuid>` must boot a workspace without a WebView2 window existing, so all launching, config I/O, tray, hotkeys, and shortcut generation are native. `src/` is a pure editor that reads and writes config through `invoke` and spawns nothing.

All four "one click" triggers — UI button, tray menu, global hotkey, CLI/shortcut — funnel through `commands::launch_by_id` (`src-tauri/src/commands.rs`), which looks up the workspace and calls `launch::launch_workspace`. Add a trigger by calling that function, not by reimplementing launch.

Module map (`src-tauri/src/`): `lib.rs` (builder, plugin order, state, close-to-tray) · `model.rs` (serde data model) · `store.rs` (load/save + quarantine) · `vars.rs` (`${VAR}` resolution) · `launch.rs` (the engine) · `commands.rs` (the `#[tauri::command]` surface) · `tray.rs` · `hotkeys.rs` · `cli.rs` · `installed_apps.rs` (Start Menu scan for the app picker) · `shortcut.rs` (`.lnk` generation).

`src/api.ts` is the single typed wrapper around every `invoke`; components never call `invoke` directly.

### Load-bearing invariants

These encode fixed bugs — breaking one silently regresses real data loss or crash behavior.

- **`src/types.ts` hand-mirrors the Rust serde types.** No codegen. Change `model.rs`, `store.rs::LoadStatus`, `hotkeys.rs::HotkeyStatus`, or `installed_apps.rs::InstalledApp` and you must update `types.ts` in the same change.
- **Wire format is camelCase.** On enums with struct variants, `rename_all = "camelCase"` renames only the *variant* names — inner fields need `rename_all_fields = "camelCase"` too. Omitting it silently broke `delayAfterMs` once; `model.rs` and `store.rs` both carry regression tests pinning the exact wire shape.
- **Never overwrite a config that failed to load.** `store::load` returns a `LoadStatus`: `Ok`, `Recovered` (unparseable file was renamed to `workspaces.json.corrupt-<stamp>`, saving is safe), or `Blocked` (couldn't read *or* preserve it — `commands::persist` refuses to write). Saves go through the atomic temp-file + rename in `store::save` and the versioned `migrate()` hook.
- **One failing action never aborts a run.** `launch.rs` records `Failed` with a message naming the action and continues to the next. No `unwrap()` anywhere in the launch path.
- **No blocking work on the async runtime** (issue #3). Slow native work — the Start Menu scan, launch delays — must go through `tauri::async_runtime::spawn_blocking` or a dedicated thread, as `commands::list_installed_apps` does.
- **`.cmd`/`.bat` targets route through `cmd /C`** (`Command::new` can't spawn them), and every spawn sets `CREATE_NO_WINDOW` so there's no console flash.
- **Hotkey capture must suspend registrations first.** Windows `RegisterHotKey` swallows a bound combo system-wide, including from Click's own window, so the capture widget calls `suspend_hotkeys` → capture → `resume_hotkeys`. `hotkeys::register_all` re-registers from disk after every save and records a per-workspace `HotkeyStatus` the editor reads back; one bad combo must not take down the others.
- **Tray and hotkeys are rebuilt on every config change** — that happens inside `commands::persist`, so new mutating commands should call it rather than saving directly.

## Frontend tests

Vitest + React Testing Library, jsdom, `src/test/setup.ts`. Test files sit next to components as `*.test.tsx`.

Pattern: `vi.mock("../api")` at module top, then `import { mockedApi, resetApiMocks } from "../test/mockApi"` and `beforeEach(resetApiMocks)`. `resetApiMocks` pre-stubs the calls components fire on mount (hotkey status, validation, installed apps) so unrelated tests don't crash on unstubbed promises.

**`it.fails(...)` is deliberate here.** Some tests assert the *correct* behavior for known-open bugs (#2 variables-editor focus loss, #9 editor Launch using the saved record rather than the draft) and are marked `it.fails` so CI stays green today but turns red the moment the bug is fixed without updating the test. If you fix one of those bugs, flip the test to `it(...)` in the same change — don't delete it.

## Conventions

- Version lives in three places that must stay in sync: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
- Update `CHANGELOG.md` under `## [Unreleased]` with the issue link (`([#32])`) for user-facing changes.
- Branch from `main`; imperative commit subjects.
