import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type { InstalledApp } from "../types";

export function AppPicker({
  onSelect,
  onClose,
}: {
  onSelect: (app: InstalledApp) => void;
  onClose: () => void;
}) {
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [query, setQuery] = useState("");
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    api.listInstalledApps(false).then(setApps).catch(console.error);
  }, []);

  const filtered = useMemo(() => {
    if (!apps) return [];
    const q = query.trim().toLowerCase();
    if (!q) return apps;
    return apps.filter((app) => app.name.toLowerCase().includes(q));
  }, [apps, query]);

  async function handleRescan() {
    setRefreshing(true);
    try {
      setApps(await api.listInstalledApps(true));
    } finally {
      setRefreshing(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  return (
    <div className="modal-overlay" onClick={onClose} onKeyDown={handleKeyDown}>
      <div
        className="modal app-picker"
        role="dialog"
        aria-modal="true"
        aria-label="Choose an installed app"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="field-row">
          <input
            className="field-grow"
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            placeholder="Search installed apps…"
          />
          <button type="button" onClick={handleRescan} disabled={refreshing}>
            {refreshing ? "Rescanning…" : "Rescan"}
          </button>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        {apps === null ? (
          <p className="app-picker-status">Scanning installed apps…</p>
        ) : filtered.length === 0 ? (
          <p className="app-picker-status">
            {apps.length === 0 ? "No installed apps found." : "No matches."}
          </p>
        ) : (
          <ul className="app-picker-list">
            {filtered.map((app) => (
              <li key={app.path}>
                <button
                  type="button"
                  className="app-picker-item"
                  onClick={() => onSelect(app)}
                >
                  <span className="app-picker-name">{app.name}</span>
                  <span className="app-picker-path">{app.path}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
