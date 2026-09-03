use crate::hotkeys::HotkeyStatus;
use crate::installed_apps::{self, InstalledApp};
use crate::launch::{self, ActionStatus, LaunchReport};
use crate::model::{Action, Workspace};
use crate::store;
use crate::updates::{self, UpdateStatus, UpdateTrigger};
use crate::AppState;
use std::fmt;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

/// Which of the four ways a launch can be triggered fired this one. Logged
/// alongside every outcome so a report can distinguish "the tray launch
/// failed" from "the Launch button failed" — before this, three of the four
/// triggers (tray, hotkey, CLI) discarded the `LaunchReport` entirely and
/// left no record of success or failure anywhere.
#[derive(Debug, Clone, Copy)]
pub enum LaunchTrigger {
    Ui,
    Tray,
    Hotkey,
    Cli,
    /// The editor's Launch button, launching the current draft directly
    /// rather than the last-saved record (issue #9). Kept distinct from
    /// `Ui` so a log entry never claims to have run the persisted workspace
    /// when it actually ran unsaved edits.
    Draft,
}

impl fmt::Display for LaunchTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LaunchTrigger::Ui => "ui",
            LaunchTrigger::Tray => "tray",
            LaunchTrigger::Hotkey => "hotkey",
            LaunchTrigger::Cli => "cli",
            LaunchTrigger::Draft => "draft",
        })
    }
}

fn config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

fn persist(app: &AppHandle, state: &State<AppState>) -> Result<(), String> {
    // Never write over a config we failed to read but couldn't set aside —
    // it may still be intact, and overwriting it would destroy the user's
    // workspaces for good.
    let blocked = state
        .config_status
        .lock()
        .save_block_reason()
        .map(str::to_string);
    if let Some(reason) = blocked {
        log::error!("save refused: config load was blocked ({reason})");
        return Err(format!(
            "Click won't save because it couldn't read your existing config \
             and won't risk overwriting it ({reason}). Move or fix that file, \
             then restart Click."
        ));
    }

    // Snapshot under the lock, then write to disk outside it (issue #4):
    // store::save does create_dir_all + serialize + write + rename, and
    // holding the lock across that blocked the tray rebuild, the hotkey
    // handler, and any in-flight launch for the duration of a disk write.
    // Tradeoff: this no longer serializes concurrent saves against each
    // other. That's fine today — every mutating command runs on the main
    // thread — but if one ever moves to spawn_blocking, two overlapping
    // saves could hit disk out of order and need a dedicated save lock.
    let snapshot = state.file.lock().clone();
    let dir = config_dir(app)?;
    if let Err(err) = store::save(&dir, &snapshot) {
        log::error!("failed to save config: {err}");
        return Err(err.to_string());
    }

    crate::tray::rebuild(app);
    crate::hotkeys::register_all(app);
    Ok(())
}

/// Lets the UI warn the user when their config was quarantined or when
/// saving is disabled (issue #1).
#[tauri::command]
pub fn config_status(state: State<AppState>) -> store::LoadStatus {
    state.config_status.lock().clone()
}

#[tauri::command]
pub fn list_workspaces(state: State<AppState>) -> Vec<Workspace> {
    state.file.lock().workspaces.clone()
}

#[tauri::command]
pub fn get_workspace(state: State<AppState>, id: String) -> Result<Workspace, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state
        .file
        .lock()
        .workspaces
        .iter()
        .find(|w| w.id == uuid)
        .cloned()
        .ok_or_else(|| format!("workspace {id} not found"))
}

/// Inserts on new id, replaces on existing id — the editor always sends a
/// full `Workspace`, so upsert-by-id keeps the frontend from needing two
/// separate calls.
#[tauri::command]
pub fn save_workspace(
    app: AppHandle,
    state: State<AppState>,
    workspace: Workspace,
) -> Result<(), String> {
    {
        let mut file = state.file.lock();
        if let Some(existing) = file.workspaces.iter_mut().find(|w| w.id == workspace.id) {
            *existing = workspace;
        } else {
            file.workspaces.push(workspace);
        }
    }
    persist(&app, &state)
}

