# One-Click Workspace Launcher — Software Requirements Specification

> Working title during design: **LaunchPad** (shipped as **Click**).
> Version: 1.0 (draft) · Status: For reference · Type: Desktop application (Windows-first, cross-platform target)

> **Note:** This is the original design specification / SRS that guided the build. The shipped product is named **Click**. This document is preserved as a design reference; where it says "LaunchPad", read "Click".

---

## 1. Overview

### 1.1 Problem statement
Starting a development session means manually opening the same collection of tools every time: an IDE (with the right folder), a database client, Docker, a terminal running the dev server, and several browser tabs (localhost, the repo, the ticket board, docs). This is repetitive, slow, easy to get wrong, and has to be repeated per project. There is no single artifact that captures "everything I need for *this* project" and boots it in one action.

### 1.2 Product vision
A lightweight desktop app that lets anyone define a named **Workspace** — a bundle of apps, URLs, commands, and files — and launch the entire bundle with one click, one hotkey, or one desktop shortcut. Think "a startup script for your whole working environment, but visual, shareable, and reversible."

### 1.3 Goals
- Define reusable workspaces that open multiple apps + URLs + commands together.
- Launch a workspace in **one action** (button, global hotkey, desktop shortcut, or CLI).
- Make workspaces **shareable and portable** across machines.
- Keep the app fast, small, and unobtrusive (lives in the system tray).

### 1.4 Non-goals (explicitly out of scope for v1)
- Not a task runner / build system replacement (Make, npm scripts, Taskfile) — it *invokes* those, it doesn't replace them.
- Not a container orchestrator — it can run `docker compose up`, but it doesn't manage container lifecycles beyond invoking commands.
- Not a window manager / tiling tool — it launches apps but does not arrange or snap windows in v1 (candidate for a later phase).
- Not a remote-access or SSH session manager (later phase, optional).

### 1.5 Target users
| Persona | Primary need |
|---|---|
| **Developer** (primary) | Boot a full dev environment per project: IDE + DB + Docker + terminal + localhost tabs. |
| **Designer / creator** | Open design tools + reference tabs + asset folders for a given project. |
| **Data analyst** | Open notebook/IDE + DB client + dashboards + data folder. |
| **General power user** | "Start of workday" bundle: mail, chat, calendar, music, notes. |

---

## 2. Glossary

| Term | Definition |
|---|---|
| **Workspace** | A named, reusable collection of actions that are launched together. (Also called a "profile" or "preset".) |
| **Action** | A single thing to launch or run: an app, a URL, a command, or a file/folder. |
| **Target** | The concrete thing an action points to (an executable path, a URL, a shell command). |
| **Launch** | The act of executing all enabled actions in a workspace. |
| **Teardown** | The reverse of launch — closing/stopping what a workspace started. |
| **Readiness gate** | A condition that must be met before the next action runs (e.g., "port 5432 is listening"). |
| **Variable** | A named placeholder (e.g., `${PROJECT_DIR}`) resolved at launch time to make configs portable. |

---

## 3. Functional requirements

Requirements are grouped and numbered (FR-*n.m*) for traceability. Each has a priority: **[MVP]**, **[V1]**, or **[Later]**.

### 3.1 Workspace management
- **FR-1.1 [MVP]** Users can create, rename, edit, duplicate, and delete workspaces.
- **FR-1.2 [MVP]** Each workspace has a name and optional description.
- **FR-1.3 [V1]** Each workspace supports an icon (emoji or image) and a color for quick visual identification.
- **FR-1.4 [V1]** Each workspace supports tags/categories for grouping and filtering.
- **FR-1.5 [V1]** The main view lists all workspaces with search/filter by name and tag.
- **FR-1.6 [V1]** Users can reorder workspaces (drag-and-drop) and pin favorites.

