use serde::Serialize;
use std::fmt;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

/// Which of the three ways an update check can fire triggered this one —
/// mirrors `commands::LaunchTrigger`, logged alongside every outcome for the
/// same reason: distinguishing "the startup check failed" from "the user's
/// button failed" in the log.
#[derive(Debug, Clone, Copy)]
pub enum UpdateTrigger {
    Startup,
    Tray,
    Ui,
}

impl fmt::Display for UpdateTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UpdateTrigger::Startup => "startup",
            UpdateTrigger::Tray => "tray",
            UpdateTrigger::Ui => "ui",
        })
    }
}

/// `rename_all_fields` is required alongside `rename_all`, or struct-variant
/// fields silently stay snake_case — the same bug `store::LoadStatus` and
/// `hotkeys::HotkeyStatus` both carry a regression test for.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateStatus {
    UpToDate { current_version: String },
    Declined { available_version: String },
    Installing { available_version: String },
    Unavailable { reason: String },
}

/// Checks for an update and, if one exists, asks via a native dialog rather
/// than in-app UI — the main window is hidden by default
/// (`tauri.conf.json`'s `"visible": false`), so an in-app prompt would have
/// nowhere to render on the startup or tray-triggered paths (issue #25).
///
/// A failed or unconfigured check is `warn!`, never `error!` — no
/// `latest.json` has been published yet, and network unavailability is
/// expected, not exceptional. Never logs the download URL or signature
/// above DEBUG, matching the issue-#6 invariant that resolved
/// arguments/paths stay out of the default log.
pub async fn check_and_prompt(app: AppHandle, trigger: UpdateTrigger) -> UpdateStatus {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => {
            log::warn!("update check ({trigger}) unavailable: {err}");
            return UpdateStatus::Unavailable {
                reason: err.to_string(),
            };
        }
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            log::info!("update check ({trigger}): already up to date");
            return UpdateStatus::UpToDate {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
            };
        }
        Err(err) => {
            log::warn!("update check ({trigger}) failed: {err}");
            return UpdateStatus::Unavailable {
                reason: err.to_string(),
            };
        }
    };

    log::info!("update check ({trigger}): {} available", update.version);
    let available_version = update.version.clone();

    let message = format!(
        "Click {} is available — you're on {}. Install and restart now?",
        update.version, update.current_version
    );
    let app_for_dialog = app.clone();
    let confirmed = tauri::async_runtime::spawn_blocking(move || {
        app_for_dialog
            .dialog()
            .message(message)
            .title("Update available")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Install and Restart".to_string(),
                "Later".to_string(),
            ))
            .kind(MessageDialogKind::Info)
            .blocking_show()
    })
    .await
    .unwrap_or(false);

    if !confirmed {
        log::info!("update {available_version} declined by user");
        return UpdateStatus::Declined { available_version };
    }

    log::info!("installing update {available_version}");
    if let Err(err) = update.download_and_install(|_, _| {}, || {}).await {
        log::warn!("update {available_version} failed to install: {err}");
        return UpdateStatus::Unavailable {
            reason: err.to_string(),
        };
    }

    // restart() diverges (-> !), so it can't also be the tail expression
    // that produces this function's return value — spawn it instead of
    // calling it inline, so the caller (the IPC command, for the Ui
    // trigger) still gets Installing back rather than the process tearing
    // down before any response is sent.
    let app_for_restart = app.clone();
    tauri::async_runtime::spawn(async move {
        app_for_restart.restart();
    });
    UpdateStatus::Installing { available_version }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_trigger_displays_distinctly() {
        assert_eq!(UpdateTrigger::Startup.to_string(), "startup");
        assert_eq!(UpdateTrigger::Tray.to_string(), "tray");
        assert_eq!(UpdateTrigger::Ui.to_string(), "ui");
    }

    // Pins the wire shape types.ts expects — the same regression class as
    // store::LoadStatus and hotkeys::HotkeyStatus's tests.
    #[test]
    fn update_status_serializes_in_the_shape_the_ui_expects() {
        let up_to_date = serde_json::to_value(UpdateStatus::UpToDate {
            current_version: "0.2.4".to_string(),
        })
        .unwrap();
        assert_eq!(up_to_date["kind"], "upToDate");
        assert_eq!(up_to_date["currentVersion"], "0.2.4");

        let declined = serde_json::to_value(UpdateStatus::Declined {
            available_version: "0.3.0".to_string(),
        })
        .unwrap();
        assert_eq!(declined["kind"], "declined");
        assert_eq!(declined["availableVersion"], "0.3.0");

        let installing = serde_json::to_value(UpdateStatus::Installing {
            available_version: "0.3.0".to_string(),
        })
        .unwrap();
        assert_eq!(installing["kind"], "installing");
        assert_eq!(installing["availableVersion"], "0.3.0");

        let unavailable = serde_json::to_value(UpdateStatus::Unavailable {
            reason: "network error".to_string(),
        })
        .unwrap();
        assert_eq!(unavailable["kind"], "unavailable");
        assert_eq!(unavailable["reason"], "network error");
    }
}
