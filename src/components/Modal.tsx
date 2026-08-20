import { useEffect, useRef } from "react";

const FOCUSABLE_SELECTOR =
  'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Extracted from AppPicker (issue #22 → #10): focus trap, focus restore,
// Escape, and backdrop-click-to-dismiss, so a second dialog doesn't
// duplicate this a11y-critical behavior. Focuses the first focusable
// descendant on open — for a two-button confirm, put the safer action
// first in DOM order so it's what gets focused by default.
export function Modal({
  label,
  onClose,
  className,
  children,
}: {
  label: string;
  onClose: () => void;
  className?: string;
  children: React.ReactNode;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);

  // Move focus into the dialog on open, and hand it back to whatever opened
  // it on close. Reading activeElement here rather than using `autoFocus` is
  // what makes the restore possible at all: autoFocus fires during commit,
  // before effects run, so by this point the trigger would already have lost
  // focus and there'd be nothing to return to (issue #22).
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const first = dialogRef.current?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR);
    first?.focus();
    return () => previouslyFocused?.focus();
  }, []);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key !== "Tab") return;

    // This dialog declares aria-modal="true", which promises focus cannot
    // reach whatever is behind the overlay. Nothing enforces that promise on
    // its own — Tab would otherwise walk straight out of it (issue #22).
    const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
    if (!focusable || focusable.length === 0) return;

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  return (
    // Backdrop click-to-dismiss is a mouse-only affordance; Escape and
    // whatever close/cancel button the caller renders are the keyboard
    // equivalents, so this needs no key handler.
    <div className="modal-overlay" onClick={onClose}>
      <div
        className={className ? `modal ${className}` : "modal"}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        ref={dialogRef}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {children}
      </div>
    </div>
  );
}
