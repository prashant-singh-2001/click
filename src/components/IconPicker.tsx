// A curated, offline palette rather than a picker library — the app is
// CSP-strict with zero UI dependencies today, and any emoji the user can
// type or paste (e.g. via the Windows emoji panel, Win+.) works too.
const EMOJI_PALETTE = [
  "🚀", "💻", "🔧", "🐳", "🎨", "📊", "🔍", "⚡", "📦", "🎯",
  "🖥️", "🌐", "📝", "🎬", "🎮", "🧪", "🔐", "📱", "⚙️", "🌟",
];

export function IconPicker({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (next: string | null) => void;
}) {
  return (
    <div className="icon-picker">
      <div className="field-row">
        <span className="icon-picker-current" aria-hidden="true">
          {value ?? "🚀"}
        </span>
        <input
          className="icon-picker-input"
          value={value ?? ""}
          onChange={(e) => onChange(e.currentTarget.value || null)}
          placeholder="Emoji"
          aria-label="Workspace icon"
        />
        {value && (
          <button type="button" aria-label="Clear icon" onClick={() => onChange(null)}>
            ✕
          </button>
        )}
      </div>
      <div className="icon-picker-palette" role="group" aria-label="Choose an icon">
        {EMOJI_PALETTE.map((emoji) => (
          <button
            key={emoji}
            type="button"
            className="icon-picker-option"
            aria-label={`Use ${emoji} as icon`}
            aria-pressed={value === emoji}
            onClick={() => onChange(emoji)}
          >
            {emoji}
          </button>
        ))}
      </div>
    </div>
  );
}
