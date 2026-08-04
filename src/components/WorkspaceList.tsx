import { useMemo, useState } from "react";
import { api } from "../api";
import type { LaunchReport, Workspace } from "../types";
import { LaunchProgress } from "./LaunchProgress";

export function WorkspaceList({
  workspaces,
  onEdit,
  onNew,
  onDeleted,
}: {
  workspaces: Workspace[];
  onEdit: (workspace: Workspace) => void;
  onNew: () => void;
  onDeleted: () => void;
}) {
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [reportFor, setReportFor] = useState<{ id: string; report: LaunchReport } | null>(null);
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return workspaces;
    return workspaces.filter(
      (w) =>
        w.name.toLowerCase().includes(q) ||
        w.tags.some((tag) => tag.toLowerCase().includes(q)),
    );
  }, [workspaces, query]);

  async function handleLaunch(id: string) {
    setLaunchingId(id);
    setReportFor(null);
    try {
      const report = await api.launchWorkspace(id);
      setReportFor({ id, report });
    } finally {
      setLaunchingId(null);
    }
  }

  async function handleDelete(id: string) {
    await api.deleteWorkspace(id);
    onDeleted();
  }

  return (
    <div className="workspace-list">
      <div className="field-row">
        <button type="button" onClick={onNew}>+ New workspace</button>
      </div>

      {workspaces.length > 0 && (
        <div className="field-row">
          <input
            className="field-grow"
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            placeholder="Search workspaces or tags…"
            aria-label="Search workspaces"
          />
        </div>
      )}

      {workspaces.length === 0 && <p>No workspaces yet. Create one to get started.</p>}
      {workspaces.length > 0 && filtered.length === 0 && (
        <p>No workspaces match your search.</p>
      )}

      <ul>
        {filtered.map((workspace) => (
          <li
            key={workspace.id}
            className="workspace-card"
            style={
              workspace.color
                ? { borderLeftColor: workspace.color, borderLeftWidth: "4px" }
                : undefined
            }
          >
            <div className="workspace-card-header">
              <span className="workspace-icon">{workspace.icon ?? "🚀"}</span>
              <strong>{workspace.name}</strong>
            </div>
            {workspace.description && <p>{workspace.description}</p>}
            {workspace.tags.length > 0 && (
              <div className="tag-chip-list">
                {workspace.tags.map((tag) => (
                  <span className="tag-chip" key={tag}>
                    {tag}
                  </span>
                ))}
              </div>
            )}
            <div className="field-row">
              <button
                type="button"
                onClick={() => handleLaunch(workspace.id)}
                disabled={launchingId === workspace.id}
              >
                {launchingId === workspace.id ? "Launching…" : "Launch"}
              </button>
              <button type="button" onClick={() => onEdit(workspace)}>Edit</button>
              <button type="button" onClick={() => handleDelete(workspace.id)}>Delete</button>
            </div>
            {reportFor?.id === workspace.id && <LaunchProgress report={reportFor.report} />}
          </li>
        ))}
      </ul>
    </div>
  );
}
