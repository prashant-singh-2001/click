use parking_lot::Mutex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// One entry in the installed-apps picker (issue #32). `path` is always a
/// resolved `.exe`, never a `.lnk` — launching a shortcut directly isn't
/// supported by the launch engine (issue #17), so resolution happens once
/// here rather than at every launch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
}

/// Caches the scan — walking both Start Menu trees and resolving every
/// `.lnk` via COM is too slow to repeat on every picker open, but cheap
/// enough to do once per app session.
pub struct InstalledAppsState {
    pub cache: Mutex<Option<Vec<InstalledApp>>>,
}

pub fn init(app: &tauri::App) {
    app.manage(InstalledAppsState {
        cache: Mutex::new(None),
    });
}

/// Both Start Menu `Programs` trees. A missing root (e.g. no machine-wide
/// tree) is skipped, not an error.
fn start_menu_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(data_dir) = app.path().data_dir() {
        roots.push(
            data_dir
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(program_data) = std::env::var("ProgramData") {
        roots.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    roots
}

fn walk_lnk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_lnk_files(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            out.push(path);
        }
    }
}

/// Filters and dedupes raw (display name, resolved target) pairs. Pulled
/// out of the COM-dependent scan so it's unit-testable without touching
/// the shell.
fn filter_and_dedup(mut apps: Vec<(String, String)>) -> Vec<InstalledApp> {
    apps.retain(|(name, target)| {
        let target_path = Path::new(target);
        let is_exe = target_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        let looks_like_uninstaller = name.to_ascii_lowercase().contains("uninstall")
            || target_path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.to_ascii_lowercase().starts_with("unins"));

        is_exe && !looks_like_uninstaller && target_path.exists()
    });

    // Dedupe by resolved path (case-insensitive — Windows paths are), keeping
    // whichever display name is shortest (fewer decorations like "(64-bit)").
    let mut by_path: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for (name, target) in apps {
        let key = target.to_ascii_lowercase();
        by_path
            .entry(key)
            .and_modify(|(existing_name, _)| {
                if name.len() < existing_name.len() {
                    *existing_name = name.clone();
                }
            })
            .or_insert((name, target));
    }

    let mut result: Vec<InstalledApp> = by_path
        .into_values()
        .map(|(name, path)| InstalledApp { name, path })
        .collect();
    result.sort_by_key(|a| a.name.to_ascii_lowercase());
    result
}

