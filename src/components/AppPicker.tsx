import { useEffect, useMemo, useRef, useState } from "react";
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
  const dialogRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.listInstalledApps(false).then(setApps).catch(console.error);
  }, []);

  // Move focus into the dialog on open, and hand it back to whatever opened
  // it on close. Reading activeElement here rather than using `autoFocus` is
  // what makes the restore possible at all: autoFocus fires during commit,
  // before effects run, so by this point the trigger would already have lost
  // focus and there'd be nothing to return to (issue #22).
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    inputRef.current?.focus();
    return () => previouslyFocused?.focus();
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
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key !== "Tab") return;

    // This dialog declares aria-modal="true", which promises focus cannot
    // reach the editor behind the overlay. Nothing enforced that promise, so
    // Tab walked straight out of it (issue #22). Rescan disables itself while
    // scanning, hence :not([disabled]).
    const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    if (!focusable || focusable.length === 0) return;

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  return (
    // Backdrop click-to-dismiss is a mouse-only affordance; Escape and the
    // Close button are the keyboard equivalents, so this needs no key handler.
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal app-picker"
        role="dialog"
        aria-modal="true"
        aria-label="Choose an installed app"
        ref={dialogRef}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="field-row">
          <input
            className="field-grow"
            ref={inputRef}
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
