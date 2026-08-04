import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceList } from "./WorkspaceList";
import { newWorkspace } from "../types";
import type { Workspace } from "../types";

vi.mock("../api");

import { resetApiMocks } from "../test/mockApi";

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
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onDeleted={noop} />,
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
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onDeleted={noop} />,
    );

    await user.type(screen.getByPlaceholderText(/search workspaces/i), "backend");

    expect(screen.getByText("Backend Dev")).toBeInTheDocument();
    expect(screen.queryByText("Side Project")).not.toBeInTheDocument();
  });

  it("shows a no-match message when the search filters everything out", async () => {
    const user = userEvent.setup();
    const workspaces = [makeWorkspace({ name: "Backend Dev" })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onDeleted={noop} />,
    );

    await user.type(screen.getByPlaceholderText(/search workspaces/i), "nonexistent");

    expect(screen.getByText(/no workspaces match/i)).toBeInTheDocument();
    expect(screen.queryByText("Backend Dev")).not.toBeInTheDocument();
  });

  it("shows the empty state instead of a search box when there are no workspaces at all", () => {
    render(<WorkspaceList workspaces={[]} onEdit={noop} onNew={noop} onDeleted={noop} />);

    expect(screen.getByText(/no workspaces yet/i)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/search workspaces/i)).not.toBeInTheDocument();
  });

  it("renders tag chips on a workspace card", () => {
    const workspaces = [makeWorkspace({ name: "Backend Dev", tags: ["work", "backend"] })];

    render(
      <WorkspaceList workspaces={workspaces} onEdit={noop} onNew={noop} onDeleted={noop} />,
    );

    expect(screen.getByText("work")).toBeInTheDocument();
    expect(screen.getByText("backend")).toBeInTheDocument();
  });
});