#[tauri::command]
pub fn delete_workspace(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut file = state.file.lock();
        file.workspaces.retain(|w| w.id != uuid);
    }
    persist(&app, &state)
}

/// Re-inserts `workspace` at `index`. Pure and unit-tested on its own: `save`
/// upserts by id and pushes when the id is absent, so undoing a delete
/// through `save_workspace` would silently move the workspace to the end of
/// the list — this exists so undo (issue #10) can restore its original
/// position instead. Defensive on both edges: the index is clamped rather
/// than panicking on an out-of-range value, and an id that's somehow already
/// present is replaced in place rather than duplicated.
fn restore_at(workspaces: &mut Vec<Workspace>, workspace: Workspace, index: usize) {
    workspaces.retain(|w| w.id != workspace.id);
    let at = index.min(workspaces.len());
    workspaces.insert(at, workspace);
}

/// Undoes a delete (issue #10): re-inserts `workspace` at `index` rather
/// than appending it, so the list looks exactly as it did before. Runs
/// through `persist` like every other mutation, so the restored workspace's
/// hotkey and tray entry come back too.
#[tauri::command]
pub fn restore_workspace(
    app: AppHandle,
    state: State<AppState>,
    workspace: Workspace,
    index: usize,
) -> Result<(), String> {
    {
        let mut file = state.file.lock();
        restore_at(&mut file.workspaces, workspace, index);
    }
    persist(&app, &state)
}

#[tauri::command]
pub fn duplicate_workspace(
    app: AppHandle,
    state: State<AppState>,
    id: String,
) -> Result<Workspace, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let duplicate = {
        let mut file = state.file.lock();
        let original = file
            .workspaces
            .iter()
            .find(|w| w.id == uuid)
            .cloned()
            .ok_or_else(|| format!("workspace {id} not found"))?;
        let mut copy = original;
        copy.id = Uuid::new_v4();
        copy.name = format!("{} (copy)", copy.name);
        file.workspaces.push(copy.clone());
        copy
    };
    persist(&app, &state)?;
    Ok(duplicate)
}

/// FR-7.1: flags a missing app path, a URL without a scheme, or args/cwd on
/// a target that can't accept them (issue #17 — see `launch::route_for`).
/// Never blocks saving — the path may legitimately not exist on this
/// machine yet.
fn validate(action: Action) -> Option<String> {
    match action {
        Action::App {
            path, args, cwd, ..
        } => {
            if path.trim().is_empty() {
                Some("path is empty".to_string())
            } else if path.contains("${") {
                None // contains a variable; can't validate until launch time
            } else if !Path::new(&path).exists() {
                Some(format!("path does not exist: {path}"))
            } else if matches!(launch::route_for(&path), launch::Route::Shell)
                && (!args.is_empty() || cwd.is_some())
            {
                Some(format!(
                    "'{path}' opens with its associated app, which can't accept arguments or \
                     a working directory"
                ))
            } else {
                None
            }
        }
        Action::Url { url, .. } => {
            if url.contains("${") {
                None
            } else {
                match url::Url::parse(&url) {
                    Ok(parsed) if !parsed.scheme().is_empty() => None,
                    _ => Some(format!("not a valid URL: {url}")),
                }
            }
        }
    }
}

/// Fired from the editor on every keystroke rather than an explicit click, so
/// unlike its sync siblings this one must not run on the UI thread: a
/// non-async command runs inline on the WebView2 message pump, and the
/// `Path::exists()` in `validate` is a `stat` that blocks to the SMB timeout
/// on a dead UNC path or a spun-down mapped drive — freezing the whole
/// window, not just a worker (issue #11).
#[tauri::command]
pub async fn validate_action(action: Action) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || validate(action))
        .await
        .unwrap_or_else(|err| {
            log::error!("validation task panicked: {err}");
            None
        })
}

