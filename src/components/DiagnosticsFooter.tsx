import { useEffect, useState } from "react";
import { api } from "../api";
import type { UpdateStatus } from "../types";

// The log dir (app_log_dir) is a different root from the config dir
// (app_config_dir) shown nowhere else in the UI — logs are machine-local and
// must never sync with a roaming profile, which makes the path unguessable
// from anything else the user already knows about Click (issue #6).
export function DiagnosticsFooter() {
  const [logDir, setLogDir] = useState<string | null>(null);
  const [checkingForUpdates, setCheckingForUpdates] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus | null>(null);

  useEffect(() => {
    api.logDir().then(setLogDir).catch(console.error);
  }, []);

  // The confirm/decline prompt is a native dialog shown from Rust (issue
  // #25) — by the time this promise resolves, the user has already made
  // their choice (or the app is about to restart, if they installed). This
  // just reflects what already happened; it doesn't drive the decision.
  async function handleCheckForUpdates() {
    setCheckingForUpdates(true);
    try {
      setUpdateStatus(await api.checkForUpdates());
    } catch (err) {
      setUpdateStatus({ kind: "unavailable", reason: String(err) });
    } finally {
      setCheckingForUpdates(false);
    }
  }

  if (!logDir) return null;

  return (
    <footer className="diagnostics-footer">
      <span className="diagnostics-path">
        Logs: <code>{logDir}</code>
      </span>
      <div className="diagnostics-updates">
        {updateStatus && (
          <span className="diagnostics-update-status">{describeUpdateStatus(updateStatus)}</span>
        )}
        <button type="button" onClick={handleCheckForUpdates} disabled={checkingForUpdates}>
          {checkingForUpdates ? "Checking…" : "Check for updates"}
        </button>
      </div>
      <button type="button" onClick={() => api.openLogDir().catch(console.error)}>
        Open logs folder
      </button>
    </footer>
  );
}

function describeUpdateStatus(status: UpdateStatus): string {
  switch (status.kind) {
    case "upToDate":
      return `You're up to date (v${status.currentVersion})`;
    case "declined":
      return `Update to v${status.availableVersion} declined`;
    case "installing":
      return `Installing v${status.availableVersion} — Click will restart shortly`;
    case "unavailable":
      return `Update check unavailable: ${status.reason}`;
  }
}
