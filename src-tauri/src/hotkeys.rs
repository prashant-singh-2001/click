use crate::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;

/// Maps registered shortcuts back to the workspace they launch. The
/// plugin fires one shared handler for every registered shortcut, so this
/// is how that handler figures out which workspace was pressed.
pub struct HotkeyState {
    pub bindings: Mutex<Vec<(Shortcut, Uuid)>>,
}

pub fn init(app: &tauri::App) -> tauri::Result<()> {
    app.manage(HotkeyState {
        bindings: Mutex::new(Vec::new()),
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
/// workspace — one bad hotkey must not take down the rest.
pub fn register_all(app: &AppHandle) {
    let workspaces = {
        let state = app.state::<AppState>();
        let workspaces = state.file.lock().unwrap().workspaces.clone();
        workspaces
    };

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let mut bindings = Vec::new();
    for ws in workspaces {
        let Some(hotkey) = ws.hotkey.as_ref().filter(|h| !h.is_empty()) else {
            continue;
        };
        let Ok(shortcut) = hotkey.parse::<Shortcut>() else {
            continue;
        };
        if gs.register(shortcut).is_ok() {
            bindings.push((shortcut, ws.id));
        }
    }

    let hotkey_state = app.state::<HotkeyState>();
    *hotkey_state.bindings.lock().unwrap() = bindings;
}
