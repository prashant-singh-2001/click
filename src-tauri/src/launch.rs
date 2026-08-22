use crate::model::{Action, Workspace};
use crate::vars;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub action_id: String,
    pub label: String,
    pub status: ActionStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionStatus {
    Skipped,
    Started,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub outcomes: Vec<ActionOutcome>,
}

impl LaunchReport {
    /// A one-line human summary for the log — "2 started, 1 failed". Omits
    /// any status with a zero count so an all-success run reads as "3
    /// started" rather than "3 started, 0 failed, 0 skipped".
    pub fn summary(&self) -> String {
        let started = self.count(ActionStatus::Started);
        let failed = self.count(ActionStatus::Failed);
        let skipped = self.count(ActionStatus::Skipped);

        let parts: Vec<String> = [
            (started, "started"),
            (failed, "failed"),
            (skipped, "skipped"),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect();

        if parts.is_empty() {
            "no actions".to_string()
        } else {
            parts.join(", ")
        }
    }

    fn count(&self, status: ActionStatus) -> usize {
        self.outcomes.iter().filter(|o| o.status == status).count()
    }
}

/// An action resolved down to something concrete and executable — variables
/// substituted, `.cmd`/`.bat` routing already decided. Pure data: holds no
/// `AppHandle` and runs nothing. Not `Serialize` yet — nothing needs to send
/// this over IPC until a dry-run command exists, and adding a wire format
/// with no `types.ts` mirror would violate the hand-mirroring invariant in
/// `CLAUDE.md`.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedCommand {
    Spawn {
        /// What `Command::new()` actually targets — `"cmd"` when routed
        /// through a `.cmd`/`.bat` shim, the resolved path otherwise.
        program: String,
        args: Vec<String>,
        cwd: Option<String>,
        /// The action's resolved target path, used only in error messages —
        /// always the original path, even when spawning goes through the
        /// `cmd /C` shim, so a failure is attributed to the script rather
        /// than to `cmd` itself.
        display_path: String,
    },
    OpenUrl {
        url: String,
    },
    /// A `.lnk`, `.msi`, or a document — handed to the OS shell so it opens
    /// exactly as double-clicking it would (issue #17). Never carries args
    /// or a cwd: the opener API has no channel for either, so a target that
    /// needs them is rejected in `resolve_app` before this variant exists.
    OpenPath {
        path: String,
    },
}

impl ResolvedCommand {
    /// What gets logged at the default level — enough to say what launched,
    /// never enough to leak a secret-bearing argument. See `describe_verbose`
    /// for the full command, which only ever reaches the log at DEBUG.
    fn describe(&self) -> String {
        match self {
            ResolvedCommand::Spawn { display_path, .. } => format!("run '{display_path}'"),
            ResolvedCommand::OpenUrl { url } => format!("open url '{url}'"),
            ResolvedCommand::OpenPath { path } => format!("open '{path}'"),
        }
    }

    /// The full resolved command, args and cwd included. An arg can carry a
    /// value pulled from `${VAR}` or the process environment, and this
    /// project's own SECURITY.md already warns against plaintext secrets
    /// reaching disk — so this is only ever logged at DEBUG (`CLICK_LOG=debug`),
    /// never at the default level.
    fn describe_verbose(&self) -> String {
        match self {
            ResolvedCommand::Spawn {
                program,
                args,
                cwd,
                display_path,
            } => {
                let cwd = cwd.as_deref().unwrap_or("(inherited)");
                format!("run '{display_path}' via '{program}' args={args:?} cwd={cwd}")
            }
            ResolvedCommand::OpenUrl { url } => format!("open url '{url}'"),
            ResolvedCommand::OpenPath { path } => format!("open '{path}'"),
        }
    }
}

