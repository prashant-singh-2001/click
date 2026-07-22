import type { ConfigStatus } from "../types";

// Surfaces a damaged or unreadable config to the user. Without this, the app
// silently looks like it lost every workspace (see issue #1).
export function ConfigWarning({ status }: { status: ConfigStatus }) {
  if (status.kind === "ok") return null;

  if (status.kind === "recovered") {
    return (
      <div className="banner banner-warning" role="alert">
        <strong>Your workspaces couldn't be read.</strong>
        <p>
          The config file was damaged, so Click started with an empty list. Your original file
          was kept — nothing was overwritten. You can inspect or repair it at:
        </p>
        <p>
          <code className="banner-path">{status.backupPath}</code>
        </p>
        <p className="banner-detail">Reason: {status.reason}</p>
      </div>
    );
  }

  return (
    <div className="banner banner-error" role="alert">
      <strong>Your workspaces couldn't be read, and saving is turned off.</strong>
      <p>
        Click won't overwrite the existing config file, because it may still be intact. Any
        changes you make now will not be saved. Move or fix the file, then restart Click.
      </p>
      <p className="banner-detail">Reason: {status.reason}</p>
    </div>
  );
}
