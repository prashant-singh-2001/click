import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HotkeyInput } from "./HotkeyInput";

vi.mock("../api");

import { resetApiMocks } from "../test/mockApi";

beforeEach(resetApiMocks);

describe("HotkeyInput", () => {
  it("captures Ctrl+Alt+1 into an accelerator string", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<HotkeyInput value={null} status={null} onCommit={onCommit} />);

    await user.click(screen.getByRole("button", { name: /click to set a hotkey/i }));
    await user.keyboard("{Control>}{Alt>}1{/Alt}{/Control}");

    expect(onCommit).toHaveBeenCalledWith("Ctrl+Alt+1");
  });

  it("cancels capture on Escape without committing", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<HotkeyInput value="Ctrl+Alt+2" status={{ kind: "registered" }} onCommit={onCommit} />);

    await user.click(screen.getByRole("button", { name: "Ctrl+Alt+2" }));
    expect(screen.getByRole("button", { name: /press a key combo/i })).toBeInTheDocument();

    await user.keyboard("{Escape}");

    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Ctrl+Alt+2" })).toBeInTheDocument();
  });

  it("clears via Backspace while capturing", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<HotkeyInput value="Ctrl+Alt+2" status={{ kind: "registered" }} onCommit={onCommit} />);

    await user.click(screen.getByRole("button", { name: "Ctrl+Alt+2" }));
    await user.keyboard("{Backspace}");

    expect(onCommit).toHaveBeenCalledWith(null);
  });

  it("rejects a bare letter with no modifier and shows the hint", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    render(<HotkeyInput value={null} status={null} onCommit={onCommit} />);

    await user.click(screen.getByRole("button", { name: /click to set a hotkey/i }));
    await user.keyboard("a");

    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByText(/add a modifier/i)).toBeInTheDocument();
  });

  // Issue #22 — a WCAG 2.1.2 keyboard trap. handleKeyDown used to
  // preventDefault every key while capturing, Tab included, so once a
  // keyboard user entered capture the only ways out were Escape (undocumented
  // at the time) or a mouse click. userEvent honours defaultPrevented, so
  // this genuinely fails against the old handler.
  it("lets Tab move focus out of the capture button rather than trapping it (#22)", async () => {
    const user = userEvent.setup();
    render(
      <>
        <HotkeyInput value={null} status={null} onCommit={vi.fn()} />
        <button type="button">after</button>
      </>,
    );

    await user.click(screen.getByRole("button", { name: /click to set a hotkey/i }));
    expect(screen.getByRole("button", { name: /press a key combo/i })).toBeInTheDocument();

    await user.tab();

    expect(screen.getByRole("button", { name: "after" })).toHaveFocus();
  });

  it("surfaces the Escape affordance and links it to the button (#22)", async () => {
    const user = userEvent.setup();
    render(<HotkeyInput value={null} status={null} onCommit={vi.fn()} />);

    expect(screen.queryByText(/press esc to cancel/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /click to set a hotkey/i }));

    expect(screen.getByText(/press esc to cancel/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /press a key combo/i }),
    ).toHaveAccessibleDescription(/press esc to cancel/i);
  });
});
