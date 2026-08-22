mod cli;
mod commands;
mod hotkeys;
mod installed_apps;
mod launch;
mod logging;
mod model;
mod shortcut;
mod store;
mod tray;
mod vars;

use model::WorkspaceFile;
use parking_lot::Mutex;
use store::LoadStatus;
use tauri::Manager;

// Issue #4: std::sync::Mutex poisons on panic — one panic while holding a
// lock makes every subsequent .lock().unwrap() on it panic forever, and
// several of these locks are reachable from spawned launch/hotkey workers.
// parking_lot's Mutex has no poisoning and lock() returns the guard
// directly rather than a Result, so the old .lock().unwrap() pattern can't
// be reintroduced by accident — it won't compile. See also clippy.toml.
pub struct AppState {
    pub file: Mutex<WorkspaceFile>,
    /// How the config loaded at startup. Gates saving when the on-disk file
    /// couldn't be read and couldn't be safely set aside, so a config we
    /// merely failed to parse is never overwritten with an empty one.
    pub config_status: Mutex<LoadStatus>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();

    let result = tauri::Builder::default()
        // Registered before single_instance: that plugin's own setup hook
        // calls std::process::exit(0) when it detects a second instance
        // (after forwarding that instance's argv to the first), so any
        // plugin registered after it never initializes in the forwarding
        // process — which is exactly the desktop-shortcut / CLI path this
        // logging exists to cover. The log plugin is passive (it only
        // installs a log::Log sink and intercepts nothing), so putting it
        // first costs nothing single_instance's own ordering needs below.
        .plugin(logging::builder().build())
        // Must be registered next: plugins run in registration order, and
        // this one needs to intercept a second launch before anything else
        // (other than logging, above) sees it.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            cli::handle(app, Some(argv));
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            log::info!("click v{} starting", env!("CARGO_PKG_VERSION"));

            let config_dir = app.path().app_config_dir()?;
            let loaded = store::load(&config_dir);
            match &loaded.status {
                store::LoadStatus::Ok => {
                    log::info!(
                        "config loaded: {} workspace(s) from {}",
                        loaded.file.workspaces.len(),
                        config_dir.display()
                    );
                }
                store::LoadStatus::Recovered {
                    backup_path,
                    reason,
                } => {
                    log::warn!(
                        "config could not be parsed and was quarantined to {backup_path}: {reason}"
                    );
                }
                store::LoadStatus::Blocked { reason } => {
                    log::error!("config could not be read and saving is disabled: {reason}");
                }
            }

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
            commands::restore_workspace,
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
            commands::log_dir,
            commands::open_log_dir,
        ])
        .on_window_event(|window, event| {
            // Closing the main window hides it into the tray instead of
            // exiting — Click is a tray utility; "Quit" from the tray
            // menu is the real exit path.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(err) = window.hide() {
                    log::warn!("failed to hide main window on close: {err}");
                }
            }
        })
        .run(tauri::generate_context!());

    match result {
        Ok(()) => log::info!("click exiting normally"),
        Err(err) => {
            // No console exists in a release build (windows_subsystem =
            // "windows"), so without this a startup failure — an
            // unresolvable config dir, hotkeys::init or tray::build
            // erroring — used to just make the app vanish with nothing to
            // go on anywhere. This is also the one failure the log file
            // itself can't be trusted to cover: it can happen before the
            // log plugin's own setup() has attached a logger.
            let reason = err.to_string();
            log::error!("fatal startup/runtime error: {reason}");
            #[cfg(windows)]
            logging::fatal_startup_dialog(&reason);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};

    /// Issue #4: a panic while holding a `std::sync::Mutex` guard poisons it
    /// forever — every later `.lock()` on that mutex panics too, permanently.
    /// `parking_lot::Mutex` has no poisoning, so the state must still be
    /// readable right after a panic mid-guard. This test fails against
    /// `std::sync::Mutex` (confirmed by temporarily reverting the import),
    /// so don't "fix" the `thread panicked` line in the test output — it's
    /// the panic this test deliberately triggers, not a real failure.
    #[test]
    fn app_state_survives_a_panic_while_locked() {
        let state = AppState {
            file: Mutex::new(WorkspaceFile::default()),
            config_status: Mutex::new(LoadStatus::Ok),
        };

        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.file.lock();
            panic!("simulated panic while holding the lock");
        }));

        assert_eq!(state.file.lock().workspaces.len(), 0);
        assert!(matches!(*state.config_status.lock(), LoadStatus::Ok));
    }

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
