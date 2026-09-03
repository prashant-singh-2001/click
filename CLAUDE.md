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

All four "one click" triggers — UI button, tray menu, global hotkey, CLI/shortcut — funnel through `commands::launch_by_id` (`src-tauri/src/commands.rs`), which looks up the workspace and calls `commands::launch_resolved`. The editor's Launch button is a fifth path, `commands::launch_workspace_draft`, which skips the lookup — the frontend hands it the current draft directly rather than an id, so Launch runs exactly what's on screen, saved or not (issue #9). Both paths converge on `launch_resolved`, which calls `launch::launch_workspace`; add a trigger by calling `launch_resolved`, not by reimplementing launch.

Module map (`src-tauri/src/`): `lib.rs` (builder, plugin order, state, close-to-tray) · `model.rs` (serde data model) · `store.rs` (load/save + quarantine) · `vars.rs` (`${VAR}` resolution) · `launch.rs` (the engine) · `logging.rs` (rotating log, panic hook, fatal-startup dialog) · `commands.rs` (the `#[tauri::command]` surface) · `tray.rs` · `hotkeys.rs` · `cli.rs` · `installed_apps.rs` (Start Menu scan for the app picker) · `shortcut.rs` (`.lnk` generation) · `updates.rs` (updater check/prompt/install, issue #25).

`src/api.ts` is the single typed wrapper around every `invoke`; components never call `invoke` directly.

### Load-bearing invariants

These encode fixed bugs — breaking one silently regresses real data loss or crash behavior.

- **`src/types.ts` hand-mirrors the Rust serde types.** No codegen. Change `model.rs`, `store.rs::LoadStatus`, `hotkeys.rs::HotkeyStatus`, or `installed_apps.rs::InstalledApp` and you must update `types.ts` in the same change.
- **Wire format is camelCase.** On enums with struct variants, `rename_all = "camelCase"` renames only the *variant* names — inner fields need `rename_all_fields = "camelCase"` too. Omitting it silently broke `delayAfterMs` once; `model.rs` and `store.rs` both carry regression tests pinning the exact wire shape.
- **Never overwrite a config that failed to load.** `store::load` returns a `LoadStatus`: `Ok`, `Recovered` (unparseable file was renamed to `workspaces.json.corrupt-<stamp>`, saving is safe), or `Blocked` (couldn't read *or* preserve it — `commands::persist` refuses to write). Saves go through the atomic temp-file + rename in `store::save` and the versioned `migrate()` hook.
- **One failing action never aborts a run.** `launch.rs` records `Failed` with a message naming the action and continues to the next. No `unwrap()` anywhere in the launch path.
- **No blocking work on the async runtime** (issue #3). Slow native work — the Start Menu scan, launch delays — must go through `tauri::async_runtime::spawn_blocking` or a dedicated thread, as `commands::list_installed_apps` does.
- **A non-`async` `#[tauri::command]` runs inline on the WebView2 message-pump (UI) thread, not on a worker.** Traced through the vendored `tauri`/`wry` sources for issue #11: a command with no `async` is classified `ExecutionContext::Blocking` and called directly from inside the `ipc` custom-protocol handler, which on Windows is a `WebResourceRequested` COM callback on the main thread. Any filesystem or other slow work in one of those stalls the whole window, not just a request. This does **not** mean every command must be `async` — most are one-shot, click-initiated mutations (`save_workspace` and friends) and stay synchronous — but a command fired automatically from user input, like `validate_action`, must be `async` + `spawn_blocking` so a dead UNC path or slow disk can't freeze typing.
- **`.cmd`/`.bat` targets route through `cmd /C`** (`Command::new` can't spawn them), and every spawn sets `CREATE_NO_WINDOW` so there's no console flash.
- **`.cmd`/`.bat` targets (`ResolvedCommand::RunScript`) never go through `Command::args`** (issue #8). `Command::args` applies C-runtime quoting, which is the wrong dialect for a command line `cmd.exe` itself parses — a spacey script path plus a spacey argument produces four quote characters, which cmd's own quote-counting heuristic mangles. `launch.rs::execute` instead builds the whole command line with `cmd_command_line` and passes it via `CommandExt::raw_arg` with a forced `/S`, which removes that heuristic rather than trying to stay on its lucky side. Don't "simplify" this back to `.args()` — `cmd_quoting_survives_a_real_cmd_exe_round_trip` spawns a real `cmd.exe` to pin it, because neither Rust's exact `raw_arg` behavior nor cmd's quoting rule is documented anywhere else in this repo.
- **The args field's UI text and the stored `Vec<String>` are two different representations, converted by `src/args.ts`'s `parseArgs`/`formatArgs`.** They must stay inverses of each other — in particular, `["a","b"]` and `["a b"]` must format to visibly different strings, which is the whole point of issue #8. Backslash is deliberately never an escape character in this grammar (unlike `CommandLineToArgvW`), so a Windows path never needs special-casing.
- **Hotkey capture must suspend registrations first.** Windows `RegisterHotKey` swallows a bound combo system-wide, including from Click's own window, so the capture widget calls `suspend_hotkeys` → capture → `resume_hotkeys`. `hotkeys::register_all` re-registers from disk after every save and records a per-workspace `HotkeyStatus` the editor reads back; one bad combo must not take down the others.
- **Tray and hotkeys are rebuilt on every config change** — that happens inside `commands::persist`, so new mutating commands should call it rather than saving directly.
- **The webview runs under a strict CSP** (`app.security.csp` in `tauri.conf.json`, issue #18). `connect-src` must keep `http://ipc.localhost` — that is the URL Tauri's IPC `fetch` actually targets on Windows, so dropping it makes every `invoke` fail and bricks the whole UI. **`npm run tauri dev` does not enforce the CSP**: the webview loads `devUrl` (Vite) directly, so Tauri never serves the document and never sets the header. Only a bundled build applies it — a dev-mode pass proves nothing. Anything new that loads a remote resource, adds an inline `<style>`/`<script>`, or uses `convertFileSrc` needs a matching directive.
- **Nothing above DEBUG may log a resolved argument or working directory** (issue #6). `${VAR}` resolution can pull a secret from the environment, and this project's own `SECURITY.md` warns against plaintext secrets reaching disk. `ResolvedCommand::describe()` (logged at INFO in `launch.rs::execute`) must stay limited to the target path; only `describe_verbose()` (DEBUG, gated behind `CLICK_LOG=debug`) may include `args`/`cwd`. `launch.rs`'s `describe_never_contains_args_or_cwd` test pins this.
- **The log plugin is registered before `tauri-plugin-single-instance`** in `lib.rs`, not after. `single-instance`'s own setup hook calls `std::process::exit(0)` when it detects a second instance (after forwarding that instance's argv), so any plugin registered later never initializes in that forwarding process — which is exactly the desktop-shortcut / CLI path most in need of a log record. Don't reorder these without re-reading that comment in `lib.rs`.
- **`bundle.createUpdaterArtifacts` lives only in `src-tauri/tauri.release.conf.json`, never in the base `tauri.conf.json`** (issue #25). Setting it in the base config makes `tauri build` hard-fail without `TAURI_SIGNING_PRIVATE_KEY` set — which would break `smoke.yml`'s bare `npm run tauri build` and every contributor's local build. `plugins.updater.endpoints`/`pubkey` *are* in the base config and stay there safely: a configured pubkey alone doesn't require a private key at build time, only generating signed artifacts does. `release.yml` passes the overlay via `--config tauri.release.conf.json`, and `lib.rs`'s `updater_artifacts_are_release_only_not_in_the_base_config` test pins the split.

## Frontend tests

Vitest + React Testing Library, jsdom, `src/test/setup.ts`. Test files sit next to components as `*.test.tsx`.

Pattern: `vi.mock("../api")` at module top, then `import { mockedApi, resetApiMocks } from "../test/mockApi"` and `beforeEach(resetApiMocks)`. `resetApiMocks` pre-stubs the calls components fire on mount (hotkey status, validation, installed apps) so unrelated tests don't crash on unstubbed promises.

**`it.fails(...)` is deliberate here.** Some tests assert the *correct* behavior for known-open bugs (currently just #2, variables-editor focus loss) and are marked `it.fails` so CI stays green today but turns red the moment the bug is fixed without updating the test. If you fix one of those bugs, flip the test to `it(...)` in the same change — don't delete it.

## Conventions

- Version lives in three places that must stay in sync: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` — `release.yml` now checks this at tag time (issue #25); see `docs/RELEASING.md` for the full release checklist.
- Update `CHANGELOG.md` under `## [Unreleased]` with the issue link (`([#32])`) for user-facing changes.
- Branch from `main`; imperative commit subjects.
