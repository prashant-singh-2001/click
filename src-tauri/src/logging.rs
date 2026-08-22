use log::LevelFilter;
use std::str::FromStr;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// Parses `CLICK_LOG`'s value (`trace`/`debug`/`info`/`warn`/`error`/`off`,
/// case-insensitive). Unset or unrecognized falls back to `Info` rather than
/// failing to start — a typo'd env var must never cost the user their app.
/// Pure and separate from `level_from_env` below so it's testable without
/// touching process-wide env state, which `cargo test`'s parallel threads
/// would otherwise race on.
fn parse_level(value: Option<&str>) -> LevelFilter {
    value
        .and_then(|v| LevelFilter::from_str(v).ok())
        .unwrap_or(LevelFilter::Info)
}

/// `CLICK_LOG` raises or lowers Click's own verbosity — third-party crate
/// noise (tauri, wry, hyper, ...) always stays capped at `Warn` regardless,
/// see `builder()`.
fn level_from_env() -> LevelFilter {
    parse_level(std::env::var("CLICK_LOG").ok().as_deref())
}

/// The name our own crate's log records carry as `target()` — `[lib] name =
/// "click_lib"` in Cargo.toml, and `log`'s default target is the module path,
/// which is rooted at the crate name. `level_for` matches on this prefix.
const OWN_CRATE_TARGET: &str = "click_lib";

/// Rotating file in `app_log_dir()` (`%LOCALAPPDATA%\com.launchpad.app\logs\
/// click.log` on Windows — a different root from the config dir, since logs
/// are machine-local and must never sync with a roaming profile), plus a
/// stdout target in dev builds only — release ships with
/// `windows_subsystem = "windows"`, so there is no console to write to there.
///
/// Args and a working directory are deliberately never logged above DEBUG —
/// see `ResolvedCommand::describe` vs `describe_verbose` in `launch.rs`. This
/// builder controls the other half of that contract: `CLICK_LOG=debug` is
/// the only way to see them, and third-party crates never get to DEBUG at
/// all, so raising our own level can't accidentally spam the file with theirs.
pub(crate) fn builder() -> Builder {
    let mut targets = vec![Target::new(TargetKind::LogDir {
        file_name: Some("click".into()),
    })];
    #[cfg(debug_assertions)]
    targets.push(Target::new(TargetKind::Stdout));

    Builder::new()
        .targets(targets)
        // One previous file kept alongside the active one, capped at 1MB —
        // launch/lifecycle events are low-volume, so this comfortably spans
        // many sessions without growing unbounded (KeepAll would).
        .max_file_size(1_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(1))
        // Local time, not UTC: this file exists to be read by a person
        // correlating "it didn't launch just now" with a timestamp.
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .level(LevelFilter::Warn)
        .level_for(OWN_CRATE_TARGET, level_from_env())
}

/// Chains onto the default hook (so debug builds keep their stderr output)
/// and logs the panic payload, location, and a forced backtrace. Without
/// this, a panic on a spawned worker — a launch, a hotkey press — left no
/// trace anywhere: the process survives it (that's the whole point of
/// `parking_lot::Mutex` over poisoning `std::sync::Mutex`, see `lib.rs`),
/// but until now nothing recorded that it happened at all.
pub(crate) fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {info}\n{backtrace}");
        default_hook(info);
    }));
}

/// A native message box for the one failure a log file structurally can't
/// help with: `tauri::Builder::run` failing during startup, which can happen
/// before the log plugin's own `setup()` has attached a logger, and which —
/// with no console in a release build — otherwise leaves the app vanishing
/// with zero visible trace. `reason` is shown inline rather than only
/// pointing at the log, for exactly that reason.
#[cfg(windows)]
pub(crate) fn fatal_startup_dialog(reason: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    // No AppHandle exists when setup() itself is what failed, so this can't
    // ask `app.path().app_log_dir()` — it duplicates that resolution instead.
    // Keep in sync with the bundle identifier (tauri.conf.json) and the log
    // target's file name (builder(), above) if either ever changes.
    let log_hint = std::env::var("LOCALAPPDATA")
        .map(|dir| format!(r"{dir}\com.launchpad.app\logs\click.log"))
        .unwrap_or_else(|_| "the app's logs folder".to_string());

    let text = format!("Click couldn't start.\n\n{reason}\n\nDetails were written to:\n{log_hint}");
    let wide_text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_title: Vec<u16> = "Click".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide_text.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_documented_value_case_insensitively() {
        for (input, expected) in [
            ("trace", LevelFilter::Trace),
            ("DEBUG", LevelFilter::Debug),
            ("Info", LevelFilter::Info),
            ("warn", LevelFilter::Warn),
            ("error", LevelFilter::Error),
            ("off", LevelFilter::Off),
        ] {
            assert_eq!(parse_level(Some(input)), expected, "input was {input}");
        }
    }

    #[test]
    fn unset_defaults_to_info() {
        assert_eq!(parse_level(None), LevelFilter::Info);
    }

    #[test]
    fn garbage_falls_back_to_info_rather_than_panicking() {
        assert_eq!(parse_level(Some("not-a-real-level")), LevelFilter::Info);
    }
}
