use crate::AppState;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

const TRAY_ID: &str = "main-tray";

pub fn build(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let menu = build_menu(handle)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Click")
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .build(app)?;
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let workspaces = {
        let state = app.state::<AppState>();
        let workspaces = state.file.lock().unwrap().workspaces.clone();
        workspaces
    };

    let menu = Menu::new(app)?;
    for ws in &workspaces {
        let item = MenuItem::with_id(
            app,
            format!("launch:{}", ws.id),
            &ws.name,
            true,
            None::<&str>,
        )?;
        menu.append(&item)?;
    }
    if !workspaces.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(
        app,
        "open",
        "Open Click",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;
    Ok(menu)
}

/// Called after any workspace list mutation so the tray never serves a
/// stale menu (a saved rename or a new workspace must show up immediately).
pub fn rebuild(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id.as_ref();
    match id {
        "quit" => app.exit(0),
        "open" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        other if other.starts_with("launch:") => {
            let workspace_id = other.trim_start_matches("launch:").to_string();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::launch_by_id(&app, &workspace_id);
            });
        }
        _ => {}
    }
}
