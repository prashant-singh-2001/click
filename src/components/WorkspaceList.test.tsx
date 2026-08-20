import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceList } from "./WorkspaceList";
import { newWorkspace } from "../types";
import type { Workspace } from "../types";

vi.mock("../api");

import { mockedApi, resetApiMocks } from "../test/mockApi";

function makeWorkspace(overrides: Partial<Workspace> = {}): Workspace {
  return { ...newWorkspace(), ...overrides };
}

beforeEach(resetApiMocks);

const noop = () => {};

describe("WorkspaceList", () => {
  it("filters by workspace name", async () => {
    const user = userEvent.setup();
    const workspaces = [
      makeWorkspace({ name: "Backend Dev" }),
      makeWorkspace({ name: "Frontend Dev" }),
    ];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.type(screen.getByPlaceholderText(/search workspaces/i), "Backend");

    expect(screen.getByText("Backend Dev")).toBeInTheDocument();
    expect(screen.queryByText("Frontend Dev")).not.toBeInTheDocument();
  });

  it("filters by tag", async () => {
    const user = userEvent.setup();
    const workspaces = [
      makeWorkspace({ name: "Backend Dev", tags: ["work", "backend"] }),
      makeWorkspace({ name: "Side Project", tags: ["personal"] }),
    ];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.type(screen.getByPlaceholderText(/search workspaces/i), "backend");

    expect(screen.getByText("Backend Dev")).toBeInTheDocument();
    expect(screen.queryByText("Side Project")).not.toBeInTheDocument();
  });

  it("shows a no-match message when the search filters everything out", async () => {
    const user = userEvent.setup();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.type(screen.getByPlaceholderText(/search workspaces/i), "nonexistent");

    expect(screen.getByText(/no workspaces match/i)).toBeInTheDocument();
    expect(screen.queryByText("Backend Dev")).not.toBeInTheDocument();
  });

  it("shows the empty state instead of a search box when there are no workspaces at all", () => {
    render(<WorkspaceList workspaces={[]} onEdit={noop} onNew={noop} onChanged={noop} />);

    expect(screen.getByText(/no workspaces yet/i)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/search workspaces/i)).not.toBeInTheDocument();
  });

  it("renders tag chips on a workspace card", () => {
    const workspaces = [makeWorkspace({ name: "Backend Dev", tags: ["work", "backend"] })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    expect(screen.getByText("work")).toBeInTheDocument();
    expect(screen.getByText("backend")).toBeInTheDocument();
  });

  // Issue #10: this is the test that encodes the issue itself. Clicking
  // Delete must open a confirmation, not delete immediately.
  it("does not delete when Delete is clicked — it opens a confirmation instead", async () => {
    const user = userEvent.setup();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));

    expect(mockedApi.deleteWorkspace).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText(/delete "backend dev"\?/i)).toBeInTheDocument();
  });

  it("cancelling the confirmation deletes nothing and returns focus to Delete", async () => {
    const user = userEvent.setup();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    const deleteButton = screen.getByRole("button", { name: /^delete$/i });
    await user.click(deleteButton);
    await user.click(screen.getByRole("button", { name: /^cancel$/i }));

    expect(mockedApi.deleteWorkspace).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(deleteButton).toHaveFocus();
  });

  it("confirming deletes the workspace and reports the change", async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];
    mockedApi.deleteWorkspace.mockResolvedValue(undefined);

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={onChanged} />,
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));
    // Two "Delete" buttons exist once the dialog is open: the card's and the
    // confirm dialog's. Scope to the dialog to hit the right one.
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: /^delete$/i }),
    );

    expect(mockedApi.deleteWorkspace).toHaveBeenCalledWith(workspaces[0].id);
    expect(onChanged).toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows an error banner instead of closing when delete rejects", async () => {
    const user = userEvent.setup();
    mockedApi.deleteWorkspace.mockRejectedValue("disk error");
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: /^delete$/i }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("Delete failed: disk error");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  // The whole reason restore_workspace exists rather than reusing
  // saveWorkspace: it must restore the ORIGINAL index, not append. Deletes
  // from the middle of a 3-item list so a push-to-end regression would fail
  // this even though it would pass a 1-item version of the same test.
  it("Undo restores the workspace at its original index, not the end of the list", async () => {
    const user = userEvent.setup();
    const workspaces = [
      makeWorkspace({ name: "A" }),
      makeWorkspace({ name: "B" }),
      makeWorkspace({ name: "C" }),
    ];
    mockedApi.deleteWorkspace.mockResolvedValue(undefined);
    mockedApi.restoreWorkspace.mockResolvedValue(undefined);

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    const cards = screen.getAllByRole("listitem");
    await user.click(within(cards[1]).getByRole("button", { name: /^delete$/i }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: /^delete$/i }),
    );

    await user.click(screen.getByRole("button", { name: /^undo$/i }));

    expect(mockedApi.restoreWorkspace).toHaveBeenCalledWith(workspaces[1], 1);
  });

  it("dismissing the undo banner clears it without restoring", async () => {
    const user = userEvent.setup();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];
    mockedApi.deleteWorkspace.mockResolvedValue(undefined);

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onChanged={noop} />,
    );

    await user.click(screen.getByRole("button", { name: /^delete$/i }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: /^delete$/i }),
    );
    await user.click(screen.getByRole("button", { name: /dismiss undo/i }));

    expect(screen.queryByRole("button", { name: /^undo$/i })).not.toBeInTheDocument();
    expect(mockedApi.restoreWorkspace).not.toHaveBeenCalled();
  });
});
