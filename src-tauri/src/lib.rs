mod cli;
mod commands;
mod hotkeys;
mod launch;
mod model;
mod shortcut;
mod store;
mod tray;
mod vars;

use model::WorkspaceFile;
use std::sync::Mutex;
use store::LoadStatus;
use tauri::Manager;

pub struct AppState {
    pub file: Mutex<WorkspaceFile>,
    /// How the config loaded at startup. Gates saving when the on-disk file
    /// couldn't be read and couldn't be safely set aside, so a config we
    /// merely failed to parse is never overwritten with an empty one.
    pub config_status: Mutex<LoadStatus>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first: plugins run in registration order, and
        // this one needs to intercept a second launch before anything else
        // sees it.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            cli::handle(app, Some(argv));
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let loaded = store::load(&config_dir);
            app.manage(AppState {
                file: Mutex::new(loaded.file),
                config_status: Mutex::new(loaded.status),
            });

            hotkeys::init(app)?;
            hotkeys::register_all(&app.handle().clone());
            tray::build(app)?;
            cli::handle(&app.handle().clone(), None);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_workspaces,
            commands::get_workspace,
            commands::save_workspace,
            commands::delete_workspace,
            commands::duplicate_workspace,
            commands::validate_action,
            commands::launch_workspace_by_id,
            commands::create_desktop_shortcut,
            commands::config_status,
        ])
        .on_window_event(|window, event| {
            // Closing the main window hides it into the tray instead of
            // exiting — Click is a tray utility; "Quit" from the tray
            // menu is the real exit path.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
