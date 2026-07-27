use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

/// Outcome of trying to bind a workspace's hotkey to the OS. Mirrors
/// `store::LoadStatus`'s pattern: `rename_all_fields` is required alongside
/// `rename_all`, or struct-variant fields silently stay snake_case (see the
/// regression test in `model.rs` for the bug this caused once already).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HotkeyStatus {
    /// No hotkey is set on this workspace.
    Unset,
    /// Bound and live.
    Registered,
    /// Not a parseable accelerator.
    Invalid { reason: String },
    /// Another workspace in this config already claims the same combo.
    Duplicate { workspace_name: String },
    /// The OS refused registration — almost always another app owns it.
    Conflict { reason: String },
}

/// Maps registered shortcuts back to the workspace they launch, and records
/// why every workspace's hotkey did or didn't register. The plugin fires one
/// shared handler for every registered shortcut, so `bindings` is how that
/// handler figures out which workspace was pressed; `statuses` is what the
/// editor reads back after a save.
pub struct HotkeyState {
    pub bindings: Mutex<Vec<(Shortcut, Uuid)>>,
    pub statuses: Mutex<HashMap<Uuid, HotkeyStatus>>,
}

pub fn init(app: &tauri::App) -> tauri::Result<()> {
    app.manage(HotkeyState {
        bindings: Mutex::new(Vec::new()),
        statuses: Mutex::new(HashMap::new()),
    });
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                let hotkey_state = app.state::<HotkeyState>();
                let workspace_id = hotkey_state
                    .bindings
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(bound, _)| bound == shortcut)
                    .map(|(_, id)| *id);

                if let Some(id) = workspace_id {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::launch_by_id(&app, &id.to_string());
                    });
                }
            })
            .build(),
    )
}

/// Re-registers every workspace's hotkey against the OS. Called at startup
/// and after every save so bindings never drift from what's on disk. A
/// combo already owned by another app fails to register for just that one
/// workspace — one bad hotkey must not take down the rest. Records why each
/// workspace's hotkey did or didn't take, so the editor can show it.
pub fn register_all(app: &AppHandle) {
    let workspaces = {
        let state = app.state::<AppState>();
        let workspaces = state.file.lock().unwrap().workspaces.clone();
        workspaces
    };

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let mut bindings: Vec<(Shortcut, Uuid)> = Vec::new();
    let mut statuses = HashMap::new();

    for ws in &workspaces {
        let Some(hotkey) = ws.hotkey.as_ref().filter(|h| !h.trim().is_empty()) else {
            statuses.insert(ws.id, HotkeyStatus::Unset);
            continue;
        };

        let shortcut = match Shortcut::from_str(hotkey) {
            Ok(shortcut) => shortcut,
            Err(err) => {
                statuses.insert(
                    ws.id,
                    HotkeyStatus::Invalid {
                        reason: err.to_string(),
                    },
                );
                continue;
            }
        };

        if let Some((_, owner)) = bindings.iter().find(|(bound, _)| *bound == shortcut) {
            let owner_name = workspaces
                .iter()
                .find(|w| w.id == *owner)
                .map(|w| w.name.clone())
                .unwrap_or_default();
            statuses.insert(
                ws.id,
                HotkeyStatus::Duplicate {
                    workspace_name: owner_name,
                },
            );
            continue;
        }

        match gs.register(shortcut) {
            Ok(()) => {
                bindings.push((shortcut, ws.id));
                statuses.insert(ws.id, HotkeyStatus::Registered);
            }
            Err(err) => {
                statuses.insert(
                    ws.id,
                    HotkeyStatus::Conflict {
                        reason: err.to_string(),
                    },
                );
            }
        }
    }

    let hotkey_state = app.state::<HotkeyState>();
    *hotkey_state.bindings.lock().unwrap() = bindings;
    *hotkey_state.statuses.lock().unwrap() = statuses;
}