### 3.2 Action types
- **FR-2.1 [MVP] — Launch an application.** Specify an executable path (browse or type). Support:
  - **FR-2.1.1 [MVP]** Command-line arguments (e.g., open VS Code at a folder: `code ${PROJECT_DIR}`).
  - **FR-2.1.2 [V1]** Working directory (`cwd`) for the process.
  - **FR-2.1.3 [V1]** Custom environment variables for the process.
  - **FR-2.1.4 [V1]** "Run as administrator / elevated" option (Windows) / sudo prompt awareness.
  - **FR-2.1.5 [V1]** "Skip if already running" — detect a running instance and don't launch a duplicate (e.g., don't open a second Docker Desktop).
- **FR-2.2 [MVP] — Open a URL.**
  - **FR-2.2.1 [MVP]** Open in the system default browser.
  - **FR-2.2.2 [V1]** Open in a specific browser (Chrome, Firefox, Edge, Brave, …).
  - **FR-2.2.3 [V1]** Open in a specific browser profile (e.g., work vs personal Chrome profile).
  - **FR-2.2.4 [V1]** Choose window behavior: new tab, new window, or group all workspace URLs into one window.
  - **FR-2.2.5 [Later]** Incognito/private mode option.
- **FR-2.3 [V1] — Run a command / script.**
  - **FR-2.3.1 [V1]** Run a shell command (e.g., `docker compose up -d`, `npm run dev`, `pg_ctl start`).
  - **FR-2.3.2 [V1]** Choose the shell (cmd, PowerShell, bash, zsh) and working directory.
  - **FR-2.3.3 [V1]** Choose visibility: run in a visible terminal window, or run hidden in the background.
  - **FR-2.3.4 [V1]** "Keep terminal open after exit" — needed for long-running dev servers vs run-and-close for setup commands.
- **FR-2.4 [V1] — Open a file or folder** in its default app or a specified app (e.g., open a `.sql` file in a DB client, open a project folder in Explorer/Finder).
- **FR-2.5 [V1]** Every action can be individually enabled/disabled without deleting it (toggle for quick experimentation).
- **FR-2.6 [V1]** Each action has a user-friendly label independent of its target.

### 3.3 Launch orchestration
- **FR-3.1 [MVP]** Launch all enabled actions in a workspace with one action (button click).
- **FR-3.2 [V1]** Choose launch strategy per workspace: **sequential** (one after another) or **parallel** (all at once).
- **FR-3.3 [V1]** Configurable delay between actions (global default + per-action override) to avoid overwhelming the machine and to respect ordering.
- **FR-3.4 [V1] — Readiness gates.** An action can wait for a condition before proceeding:
  - **FR-3.4.1 [V1]** Wait until a TCP port is listening (e.g., DB on `5432`) before opening its client.
  - **FR-3.4.2 [V1]** Wait until an HTTP endpoint returns an expected status (e.g., `localhost:3000` → 200) before opening the browser tab.
  - **FR-3.4.3 [V1]** Wait until a process/service is running (e.g., Docker daemon ready) before running `docker compose`.
  - **FR-3.4.4 [V1]** Each gate has a timeout and a poll interval; on timeout, apply the configured failure policy.
- **FR-3.5 [V1] — Failure policy** per action: *continue on error* vs *abort remaining actions*. Failures are surfaced to the user (see FR-6.4).
- **FR-3.6 [V1]** Show live launch progress: which actions have started, are waiting on a gate, succeeded, or failed.

### 3.4 Triggers (the "one click")
- **FR-4.1 [MVP]** Launch a workspace from within the app's main window.
- **FR-4.2 [V1]** **System tray icon** with a menu listing workspaces for quick launch without opening the main window.
- **FR-4.3 [V1]** **Global hotkey** assignable per workspace (e.g., `Ctrl+Alt+1`).
- **FR-4.4 [V1]** **Desktop shortcut generation** — create a per-workspace shortcut (`.lnk` on Windows, `.desktop` on Linux, `.app`/alias on macOS) that launches that workspace directly. This is the literal one-click-from-desktop experience.
- **FR-4.5 [V1]** **CLI invocation** — `launchpad run "Project X"` to launch by name/ID (enables scripting and Start-menu/Spotlight use).
- **FR-4.6 [Later]** Custom URI scheme — `launchpad://run/<id>` for launching from links/other apps.
- **FR-4.7 [Later]** Scheduling / run-on-login — auto-launch a workspace at a time or at system startup.

