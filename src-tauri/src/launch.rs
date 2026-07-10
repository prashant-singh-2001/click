use crate::model::{Action, Workspace};
use crate::vars;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub action_id: String,
    pub label: String,
    pub status: ActionStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionStatus {
    Skipped,
    Started,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub outcomes: Vec<ActionOutcome>,
}

/// Runs every enabled action in order, honoring per-action / default delays.
/// A failing action is recorded and the run continues (NFR-3) — one bad
/// path must never block the rest of a workspace from launching.
pub fn launch_workspace<F: FnMut(&ActionOutcome)>(
    app: &AppHandle,
    workspace: &Workspace,
    mut on_outcome: F,
) -> LaunchReport {
    let mut outcomes = Vec::with_capacity(workspace.actions.len());

    for action in &workspace.actions {
        let outcome = if !action.enabled() {
            ActionOutcome {
                action_id: action.id().to_string(),
                label: action.label().to_string(),
                status: ActionStatus::Skipped,
                message: None,
            }
        } else {
            run_action(app, action, workspace)
        };

        on_outcome(&outcome);
        outcomes.push(outcome.clone());

        if !matches!(outcome.status, ActionStatus::Skipped) {
            let delay = action
                .delay_after_ms()
                .unwrap_or(workspace.default_delay_ms);
            if delay > 0 {
                std::thread::sleep(Duration::from_millis(delay));
            }
        }
    }

    LaunchReport { outcomes }
}

fn run_action(app: &AppHandle, action: &Action, workspace: &Workspace) -> ActionOutcome {
    let id = action.id().to_string();
    let label = action.label().to_string();

    let result = match action {
        Action::App {
            path, args, cwd, ..
        } => run_app(path, args, cwd.as_deref(), &workspace.variables),
        Action::Url { url, .. } => run_url(app, url, &workspace.variables),
    };

    match result {
        Ok(()) => ActionOutcome {
            action_id: id,
            label,
            status: ActionStatus::Started,
            message: None,
        },
        Err(message) => ActionOutcome {
            action_id: id,
            label,
            status: ActionStatus::Failed,
            message: Some(message),
        },
    }
}

fn run_app(
    path: &str,
    args: &[String],
    cwd: Option<&str>,
    variables: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let resolved_path = vars::resolve(path, variables).map_err(|e| e.to_string())?;
    let resolved_args = args
        .iter()
        .map(|a| vars::resolve(a, variables))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let resolved_cwd = cwd
        .map(|c| vars::resolve(c, variables))
        .transpose()
        .map_err(|e| e.to_string())?;

    // `code`, `npm`, and similar dev-tool shims on Windows are .cmd/.bat
    // files, not native executables — Command::new() cannot spawn them
    // directly and fails with an opaque OS error, so route through cmd /C.
    let is_script = matches!(
        Path::new(&resolved_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase()),
        Some(ext) if ext == "cmd" || ext == "bat"
    );

    let mut command = if is_script {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&resolved_path).args(&resolved_args);
        c
    } else {
        let mut c = Command::new(&resolved_path);
        c.args(&resolved_args);
        c
    };

    if let Some(dir) = &resolved_cwd {
        command.current_dir(dir);
    }

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map(|_child| ())
        .map_err(|e| format!("failed to launch '{}': {}", resolved_path, e))
}

fn run_url(
    app: &AppHandle,
    url: &str,
    variables: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let resolved = vars::resolve(url, variables).map_err(|e| e.to_string())?;
    app.opener()
        .open_url(&resolved, None::<&str>)
        .map_err(|e| format!("failed to open '{}': {}", resolved, e))
}
