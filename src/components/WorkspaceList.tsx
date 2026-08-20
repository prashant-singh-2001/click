import { useMemo, useState } from "react";
import { api } from "../api";
import type { LaunchReport, Workspace } from "../types";
import { LaunchProgress } from "./LaunchProgress";
import { Modal } from "./Modal";

type UndoState = { workspace: Workspace; index: number };

export function WorkspaceList({
  workspaces,
  onEdit,
  onNew,
  onChanged,
}: {
  workspaces: Workspace[];
  onEdit: (workspace: Workspace) => void;
  onNew: () => void;
  onChanged: () => void;
}) {
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [reportFor, setReportFor] = useState<{ id: string; report: LaunchReport } | null>(null);
  const [query, setQuery] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Workspace | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [undoState, setUndoState] = useState<UndoState | null>(null);
  const [undoError, setUndoError] = useState<string | null>(null);

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

  async function confirmDelete() {
    if (!pendingDelete) return;
    const workspace = pendingDelete;
    // Capture position before deleting — it's what undo restores to. List
    // order is plain Vec/array insertion order (no sort anywhere), so the
    // index found here is still meaningful once the workspace is gone.
    const index = workspaces.findIndex((w) => w.id === workspace.id);

    setDeleteError(null);
    try {
      await api.deleteWorkspace(workspace.id);
      setPendingDelete(null);
      setUndoError(null);
      setUndoState({ workspace, index });
      onChanged();
    } catch (err) {
      setDeleteError(String(err));
    }
  }

  async function handleUndo() {
    if (!undoState) return;
    setUndoError(null);
    try {
      await api.restoreWorkspace(undoState.workspace, undoState.index);
      setUndoState(null);
      onChanged();
    } catch (err) {
      setUndoError(String(err));
    }
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

      {undoState && (
        <div className="banner banner-warning">
          <p>
            Deleted "{undoState.workspace.name}".{" "}
            <button type="button" onClick={() => void handleUndo()}>
              Undo
            </button>{" "}
            <button
              type="button"
              aria-label="Dismiss undo"
              onClick={() => setUndoState(null)}
            >
              Dismiss
            </button>
          </p>
          {undoError && <p className="banner-detail">Undo failed: {undoError}</p>}
        </div>
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
                onClick={() => void handleLaunch(workspace.id)}
                disabled={launchingId === workspace.id}
              >
                {launchingId === workspace.id ? "Launching…" : "Launch"}
              </button>
              <button type="button" onClick={() => onEdit(workspace)}>Edit</button>
              <button
                type="button"
                className="button-danger"
                onClick={() => {
                  setDeleteError(null);
                  setPendingDelete(workspace);
                }}
              >
                Delete
              </button>
            </div>
            {reportFor?.id === workspace.id && <LaunchProgress report={reportFor.report} />}
          </li>
        ))}
      </ul>

      {pendingDelete && (
        <Modal label={`Delete "${pendingDelete.name}"?`} onClose={() => setPendingDelete(null)}>
          <p>
            Delete "{pendingDelete.name}"? This removes every action, variable, and hotkey
            binding in it.
          </p>
          {deleteError && (
            <div className="banner banner-error" role="alert">
              Delete failed: {deleteError}
            </div>
          )}
          <div className="field-row">
            <button type="button" onClick={() => setPendingDelete(null)}>
              Cancel
            </button>
            <button type="button" className="button-danger" onClick={() => void confirmDelete()}>
              Delete
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
