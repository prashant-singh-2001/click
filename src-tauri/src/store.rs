use crate::model::WorkspaceFile;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to read config: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Why the app may be starting without the user's saved workspaces, and
/// whether it is safe to write over the config file.
///
/// This exists to prevent a specific data-loss bug: a config that failed to
/// load used to be silently replaced by an empty one on the next save,
/// permanently destroying every workspace the user had.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoadStatus {
    /// Loaded cleanly, or there was no config yet (a normal first run).
    Ok,
    /// The config could not be parsed, so it was moved aside and we started
    /// empty. Saving is safe: the original is preserved at `backup_path`.
    Recovered { backup_path: String, reason: String },
    /// The config exists but could not be read, or could not be preserved.
    /// We started empty and saving is refused so an intact file on disk is
    /// never clobbered.
    Blocked { reason: String },
}

impl LoadStatus {
    /// `Some(reason)` when writing would risk destroying a config we failed
    /// to read but could not safely set aside.
    pub fn save_block_reason(&self) -> Option<&str> {
        match self {
            LoadStatus::Blocked { reason } => Some(reason),
            _ => None,
        }
    }
}

pub struct LoadResult {
    pub file: WorkspaceFile,
    pub status: LoadStatus,
}

pub fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("workspaces.json")
}

/// Always yields a usable `WorkspaceFile`. The accompanying `LoadStatus`
/// reports whether the on-disk config was quarantined or is being protected
/// from overwrite, so the caller can warn the user and gate saving.
pub fn load(app_config_dir: &Path) -> LoadResult {
    let path = config_path(app_config_dir);
    if !path.exists() {
        return LoadResult {
            file: WorkspaceFile::default(),
            status: LoadStatus::Ok,
        };
    }

    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) => {
            // The file may be perfectly intact and merely locked (antivirus,
            // cloud sync, permissions). Start empty but refuse to save rather
            // than risk replacing good data we simply couldn't read.
            return LoadResult {
                file: WorkspaceFile::default(),
                status: LoadStatus::Blocked {
                    reason: format!("could not read {}: {e}", path.display()),
                },
            };
        }
    };

    match serde_json::from_str::<WorkspaceFile>(&raw) {
        Ok(file) => LoadResult {
            file: migrate(file),
            status: LoadStatus::Ok,
        },
        Err(parse_err) => match quarantine(&path) {
            // Original moved aside, so a later save can't destroy it.
            Ok(backup) => LoadResult {
                file: WorkspaceFile::default(),
                status: LoadStatus::Recovered {
                    backup_path: backup.to_string_lossy().to_string(),
                    reason: parse_err.to_string(),
                },
            },
            // Couldn't parse it and couldn't preserve it — writing now would
            // be destructive, so block saving entirely.
            Err(move_err) => LoadResult {
                file: WorkspaceFile::default(),
                status: LoadStatus::Blocked {
                    reason: format!(
                        "config could not be parsed ({parse_err}) and could not be backed up ({move_err})"
                    ),
                },
            },
        },
    }
}

fn migrate(file: WorkspaceFile) -> WorkspaceFile {
    // No migrations yet; CURRENT_VERSION is the only version that has
    // ever existed. Future versions branch on `file.version` here and
    // transform forward before returning.
    let mut file = file;
    file.version = CURRENT_VERSION;
    file
}