/// Runs the launch on a blocking thread — `launch::launch_workspace` sleeps
/// between actions with `std::thread::sleep`, and doing that on an async
/// worker thread pins it for the sum of every delay (issue #3).
#[tauri::command]
pub async fn launch_workspace_by_id(app: AppHandle, id: String) -> Result<LaunchReport, String> {
    tauri::async_runtime::spawn_blocking(move || launch_by_id(&app, &id, LaunchTrigger::Ui))
        .await
        .map_err(|e| format!("launch task failed: {e}"))?
}

/// Launches the workspace exactly as the editor currently shows it, saved or
/// not — the tray menu, global hotkeys, and the CLI's `run` subcommand all
/// launch a persisted record by id (`launch_workspace_by_id`); this is the
/// editor's Launch button, which must run the draft on screen instead
/// (issue #9). Persists nothing: no disk write, no tray rebuild, no hotkey
/// re-registration — the workspace may still be discarded.
#[tauri::command]
pub async fn launch_workspace_draft(
    app: AppHandle,
    workspace: Workspace,
) -> Result<LaunchReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        launch_resolved(&app, &workspace, LaunchTrigger::Draft)
    })
    .await
    .map_err(|e| format!("launch task failed: {e}"))
}

/// Shared by the `launch_workspace_by_id` command, the tray menu, global
/// hotkeys, and the CLI's `run` subcommand — every trigger in FR-4 funnels
/// through this one lookup-then-launch path, which is why `trigger` is
/// enough to log a complete picture of every launch regardless of where it
/// came from.
pub fn launch_by_id(
    app: &AppHandle,
    id: &str,
    trigger: LaunchTrigger,
) -> Result<LaunchReport, String> {
    let uuid = Uuid::parse_str(id).map_err(|e| {
        log::warn!("launch ({trigger}) requested an invalid workspace id '{id}': {e}");
        e.to_string()
    })?;
    let state = app.state::<AppState>();
    let workspace = state
        .file
        .lock()
        .workspaces
        .iter()
        .find(|w| w.id == uuid)
        .cloned()
        .ok_or_else(|| {
            log::warn!("launch ({trigger}) requested an unknown workspace {id}");
            format!("workspace {id} not found")
        })?;

    Ok(launch_resolved(app, &workspace, trigger))
}

/// Everything a launch needs once the `Workspace` is already in hand — no
/// lookup. Shared by `launch_by_id` (which looks the workspace up first) and
/// `launch_workspace_draft` (which is handed one directly), so the two
/// paths can never drift apart on logging or progress reporting.
fn launch_resolved(app: &AppHandle, workspace: &Workspace, trigger: LaunchTrigger) -> LaunchReport {
    log::info!(
        "launch ({trigger}): \"{}\" ({})",
        workspace.name,
        workspace.id
    );

    let app_for_events = app.clone();
    let report = launch::launch_workspace(app, workspace, move |outcome| {
        match outcome.status {
            ActionStatus::Started => log::info!("  started: {}", outcome.label),
            ActionStatus::Skipped => log::debug!("  skipped: {}", outcome.label),
            ActionStatus::Failed => log::warn!(
                "  failed: {} ({})",
                outcome.label,
                outcome.message.as_deref().unwrap_or("no error message")
            ),
        }
        let _ = app_for_events.emit("launch:progress", outcome);
    });

    log::info!("launch ({trigger}) finished: {}", report.summary());
    report
}

#[tauri::command]
pub fn create_desktop_shortcut(
    app: AppHandle,
    state: State<AppState>,
    id: String,
) -> Result<String, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let workspace = state
        .file
        .lock()
        .workspaces
        .iter()
        .find(|w| w.id == uuid)
        .cloned()
        .ok_or_else(|| format!("workspace {id} not found"))?;
    crate::shortcut::create_desktop_shortcut(&app, &workspace)
        .map(|path| path.to_string_lossy().to_string())
}

