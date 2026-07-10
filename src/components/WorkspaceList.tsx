import { useState } from "react";
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

      {workspaces.length === 0 && <p>No workspaces yet. Create one to get started.</p>}

      <ul>
        {workspaces.map((workspace) => (
          <li key={workspace.id} className="workspace-card">
            <div className="workspace-card-header">
              <span className="workspace-icon">{workspace.icon ?? "🚀"}</span>
              <strong>{workspace.name}</strong>
            </div>
            {workspace.description && <p>{workspace.description}</p>}
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