/// Moves an unparseable config aside so it is never overwritten, returning
/// the backup path. Never clobbers an existing backup.
fn quarantine(path: &Path) -> io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut backup = with_suffix(path, &format!("corrupt-{stamp}"));
    let mut n = 1;
    while backup.exists() {
        backup = with_suffix(path, &format!("corrupt-{stamp}-{n}"));
        n += 1;
    }
    fs::rename(path, &backup)?;
    Ok(backup)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{suffix}"));
    path.with_file_name(name)
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
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_file() -> WorkspaceFile {
        WorkspaceFile {
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
        }
    }

    fn backups_in(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(".corrupt-"))
            })
            .collect()
    }

    #[test]
    fn round_trips_workspaces() {
        let dir = temp_dir("roundtrip");
        save(&dir, &sample_file()).unwrap();

        let loaded = load(&dir);
        assert!(matches!(loaded.status, LoadStatus::Ok));
        assert_eq!(loaded.file.workspaces.len(), 1);
        assert_eq!(loaded.file.workspaces[0].name, "Test");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_yields_empty_default() {
        let dir = temp_dir("missing");
        let loaded = load(&dir);
        assert!(matches!(loaded.status, LoadStatus::Ok));
        assert_eq!(loaded.file.workspaces.len(), 0);
        assert_eq!(loaded.file.version, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_leaves_no_tmp_file_behind() {
        let dir = temp_dir("atomic");
        save(&dir, &WorkspaceFile::default()).unwrap();
        assert!(!dir.join("workspaces.json.tmp").exists());
        assert!(dir.join("workspaces.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A corrupt config must be preserved, not thrown away.
    #[test]
    fn corrupt_config_is_quarantined_and_reported() {
        let dir = temp_dir("corrupt");
        fs::write(config_path(&dir), "{ this is not valid json").unwrap();

        let loaded = load(&dir);

        let LoadStatus::Recovered {
            backup_path,
            reason,
        } = &loaded.status
        else {
            panic!("expected Recovered, got {:?}", loaded.status);
        };
        assert!(!reason.is_empty());
        assert!(loaded.file.workspaces.is_empty());

        // The original bytes survive, under the backup name.
        let backup = PathBuf::from(backup_path);
        assert!(backup.exists(), "backup {backup:?} should exist");
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "{ this is not valid json"
        );
        assert!(
            backup
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("workspaces.json.corrupt-"),
            "unexpected backup name: {backup:?}"
        );
        // It was moved, not copied.
        assert!(!config_path(&dir).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The actual data-loss regression (issue #1): after a corrupt load, the
    /// next save must not destroy the user's original config.
    #[test]
    fn saving_after_a_corrupt_load_does_not_destroy_the_original() {
        let dir = temp_dir("corrupt-then-save");
        let original = r#"{"version":1,"workspaces":[TRUNCATED"#;
        fs::write(config_path(&dir), original).unwrap();

        let loaded = load(&dir);
        assert!(matches!(loaded.status, LoadStatus::Recovered { .. }));

        // Simulate the app saving an empty list right after startup.
        save(&dir, &WorkspaceFile::default()).unwrap();

        let backups = backups_in(&dir);
        assert_eq!(backups.len(), 1, "expected exactly one backup");
        assert_eq!(
            fs::read_to_string(&backups[0]).unwrap(),
            original,
            "the user's original config must survive a subsequent save"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarantine_does_not_clobber_an_existing_backup() {
        let dir = temp_dir("corrupt-twice");

        fs::write(config_path(&dir), "first corruption").unwrap();
        let first = load(&dir);
        assert!(matches!(first.status, LoadStatus::Recovered { .. }));

        fs::write(config_path(&dir), "second corruption").unwrap();
        let second = load(&dir);
        assert!(matches!(second.status, LoadStatus::Recovered { .. }));

        let backups = backups_in(&dir);
        assert_eq!(backups.len(), 2, "each corruption keeps its own backup");
        let mut contents: Vec<String> = backups
            .iter()
            .map(|p| fs::read_to_string(p).unwrap())
            .collect();
        contents.sort();
        assert_eq!(contents, vec!["first corruption", "second corruption"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ok_and_recovered_allow_saving_blocked_does_not() {
        assert!(LoadStatus::Ok.save_block_reason().is_none());
        assert!(LoadStatus::Recovered {
            backup_path: "b".into(),
            reason: "r".into(),
        }
        .save_block_reason()
        .is_none());
        assert_eq!(
            LoadStatus::Blocked {
                reason: "locked".into()
            }
            .save_block_reason(),
            Some("locked")
        );
    }

    /// The UI matches on `kind` and reads `backupPath`. `rename_all` alone
    /// does not rename fields *inside* enum variants (the bug that silently
    /// broke `delayAfterMs`), so pin the exact wire shape the frontend
    /// expects in `types.ts`.
    #[test]
    fn load_status_serializes_in_the_shape_the_ui_expects() {
        let ok = serde_json::to_value(LoadStatus::Ok).unwrap();
        assert_eq!(ok["kind"], "ok");

        let recovered = serde_json::to_value(LoadStatus::Recovered {
            backup_path: "C:/x/workspaces.json.corrupt-1".into(),
            reason: "expected value".into(),
        })
        .unwrap();
        assert_eq!(recovered["kind"], "recovered");
        assert_eq!(recovered["backupPath"], "C:/x/workspaces.json.corrupt-1");
        assert_eq!(recovered["reason"], "expected value");
        assert!(recovered.get("backup_path").is_none());

        let blocked = serde_json::to_value(LoadStatus::Blocked {
            reason: "locked".into(),
        })
        .unwrap();
        assert_eq!(blocked["kind"], "blocked");
        assert_eq!(blocked["reason"], "locked");
    }

    /// A schema-valid file that isn't a WorkspaceFile still counts as corrupt.
    #[test]
    fn valid_json_with_wrong_shape_is_quarantined() {
        let dir = temp_dir("wrong-shape");
        fs::write(config_path(&dir), r#"{"totally":"different"}"#).unwrap();

        let loaded = load(&dir);
        assert!(
            matches!(loaded.status, LoadStatus::Recovered { .. }),
            "got {:?}",
            loaded.status
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
