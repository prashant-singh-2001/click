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
});