/// Releases every hotkey Click currently holds with the OS. `RegisterHotKey`
/// intercepts a bound combo system-wide — it never reaches any window as a
/// normal keypress, not even Click's own — so without this, the capture
/// widget could never actually receive a keystroke for a combo one of the
/// user's own *other* workspaces already holds. Call before entering
/// capture mode; pair with `resume_hotkeys` when capture ends.
#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) {
    if let Err(err) = app.global_shortcut().unregister_all() {
        log::warn!("failed to suspend hotkeys before capture: {err}");
    }
}

/// Re-registers every workspace's hotkey from the saved config, undoing
/// `suspend_hotkeys`. Just `register_all` — already idempotent and safe to
/// call unconditionally.
#[tauri::command]
pub fn resume_hotkeys(app: AppHandle) {
    crate::hotkeys::register_all(&app);
}

/// Checks whether `accelerator` could be bound right now, without saving
/// anything or disturbing an existing live binding. Used by the editor's
/// hotkey capture widget for immediate feedback (issue #19); the check
/// itself lives in `hotkeys::probe` (issue #5).
#[tauri::command]
pub fn probe_hotkey(
    app: AppHandle,
    accelerator: String,
    workspace_id: Option<String>,
) -> Result<HotkeyStatus, String> {
    let for_workspace = workspace_id
        .map(|id| Uuid::parse_str(&id).map_err(|e| e.to_string()))
        .transpose()?;
    Ok(crate::hotkeys::probe(&app, &accelerator, for_workspace))
}

/// The authoritative registration outcome from the last `register_all` pass
/// (run at startup and after every save) for one workspace.
#[tauri::command]
pub fn hotkey_status(
    state: State<crate::hotkeys::HotkeyState>,
    id: String,
) -> Result<HotkeyStatus, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    Ok(state
        .statuses
        .lock()
        .get(&uuid)
        .cloned()
        .unwrap_or(HotkeyStatus::Unset))
}

/// Scans Start Menu shortcuts for the app picker (issue #32). Walking both
/// trees and resolving every `.lnk` via COM is slow enough to belong off the
/// async runtime — matches the "no blocking work on the async runtime" rule
/// from issue #3.
#[tauri::command]
pub async fn list_installed_apps(app: AppHandle, refresh: bool) -> Vec<InstalledApp> {
    tauri::async_runtime::spawn_blocking(move || installed_apps::list(&app, refresh))
        .await
        .unwrap_or_else(|err| {
            log::error!("installed-apps scan task panicked: {err}");
            Vec::new()
        })
}

