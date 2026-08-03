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

    // The action whose delay would actually be observed — a trailing run of
    // disabled actions must not force a wasted sleep before returning.
    let last_active = workspace.actions.iter().rposition(|a| a.enabled());

    for (index, action) in workspace.actions.iter().enumerate() {
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

        let is_last = last_active == Some(index);
        if let Some(delay) = delay_after(
            outcome.status,
            action.delay_after_ms(),
            workspace.default_delay_ms,
            is_last,
        ) {
            std::thread::sleep(delay);
        }

        outcomes.push(outcome);
    }

    LaunchReport { outcomes }
}

/// How long to pause after an action, if at all. Pure so the policy is
/// unit-testable without an `AppHandle`. A skipped action never delays —
/// nothing was started to space out — and neither does the last action to
/// actually run, since nothing follows it and returning sooner is strictly
/// better.
fn delay_after(
    status: ActionStatus,
    delay_after_ms: Option<u64>,
    default_delay_ms: u64,
    is_last: bool,
) -> Option<Duration> {
    if matches!(status, ActionStatus::Skipped) || is_last {
        return None;
    }
    match delay_after_ms.unwrap_or(default_delay_ms) {
        0 => None,
        ms => Some(Duration::from_millis(ms)),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_action_never_delays() {
        assert_eq!(delay_after(ActionStatus::Skipped, None, 300, false), None);
        assert_eq!(
            delay_after(ActionStatus::Skipped, Some(500), 300, false),
            None
        );
    }

    #[test]
    fn last_action_never_delays() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(500), 300, true),
            None
        );
        assert_eq!(delay_after(ActionStatus::Failed, None, 300, true), None);
    }

    #[test]
    fn per_action_override_wins_over_default() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(500), 300, false),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn falls_back_to_workspace_default() {
        assert_eq!(
            delay_after(ActionStatus::Started, None, 300, false),
            Some(Duration::from_millis(300))
        );
    }

    #[test]
    fn explicit_zero_delay_is_none() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(0), 300, false),
            None
        );
        assert_eq!(delay_after(ActionStatus::Failed, None, 0, false), None);
    }
}
