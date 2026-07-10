import { useState } from "react";
import { api } from "../api";
import { newAppAction, newUrlAction } from "../types";
import type { LaunchReport, Workspace } from "../types";
import { ActionEditor } from "./ActionEditor";
import { LaunchProgress } from "./LaunchProgress";

export function WorkspaceEditor({
  workspace,
  onSaved,
  onCancel,
}: {
  workspace: Workspace;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<Workspace>(workspace);
  const [saving, setSaving] = useState(false);
  const [launching, setLaunching] = useState(false);
  const [report, setReport] = useState<LaunchReport | null>(null);
  const [shortcutMessage, setShortcutMessage] = useState<string | null>(null);

  function updateAction(index: number, next: Workspace["actions"][number]) {
    const actions = [...draft.actions];
    actions[index] = next;
    setDraft({ ...draft, actions });
  }

  function removeAction(index: number) {
    setDraft({ ...draft, actions: draft.actions.filter((_, i) => i !== index) });
  }

  function moveAction(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= draft.actions.length) return;
    const actions = [...draft.actions];
    [actions[index], actions[target]] = [actions[target], actions[index]];
    setDraft({ ...draft, actions });
  }

  function updateVariable(oldKey: string, newKey: string, value: string) {
    const variables = { ...draft.variables };
    if (oldKey !== newKey) delete variables[oldKey];
    variables[newKey] = value;
    setDraft({ ...draft, variables });
  }

  function removeVariable(key: string) {
    const variables = { ...draft.variables };
    delete variables[key];
    setDraft({ ...draft, variables });
  }

  async function handleSave() {
    setSaving(true);
    try {
      await api.saveWorkspace(draft);
      onSaved();
    } finally {
      setSaving(false);
    }
  }

  async function handleCreateShortcut() {
    setShortcutMessage(null);
    try {
      await api.saveWorkspace(draft);
      const path = await api.createDesktopShortcut(draft.id);
      setShortcutMessage(`Created: ${path}`);
    } catch (err) {
      setShortcutMessage(`Failed: ${err}`);
    }
  }

  async function handleLaunch() {
    setLaunching(true);
    setReport(null);
    try {
      const result = await api.launchWorkspace(draft.id);
      setReport(result);
    } finally {
      setLaunching(false);
    }
  }

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
      <textarea
        value={draft.description}
        onChange={(e) => setDraft({ ...draft, description: e.currentTarget.value })}
        placeholder="Description (optional)"
        rows={2}
      />

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
            <button type="button" onClick={() => removeVariable(key)}>✕</button>
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
          <label>
            Global hotkey (optional):
            <input
              value={draft.hotkey ?? ""}
              onChange={(e) => setDraft({ ...draft, hotkey: e.currentTarget.value || null })}
              placeholder="e.g. Ctrl+Alt+1"
            />
          </label>
        </div>
      </section>

      <div className="field-row editor-actions">
        <button type="button" onClick={handleSave} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
        <button type="button" onClick={handleLaunch} disabled={launching}>
          {launching ? "Launching…" : "Launch"}
        </button>
        <button type="button" onClick={handleCreateShortcut}>
          Create desktop shortcut
        </button>
        <button type="button" onClick={onCancel}>
          Close
        </button>
      </div>

      {shortcutMessage && <p className="shortcut-message">{shortcutMessage}</p>}
      {report && <LaunchProgress report={report} />}
    </div>
  );
}