/// The directory Click writes its rotating log file to — a different root
/// from the config dir (`app_log_dir` vs `app_config_dir`), since logs are
/// machine-local and must never sync with a roaming profile. Exposed so the
/// diagnostics footer can show the user where to look without them needing
/// to already know that.
#[tauri::command]
pub fn log_dir(app: AppHandle) -> Result<String, String> {
    app.path()
        .app_log_dir()
        .map(|dir| dir.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Opens the log folder in Explorer. Rust-side `app.opener()` calls bypass
/// the IPC capability ACL entirely (confirmed while doing issue #17), so
/// this needs no new capability grant.
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    // The log plugin only creates this on first write, and store::load has
    // the same "may not exist yet" trap for the config dir on first run.
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Manual update check from the editor's diagnostics footer
/// (`DiagnosticsFooter.tsx`) -- the same `updates::check_and_prompt` the
/// startup check and the tray's "Check for updates..." item use, so all
/// three triggers share one code path and only differ by `UpdateTrigger`
/// (issue #25).
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> UpdateStatus {
    updates::check_and_prompt(app, UpdateTrigger::Ui).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn workspace(name: &str) -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: String::new(),
            icon: None,
            color: None,
            tags: Vec::new(),
            variables: HashMap::new(),
            launch_strategy: crate::model::LaunchStrategy::Sequential,
            default_delay_ms: 300,
            hotkey: None,
            actions: Vec::new(),
        }
    }

    #[test]
    fn restores_at_the_original_middle_index() {
        let mut workspaces = vec![workspace("A"), workspace("C")];
        let b = workspace("B");
        let b_id = b.id;

        restore_at(&mut workspaces, b, 1);

        let names: Vec<&str> = workspaces.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["A", "B", "C"]);
        assert_eq!(workspaces[1].id, b_id);
    }

    #[test]
    fn restores_at_index_zero() {
        let mut workspaces = vec![workspace("B")];
        let a = workspace("A");

        restore_at(&mut workspaces, a, 0);

        let names: Vec<&str> = workspaces.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["A", "B"]);
    }

    #[test]
    fn out_of_range_index_clamps_to_the_end() {
        let mut workspaces = vec![workspace("A")];
        let b = workspace("B");

        restore_at(&mut workspaces, b, 999);

        let names: Vec<&str> = workspaces.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["A", "B"]);
    }

    #[test]
    fn restoring_into_an_empty_list_works() {
        let mut workspaces = Vec::new();
        let a = workspace("A");

        restore_at(&mut workspaces, a, 0);

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name, "A");
    }

    #[test]
    fn an_id_already_present_is_replaced_not_duplicated() {
        let a = workspace("A original");
        let a_id = a.id;
        let mut workspaces = vec![a];

        let mut a_restored = workspace("A restored");
        a_restored.id = a_id;
        restore_at(&mut workspaces, a_restored, 0);

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name, "A restored");
    }

    // ---- validate_action: issue #17 args/cwd-on-a-shell-target warning ----
    // `Path::exists()` must be true to reach that check at all, so these use
    // real files on disk — same fixture pattern as installed_apps.rs.

    struct Fixture {
        dir: std::path::PathBuf,
    }

    impl Fixture {
        fn new(test_name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("click-commands-test-{test_name}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Fixture { dir }
        }

        fn touch(&self, filename: &str) -> String {
            let path = self.dir.join(filename);
            std::fs::write(&path, b"").unwrap();
            path.to_string_lossy().to_string()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn app_action(path: String, args: Vec<&str>, cwd: Option<&str>) -> Action {
        Action::App {
            id: Uuid::new_v4(),
            label: "App".to_string(),
            path,
            args: args.into_iter().map(String::from).collect(),
            cwd: cwd.map(String::from),
            enabled: true,
            delay_after_ms: None,
        }
    }

    #[test]
    fn warns_when_a_shell_routed_target_has_args() {
        let fx = Fixture::new("warns_when_a_shell_routed_target_has_args");
        let target = fx.touch("Shortcut.lnk");

        let warning = validate(app_action(target, vec!["some-arg"], None));

        assert!(warning.is_some());
    }

    #[test]
    fn warns_when_a_shell_routed_target_has_a_cwd() {
        let fx = Fixture::new("warns_when_a_shell_routed_target_has_a_cwd");
        let target = fx.touch("installer.msi");

        let warning = validate(app_action(target, vec![], Some("C:/Users/me")));

        assert!(warning.is_some());
    }

    #[test]
    fn stays_silent_for_a_shell_routed_target_with_no_args_or_cwd() {
        let fx = Fixture::new("stays_silent_for_a_shell_routed_target_with_no_args_or_cwd");
        let target = fx.touch("Shortcut.lnk");

        let warning = validate(app_action(target, vec![], None));

        assert_eq!(warning, None);
    }

    #[test]
    fn stays_silent_for_args_on_a_native_exe() {
        let fx = Fixture::new("stays_silent_for_args_on_a_native_exe");
        let target = fx.touch("app.exe");

        let warning = validate(app_action(target, vec!["some-arg"], Some("C:/Users/me")));

        assert_eq!(warning, None);
    }

    // issue #9: a log entry naming the wrong trigger for a draft launch
    // would claim the persisted workspace ran when unsaved edits did.
    #[test]
    fn draft_trigger_displays_distinctly_from_the_other_triggers() {
        assert_eq!(LaunchTrigger::Draft.to_string(), "draft");
        assert_ne!(
            LaunchTrigger::Draft.to_string(),
            LaunchTrigger::Ui.to_string()
        );
    }
}
