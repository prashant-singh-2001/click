import { useState } from "react";

export function TagEditor({
  value,
  onChange,
}: {
  value: string[];
  onChange: (next: string[]) => void;
}) {
  const [draft, setDraft] = useState("");

  // Normalise on add, not on save — trims, rejects blanks, and dedupes
  // case-insensitively, so an empty or duplicate chip never enters the
  // draft in the first place (the same class of bug issue #21 tracks for
  // names and labels).
  function addTag() {
    const trimmed = draft.trim();
    if (!trimmed) return;
    const exists = value.some((t) => t.toLowerCase() === trimmed.toLowerCase());
    if (!exists) {
      onChange([...value, trimmed]);
    }
    setDraft("");
  }

  function removeTag(tag: string) {
    onChange(value.filter((t) => t !== tag));
  }

  return (
    <div className="tag-editor">
      {value.length > 0 && (
        <div className="tag-chip-list">
          {value.map((tag) => (
            <span className="tag-chip" key={tag}>
              {tag}
              <button
                type="button"
                className="tag-chip-remove"
                aria-label={`Remove tag ${tag}`}
                onClick={() => removeTag(tag)}
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="field-row">
        <input
          value={draft}
          onChange={(e) => setDraft(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addTag();
            }
          }}
          placeholder="Add a tag"
          aria-label="New tag"
        />
        <button type="button" onClick={addTag}>
          + Add tag
        </button>
      </div>
    </div>
  );
}
