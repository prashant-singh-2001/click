import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
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

  useEffect(() => {
    api.validateAction(action).then(setWarning);
  }, [action]);

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
        />
        <input
          className="action-label"
          value={action.label}
          onChange={(e) => onChange({ ...action, label: e.currentTarget.value })}
          placeholder="Label"
        />
        <span className="action-type-badge">{action.type}</span>
        <button type="button" onClick={onMoveUp} title="Move up">↑</button>
        <button type="button" onClick={onMoveDown} title="Move down">↓</button>
        <button type="button" onClick={onRemove} title="Remove">✕</button>
      </div>

      {action.type === "app" ? (
        <div className="action-editor-fields">
          <div className="field-row">
            <input
              className="field-grow"
              value={action.path}
              onChange={(e) => onChange({ ...action, path: e.currentTarget.value })}
              placeholder="Executable path, e.g. C:/Program Files/.../app.exe"
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
            value={action.args.join(" ")}
            onChange={(e) =>
              onChange({
                ...action,
                args: e.currentTarget.value.split(" ").filter((a) => a.length > 0),
              })
            }
            placeholder="Arguments, e.g. ${PROJECT_DIR}"
          />
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