### 3.5 Teardown (close a workspace)
- **FR-5.1 [Later]** Track processes started by a launch so the app can offer a **"Close workspace"** action.
- **FR-5.2 [Later]** Gracefully close launched apps and stop commands/services the workspace started (e.g., `docker compose down`).
- **FR-5.3 [Later]** Distinguish "close only what this workspace started" from "close everything" to avoid killing unrelated user work.
  - *Note: reliable teardown is technically hard — apps fork, detach, and share processes. This is intentionally deferred and scoped conservatively.*

### 3.6 Configuration, persistence & portability
- **FR-6.1 [MVP]** Persist all workspaces and settings locally between sessions (survives app restart and reboot).
- **FR-6.2 [V1]** Store configuration in a **human-readable, version-controllable format** (JSON/YAML) so users can commit a workspace config into a project repo.
- **FR-6.3 [V1] — Variables & path portability.** Support variables (e.g., `${PROJECT_DIR}`, `${HOME}`) so the same workspace config works on machines where paths differ. Variables are resolved at launch time.
- **FR-6.4 [V1]** **Import / export** workspaces (single or bulk) as a file for backup and sharing.
- **FR-6.5 [V1]** On import, if referenced apps/paths are missing on this machine, prompt the user to locate or remap them rather than failing silently.
- **FR-6.6 [Later]** Optional cloud sync of workspaces across a user's machines.
- **FR-6.7 [Later]** Workspace templates and a starter library (e.g., "MERN dev", "Spring Boot dev", "Data analysis").

### 3.7 Error handling & feedback (cross-cutting)
- **FR-7.1 [MVP]** Validate an executable path/URL when added; warn if a path doesn't exist.
- **FR-7.2 [V1]** If an action fails at launch (missing binary, non-zero exit, gate timeout), show a clear, specific error naming the action and reason.
- **FR-7.3 [V1]** Maintain a launch log/history viewable in-app for troubleshooting.
- **FR-7.4 [V1]** A "Test / dry-run" mode that shows exactly what *would* run without executing it — especially important before running an imported workspace (see §7 Security).

---

## 4. Non-functional requirements

- **NFR-1 Performance.** App cold-start ≤ 2s; tray-menu launch of a workspace begins executing in < 200ms after click. Idle RAM footprint should be modest (a tray utility, not a heavy app).
- **NFR-2 Footprint.** Prefer a small installer/binary; the app should feel like a utility, not a suite.
- **NFR-3 Reliability.** A single failing action must not crash the app or block unrelated actions beyond the configured failure policy.
- **NFR-4 Usability.** A new user can create and launch their first workspace in under 3 minutes with no documentation.
- **NFR-5 Portability.** Windows is the primary target (the requested `.exe`). Architecture should not preclude macOS and Linux (deferred, not designed out).
- **NFR-6 Single instance.** Only one instance of the app runs; a second invocation focuses the existing window or forwards a CLI command to it.
- **NFR-7 Accessibility.** Keyboard-navigable UI; respects system light/dark theme.
- **NFR-8 Maintainability.** Config schema is versioned with a migration path for future changes.
- **NFR-9 Auto-update.** Support in-app or background updates (V1/Later), so users stay current.
- **NFR-10 Observability.** Local crash/error logs to aid debugging and user support.

---

## 5. Data model (illustrative)

A workspace config, conceptually:

