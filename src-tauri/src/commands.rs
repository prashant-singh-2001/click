use crate::hotkeys::HotkeyStatus;
use crate::installed_apps::{self, InstalledApp};
use crate::launch::{self, LaunchReport};
use crate::model::{Action, Workspace};
use crate::store;
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use uuid::Uuid;

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
    store::save(&dir, &snapshot).map_err(|e| e.to_string())?;

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

/// FR-7.1: flags a missing app path or a URL without a scheme. Never
/// blocks saving — the path may legitimately not exist on this machine yet.
#[tauri::command]
pub fn validate_action(action: Action) -> Option<String> {
    match action {
        Action::App { path, .. } => {
            if path.trim().is_empty() {
                Some("path is empty".to_string())
            } else if path.contains("${") {
                None // contains a variable; can't validate until launch time
            } else if !Path::new(&path).exists() {
                Some(format!("path does not exist: {path}"))
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

/// Runs the launch on a blocking thread — `launch::launch_workspace` sleeps
/// between actions with `std::thread::sleep`, and doing that on an async
/// worker thread pins it for the sum of every delay (issue #3).
#[tauri::command]
pub async fn launch_workspace_by_id(app: AppHandle, id: String) -> Result<LaunchReport, String> {
    tauri::async_runtime::spawn_blocking(move || launch_by_id(&app, &id))
        .await
        .map_err(|e| format!("launch task failed: {e}"))?
}

/// Shared by the `launch_workspace_by_id` command, the tray menu, global
/// hotkeys, and the CLI's `run` subcommand — every trigger in FR-4 funnels
/// through this one lookup-then-launch path.
pub fn launch_by_id(app: &AppHandle, id: &str) -> Result<LaunchReport, String> {
    let uuid = Uuid::parse_str(id).map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let workspace = state
        .file
        .lock()
        .workspaces
        .iter()
        .find(|w| w.id == uuid)
        .cloned()
        .ok_or_else(|| format!("workspace {id} not found"))?;

    let app_for_events = app.clone();
    let report = launch::launch_workspace(app, &workspace, move |outcome| {
        let _ = app_for_events.emit("launch:progress", outcome);
    });
    Ok(report)
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
    let _ = app.global_shortcut().unregister_all();
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
        .unwrap_or_default()
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
}
