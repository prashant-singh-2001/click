import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceEditor } from "./WorkspaceEditor";
import { newWorkspace } from "../types";
import type { Workspace } from "../types";

vi.mock("../api");

import { mockedApi, resetApiMocks } from "../test/mockApi";

function makeWorkspace(overrides: Partial<Workspace> = {}): Workspace {
  return { ...newWorkspace(), ...overrides };
}

beforeEach(resetApiMocks);

describe("WorkspaceEditor", () => {
  // Issue #2 was closed as COMPLETED, but the underlying bug is still in
  // WorkspaceEditor.tsx: variable rows are keyed by the name being edited,
  // so every keystroke swaps the row's React key and remounts the input,
  // destroying focus. This test asserts the CORRECT behavior (focus stays)
  // and is expected to fail until that's actually fixed — it.fails() means
  // CI stays green today but turns red the moment someone "fixes" #2 without
  // updating this test, which is the whole point.
  it.fails("keeps focus in the variable-name input while renaming (#2)", async () => {
    const user = userEvent.setup();
    const workspace = makeWorkspace({ variables: { OLD: "value" } });
    render(<WorkspaceEditor workspace={workspace} onSaved={vi.fn()} onCancel={vi.fn()} />);

    const nameInput = screen.getByPlaceholderText("NAME");
    await user.click(nameInput);
    await user.type(nameInput, "X");

    expect(document.activeElement).toBe(nameInput);
  });

  // Issue #9: handleLaunch calls api.launchWorkspace(draft.id) — only the id,
  // never the current draft's content — so an edit made but not yet saved is
  // silently discarded and the backend launches whatever was last persisted.
  // The eventual fix (new Rust command, out of scope here) will need to send
  // the actual draft rather than just its id; this test encodes that as the
  // desired contract and is expected to fail against the current bare-id call.
  it.fails("Launch reflects unsaved draft edits, not the last-saved record (#9)", async () => {
    const user = userEvent.setup();
    const workspace = makeWorkspace({
      actions: [
        {
          type: "url",
          id: "action-1",
          label: "Local",
          url: "http://localhost:3000",
          enabled: true,
          delayAfterMs: null,
        },
      ],
    });
    mockedApi.launchWorkspace.mockResolvedValue({ outcomes: [] });

    render(<WorkspaceEditor workspace={workspace} onSaved={vi.fn()} onCancel={vi.fn()} />);

    const urlInput = screen.getByDisplayValue("http://localhost:3000");
    await user.clear(urlInput);
    await user.type(urlInput, "http://localhost:9999");

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(mockedApi.launchWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        actions: expect.arrayContaining([
          expect.objectContaining({ url: "http://localhost:9999" }),
        ]),
      }),
    );
  });

  it("shows a Save failed banner when saving rejects", async () => {
    const user = userEvent.setup();
    mockedApi.saveWorkspace.mockRejectedValue("disk error");

    render(<WorkspaceEditor workspace={makeWorkspace()} onSaved={vi.fn()} onCancel={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Save failed: disk error");
  });

  it("shows a Launch failed banner when launching rejects", async () => {
    const user = userEvent.setup();
    mockedApi.launchWorkspace.mockRejectedValue("workspace abc123 not found");

    render(<WorkspaceEditor workspace={makeWorkspace()} onSaved={vi.fn()} onCancel={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Launch failed: workspace abc123 not found",
    );
  });
});
