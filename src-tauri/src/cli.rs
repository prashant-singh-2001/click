use tauri::{AppHandle, Manager};
use tauri_plugin_cli::{CliExt, Matches};

/// Single entry point for both places CLI args can arrive: this process's
/// own argv on first launch (`args: None`), and the argv single-instance
/// forwards from a second invocation (`args: Some(argv)`). A `run --id
/// <uuid>` never shows the main window — that's what makes the
/// desktop-shortcut flow (FR-4.4) headless.
///
/// Returns whether this call resulted in a headless launch (`true`) or the
/// normal interactive path (`false`) — used to gate the startup update
/// check (issue #25), which must not fire on a shortcut click: the main
/// window stays hidden on that path, so a dialog would have nowhere to
/// render, and it's exactly the path `smoke.yml`'s boot test drives.
pub fn handle(app: &AppHandle, args: Option<Vec<String>>) -> bool {
    let result = match args {
        Some(argv) => app.cli().matches_from(argv),
        None => app.cli().matches(),
    };
    let matches = match result {
        Ok(matches) => matches,
        Err(err) => {
            log::warn!("failed to parse CLI arguments: {err}");
            return false;
        }
    };

    if try_launch(app, &matches) {
        return true;
    }
    focus_main_window(app);
    false
}

fn try_launch(app: &AppHandle, matches: &Matches) -> bool {
    let Some(subcommand) = &matches.subcommand else {
        log::debug!("no CLI subcommand given");
        return false;
    };
    if subcommand.name != "run" {
        log::debug!("ignoring unrecognized subcommand '{}'", subcommand.name);
        return false;
    }
    let Some(id_arg) = subcommand.matches.args.get("id") else {
        log::debug!("'run' subcommand invoked without an --id argument");
        return false;
    };
    let Some(id) = id_arg.value.as_str() else {
        log::debug!("'run' subcommand's --id argument was not a string");
        return false;
    };

    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) =
            crate::commands::launch_by_id(&app, &id, crate::commands::LaunchTrigger::Cli)
        {
            log::error!("CLI launch failed for workspace {id}: {err}");
        }
    });
    true
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
