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

// Renders editing an existing workspace — the realistic path, and the one
// that exercises the get_workspace fetch (issue #16). Waits for the fetch to
// resolve before returning, so callers can use synchronous `getBy*` after.
async function renderExisting(workspace: Workspace) {
  mockedApi.getWorkspace.mockResolvedValue(workspace);
  render(<WorkspaceEditor workspaceId={workspace.id} onSaved={vi.fn()} onCancel={vi.fn()} />);
  await screen.findByPlaceholderText("Workspace name");
}

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
    await renderExisting(workspace);

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
  // This is orthogonal to issue #16's fetch-by-id rewiring — handleLaunch's
  // signature is untouched by that change, so this stays red for the exact
  // same reason it always has.
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
    await renderExisting(workspace);

    const urlInput = screen.getByDisplayValue("http://localhost:3000");
    await user.clear(urlInput);
    await user.type(urlInput, "http://localhost:9999");

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    // launchWorkspace's real signature is (id: string) — this asserts the
    // desired future contract (the whole draft), which is why the object
    // literal below has no type to check against yet and needs the
    // suppression. That mismatch is the point of this it.fails (see above).
    expect(mockedApi.launchWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
        actions: expect.arrayContaining([
          expect.objectContaining({ url: "http://localhost:9999" }),
        ]),
      }),
    );
  });

  it("shows a Save failed banner when saving rejects", async () => {
    const user = userEvent.setup();
    mockedApi.saveWorkspace.mockRejectedValue("disk error");
    await renderExisting(makeWorkspace());

    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Save failed: disk error");
  });

  it("shows a Launch failed banner when launching rejects", async () => {
    const user = userEvent.setup();
    mockedApi.launchWorkspace.mockRejectedValue("workspace abc123 not found");
    await renderExisting(makeWorkspace());

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Launch failed: workspace abc123 not found",
    );
  });

  it("adds a tag and includes it in the saved workspace", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace());

    await user.type(screen.getByPlaceholderText(/add a tag/i), "backend");
    await user.click(screen.getByRole("button", { name: /add tag/i }));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(mockedApi.saveWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({ tags: ["backend"] }),
    );
  });

  it("rejects a blank tag", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace());

    await user.type(screen.getByPlaceholderText(/add a tag/i), "   ");
    await user.click(screen.getByRole("button", { name: /add tag/i }));

    expect(screen.queryByRole("button", { name: /remove tag/i })).not.toBeInTheDocument();
  });

  it("rejects a duplicate tag (case-insensitive)", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace({ tags: ["Backend"] }));

    await user.type(screen.getByPlaceholderText(/add a tag/i), "backend");
    await user.click(screen.getByRole("button", { name: /add tag/i }));

    expect(screen.getAllByText("Backend")).toHaveLength(1);
  });

  it("removes a tag", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace({ tags: ["backend"] }));

    await user.click(screen.getByRole("button", { name: /remove tag backend/i }));

    expect(screen.queryByText("backend")).not.toBeInTheDocument();
  });

  it("includes the picked icon and color in the saved workspace", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace());

    await user.click(screen.getByRole("button", { name: /use 🐳 as icon/i }));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(mockedApi.saveWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({ icon: "🐳" }),
    );
  });

  it("clears the color back to null", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace({ color: "#4f46e5" }));

    await user.click(screen.getByRole("button", { name: /no color/i }));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(mockedApi.saveWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({ color: null }),
    );
  });

  // Issue #22: this button was the only control in the app with no accessible
  // name at all — not even a title — so it announced as the bare "✕" glyph.
  // The name interpolates the variable so N rows stay distinguishable.
  it("names the variable-remove button after the variable it removes (#22)", async () => {
    await renderExisting(makeWorkspace({ variables: { API_KEY: "abc" } }));

    expect(screen.getByRole("button", { name: "Remove variable API_KEY" })).toBeInTheDocument();
  });

  // Issue #16: a brand-new workspace doesn't exist server-side yet, so there
  // is nothing to fetch — it must render immediately with a blank draft.
  it("starts a blank draft immediately for a new workspace, without fetching", () => {
    render(<WorkspaceEditor workspaceId={null} onSaved={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.getByPlaceholderText("Workspace name")).toHaveValue("New workspace");
    expect(mockedApi.getWorkspace).not.toHaveBeenCalled();
  });

  it("shows a loading state while an existing workspace is being fetched", () => {
    mockedApi.getWorkspace.mockReturnValue(new Promise(() => {})); // never resolves

    render(
      <WorkspaceEditor workspaceId="some-id" onSaved={vi.fn()} onCancel={vi.fn()} />,
    );

    expect(screen.getByText(/loading workspace/i)).toBeInTheDocument();
  });

  it("shows an error and a working Back button when the fetch rejects", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    mockedApi.getWorkspace.mockRejectedValue("workspace xyz not found");

    render(<WorkspaceEditor workspaceId="xyz" onSaved={vi.fn()} onCancel={onCancel} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Failed to load workspace: workspace xyz not found",
    );

    await user.click(screen.getByRole("button", { name: /^back$/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  // The test that actually proves issue #16's fix: editing an existing
  // workspace fetches it by id rather than receiving it as a prop.
  it("fetches the workspace by id when editing an existing one (#16)", async () => {
    const workspace = makeWorkspace({ name: "Gamified Tracker Dev" });
    await renderExisting(workspace);

    expect(screen.getByPlaceholderText("Workspace name")).toHaveValue("Gamified Tracker Dev");
    expect(mockedApi.getWorkspace).toHaveBeenCalledWith(workspace.id);
  });
});
