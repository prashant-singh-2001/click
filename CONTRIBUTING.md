# Contributing to Click

Thanks for your interest in improving Click! This guide covers setting up a dev environment, the project's conventions, and how to get a change merged.

By participating, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

Click is a [Tauri v2](https://tauri.app) app — a Rust core with a React + TypeScript UI.

### Prerequisites (Windows)

| Tool | Notes |
|---|---|
| **Rust** | Install via [rustup](https://rustup.rs); use the **MSVC** toolchain (`stable-x86_64-pc-windows-msvc`). |
| **Visual Studio Build Tools 2022** | Install the **Desktop development with C++** workload. Tauri links against MSVC. |
| **Node.js** | 18 or newer. |
| **WebView2** | Preinstalled on Windows 11. |

### Getting started

```bash
git clone <your-fork-url>
cd workspace_launcher
npm install
npm run tauri dev      # builds Rust + starts Vite, opens the window with hot reload
```

The first `cargo` build compiles the full dependency tree and takes a few minutes; subsequent builds are incremental.

## Running checks

Please make sure all of these pass before opening a PR — CI runs the same set:

```bash
# Rust
cd src-tauri
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test

# Frontend (from the repo root)
npm run build          # tsc + vite
npx tsc --noEmit       # type-check only
```

## Project layout

```
src/                     React + TypeScript UI (a pure editor — never spawns processes)
  components/            WorkspaceList, WorkspaceEditor, ActionEditor, LaunchProgress
  api.ts, types.ts       invoke() wrappers and TS mirrors of the Rust model
src-tauri/src/           Rust core (all OS integration lives here)
  lib.rs                 builder, plugin registration, tray/hotkey/CLI wiring
  model.rs, store.rs     data model + persistence
  vars.rs, launch.rs     variable resolution + the launch engine
  commands.rs            the #[tauri::command] surface
  tray.rs, hotkeys.rs, cli.rs, shortcut.rs   the "one-click" triggers
docs/                    design spec (REQUIREMENTS.md) and assets
```

## Conventions

A few load-bearing rules that keep the codebase coherent:

- **The launch engine lives in Rust, not the webview.** Anything that spawns a process, touches the OS, or must work headless (desktop shortcuts, CLI) belongs in `src-tauri/src/`. The React UI only reads and edits config via `invoke`. Please don't move launch logic into TypeScript.
- **`src/types.ts` hand-mirrors the Rust serde types.** There's no codegen, so when you change `model.rs`, update `types.ts` in the same PR. Serialization is **camelCase** — remember `rename_all_fields = "camelCase"` on enums with struct variants (variant `rename_all` alone does *not* rename inner fields).
- **No `unwrap()` in the launch path.** A single failing action must never crash the app or abort the rest of the run — catch it, convert it to a message naming the action, and continue.
- **Rust formatting:** `cargo fmt` (rustfmt defaults). **Lints:** keep `cargo clippy -D warnings` clean.
- **Config safety:** writes to `workspaces.json` must stay atomic (temp file + rename) and go through the versioned `migrate()` hook in `store.rs`.

## Pull requests

1. Fork and branch from `main` (e.g. `feature/parallel-launch` or `fix/hotkey-conflict`).
2. Keep PRs focused; write a clear description of *what* and *why*.
3. Make sure the checks above pass and add tests for new logic where practical.
4. Update `CHANGELOG.md` under an `## [Unreleased]` heading and, if user-facing, the `README.md`.

### Commit messages

Write imperative, present-tense subjects ("Add parallel launch strategy"), with a body explaining the reasoning when it isn't obvious.

## Reporting bugs & requesting features

Use the [issue templates](https://github.com/) — **Bug report** or **Feature request**. For anything security-sensitive, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.
