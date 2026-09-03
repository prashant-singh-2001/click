import { invoke } from "@tauri-apps/api/core";
import type {
  Action,
  ConfigStatus,
  HotkeyStatus,
  InstalledApp,
  LaunchReport,
  UpdateStatus,
  Workspace,
} from "./types";

export const api = {
  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),
  getWorkspace: (id: string) => invoke<Workspace>("get_workspace", { id }),
  saveWorkspace: (workspace: Workspace) =>
    invoke<void>("save_workspace", { workspace }),
  deleteWorkspace: (id: string) => invoke<void>("delete_workspace", { id }),
  restoreWorkspace: (workspace: Workspace, index: number) =>
    invoke<void>("restore_workspace", { workspace, index }),
  duplicateWorkspace: (id: string) =>
    invoke<Workspace>("duplicate_workspace", { id }),
  validateAction: (action: Action) =>
    invoke<string | null>("validate_action", { action }),
  launchWorkspace: (id: string) =>
    invoke<LaunchReport>("launch_workspace_by_id", { id }),
  launchDraft: (workspace: Workspace) =>
    invoke<LaunchReport>("launch_workspace_draft", { workspace }),
  createDesktopShortcut: (id: string) =>
    invoke<string>("create_desktop_shortcut", { id }),
  configStatus: () => invoke<ConfigStatus>("config_status"),
  probeHotkey: (accelerator: string, workspaceId: string) =>
    invoke<HotkeyStatus>("probe_hotkey", { accelerator, workspaceId }),
  hotkeyStatus: (id: string) => invoke<HotkeyStatus>("hotkey_status", { id }),
  suspendHotkeys: () => invoke<void>("suspend_hotkeys"),
  resumeHotkeys: () => invoke<void>("resume_hotkeys"),
  listInstalledApps: (refresh: boolean) =>
    invoke<InstalledApp[]>("list_installed_apps", { refresh }),
  logDir: () => invoke<string>("log_dir"),
  openLogDir: () => invoke<void>("open_log_dir"),
  checkForUpdates: () => invoke<UpdateStatus>("check_for_updates"),
};
