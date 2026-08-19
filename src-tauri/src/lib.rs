mod cli;
mod commands;
mod hotkeys;
mod installed_apps;
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
            installed_apps::init(app);
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
            commands::probe_hotkey,
            commands::hotkey_status,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::list_installed_apps,
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

#[cfg(test)]
mod tests {
    /// Pins the webview's Content-Security-Policy (issue #18).
    ///
    /// This needs a test because breaking it is invisible everywhere else:
    /// `tauri dev` points the webview straight at the Vite dev server, so
    /// Tauri never serves the document and never sets the header — the CSP
    /// only ever applies to a bundled build. Drop `http://ipc.localhost` from
    /// `connect-src` and every `invoke` fails, i.e. a completely dead UI in
    /// the shipped installer, while `npm run tauri dev`, the frontend suite,
    /// and the rest of CI all stay green.
    #[test]
    fn csp_is_set_and_allows_tauri_ipc() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json should parse");

        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("app.security.csp must be a string, not null (issue #18)");

        assert!(
            csp.contains("default-src 'self'"),
            "CSP must lock the default source down to 'self', got: {csp}"
        );

        // Tauri's IPC is a `fetch` to http://ipc.localhost/<cmd> on Windows
        // (see `convertFileSrc` in tauri's injected core.js). That is a
        // different origin from the app's own http://tauri.localhost, so
        // `default-src 'self'` alone blocks it — it needs its own connect-src.
        let connect_src = csp
            .split(';')
            .map(str::trim)
            .find(|directive| directive.starts_with("connect-src"))
            .expect("CSP needs an explicit connect-src for the IPC endpoint");

        assert!(
            connect_src.contains("http://ipc.localhost"),
            "connect-src must allow Tauri's IPC endpoint or every invoke fails, got: {connect_src}"
        );
    }
}