#[cfg(windows)]
mod resolve {
    use windows::core::{Interface, GUID, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    /// Resolves a `.lnk` file to its target path via the same COM shell
    /// APIs Explorer itself uses — the canonical resolver, correctly
    /// expanding environment-style targets that a pure-Rust `.lnk` parser
    /// would get wrong.
    pub fn resolve_lnk(lnk_path: &std::path::Path) -> Option<String> {
        unsafe {
            let shell_link: IShellLinkW = CoCreateInstance(
                &ShellLink as *const GUID as *const _,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .ok()?;
            let persist_file: IPersistFile = shell_link.cast().ok()?;

            let wide: Vec<u16> = lnk_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            persist_file.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

            let mut buf = [0u16; 260]; // MAX_PATH
            let mut find_data = std::mem::zeroed();
            shell_link.GetPath(&mut buf, &mut find_data, 0).ok()?;

            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if end == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..end]))
        }
    }

    use std::os::windows::ffi::OsStrExt;

    pub struct ComGuard;

    impl ComGuard {
        pub fn new() -> Option<Self> {
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok();
            result.ok()?;
            Some(ComGuard)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
}

fn scan(app: &AppHandle) -> Vec<InstalledApp> {
    #[cfg(windows)]
    {
        let Some(_com) = resolve::ComGuard::new() else {
            return Vec::new();
        };

        let mut lnk_files = Vec::new();
        for root in start_menu_roots(app) {
            walk_lnk_files(&root, &mut lnk_files);
        }

        let raw: Vec<(String, String)> = lnk_files
            .iter()
            .filter_map(|lnk| {
                let name = lnk.file_stem()?.to_str()?.to_string();
                let target = resolve::resolve_lnk(lnk)?;
                Some((name, target))
            })
            .collect();

        filter_and_dedup(raw)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Vec::new()
    }
}

/// Returns the cached scan, or performs one if `refresh` is true or nothing
/// is cached yet.
pub fn list(app: &AppHandle, refresh: bool) -> Vec<InstalledApp> {
    let state = app.state::<InstalledAppsState>();
    if !refresh {
        if let Some(cached) = state.cache.lock().clone() {
            return cached;
        }
    }

    let apps = scan(app);
    *state.cache.lock() = Some(apps.clone());
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};

    /// Issue #4 — see the matching test on `AppState` in `lib.rs` for the
    /// full rationale.
    #[test]
    fn installed_apps_state_survives_a_panic_while_locked() {
        let state = InstalledAppsState {
            cache: Mutex::new(None),
        };

        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.cache.lock();
            panic!("simulated panic while holding the lock");
        }));

        assert!(state.cache.lock().is_none());
    }

    /// A fixture directory holding real (empty) files, so `.exists()` and
    /// extension/name checks in `filter_and_dedup` are exercised against
    /// genuine paths rather than a path that happens to already exist.
    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(test_name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("click-installed-apps-test-{test_name}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Fixture { dir }
        }

        /// Creates an empty file named `filename` and returns its path.
        fn touch(&self, filename: &str) -> String {
            let path = self.dir.join(filename);
            std::fs::write(&path, b"").unwrap();
            path.to_string_lossy().to_string()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn drops_uninstallers() {
        let fx = Fixture::new("drops_uninstallers");
        let apps = filter_and_dedup(vec![
            ("VS Code".to_string(), fx.touch("Code.exe")),
            ("Uninstall VS Code".to_string(), fx.touch("unins000.exe")),
        ]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "VS Code");
    }

    #[test]
    fn drops_non_exe_targets() {
        let fx = Fixture::new("drops_non_exe_targets");
        let apps = filter_and_dedup(vec![("Readme".to_string(), fx.touch("readme.txt"))]);
        assert!(apps.is_empty());
    }

    #[test]
    fn drops_missing_targets() {
        let apps = filter_and_dedup(vec![(
            "Ghost".to_string(),
            r"C:\definitely\does\not\exist\ghost.exe".to_string(),
        )]);
        assert!(apps.is_empty());
    }

    #[test]
    fn dedupes_by_path_keeping_shortest_name() {
        let fx = Fixture::new("dedupes_by_path_keeping_shortest_name");
        let target = fx.touch("Code.exe");
        let apps = filter_and_dedup(vec![
            ("Visual Studio Code (User)".to_string(), target.clone()),
            ("VS Code".to_string(), target.clone()),
        ]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "VS Code");
    }

    #[test]
    fn dedup_is_case_insensitive_on_path() {
        let fx = Fixture::new("dedup_is_case_insensitive_on_path");
        let target = fx.touch("Code.exe");
        let uppercased = target.to_ascii_uppercase();
        let apps = filter_and_dedup(vec![
            ("VS Code".to_string(), target),
            ("VS Code Alt Casing".to_string(), uppercased),
        ]);
        assert_eq!(apps.len(), 1);
    }

    #[test]
    fn sorts_case_insensitively_by_name() {
        let fx = Fixture::new("sorts_case_insensitively_by_name");
        let apps = filter_and_dedup(vec![
            ("zebra".to_string(), fx.touch("zebra.exe")),
            ("Apple".to_string(), fx.touch("apple.exe")),
        ]);
        assert_eq!(apps[0].name, "Apple");
        assert_eq!(apps[1].name, "zebra");
    }

    /// Proves the `unsafe` COM resolution path actually works, rather than
    /// merely compiling: creates a real `.lnk` with `mslnk` (the same crate
    /// `shortcut.rs` uses to write shortcuts) and resolves it back through
    /// `resolve_lnk`. Compares canonicalized paths since `IShellLinkW::GetPath`
    /// may return an 8.3 short path rather than the original long form.
    #[cfg(windows)]
    #[test]
    fn resolves_a_real_lnk_file_via_com() {
        use mslnk::ShellLink;

        let dir = std::env::temp_dir().join("click-installed-apps-com-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let target = std::env::current_exe().unwrap();
        let target_str = target.to_string_lossy().to_string();

        let link = ShellLink::new(&target_str).unwrap();
        let lnk_path = dir.join("Test App.lnk");
        link.create_lnk(&lnk_path).unwrap();

        let _com = resolve::ComGuard::new().expect("COM should initialize");
        let resolved = resolve::resolve_lnk(&lnk_path).expect("should resolve the lnk target");

        assert_eq!(
            std::fs::canonicalize(&resolved).unwrap(),
            std::fs::canonicalize(&target).unwrap(),
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
