import { invoke } from "@tauri-apps/api/core";
import type { Action, ConfigStatus, LaunchReport, Workspace } from "./types";

export const api = {
  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),
  getWorkspace: (id: string) => invoke<Workspace>("get_workspace", { id }),
  saveWorkspace: (workspace: Workspace) =>
    invoke<void>("save_workspace", { workspace }),
  deleteWorkspace: (id: string) => invoke<void>("delete_workspace", { id }),
  duplicateWorkspace: (id: string) =>
    invoke<Workspace>("duplicate_workspace", { id }),
  validateAction: (action: Action) =>
    invoke<string | null>("validate_action", { action }),
  launchWorkspace: (id: string) =>
    invoke<LaunchReport>("launch_workspace_by_id", { id }),
  createDesktopShortcut: (id: string) =>
    invoke<string>("create_desktop_shortcut", { id }),
  configStatus: () => invoke<ConfigStatus>("config_status"),
};
