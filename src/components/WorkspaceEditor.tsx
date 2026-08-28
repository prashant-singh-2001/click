import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import {
  DEFAULT_APP_LABEL,
  DEFAULT_URL_LABEL,
  newAppAction,
  newUrlAction,
  newWorkspace,
} from "../types";
import type { HotkeyStatus, LaunchReport, Workspace } from "../types";
import { ActionEditor } from "./ActionEditor";
import { HotkeyInput } from "./HotkeyInput";
import { IconPicker } from "./IconPicker";
import { LaunchProgress } from "./LaunchProgress";
import { TagEditor } from "./TagEditor";

// A blank label never shows up in any list or tray row today (only inside
// ActionEditor's own input), but saving one anyway would just be storing bad
// data — this defaults it to the same label a freshly-added action gets,
// same as the name field is required rather than left blank (issue #21).
function withDefaultedLabels(workspace: Workspace): Workspace {
  return {
    ...workspace,
    actions: workspace.actions.map((action) =>
      action.label.trim()
        ? action
        : { ...action, label: action.type === "app" ? DEFAULT_APP_LABEL : DEFAULT_URL_LABEL },
    ),
  };
}

export function WorkspaceEditor({
  workspaceId,
  onSaved,
  onCancel,
}: {
  workspaceId: string | null;
  onSaved: () => void;
  onCancel: () => void;
}) {
  // null means "new" — nothing exists server-side yet to fetch, so start a
  // blank draft immediately (issue #16).
  const [draft, setDraft] = useState<Workspace | null>(
    workspaceId === null ? newWorkspace() : null,
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [launching, setLaunching] = useState(false);
  const [report, setReport] = useState<LaunchReport | null>(null);
  const [reportWasUnsaved, setReportWasUnsaved] = useState(false);
  const [shortcutMessage, setShortcutMessage] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [hotkeyStatus, setHotkeyStatus] = useState<HotkeyStatus | null>(null);
  // The last-saved shape of this workspace, for the "you're launching unsaved
  // edits" hint (issue #9) — null means there's nothing saved to compare
  // against yet, which is also true and correct for a brand-new workspace.
  // Updated after the initial fetch and after every successful save; a plain
  // JSON.stringify comparison can over-report dirty on key-order differences
  // but won't under-report for this shape (every update spreads the existing
  // object), which is the right trade for a hint rather than a hard gate.
  const savedJson = useRef<string | null>(null);

  useEffect(() => {
    if (workspaceId === null) return;
    api
      .getWorkspace(workspaceId)
      .then((workspace) => {
        setDraft(workspace);
        savedJson.current = JSON.stringify(workspace);
      })
      .catch((err) => setLoadError(String(err)));
  }, [workspaceId]);

  const draftId = draft?.id;
  useEffect(() => {
    // Depends on draftId, not draft, so this refetches only when the loaded
    // workspace's identity changes — not on every keystroke. The editor only
    // ever hosts one workspace for its whole lifetime (opening a different
    // one goes through the list first, which passes a new workspaceId and
    // re-runs the fetch effect above).
    if (!draftId) return;
    api.hotkeyStatus(draftId).then(setHotkeyStatus).catch(console.error);
  }, [draftId]);

  if (loadError) {
    return (
      <div className="workspace-editor">
        <div className="banner banner-error" role="alert">
          Failed to load workspace: {loadError}
        </div>
        <button type="button" onClick={onCancel}>
          Back
        </button>
      </div>
    );
  }

  if (!draft) {
    return (
      <div className="workspace-editor">
        <p>Loading workspace…</p>
      </div>
    );
  }

  const handleHotkeyChange = (next: string | null) => {
    setDraft({ ...draft, hotkey: next });
    if (!next) {
      setHotkeyStatus({ kind: "unset" });
      return;
    }
    api.probeHotkey(next, draft.id).then(setHotkeyStatus).catch(console.error);
  };

  const updateAction = (index: number, next: Workspace["actions"][number]) => {
    const actions = [...draft.actions];
    actions[index] = next;
    setDraft({ ...draft, actions });
  };

  const removeAction = (index: number) => {
    setDraft({ ...draft, actions: draft.actions.filter((_, i) => i !== index) });
  };

  const moveAction = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= draft.actions.length) return;
    const actions = [...draft.actions];
    [actions[index], actions[target]] = [actions[target], actions[index]];
    setDraft({ ...draft, actions });
  };

  const updateVariable = (oldKey: string, newKey: string, value: string) => {
    const variables = { ...draft.variables };
    if (oldKey !== newKey) delete variables[oldKey];
    variables[newKey] = value;
    setDraft({ ...draft, variables });
  };

  const removeVariable = (key: string) => {
    const variables = { ...draft.variables };
    delete variables[key];
    setDraft({ ...draft, variables });
  };

  const handleSave = async () => {
    setSaving(true);
    setActionError(null);
    try {
      const sanitized = withDefaultedLabels(draft);
      setDraft(sanitized);
      await api.saveWorkspace(sanitized);
      savedJson.current = JSON.stringify(sanitized);
      onSaved();
    } catch (err) {
      setActionError(`Save failed: ${String(err)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleCreateShortcut = async () => {
    setShortcutMessage(null);
    try {
      const sanitized = withDefaultedLabels(draft);
      setDraft(sanitized);
      await api.saveWorkspace(sanitized);
      savedJson.current = JSON.stringify(sanitized);
      const path = await api.createDesktopShortcut(sanitized.id);
      setShortcutMessage(`Created: ${path}`);
    } catch (err) {
      setShortcutMessage(`Failed: ${String(err)}`);
    }
  };

  // Launches the draft exactly as shown, saved or not (issue #9) — the
  // previous behavior launched by id, which ran the last-saved record and
  // couldn't launch a brand-new workspace at all (nothing to look up yet).
  const handleLaunch = async () => {
    setLaunching(true);
    setReport(null);
    setActionError(null);
    const wasUnsaved = savedJson.current === null || JSON.stringify(draft) !== savedJson.current;
    try {
      const result = await api.launchDraft(draft);
      setReport(result);
      setReportWasUnsaved(wasUnsaved);
    } catch (err) {
      setActionError(`Launch failed: ${String(err)}`);
    } finally {
      setLaunching(false);
    }
  };

  const nameIsBlank = draft.name.trim() === "";

  return (
    <div className="workspace-editor">
      <div className="field-row">
        <input
          className="field-grow workspace-name-input"
          value={draft.name}
          onChange={(e) => setDraft({ ...draft, name: e.currentTarget.value })}
          placeholder="Workspace name"
        />
      </div>
      {nameIsBlank && <div className="action-warning">⚠ Name is required</div>}
      <textarea
        value={draft.description}
        onChange={(e) => setDraft({ ...draft, description: e.currentTarget.value })}
        placeholder="Description (optional)"
        rows={2}
      />

      <section>
        <h3>Appearance</h3>
        <div className="field-row">
          <label>Icon:</label>
          <IconPicker
            value={draft.icon ?? null}
            onChange={(icon) => setDraft({ ...draft, icon })}
          />
        </div>
        <div className="field-row">
          <label className="color-picker-label">
            Color:
            <input
              type="color"
              value={draft.color ?? "#000000"}
              onChange={(e) => setDraft({ ...draft, color: e.currentTarget.value })}
              aria-label="Workspace color"
            />
          </label>
          {draft.color && (
            <button type="button" onClick={() => setDraft({ ...draft, color: null })}>
              No color
            </button>
          )}
        </div>
        <div>
          <label>Tags:</label>
          <TagEditor value={draft.tags} onChange={(tags) => setDraft({ ...draft, tags })} />
        </div>
      </section>

      <section>
        <h3>Variables</h3>
        {Object.entries(draft.variables).map(([key, value]) => (
          <div className="field-row" key={key}>
            <input
              value={key}
              onChange={(e) => updateVariable(key, e.currentTarget.value, value)}
              placeholder="NAME"
            />
            <input
              className="field-grow"
              value={value}
              onChange={(e) => updateVariable(key, key, e.currentTarget.value)}
              placeholder="value"
            />
            {/* Interpolate the name so N rows don't all announce the same
                thing, matching TagEditor's `Remove tag ${tag}` (issue #22). */}
            <button
              type="button"
              aria-label={`Remove variable ${key}`}
              onClick={() => removeVariable(key)}
            >
              ✕
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={() => updateVariable("", `VAR_${Object.keys(draft.variables).length + 1}`, "")}
        >
          + Add variable
        </button>
      </section>

      <section>
        <h3>Actions</h3>
        {draft.actions.map((action, index) => (
          <ActionEditor
            key={action.id}
            action={action}
            onChange={(next) => updateAction(index, next)}
            onRemove={() => removeAction(index)}
            onMoveUp={() => moveAction(index, -1)}
            onMoveDown={() => moveAction(index, 1)}
          />
        ))}
        <div className="field-row">
          <button
            type="button"
            onClick={() => setDraft({ ...draft, actions: [...draft.actions, newAppAction()] })}
          >
            + Add app
          </button>
          <button
            type="button"
            onClick={() => setDraft({ ...draft, actions: [...draft.actions, newUrlAction()] })}
          >
            + Add URL
          </button>
        </div>
      </section>

      <section>
        <h3>Launch settings</h3>
        <label>
          Default delay between actions (ms):
          <input
            type="number"
            value={draft.defaultDelayMs}
            onChange={(e) => setDraft({ ...draft, defaultDelayMs: Number(e.currentTarget.value) })}
          />
        </label>
        <div className="field-row">
          <label>Global hotkey (optional):</label>
          <HotkeyInput
            value={draft.hotkey ?? null}
            status={hotkeyStatus}
            onCommit={handleHotkeyChange}
          />
        </div>
      </section>

      <div className="field-row editor-actions">
        <button type="button" onClick={handleSave} disabled={saving || nameIsBlank}>
          {saving ? "Saving…" : "Save"}
        </button>
        <button type="button" onClick={handleLaunch} disabled={launching}>
          {launching ? "Launching…" : "Launch"}
        </button>
        <button type="button" onClick={handleCreateShortcut} disabled={nameIsBlank}>
          Create desktop shortcut
        </button>
        <button type="button" onClick={onCancel}>
          Close
        </button>
      </div>

      {actionError && (
        <div className="banner banner-error" role="alert">
          {actionError}
        </div>
      )}
      {shortcutMessage && <p className="shortcut-message">{shortcutMessage}</p>}
      {report && (
        <>
          {reportWasUnsaved && (
            <p className="unsaved-launch-hint">This ran your unsaved edits — Save to keep them.</p>
          )}
          <LaunchProgress report={report} />
        </>
      )}
    </div>
  );
}
