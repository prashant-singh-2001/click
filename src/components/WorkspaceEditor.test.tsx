import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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

  // Issue #9: handleLaunch used to call api.launchWorkspace(draft.id) — only
  // the id, so an edit made but not yet saved was silently discarded and the
  // backend launched whatever was last persisted. It now calls
  // api.launchDraft(draft), sending the actual edited content. This was
  // it.fails against the old bare-id call; per CLAUDE.md, fixing the bug
  // flips the test rather than deleting it.
  it("Launch reflects unsaved draft edits, not the last-saved record (#9)", async () => {
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
    mockedApi.launchDraft.mockResolvedValue({ outcomes: [] });
    await renderExisting(workspace);

    const urlInput = screen.getByDisplayValue("http://localhost:3000");
    await user.clear(urlInput);
    await user.type(urlInput, "http://localhost:9999");

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(mockedApi.launchDraft).toHaveBeenCalledWith(
      expect.objectContaining({
        actions: [expect.objectContaining({ url: "http://localhost:9999" })],
      }),
    );
  });

  // Issue #9: a workspace that has never been saved has no persisted record
  // to look up by id, so the old launchWorkspace(id) call failed with
  // "workspace not found" for a brand-new workspace. launchDraft sends the
  // content directly, so there's nothing to look up.
  it("launches a brand-new, never-saved workspace (#9)", async () => {
    const user = userEvent.setup();
    mockedApi.launchDraft.mockResolvedValue({ outcomes: [] });

    render(<WorkspaceEditor workspaceId={null} onSaved={vi.fn()} onCancel={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(await screen.findByText(/launch result/i)).toBeInTheDocument();
    expect(mockedApi.launchDraft).toHaveBeenCalledWith(
      expect.objectContaining({ name: "New workspace" }),
    );
  });

  // Issue #9: launching the draft instead of the saved record means a
  // successful launch no longer implies the edits are on disk — this note
  // is what keeps that honest.
  it("shows an unsaved-edits hint after launching a modified draft (#9)", async () => {
    const user = userEvent.setup();
    mockedApi.launchDraft.mockResolvedValue({ outcomes: [] });
    await renderExisting(makeWorkspace({ name: "Original" }));

    const nameInput = screen.getByPlaceholderText("Workspace name");
    await user.clear(nameInput);
    await user.type(nameInput, "Edited");
    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(await screen.findByText(/ran your unsaved edits/i)).toBeInTheDocument();
  });

  it("shows no unsaved-edits hint launching straight after load, unedited (#9)", async () => {
    const user = userEvent.setup();
    mockedApi.launchDraft.mockResolvedValue({ outcomes: [] });
    await renderExisting(makeWorkspace());

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    await screen.findByText(/launch result/i);
    expect(screen.queryByText(/ran your unsaved edits/i)).not.toBeInTheDocument();
  });

  it("shows no unsaved-edits hint launching right after Save (#9)", async () => {
    const user = userEvent.setup();
    mockedApi.launchDraft.mockResolvedValue({ outcomes: [] });
    await renderExisting(makeWorkspace({ name: "Original" }));

    const nameInput = screen.getByPlaceholderText("Workspace name");
    await user.clear(nameInput);
    await user.type(nameInput, "Edited");
    // onSaved is a vi.fn() here (via renderExisting), so the editor stays
    // mounted and interactable after Save instead of navigating away.
    await user.click(screen.getByRole("button", { name: /^save$/i }));
    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    await screen.findByText(/launch result/i);
    expect(screen.queryByText(/ran your unsaved edits/i)).not.toBeInTheDocument();
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
    mockedApi.launchDraft.mockRejectedValue("workspace abc123 not found");
    await renderExisting(makeWorkspace());

    await user.click(screen.getByRole("button", { name: /^launch$/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Launch failed: workspace abc123 not found",
    );
  });

  // Issue #20: Close used to call onCancel unconditionally, so an edit made
  // but not yet saved was silently discarded with no warning.
  it("closes immediately when there are no unsaved changes (#20)", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const workspace = makeWorkspace();
    mockedApi.getWorkspace.mockResolvedValue(workspace);
    render(<WorkspaceEditor workspaceId={workspace.id} onSaved={vi.fn()} onCancel={onCancel} />);
    await screen.findByPlaceholderText("Workspace name");

    await user.click(screen.getByRole("button", { name: /^close$/i }));

    expect(onCancel).toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows a confirm dialog before discarding unsaved changes, and Cancel keeps editing (#20)", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const workspace = makeWorkspace({ name: "Original" });
    mockedApi.getWorkspace.mockResolvedValue(workspace);
    render(<WorkspaceEditor workspaceId={workspace.id} onSaved={vi.fn()} onCancel={onCancel} />);
    await screen.findByPlaceholderText("Workspace name");

    await user.type(screen.getByPlaceholderText("Workspace name"), " edited");
    await user.click(screen.getByRole("button", { name: /^close$/i }));

    expect(await screen.findByRole("dialog")).toHaveTextContent(/unsaved changes/i);
    expect(onCancel).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /^cancel$/i }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("discards changes and closes when confirmed (#20)", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const workspace = makeWorkspace({ name: "Original" });
    mockedApi.getWorkspace.mockResolvedValue(workspace);
    render(<WorkspaceEditor workspaceId={workspace.id} onSaved={vi.fn()} onCancel={onCancel} />);
    await screen.findByPlaceholderText("Workspace name");

    await user.type(screen.getByPlaceholderText("Workspace name"), " edited");
    await user.click(screen.getByRole("button", { name: /^close$/i }));
    await user.click(await screen.findByRole("button", { name: /discard changes/i }));

    expect(onCancel).toHaveBeenCalled();
  });

  it("does not prompt when closing a freshly-opened, untouched new workspace (#20)", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<WorkspaceEditor workspaceId={null} onSaved={vi.fn()} onCancel={onCancel} />);

    await user.click(screen.getByRole("button", { name: /^close$/i }));

    expect(onCancel).toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("does not prompt right after a successful save (#20)", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const onSaved = vi.fn();
    const workspace = makeWorkspace({ name: "Original" });
    mockedApi.getWorkspace.mockResolvedValue(workspace);
    // A prior test's mockRejectedValue on saveWorkspace would otherwise
    // survive resetApiMocks's vi.clearAllMocks(), which clears call history
    // but not implementations.
    mockedApi.saveWorkspace.mockResolvedValue(undefined);
    render(<WorkspaceEditor workspaceId={workspace.id} onSaved={onSaved} onCancel={onCancel} />);
    await screen.findByPlaceholderText("Workspace name");

    await user.type(screen.getByPlaceholderText("Workspace name"), " edited");
    await user.click(screen.getByRole("button", { name: /^save$/i }));
    // handleSave updates the dirty-tracking ref after its internal await, so
    // wait for the save to fully settle before clicking Close — otherwise
    // the click can race the save's continuation.
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: /^close$/i }));

    expect(onCancel).toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  // Issue #21: neither the workspace name nor an action label had a guard —
  // clearing either to blank and saving persisted it that way, showing up as
  // an empty row in the list and tray menu.
  it("disables Save and Create desktop shortcut when the name is blank, with a hint (#21)", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace({ name: "Original" }));

    await user.clear(screen.getByPlaceholderText("Workspace name"));

    expect(screen.getByRole("button", { name: /^save$/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /create desktop shortcut/i })).toBeDisabled();
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
  });

  it("re-enables Save once a name is typed back in (#21)", async () => {
    const user = userEvent.setup();
    await renderExisting(makeWorkspace({ name: "Original" }));

    const nameInput = screen.getByPlaceholderText("Workspace name");
    await user.clear(nameInput);
    await user.type(nameInput, "New name");

    expect(screen.getByRole("button", { name: /^save$/i })).not.toBeDisabled();
    expect(screen.queryByText(/name is required/i)).not.toBeInTheDocument();
  });

  it("defaults a blank action label to the type's default label on save (#21)", async () => {
    const user = userEvent.setup();
    await renderExisting(
      makeWorkspace({
        actions: [
          {
            type: "app",
            id: "action-1",
            label: "   ",
            path: "C:/app.exe",
            args: [],
            cwd: null,
            enabled: true,
            delayAfterMs: null,
          },
        ],
      }),
    );

    await user.click(screen.getByRole("button", { name: /^save$/i }));

    expect(mockedApi.saveWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        actions: [expect.objectContaining({ label: "New app" })],
      }),
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