/// What planning decided to do with one action, before anything runs.
#[derive(Debug, Clone, PartialEq)]
pub enum PlannedStep {
    /// `action.enabled() == false` — nothing was resolved.
    Skipped,
    Run(ResolvedCommand),
    /// Variable resolution failed; this becomes `ActionStatus::Failed` when run.
    Unresolved(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedAction {
    pub action_id: String,
    pub label: String,
    pub delay_after_ms: Option<u64>,
    pub step: PlannedStep,
}

/// Runs every enabled action in order, honoring per-action / default delays.
/// A failing action is recorded and the run continues (NFR-3) — one bad
/// path must never block the rest of a workspace from launching. Thin
/// composition of the pure planner (`plan_workspace`) and the pure loop
/// driver (`run_plan`); `execute` is the only part of this pipeline that
/// touches the OS or an `AppHandle`.
pub fn launch_workspace<F: FnMut(&ActionOutcome)>(
    app: &AppHandle,
    workspace: &Workspace,
    on_outcome: F,
) -> LaunchReport {
    let plan = plan_workspace(workspace);
    run_plan(
        &plan,
        workspace.default_delay_ms,
        |cmd| execute(app, cmd),
        std::thread::sleep,
        on_outcome,
    )
}

/// How long to pause after an action, if at all. Pure so the policy is
/// unit-testable without an `AppHandle`. A skipped action never delays —
/// nothing was started to space out — and neither does the last action to
/// actually run, since nothing follows it and returning sooner is strictly
/// better.
fn delay_after(
    status: ActionStatus,
    delay_after_ms: Option<u64>,
    default_delay_ms: u64,
    is_last: bool,
) -> Option<Duration> {
    if matches!(status, ActionStatus::Skipped) || is_last {
        return None;
    }
    match delay_after_ms.unwrap_or(default_delay_ms) {
        0 => None,
        ms => Some(Duration::from_millis(ms)),
    }
}

/// Resolves every action in `workspace` against its variables — path/args/cwd
/// or url substituted, `.cmd`/`.bat` routing decided — without executing
/// anything or touching an `AppHandle`. Pure so resolution, skip-disabled,
/// and script routing are unit-testable in isolation from the actual spawn /
/// URL open (issue #7).
pub fn plan_workspace(workspace: &Workspace) -> Vec<PlannedAction> {
    workspace
        .actions
        .iter()
        .map(|action| plan_action(action, &workspace.variables))
        .collect()
}

fn plan_action(action: &Action, variables: &HashMap<String, String>) -> PlannedAction {
    let step = if !action.enabled() {
        PlannedStep::Skipped
    } else {
        match resolve_action(action, variables) {
            Ok(cmd) => PlannedStep::Run(cmd),
            Err(message) => PlannedStep::Unresolved(message),
        }
    };

    PlannedAction {
        action_id: action.id().to_string(),
        label: action.label().to_string(),
        delay_after_ms: action.delay_after_ms(),
        step,
    }
}

fn resolve_action(
    action: &Action,
    variables: &HashMap<String, String>,
) -> Result<ResolvedCommand, String> {
    match action {
        Action::App {
            path, args, cwd, ..
        } => resolve_app(path, args, cwd.as_deref(), variables),
        Action::Url { url, .. } => resolve_url(url, variables),
    }
}

/// How a resolved path should be launched, based on its extension alone.
/// Shared between `resolve_app` (decides how to build the plan) and
/// `commands::validate_action` (warns in the editor before the same problem
/// would surface as a launch failure) — one rule, not two copies that could
/// drift apart.
pub(crate) enum Route {
    /// `Command::new` can spawn it directly. Extensionless counts —
    /// `CreateProcess` appends `.exe` on Windows — and so does `.com`.
    Native,
    /// `code`, `npm`, and similar dev-tool shims on Windows are `.cmd`/`.bat`
    /// files, not native executables — `Command::new` cannot spawn them
    /// directly and fails with an opaque OS error, so route through `cmd /C`.
    /// Unlike `Shell`, this preserves args and cwd.
    Script,
    /// A `.lnk`, `.msi`, or a document — nothing `Command::new` can run.
    /// Handed to the OS shell instead, matching what double-clicking it does
    /// (issue #17). The shell-open API has no channel for args or a cwd.
    Shell,
}

pub(crate) fn route_for(path: &str) -> Route {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("cmd") | Some("bat") => Route::Script,
        None | Some("exe") | Some("com") => Route::Native,
        Some(_) => Route::Shell,
    }
}

fn resolve_app(
    path: &str,
    args: &[String],
    cwd: Option<&str>,
    variables: &HashMap<String, String>,
) -> Result<ResolvedCommand, String> {
    let resolved_path = vars::resolve(path, variables).map_err(|e| e.to_string())?;
    let resolved_args = args
        .iter()
        .map(|a| vars::resolve(a, variables))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let resolved_cwd = cwd
        .map(|c| vars::resolve(c, variables))
        .transpose()
        .map_err(|e| e.to_string())?;

    match route_for(&resolved_path) {
        Route::Shell => {
            if !resolved_args.is_empty() || resolved_cwd.is_some() {
                return Err(format!(
                    "'{resolved_path}' isn't a program Windows can run directly, so it opens \
                     with its associated app — which can't accept arguments or a working \
                     directory. Remove them, or point this action at the .exe."
                ));
            }
            Ok(ResolvedCommand::OpenPath {
                path: resolved_path,
            })
        }
        Route::Script => {
            let mut cmd_args = vec!["/C".to_string(), resolved_path.clone()];
            cmd_args.extend(resolved_args);
            Ok(ResolvedCommand::Spawn {
                program: "cmd".to_string(),
                args: cmd_args,
                cwd: resolved_cwd,
                display_path: resolved_path,
            })
        }
        Route::Native => Ok(ResolvedCommand::Spawn {
            program: resolved_path.clone(),
            args: resolved_args,
            cwd: resolved_cwd,
            display_path: resolved_path,
        }),
    }
}

fn resolve_url(url: &str, variables: &HashMap<String, String>) -> Result<ResolvedCommand, String> {
    let resolved = vars::resolve(url, variables).map_err(|e| e.to_string())?;
    Ok(ResolvedCommand::OpenUrl { url: resolved })
}

/// Drives a resolved plan to completion: executes each `Run` step via
/// `exec`, applies the same skip/continue-on-failure/delay policy the
/// original loop used, and reports each outcome through `on_outcome` as it
/// happens. `exec` and `sleep` are injected so the loop's control flow —
/// skip-disabled, continue-on-failure (NFR-3), and delay sequencing — is
/// unit-testable without spawning a real process or a real sleep.
fn run_plan<E, S, F>(
    plan: &[PlannedAction],
    default_delay_ms: u64,
    mut exec: E,
    mut sleep: S,
    mut on_outcome: F,
) -> LaunchReport
where
    E: FnMut(&ResolvedCommand) -> Result<(), String>,
    S: FnMut(Duration),
    F: FnMut(&ActionOutcome),
{
    let mut outcomes = Vec::with_capacity(plan.len());

    // The action whose delay would actually be observed — a trailing run of
    // disabled actions must not force a wasted sleep before returning.
    let last_active = plan
        .iter()
        .rposition(|p| !matches!(p.step, PlannedStep::Skipped));

    for (index, planned) in plan.iter().enumerate() {
        let (status, message) = match &planned.step {
            PlannedStep::Skipped => (ActionStatus::Skipped, None),
            PlannedStep::Unresolved(message) => (ActionStatus::Failed, Some(message.clone())),
            PlannedStep::Run(cmd) => match exec(cmd) {
                Ok(()) => (ActionStatus::Started, None),
                Err(message) => (ActionStatus::Failed, Some(message)),
            },
        };

        let outcome = ActionOutcome {
            action_id: planned.action_id.clone(),
            label: planned.label.clone(),
            status,
            message,
        };

        on_outcome(&outcome);

        let is_last = last_active == Some(index);
        if let Some(delay) = delay_after(
            outcome.status,
            planned.delay_after_ms,
            default_delay_ms,
            is_last,
        ) {
            sleep(delay);
        }

        outcomes.push(outcome);
    }

    LaunchReport { outcomes }
}

/// Everything left that genuinely needs the OS or the `AppHandle` — no
/// branching, no resolution, nothing left worth a unit test.
fn execute(app: &AppHandle, cmd: &ResolvedCommand) -> Result<(), String> {
    log::info!("{}", cmd.describe());
    log::debug!("{}", cmd.describe_verbose());

    match cmd {
        ResolvedCommand::Spawn {
            program,
            args,
            cwd,
            display_path,
        } => {
            let mut command = Command::new(program);
            command.args(args);

            if let Some(dir) = cwd {
                command.current_dir(dir);
            }

            #[cfg(windows)]
            command.creation_flags(CREATE_NO_WINDOW);

            command
                .spawn()
                .map(|_child| ())
                .map_err(|e| format!("failed to launch '{}': {}", display_path, e))
        }
        ResolvedCommand::OpenUrl { url } => app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| format!("failed to open '{}': {}", url, e)),
        ResolvedCommand::OpenPath { path } => app
            .opener()
            .open_path(path, None::<&str>)
            .map_err(|e| format!("failed to open '{}': {}", path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LaunchStrategy;
    use uuid::Uuid;

    #[test]
    fn skipped_action_never_delays() {
        assert_eq!(delay_after(ActionStatus::Skipped, None, 300, false), None);
        assert_eq!(
            delay_after(ActionStatus::Skipped, Some(500), 300, false),
            None
        );
    }

    #[test]
    fn last_action_never_delays() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(500), 300, true),
            None
        );
        assert_eq!(delay_after(ActionStatus::Failed, None, 300, true), None);
    }

