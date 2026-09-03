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
mod updates;
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
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let headless = cli::handle(&app.handle().clone(), None);
            // A `click run --id <uuid>` desktop-shortcut launch never shows
            // the main window (see cli::handle), so a dialog would have
            // nowhere to render -- and this is exactly the path smoke.yml
            // drives in CI. spawn (not spawn_blocking): the update check is
            // network-bound, not CPU-bound, and must not sit on the async
            // runtime's blocking pool (issue #3).
            if !headless {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    updates::check_and_prompt(handle, updates::UpdateTrigger::Startup).await;
                });
            }

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
            commands::launch_workspace_draft,
            commands::create_desktop_shortcut,
            commands::config_status,
            commands::probe_hotkey,
            commands::hotkey_status,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::list_installed_apps,
            commands::log_dir,
            commands::open_log_dir,
            commands::check_for_updates,
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

    /// Pins the updater's release feed (issue #25). `pubkey` is deliberately
    /// not asserted here -- it's a placeholder in the committed config until
    /// the real one is generated and pasted in (see docs/RELEASING.md), and
    /// asserting its exact value would just pin a value that's expected to
    /// change once, on purpose.
    #[test]
    fn updater_config_points_at_this_repos_releases() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json should parse");

        let endpoints = config["plugins"]["updater"]["endpoints"]
            .as_array()
            .expect("plugins.updater.endpoints must be an array");
        assert!(!endpoints.is_empty(), "updater needs at least one endpoint");
        assert!(
            endpoints[0]
                .as_str()
                .unwrap()
                .contains("prashant-singh-2001/click"),
            "updater endpoint should point at this repo's releases, got: {endpoints:?}"
        );
    }

    /// `createUpdaterArtifacts: true` in the base config makes `tauri build`
    /// hard-fail without `TAURI_SIGNING_PRIVATE_KEY` -- that would break
    /// smoke.yml's bare `npm run tauri build` and every contributor's local
    /// build. It belongs only in the release-only overlay (issue #25).
    #[test]
    fn updater_artifacts_are_release_only_not_in_the_base_config() {
        let base: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json should parse");
        assert!(
            base["bundle"]["createUpdaterArtifacts"].is_null(),
            "createUpdaterArtifacts must not be in the base config -- it breaks unsigned builds"
        );

        let release: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.release.conf.json"))
                .expect("tauri.release.conf.json should parse");
        assert_eq!(
            release["bundle"]["createUpdaterArtifacts"], true,
            "the release overlay must turn on updater artifact generation"
        );
    }
}