/// Parses `accelerator` and classifies it against what's currently
/// registered, without touching bindings that are already live.
fn classify(
    accelerator: &str,
    for_workspace: Option<Uuid>,
    bindings: &[(Shortcut, Uuid)],
    workspace_name: impl FnOnce(Uuid) -> Option<String>,
) -> Result<Shortcut, HotkeyStatus> {
    if accelerator.trim().is_empty() {
        return Err(HotkeyStatus::Unset);
    }

    let shortcut = Shortcut::from_str(accelerator).map_err(|err| HotkeyStatus::Invalid {
        reason: err.to_string(),
    })?;

    if let Some((_, owner)) = bindings.iter().find(|(bound, _)| *bound == shortcut) {
        if Some(*owner) != for_workspace {
            return Err(HotkeyStatus::Duplicate {
                workspace_name: workspace_name(*owner).unwrap_or_default(),
            });
        }
    }

    Ok(shortcut)
}

/// Can `accelerator` be bound right now? Never disturbs a live binding: if
/// it's already registered to `for_workspace`, that's confirmed without any
/// OS call. Otherwise it's test-registered and immediately unregistered —
/// the probe window is sub-millisecond, and the combo is absent from
/// `bindings` throughout, so even if it fired, the shared handler above
/// would find no matching workspace and do nothing.
pub fn probe(app: &AppHandle, accelerator: &str, for_workspace: Option<Uuid>) -> HotkeyStatus {
    let hotkey_state = app.state::<HotkeyState>();
    let bindings = hotkey_state.bindings.lock().unwrap().clone();

    let owner_name = |id: Uuid| {
        let state = app.state::<AppState>();
        let file = state.file.lock().unwrap();
        file.workspaces
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.name.clone())
    };

    let shortcut = match classify(accelerator, for_workspace, &bindings, owner_name) {
        Ok(shortcut) => shortcut,
        Err(status) => return status,
    };

    if let Some(for_workspace) = for_workspace {
        if bindings
            .iter()
            .any(|(bound, id)| *bound == shortcut && *id == for_workspace)
        {
            return HotkeyStatus::Registered;
        }
    }

    let gs = app.global_shortcut();
    match gs.register(shortcut) {
        Ok(()) => {
            let _ = gs.unregister(shortcut);
            HotkeyStatus::Registered
        }
        Err(err) => HotkeyStatus::Conflict {
            reason: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_owner(_: Uuid) -> Option<String> {
        None
    }

    #[test]
    fn empty_and_whitespace_are_unset() {
        assert_eq!(classify("", None, &[], no_owner), Err(HotkeyStatus::Unset));
        assert_eq!(
            classify("   ", None, &[], no_owner),
            Err(HotkeyStatus::Unset)
        );
    }

    #[test]
    fn pretty_alias_and_raw_code_parse_identically() {
        let a = classify("Ctrl+Alt+1", None, &[], no_owner).unwrap();
        let b = classify("Ctrl+Alt+Digit1", None, &[], no_owner).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn two_main_keys_is_invalid() {
        assert!(matches!(
            classify("Ctrl+Shift+C+A", None, &[], no_owner),
            Err(HotkeyStatus::Invalid { .. })
        ));
    }

    #[test]
    fn empty_token_is_invalid() {
        assert!(matches!(
            classify("Ctrl+", None, &[], no_owner),
            Err(HotkeyStatus::Invalid { .. })
        ));
    }

    #[test]
    fn unrecognized_key_is_invalid() {
        assert!(matches!(
            classify("Ctrl+NotAKey", None, &[], no_owner),
            Err(HotkeyStatus::Invalid { .. })
        ));
    }

    #[test]
    fn duplicate_of_another_workspace_is_reported_by_name() {
        let shortcut = Shortcut::from_str("Ctrl+Alt+1").unwrap();
        let owner_id = Uuid::new_v4();
        let bindings = vec![(shortcut, owner_id)];
        let name = |id: Uuid| (id == owner_id).then(|| "Java".to_string());

        let result = classify("Ctrl+Alt+1", None, &bindings, name);
        assert_eq!(
            result,
            Err(HotkeyStatus::Duplicate {
                workspace_name: "Java".to_string()
            })
        );
    }

    #[test]
    fn own_existing_binding_is_not_a_duplicate() {
        let shortcut = Shortcut::from_str("Ctrl+Alt+1").unwrap();
        let owner_id = Uuid::new_v4();
        let bindings = vec![(shortcut, owner_id)];

        let result = classify("Ctrl+Alt+1", Some(owner_id), &bindings, no_owner);
        assert_eq!(result, Ok(shortcut));
    }
}
