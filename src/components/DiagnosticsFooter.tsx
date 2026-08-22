import { useEffect, useState } from "react";
import { api } from "../api";

// The log dir (app_log_dir) is a different root from the config dir
// (app_config_dir) shown nowhere else in the UI — logs are machine-local and
// must never sync with a roaming profile, which makes the path unguessable
// from anything else the user already knows about Click (issue #6).
export function DiagnosticsFooter() {
  const [logDir, setLogDir] = useState<string | null>(null);

  useEffect(() => {
    api.logDir().then(setLogDir).catch(console.error);
  }, []);

  if (!logDir) return null;

  return (
    <footer className="diagnostics-footer">
      <span className="diagnostics-path">
        Logs: <code>{logDir}</code>
      </span>
      <button type="button" onClick={() => api.openLogDir().catch(console.error)}>
        Open logs folder
      </button>
    </footer>
  );
}
