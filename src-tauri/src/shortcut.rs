use crate::model::Workspace;
use mslnk::ShellLink;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Writes `<Desktop>/<workspace name>.lnk` targeting the currently running
/// exe with `run --id <uuid>` as its argument. Resolving the target from
/// `current_exe()` at click-generation time (rather than hardcoding a dev
/// path) means a shortcut created from an installed build points at the
/// installed binary.
///
/// The Desktop path is resolved via Tauri's path resolver (backed by the
/// Windows known-folder API), not `%USERPROFILE%\Desktop` — OneDrive's
/// "Desktop" folder backup redirects the real desktop to
/// `%USERPROFILE%\OneDrive\Desktop`, and the plain env-var path silently
/// doesn't exist on those machines.
pub fn create_desktop_shortcut(app: &AppHandle, workspace: &Workspace) -> Result<PathBuf, String> {
    let desktop = app.path().desktop_dir().map_err(|e| e.to_string())?;
    create_shortcut_in(workspace, &desktop)
}

fn create_shortcut_in(workspace: &Workspace, dir: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();

    let mut link = ShellLink::new(&exe_str).map_err(|e| e.to_string())?;
    link.set_arguments(Some(format!("run --id {}", workspace.id)));
    if let Some(exe_dir) = exe.parent() {
        link.set_working_dir(Some(exe_dir.to_string_lossy().to_string()));
    }
    link.set_icon_location(Some(exe_str.clone()));

    let lnk_path = dir.join(format!("{}.lnk", sanitize_filename(&workspace.name)));
    link.create_lnk(&lnk_path).map_err(|e| e.to_string())?;
    Ok(lnk_path)
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Workspace".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn test_workspace(name: &str) -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: String::new(),
            icon: None,
            color: None,
            tags: vec![],
            variables: HashMap::new(),
            launch_strategy: crate::model::LaunchStrategy::Sequential,
            default_delay_ms: 300,
            hotkey: None,
            actions: vec![],
        }
    }

    #[test]
    fn creates_a_real_lnk_file() {
        let dir = std::env::temp_dir().join("click-shortcut-test");
        std::fs::create_dir_all(&dir).unwrap();
        let ws = test_workspace("Click Shortcut Test");

        let path = create_shortcut_in(&ws, &dir).expect("shortcut creation should succeed");
        assert!(path.exists());
        assert_eq!(path.extension().unwrap(), "lnk");
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sanitizes_illegal_filename_characters() {
        assert_eq!(sanitize_filename(r#"Test: A/B\C"#), "Test_ A_B_C");
        assert_eq!(sanitize_filename("   "), "Workspace");
    }
}
