import { describe, it, expect, vi } from "vitest";
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Modal } from "./Modal";

function Content() {
  return (
    <>
      <button type="button">First</button>
      <button type="button">Last</button>
    </>
  );
}

describe("Modal", () => {
  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(
      <Modal label="Test dialog" onClose={onClose}>
        <Content />
      </Modal>,
    );

    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalled();
  });

  it("closes on a backdrop click", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(
      <Modal label="Test dialog" onClose={onClose}>
        <Content />
      </Modal>,
    );

    await user.click(screen.getByRole("dialog").parentElement!);

    expect(onClose).toHaveBeenCalled();
  });

  it("does not close when clicking inside the dialog", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    render(
      <Modal label="Test dialog" onClose={onClose}>
        <Content />
      </Modal>,
    );

    await user.click(screen.getByRole("button", { name: "First" }));

    expect(onClose).not.toHaveBeenCalled();
  });

  it("wraps Tab at both ends", async () => {
    const user = userEvent.setup();

    render(
      <Modal label="Test dialog" onClose={vi.fn()}>
        <Content />
      </Modal>,
    );

    const first = screen.getByRole("button", { name: "First" });
    const last = screen.getByRole("button", { name: "Last" });

    last.focus();
    await user.tab();
    expect(first).toHaveFocus();

    await user.tab({ shift: true });
    expect(last).toHaveFocus();
  });

  it("focuses the first focusable descendant on open and restores focus on close", async () => {
    const user = userEvent.setup();

    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Open
          </button>
          {open && (
            <Modal label="Test dialog" onClose={() => setOpen(false)}>
              <Content />
            </Modal>
          )}
        </>
      );
    }

    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Open" });

    await user.click(trigger);
    expect(screen.getByRole("button", { name: "First" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(trigger).toHaveFocus();
  });
});
