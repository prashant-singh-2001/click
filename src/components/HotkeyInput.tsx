import { useEffect, useState } from "react";
import { api } from "../api";
import type { HotkeyStatus } from "../types";

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

// Keys allowed to stand alone, without a modifier — everything else would
// otherwise swallow an ordinary keystroke system-wide.
const BARE_KEY_ALLOWED = new Set([
  "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
  "F13", "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
  "MediaPlay", "MediaPause", "MediaPlayPause", "MediaStop",
  "MediaTrackNext", "MediaTrackPrevious",
  "AudioVolumeUp", "AudioVolumeDown", "AudioVolumeMute",
]);

// The Rust accelerator parser (global-hotkey) accepts raw KeyboardEvent.code
// names verbatim — it uppercases and matches them directly — so no
// key-mapping table is needed here. Only Digit/Key are stripped, to match
// what's already stored in existing configs and shown in the README
// ("Ctrl+Alt+1", not "Ctrl+Alt+Digit1").
function prettifyCode(code: string): string {
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Key")) return code.slice(3);
  return code;
}

type KeyLike = {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
};

// Returns the accelerator string, or null if the combo needs a modifier and
// doesn't have one.
function buildAccelerator(e: KeyLike): string | null {
  const modifiers: string[] = [];
  if (e.ctrlKey) modifiers.push("Ctrl");
  if (e.altKey) modifiers.push("Alt");
  if (e.shiftKey) modifiers.push("Shift");
  if (e.metaKey) modifiers.push("Super");

  if (modifiers.length === 0 && !BARE_KEY_ALLOWED.has(e.code)) {
    return null;
  }
  return [...modifiers, prettifyCode(e.code)].join("+");
}

function StatusLabel({ status }: { status: HotkeyStatus | null }) {
  if (!status || status.kind === "unset") return null;
  switch (status.kind) {
    case "registered":
      return <span className="hotkey-status hotkey-status-ok">Active</span>;
    case "invalid":
      return (
        <span className="hotkey-status hotkey-status-error">Invalid: {status.reason}</span>
      );
    case "duplicate":
      return (
        <span className="hotkey-status hotkey-status-error">
          Already used by "{status.workspaceName}"
        </span>
      );
    case "conflict":
      return (
        <span className="hotkey-status hotkey-status-error">
          In use by another app ({status.reason})
        </span>
      );
  }
}

export function HotkeyInput({
  value,
  status,
  onCommit,
}: {
  value: string | null;
  status: HotkeyStatus | null;
  onCommit: (value: string | null) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const [needsModifier, setNeedsModifier] = useState(false);

  // A combo already bound to anything — another of the user's own
  // workspaces, or another app — is intercepted by the OS before it ever
  // reaches this window as a normal keypress (that's what a global hotkey
  // means), so Click's own bindings are released for the duration of
  // capture. Otherwise a workspace could never "see" a combo one of its
  // siblings already holds.
  useEffect(() => {
    return () => {
      if (capturing) api.resumeHotkeys();
    };
  }, [capturing]);

  function enterCapture() {
    setCapturing(true);
    setNeedsModifier(false);
    api.suspendHotkeys();
  }

  function endCapture() {
    setCapturing(false);
    setNeedsModifier(false);
    api.resumeHotkeys();
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLButtonElement>) {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.code === "Escape") {
      endCapture();
      return;
    }
    const bare = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
    if (bare && (e.code === "Backspace" || e.code === "Delete")) {
      endCapture();
      onCommit(null);
      return;
    }
    if (MODIFIER_CODES.has(e.code)) {
      return; // keep waiting for the main key
    }

    const accelerator = buildAccelerator(e);
    if (accelerator === null) {
      setNeedsModifier(true);
      return;
    }
    endCapture();
    onCommit(accelerator);
  }

  return (
    <div className="hotkey-input">
      <button
        type="button"
        className={capturing ? "hotkey-capture-active" : undefined}
        onClick={enterCapture}
        onKeyDown={handleKeyDown}
        onBlur={endCapture}
      >
        {capturing ? "Press a key combo…" : value || "Click to set a hotkey"}
      </button>
      {value && !capturing && (
        <button
          type="button"
          className="hotkey-clear"
          aria-label="Clear hotkey"
          onClick={() => onCommit(null)}
        >
          ✕
        </button>
      )}
      {capturing && needsModifier && (
        <span className="hotkey-status hotkey-status-warn">
          Add a modifier (Ctrl, Alt, Shift, or Win)
        </span>
      )}
      {!capturing && <StatusLabel status={status} />}
    </div>
  );
}