```jsonc
{
  "version": "1.0",
  "workspaces": [
    {
      "id": "b3f1…",                       // stable UUID
      "name": "Project X — Backend Dev",
      "description": "Spring Boot API + Postgres + local frontend",
      "icon": "🚀",
      "color": "#4F46E5",
      "tags": ["work", "backend"],
      "variables": { "PROJECT_DIR": "C:/dev/project-x" },
      "launchStrategy": "sequential",       // "sequential" | "parallel"
      "defaultDelayMs": 500,
      "actions": [
        {
          "type": "app",
          "label": "IntelliJ IDEA",
          "path": "C:/Program Files/JetBrains/IDEA/bin/idea64.exe",
          "args": ["${PROJECT_DIR}"],
          "cwd": "${PROJECT_DIR}",
          "skipIfRunning": true,
          "enabled": true,
          "delayAfterMs": 1000
        },
        {
          "type": "app",
          "label": "Docker Desktop",
          "path": "C:/Program Files/Docker/Docker/Docker Desktop.exe",
          "skipIfRunning": true,
          "waitFor": { "type": "process", "name": "com.docker.backend", "timeoutMs": 60000 }
        },
        {
          "type": "command",
          "label": "Start containers",
          "shell": "powershell",
          "command": "docker compose up -d",
          "cwd": "${PROJECT_DIR}",
          "keepOpen": false,
          "waitFor": { "type": "port", "host": "localhost", "port": 5432, "timeoutMs": 30000 }
        },
        {
          "type": "command",
          "label": "Dev server",
          "shell": "powershell",
          "command": "npm run dev",
          "cwd": "${PROJECT_DIR}/web",
          "keepOpen": true
        },
        {
          "type": "url",
          "label": "App (localhost)",
          "url": "http://localhost:3000",
          "browser": "chrome",
          "browserProfile": "Work",
          "waitFor": { "type": "http", "url": "http://localhost:3000", "expectStatus": 200, "timeoutMs": 60000 }
        },
        { "type": "url", "label": "Repo", "url": "https://github.com/me/project-x" },
        { "type": "url", "label": "Board", "url": "https://jira.example.com/project-x" }
      ]
    }
  ]
}
```

This schema is a design reference, not a contract — field names firmed up during implementation. (In the shipped product, action fields use camelCase, e.g. `delayAfterMs`.)

---

## 6. UI / UX requirements

### 6.1 Screens
1. **Workspace list (home).** Grid/list of workspaces with icon, name, tags, and a prominent **Launch** button per item; search bar; "New workspace" button.
2. **Workspace editor.** Name/description/icon/color/tags; an ordered list of actions with add/edit/remove/reorder/enable-toggle; launch-strategy and delay settings; buttons for **Launch**, **Dry-run**, **Create desktop shortcut**, **Export**.
3. **Action editor.** Type selector (App / URL / Command / File); type-specific fields; optional readiness-gate config; failure policy.
4. **Launch progress panel.** Live per-action status (queued → waiting → running → done/failed) with error details.
5. **Settings.** Theme, run-on-startup, global defaults, hotkeys, import/export, about/update.
6. **System tray menu.** Quick-launch list + "Open app" + "Quit".

### 6.2 Key flows
- **Create & launch:** New workspace → add a couple of apps and URLs → Launch. (Must feel effortless — this is NFR-4.)
- **Add app by browsing:** File picker for the executable; auto-fill a sensible label from the file name.
- **Turn a workspace into a desktop shortcut:** one button → shortcut appears on the desktop → double-click launches the workspace with the app not needing to be already open.
- **Share a workspace:** Export → send file → recipient imports → gets prompted to remap any missing paths.

---

## 7. Security & safety considerations

This app launches applications and **runs arbitrary shell commands** — that power is the point, but it means configs are executable content. Treat security as a first-class requirement:

- **SEC-1** When importing a workspace from an untrusted source, **show exactly what it will run** (all commands, paths, and args) and require explicit confirmation before the first launch. Never silently execute an imported config.
- **SEC-2** A **dry-run/preview** must be available for any workspace (ties to FR-7.4).
- **SEC-3** Do not store secrets in plaintext. If actions carry environment variables that may contain credentials, warn the user and/or integrate with the OS secret store rather than the plain config file.
- **SEC-4** Keep secrets and machine-specific paths **out of exported configs** by default (use variables), so sharing a workspace doesn't leak local details.
- **SEC-5** **Code-sign the executable/installer.** An unsigned `.exe` that spawns processes will trigger SmartScreen/Gatekeeper warnings and erode trust; signing is required for a distributable build.

---

## 8. Technical architecture & stack options

The app is UI + heavy OS integration (spawning processes, detecting running apps, port/HTTP checks, registry/shortcut creation, tray, global hotkeys). Any of the following can meet the requirements; pick based on your goals.

| Option | Pros | Cons | Best when |
|---|---|---|---|
| **Tauri** (Rust core + web UI in TS) | Very small binary (~5–15 MB), low RAM, strong security model, native OS APIs, modern | Process spawning / OS bits are in Rust (learning curve) | You want the best end-product and a strong portfolio piece; willing to learn some Rust |
| **Electron** (Node + web UI in TS) | Fastest path given your JS/TS momentum; `child_process`, huge ecosystem; everything in one language | Large installer (~80–150 MB), higher RAM | You want to ship quickly leveraging current TypeScript skills |
| **Java + JavaFX + jpackage** | Plays directly to your Spring/Java strength; `ProcessBuilder` for launching; `jpackage` produces a native installer/`.exe`; cross-platform | Less trendy UI toolkit; larger runtime unless trimmed with jlink | You want to leverage deep Java expertise and ship pragmatically |
| **.NET (C#) + WinUI/WPF** | Most native Windows integration (shortcuts, registry, processes); excellent tooling | Windows-first; new language for you | Windows is the only target and native feel matters most |

**Chosen stack:** **Tauri** (Rust core + React/TypeScript UI) — smallest/fastest end product with strong OS integration.

**Core building blocks (any stack):**
- Process launching with args, cwd, env, and elevation.
- "Is it running?" detection (process enumeration / named-mutex style checks).
- Readiness probes: TCP port check, HTTP request-with-retry, process/service check.
- OS shortcut creation (`.lnk` / `.desktop` / macOS alias) and global-hotkey registration.
- System-tray integration and single-instance enforcement.
- Config storage (file-based JSON/YAML; SQLite optional if the list grows large).

---

## 9. Phased roadmap

**Phase 1 — MVP (prove the core loop):**
Create/edit/delete workspaces · App actions (path + args) and URL actions · Sequential launch with a fixed delay · Launch from the main window · Local persistence · Windows `.exe`.

**Phase 2 — V1 (make it genuinely useful):**
System tray + global hotkeys + desktop-shortcut generation + CLI (the real one-click) · Command/script actions · Parallel launch and per-action delays · Readiness gates (port/HTTP/process) · "Skip if running" · Import/export + variables for portability · Icons/colors/tags/search · Browser & profile selection · Dry-run and launch log.

**Phase 3 — Later (differentiate):**
Teardown / "close workspace" · Auto-detect installed apps · Scheduling & run-on-login · macOS/Linux support · Cloud sync · Templates/starter library · Auto-update · Optional window arrangement.

---

## 10. Acceptance criteria (MVP definition of done)

- A user can create a workspace, add at least one app action (with args) and at least one URL action, save it, close and reopen the app, and see it persisted.
- Clicking **Launch** opens the configured app(s) with the correct arguments and the URL(s) in the browser, in the configured order.
- Invalid executable paths are flagged before launch, and a failed action produces a clear, specific error without crashing the app.
- The build produces a runnable Windows `.exe`.