    #[test]
    fn per_action_override_wins_over_default() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(500), 300, false),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn falls_back_to_workspace_default() {
        assert_eq!(
            delay_after(ActionStatus::Started, None, 300, false),
            Some(Duration::from_millis(300))
        );
    }

    #[test]
    fn explicit_zero_delay_is_none() {
        assert_eq!(
            delay_after(ActionStatus::Started, Some(0), 300, false),
            None
        );
        assert_eq!(delay_after(ActionStatus::Failed, None, 0, false), None);
    }

    // ---- test helpers ----

    fn workspace_with(actions: Vec<Action>) -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            description: String::new(),
            icon: None,
            color: None,
            tags: vec![],
            variables: HashMap::new(),
            launch_strategy: LaunchStrategy::Sequential,
            default_delay_ms: 300,
            hotkey: None,
            actions,
        }
    }

    fn workspace_with_vars(actions: Vec<Action>, variables: HashMap<String, String>) -> Workspace {
        Workspace {
            variables,
            ..workspace_with(actions)
        }
    }

    fn app_action(path: &str, args: Vec<&str>, enabled: bool) -> Action {
        Action::App {
            id: Uuid::new_v4(),
            label: "App".to_string(),
            path: path.to_string(),
            args: args.into_iter().map(String::from).collect(),
            cwd: None,
            enabled,
            delay_after_ms: None,
        }
    }

    fn url_action(url: &str) -> Action {
        Action::Url {
            id: Uuid::new_v4(),
            label: "Url".to_string(),
            url: url.to_string(),
            enabled: true,
            delay_after_ms: None,
        }
    }

    fn planned(step: PlannedStep) -> PlannedAction {
        PlannedAction {
            action_id: Uuid::new_v4().to_string(),
            label: "Action".to_string(),
            delay_after_ms: None,
            step,
        }
    }

    fn spawn_step() -> PlannedStep {
        PlannedStep::Run(ResolvedCommand::Spawn {
            program: "app.exe".to_string(),
            args: vec![],
            cwd: None,
            display_path: "app.exe".to_string(),
        })
    }

    // ---- plan_workspace ----

    #[test]
    fn disabled_action_plans_as_skipped_without_resolving() {
        // An undefined variable would make resolution fail if it were ever
        // attempted — proving skip happens *before* resolution, not after.
        let action = app_action("${UNDEFINED_VAR}", vec![], false);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert_eq!(plan[0].step, PlannedStep::Skipped);
    }

    #[test]
    fn cmd_extension_routes_through_cmd_shim_with_args_preserved() {
        let action = app_action("C:/tools/code.cmd", vec!["arg1", "arg2"], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Run(ResolvedCommand::Spawn { program, args, .. }) => {
                assert_eq!(program, "cmd");
                assert_eq!(
                    args,
                    &vec![
                        "/C".to_string(),
                        "C:/tools/code.cmd".to_string(),
                        "arg1".to_string(),
                        "arg2".to_string(),
                    ]
                );
            }
            other => panic!("expected Run(Spawn), got {other:?}"),
        }
    }

    #[test]
    fn bat_extension_routes_through_cmd_shim() {
        let action = app_action("C:/tools/legacy.bat", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert!(matches!(
            &plan[0].step,
            PlannedStep::Run(ResolvedCommand::Spawn { program, .. }) if program == "cmd"
        ));
    }

    #[test]
    fn cmd_routing_is_case_insensitive() {
        let action = app_action("C:/tools/code.CMD", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert!(matches!(
            &plan[0].step,
            PlannedStep::Run(ResolvedCommand::Spawn { program, .. }) if program == "cmd"
        ));
    }

    #[test]
    fn exe_extension_spawns_directly_not_through_cmd() {
        let action = app_action("C:/tools/app.exe", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Run(ResolvedCommand::Spawn { program, .. }) => {
                assert_eq!(program, "C:/tools/app.exe");
            }
            other => panic!("expected Run(Spawn), got {other:?}"),
        }
    }

    #[test]
    fn extensionless_path_spawns_directly_not_through_cmd() {
        let action = app_action("C:/tools/app", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Run(ResolvedCommand::Spawn { program, .. }) => {
                assert_eq!(program, "C:/tools/app");
            }
            other => panic!("expected Run(Spawn), got {other:?}"),
        }
    }

    #[test]
    fn com_extension_spawns_directly_not_through_cmd() {
        let action = app_action("C:/tools/legacy.com", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Run(ResolvedCommand::Spawn { program, .. }) => {
                assert_eq!(program, "C:/tools/legacy.com");
            }
            other => panic!("expected Run(Spawn), got {other:?}"),
        }
    }

    // ---- issue #17: .lnk / .msi / documents route through the shell ----

    #[test]
    fn lnk_extension_opens_through_the_shell() {
        let action = app_action("C:/Users/me/Desktop/App.lnk", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert_eq!(
            plan[0].step,
            PlannedStep::Run(ResolvedCommand::OpenPath {
                path: "C:/Users/me/Desktop/App.lnk".to_string()
            })
        );
    }

    #[test]
    fn msi_extension_opens_through_the_shell() {
        let action = app_action("C:/downloads/installer.msi", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert_eq!(
            plan[0].step,
            PlannedStep::Run(ResolvedCommand::OpenPath {
                path: "C:/downloads/installer.msi".to_string()
            })
        );
    }

    #[test]
    fn document_extension_opens_through_the_shell() {
        let action = app_action("C:/notes/todo.docx", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert_eq!(
            plan[0].step,
            PlannedStep::Run(ResolvedCommand::OpenPath {
                path: "C:/notes/todo.docx".to_string()
            })
        );
    }

    #[test]
    fn shell_routing_is_case_insensitive() {
        let action = app_action("C:/Users/me/Desktop/App.LNK", vec![], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert!(matches!(
            &plan[0].step,
            PlannedStep::Run(ResolvedCommand::OpenPath { .. })
        ));
    }

    #[test]
    fn args_on_a_shell_routed_target_fail_to_resolve_naming_the_path() {
        let action = app_action("C:/Users/me/Desktop/App.lnk", vec!["some-arg"], true);
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Unresolved(message) => {
                assert!(message.contains("App.lnk"), "message was: {message}");
            }
            other => panic!("expected Unresolved, got {other:?}"),
        }
    }

    #[test]
    fn cwd_on_a_shell_routed_target_fails_to_resolve() {
        let action = Action::App {
            id: Uuid::new_v4(),
            label: "App".to_string(),
            path: "C:/Users/me/Desktop/App.lnk".to_string(),
            args: vec![],
            cwd: Some("C:/Users/me".to_string()),
            enabled: true,
            delay_after_ms: None,
        };
        let ws = workspace_with(vec![action]);

        let plan = plan_workspace(&ws);

        assert!(matches!(&plan[0].step, PlannedStep::Unresolved(_)));
    }

    #[test]
    fn substitutes_variables_in_path_args_and_cwd() {
        let mut vars = HashMap::new();
        vars.insert("DIR".to_string(), "C:/proj".to_string());
        let action = Action::App {
            id: Uuid::new_v4(),
            label: "App".to_string(),
            path: "${DIR}/app.exe".to_string(),
            args: vec!["${DIR}/arg".to_string()],
            cwd: Some("${DIR}".to_string()),
            enabled: true,
            delay_after_ms: None,
        };
        let ws = workspace_with_vars(vec![action], vars);

        let plan = plan_workspace(&ws);

        match &plan[0].step {
            PlannedStep::Run(ResolvedCommand::Spawn {
                program, args, cwd, ..
            }) => {
                assert_eq!(program, "C:/proj/app.exe");
                assert_eq!(args, &vec!["C:/proj/arg".to_string()]);
                assert_eq!(cwd.as_deref(), Some("C:/proj"));
            }
            other => panic!("expected Run(Spawn), got {other:?}"),
        }
    }

    #[test]
    fn url_action_resolves_variables() {
        let mut vars = HashMap::new();
        vars.insert("PORT".to_string(), "3000".to_string());
        let ws = workspace_with_vars(vec![url_action("http://localhost:${PORT}")], vars);

        let plan = plan_workspace(&ws);

        assert_eq!(
            plan[0].step,
            PlannedStep::Run(ResolvedCommand::OpenUrl {
                url: "http://localhost:3000".to_string()
            })
        );
    }

    #[test]
    fn undefined_variable_yields_unresolved_but_other_actions_still_plan() {
        let bad = app_action("${NOPE}/app.exe", vec![], true);
        let good = app_action("C:/tools/app.exe", vec![], true);
        let ws = workspace_with(vec![bad, good]);

        let plan = plan_workspace(&ws);

        assert!(matches!(plan[0].step, PlannedStep::Unresolved(_)));
        assert!(matches!(plan[1].step, PlannedStep::Run(_)));
    }

    // ---- run_plan ----

    #[test]
    fn continue_on_failure_across_a_run() {
        let plan = vec![
            planned(spawn_step()),
            planned(spawn_step()),
            planned(spawn_step()),
        ];
        let mut call = 0;

        let report = run_plan(
            &plan,
            0,
            |_cmd| {
                call += 1;
                if call == 2 {
                    Err("boom".to_string())
                } else {
                    Ok(())
                }
            },
            |_d| {},
            |_| {},
        );

        let statuses: Vec<ActionStatus> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(
            statuses,
            vec![
                ActionStatus::Started,
                ActionStatus::Failed,
                ActionStatus::Started,
            ]
        );
        assert_eq!(call, 3, "the third action must actually execute");
        assert_eq!(report.outcomes[1].message.as_deref(), Some("boom"));
    }

    #[test]
    fn unresolved_step_fails_without_calling_exec_but_still_delays() {
        let plan = vec![
            planned(PlannedStep::Unresolved("bad var".to_string())),
            planned(spawn_step()),
        ];
        let mut exec_calls = 0;
        let mut sleeps = vec![];

        let report = run_plan(
            &plan,
            250,
            |_cmd| {
                exec_calls += 1;
                Ok(())
            },
            |d| sleeps.push(d),
            |_| {},
        );

        assert_eq!(exec_calls, 1, "only the resolved action should execute");
        assert_eq!(report.outcomes[0].status, ActionStatus::Failed);
        assert_eq!(report.outcomes[0].message.as_deref(), Some("bad var"));
        assert_eq!(
            sleeps,
            vec![Duration::from_millis(250)],
            "a Failed action still delays — only Skipped and the last action don't"
        );
    }

    #[test]
    fn skip_disabled_never_executes_and_delays_only_around_active_actions() {
        let plan = vec![
            planned(PlannedStep::Skipped),
            planned(spawn_step()),
            planned(spawn_step()),
        ];
        let mut exec_calls = 0;
        let mut sleeps = vec![];

        let report = run_plan(
            &plan,
            250,
            |_cmd| {
                exec_calls += 1;
                Ok(())
            },
            |d| sleeps.push(d),
            |_| {},
        );

        assert_eq!(exec_calls, 2, "the skipped action must never execute");
        assert_eq!(report.outcomes[0].status, ActionStatus::Skipped);
        // Exactly one delay: after action 2 (the middle, non-last active
        // action). None after the skip, none after action 3 (last active).
        assert_eq!(sleeps, vec![Duration::from_millis(250)]);
    }

    #[test]
    fn delay_sequencing_uses_override_then_default_and_ignores_trailing_disabled() {
        let mut with_override = planned(spawn_step());
        with_override.delay_after_ms = Some(999);
        let uses_default = planned(spawn_step());
        let trailing_disabled = planned(PlannedStep::Skipped);

        let plan = vec![with_override, uses_default, trailing_disabled];
        let mut sleeps = vec![];

        run_plan(&plan, 300, |_cmd| Ok(()), |d| sleeps.push(d), |_| {});

        // The trailing disabled action doesn't count as "last active", so
        // action 2 (index 1) is last_active and must not delay — only
        // action 1's override delay is recorded.
        assert_eq!(sleeps, vec![Duration::from_millis(999)]);
    }

    #[test]
    fn on_outcome_fires_once_per_action_in_order_before_returning() {
        let plan = vec![
            planned(spawn_step()),
            planned(PlannedStep::Skipped),
            planned(spawn_step()),
        ];
        let mut seen = vec![];

        let report = run_plan(
            &plan,
            0,
            |_cmd| Ok(()),
            |_d| {},
            |outcome| seen.push(outcome.status),
        );

        assert_eq!(
            seen,
            vec![
                ActionStatus::Started,
                ActionStatus::Skipped,
                ActionStatus::Started,
            ]
        );
        assert_eq!(seen.len(), report.outcomes.len());
    }

    // ---- describe / describe_verbose (logging redaction contract) ----

    fn spawn_with_secret_arg() -> ResolvedCommand {
        ResolvedCommand::Spawn {
            program: "app.exe".to_string(),
            args: vec!["--token".to_string(), "sekrit-value".to_string()],
            cwd: Some(r"C:\private\project".to_string()),
            display_path: "app.exe".to_string(),
        }
    }

    #[test]
    fn describe_never_contains_args_or_cwd() {
        let cmd = spawn_with_secret_arg();
        let description = cmd.describe();

        assert!(!description.contains("sekrit-value"));
        assert!(!description.contains("private"));
        assert!(description.contains("app.exe"));
    }

    #[test]
    fn describe_verbose_contains_args_and_cwd() {
        let cmd = spawn_with_secret_arg();
        let description = cmd.describe_verbose();

        assert!(description.contains("sekrit-value"));
        assert!(description.contains("private"));
    }

    #[test]
    fn describe_covers_every_resolved_command_variant() {
        assert_eq!(
            ResolvedCommand::OpenUrl {
                url: "https://example.com".to_string(),
            }
            .describe(),
            "open url 'https://example.com'"
        );
        assert_eq!(
            ResolvedCommand::OpenPath {
                path: "C:/Shortcuts/App.lnk".to_string(),
            }
            .describe(),
            "open 'C:/Shortcuts/App.lnk'"
        );
    }

    // ---- LaunchReport::summary ----

    fn outcome(status: ActionStatus) -> ActionOutcome {
        ActionOutcome {
            action_id: Uuid::new_v4().to_string(),
            label: "Action".to_string(),
            status,
            message: None,
        }
    }

    #[test]
    fn summary_of_empty_report_says_no_actions() {
        let report = LaunchReport { outcomes: vec![] };
        assert_eq!(report.summary(), "no actions");
    }

    #[test]
    fn summary_omits_zero_counts() {
        let report = LaunchReport {
            outcomes: vec![
                outcome(ActionStatus::Started),
                outcome(ActionStatus::Started),
            ],
        };
        assert_eq!(report.summary(), "2 started");
    }

    #[test]
    fn summary_reports_mixed_statuses_in_a_fixed_order() {
        let report = LaunchReport {
            outcomes: vec![
                outcome(ActionStatus::Started),
                outcome(ActionStatus::Failed),
                outcome(ActionStatus::Skipped),
                outcome(ActionStatus::Skipped),
            ],
        };
        assert_eq!(report.summary(), "1 started, 1 failed, 2 skipped");
    }
}
