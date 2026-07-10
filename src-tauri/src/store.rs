use crate::model::WorkspaceFile;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to read config: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

pub fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("workspaces.json")
}

pub fn load(app_config_dir: &Path) -> Result<WorkspaceFile, StoreError> {
    let path = config_path(app_config_dir);
    if !path.exists() {
        return Ok(WorkspaceFile::default());
    }
    let raw = fs::read_to_string(&path)?;
    let file: WorkspaceFile = serde_json::from_str(&raw)?;
    Ok(migrate(file))
}

fn migrate(file: WorkspaceFile) -> WorkspaceFile {
    // No migrations yet; CURRENT_VERSION is the only version that has
    // ever existed. Future versions branch on `file.version` here and
    // transform forward before returning.
    let mut file = file;
    file.version = CURRENT_VERSION;
    file
}

/// Writes via a temp file + rename so a crash mid-write can never leave
/// `workspaces.json` truncated or half-written.
pub fn save(app_config_dir: &Path, file: &WorkspaceFile) -> Result<(), StoreError> {
    fs::create_dir_all(app_config_dir)?;
    let path = config_path(app_config_dir);
    let tmp_path = app_config_dir.join("workspaces.json.tmp");
    let json = serde_json::to_string_pretty(file)?;
    fs::write(&tmp_path, json)?;
    fs::rename(&tmp_path, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Workspace;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("click-store-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trips_workspaces() {
        let dir = temp_dir("roundtrip");
        let file = WorkspaceFile {
            version: 1,
            workspaces: vec![Workspace {
                id: Uuid::new_v4(),
                name: "Test".to_string(),
                description: String::new(),
                icon: None,
                color: None,
                tags: vec![],
                variables: HashMap::new(),
                launch_strategy: crate::model::LaunchStrategy::Sequential,
                default_delay_ms: 300,
                hotkey: None,
                actions: vec![],
            }],
        };

        save(&dir, &file).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.workspaces.len(), 1);
        assert_eq!(loaded.workspaces[0].name, "Test");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_yields_empty_default() {
        let dir = temp_dir("missing");
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.workspaces.len(), 0);
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn atomic_write_leaves_no_tmp_file_behind() {
        let dir = temp_dir("atomic");
        save(&dir, &WorkspaceFile::default()).unwrap();
        assert!(!dir.join("workspaces.json.tmp").exists());
        assert!(dir.join("workspaces.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
