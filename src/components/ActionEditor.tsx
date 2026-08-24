import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { parseArgs, formatArgs } from "../args";
import type { Action, InstalledApp } from "../types";
import { DEFAULT_APP_LABEL } from "../types";
import { AppPicker } from "./AppPicker";

export function ActionEditor({
  action,
  onChange,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  action: Action;
  onChange: (next: Action) => void;
  onRemove: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const [warning, setWarning] = useState<string | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  // Local text, not derived from action.args on every render — deriving it
  // ate whitespace and moved the caret mid-typing (issue #8), since the
  // field's value would immediately re-render from the parsed-and-rejoined
  // array. Safe to seed once: ActionEditor is the sole writer of args in the
  // whole frontend, and WorkspaceEditor keys this component by action.id, so
  // a genuinely different action always remounts with a fresh initializer
  // rather than reusing this state.
  const [argsText, setArgsText] = useState(() =>
    action.type === "app" ? formatArgs(action.args) : "",
  );

  useEffect(() => {
    api.validateAction(action).then(setWarning).catch(console.error);
  }, [action]);

  function handleArgsChange(text: string) {
    setArgsText(text);
    if (action.type === "app") {
      onChange({ ...action, args: parseArgs(text) });
    }
  }

  function handlePickApp(app: InstalledApp) {
    setPickerOpen(false);
    if (action.type !== "app") return;
    // Don't clobber a label the user already customized — only fill it in
    // while it's still the factory default.
    const label = action.label === DEFAULT_APP_LABEL ? app.name : action.label;
    onChange({ ...action, path: app.path, label });
  }

  return (
    <div className={`action-editor ${action.enabled ? "" : "action-disabled"}`}>
      <div className="action-editor-row">
        <input
          type="checkbox"
          checked={action.enabled}
          onChange={(e) => onChange({ ...action, enabled: e.currentTarget.checked })}
          title="Enabled"
          aria-label="Enabled"
        />
        <input
          className="action-label"
          value={action.label}
          onChange={(e) => onChange({ ...action, label: e.currentTarget.value })}
          placeholder="Label"
        />
        <span className="action-type-badge">{action.type}</span>
        {/* `title` stays as the mouse tooltip, but it's only a last-resort
            accessible name — the glyph wins, so these announced as "up arrow"
            and "multiplication x". aria-label is the real name (issue #22). */}
        <button type="button" onClick={onMoveUp} title="Move up" aria-label="Move action up">
          ↑
        </button>
        <button
          type="button"
          onClick={onMoveDown}
          title="Move down"
          aria-label="Move action down"
        >
          ↓
        </button>
        <button type="button" onClick={onRemove} title="Remove" aria-label="Remove action">
          ✕
        </button>
      </div>

      {action.type === "app" ? (
        <div className="action-editor-fields">
          <div className="field-row">
            <input
              className="field-grow"
              value={action.path}
              onChange={(e) => onChange({ ...action, path: e.currentTarget.value })}
              placeholder="Path to an app, shortcut, or document, e.g. C:/Program Files/.../app.exe"
            />
            <button
              type="button"
              onClick={async () => {
                const selected = await open({ multiple: false });
                if (typeof selected === "string") {
                  onChange({ ...action, path: selected });
                }
              }}
            >
              Browse…
            </button>
            <button type="button" onClick={() => setPickerOpen(true)}>
              Choose app…
            </button>
          </div>
          <input
            value={argsText}
            onChange={(e) => handleArgsChange(e.currentTarget.value)}
            placeholder='Arguments, e.g. --dir "${PROJECT_DIR}"'
            aria-label="Arguments"
          />
          {action.args.length > 0 && (
            <div className="args-preview">
              Parses to {action.args.length} argument{action.args.length === 1 ? "" : "s"}:{" "}
              {action.args.map((arg, i) => (
                <code className="args-preview-item" key={i}>
                  {arg}
                </code>
              ))}
            </div>
          )}
          <input
            value={action.cwd ?? ""}
            onChange={(e) => onChange({ ...action, cwd: e.currentTarget.value || null })}
            placeholder="Working directory (optional)"
          />
        </div>
      ) : (
        <div className="action-editor-fields">
          <input
            className="field-grow"
            value={action.url}
            onChange={(e) => onChange({ ...action, url: e.currentTarget.value })}
            placeholder="https://example.com or http://localhost:3000"
          />
        </div>
      )}

      <div className="field-row">
        <label>
          Delay after (ms, optional):
          <input
            type="number"
            value={action.delayAfterMs ?? ""}
            onChange={(e) =>
              onChange({
                ...action,
                delayAfterMs: e.currentTarget.value ? Number(e.currentTarget.value) : null,
              })
            }
          />
        </label>
      </div>

      {warning && <div className="action-warning">⚠ {warning}</div>}

      {pickerOpen && (
        <AppPicker onSelect={handlePickApp} onClose={() => setPickerOpen(false)} />
      )}
    </div>
  );
}
