use crate::launch::{self, LaunchReport};
use crate::model::{Action, Workspace};
use crate::store;
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

fn config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

fn persist(app: &AppHandle, state: &State<AppState>) -> Result<(), String> {
    {
        let file = state.file.lock().unwrap();
        let dir = config_dir(app)?;
        store::save(&dir, &file).map_err(|e| e.to_string())?;
    }
    crate::tray::rebuild(app);
    crate::hotkeys::register_all(app);
    Ok(())
}

#[tauri::command]
pub fn list_workspaces(state: State<AppState>) -> Vec<Workspace> {
    state.file.lock().unwrap().workspaces.clone()
}

#[tauri::command]
pub fn get_workspace(state: State<AppState>, id: String) -> Result<Workspace, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state
        .file
        .lock()
        .unwrap()
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
        let mut file = state.file.lock().unwrap();
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
        let mut file = state.file.lock().unwrap();
        file.workspaces.retain(|w| w.id != uuid);
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
        let mut file = state.file.lock().unwrap();
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

#[tauri::command]
pub async fn launch_workspace_by_id(app: AppHandle, id: String) -> Result<LaunchReport, String> {
    launch_by_id(&app, &id)
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
        .unwrap()
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
        .unwrap()
        .workspaces
        .iter()
        .find(|w| w.id == uuid)
        .cloned()
        .ok_or_else(|| format!("workspace {id} not found"))?;
    crate::shortcut::create_desktop_shortcut(&app, &workspace)
        .map(|path| path.to_string_lossy().to_string())
}
