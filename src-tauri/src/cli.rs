use tauri::{AppHandle, Manager};
use tauri_plugin_cli::{CliExt, Matches};

/// Single entry point for both places CLI args can arrive: this process's
/// own argv on first launch (`args: None`), and the argv single-instance
/// forwards from a second invocation (`args: Some(argv)`). A `run --id
/// <uuid>` never shows the main window — that's what makes the
/// desktop-shortcut flow (FR-4.4) headless.
pub fn handle(app: &AppHandle, args: Option<Vec<String>>) {
    let result = match args {
        Some(argv) => app.cli().matches_from(argv),
        None => app.cli().matches(),
    };
    let Ok(matches) = result else { return };

    if try_launch(app, &matches) {
        return;
    }
    focus_main_window(app);
}

fn try_launch(app: &AppHandle, matches: &Matches) -> bool {
    let Some(subcommand) = &matches.subcommand else {
        return false;
    };
    if subcommand.name != "run" {
        return false;
    }
    let Some(id_arg) = subcommand.matches.args.get("id") else {
        return false;
    };
    let Some(id) = id_arg.value.as_str() else {
        return false;
    };

    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let _ = crate::commands::launch_by_id(&app, &id);
    });
    true
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
