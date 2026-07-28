export type LaunchStrategy = "sequential";

export interface AppAction {
  type: "app";
  id: string;
  label: string;
  path: string;
  args: string[];
  cwd?: string | null;
  enabled: boolean;
  delayAfterMs?: number | null;
}

export interface UrlAction {
  type: "url";
  id: string;
  label: string;
  url: string;
  enabled: boolean;
  delayAfterMs?: number | null;
}

export type Action = AppAction | UrlAction;

export interface Workspace {
  id: string;
  name: string;
  description: string;
  icon?: string | null;
  color?: string | null;
  tags: string[];
  variables: Record<string, string>;
  launchStrategy: LaunchStrategy;
  defaultDelayMs: number;
  hotkey?: string | null;
  actions: Action[];
}

export type ActionStatus = "skipped" | "started" | "failed";

export interface ActionOutcome {
  actionId: string;
  label: string;
  status: ActionStatus;
  message?: string | null;
}

export interface LaunchReport {
  outcomes: ActionOutcome[];
}

// Mirrors `store::LoadStatus` in Rust — how the config loaded at startup.
export type ConfigStatus =
  | { kind: "ok" }
  | { kind: "recovered"; backupPath: string; reason: string }
  | { kind: "blocked"; reason: string };

// Mirrors `hotkeys::HotkeyStatus` in Rust — whether a workspace's global
// hotkey is bound, and why not if it isn't.
export type HotkeyStatus =
  | { kind: "unset" }
  | { kind: "registered" }
  | { kind: "invalid"; reason: string }
  | { kind: "duplicate"; workspaceName: string }
  | { kind: "conflict"; reason: string };

// Mirrors `installed_apps::InstalledApp` in Rust — one Start Menu app
// resolved to a real .exe path, for the app picker (issue #32).
export interface InstalledApp {
  name: string;
  path: string;
}

export function newWorkspace(): Workspace {
  return {
    id: crypto.randomUUID(),
    name: "New workspace",
    description: "",
    icon: null,
    color: null,
    tags: [],
    variables: {},
    launchStrategy: "sequential",
    defaultDelayMs: 300,
    hotkey: null,
    actions: [],
  };
}

// The sentinel default label — used to decide whether picking an app from
// the installed-apps picker (issue #32) should also fill in the label, or
// leave a name the user already typed alone.
export const DEFAULT_APP_LABEL = "New app";

export function newAppAction(): AppAction {
  return {
    type: "app",
    id: crypto.randomUUID(),
    label: DEFAULT_APP_LABEL,
    path: "",
    args: [],
    cwd: null,
    enabled: true,
    delayAfterMs: null,
  };
}

export function newUrlAction(): UrlAction {
  return {
    type: "url",
    id: crypto.randomUUID(),
    label: "New URL",
    url: "",
    enabled: true,
    delayAfterMs: null,
  };
}
