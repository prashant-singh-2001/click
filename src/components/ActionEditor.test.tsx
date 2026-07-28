import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionEditor } from "./ActionEditor";
import type { AppAction, InstalledApp, UrlAction } from "../types";

vi.mock("../api");

import { mockedApi, resetApiMocks } from "../test/mockApi";

beforeEach(resetApiMocks);

function makeAppAction(overrides: Partial<AppAction> = {}): AppAction {
  return {
    type: "app",
    id: "app-1",
    label: "New app",
    path: "",
    args: [],
    cwd: null,
    enabled: true,
    delayAfterMs: null,
    ...overrides,
  };
}

function makeUrlAction(overrides: Partial<UrlAction> = {}): UrlAction {
  return {
    type: "url",
    id: "url-1",
    label: "New URL",
    url: "",
    enabled: true,
    delayAfterMs: null,
    ...overrides,
  };
}

const noop = () => {};

describe("ActionEditor", () => {
  it("shows path and args fields for an app action", () => {
    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(screen.getByPlaceholderText(/executable path/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/arguments/i)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/example.com/i)).not.toBeInTheDocument();
  });

  it("shows the URL field for a url action", () => {
    render(
      <ActionEditor
        action={makeUrlAction()}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(screen.getByPlaceholderText(/example.com/i)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/executable path/i)).not.toBeInTheDocument();
  });

  it("surfaces the validation warning returned by the backend", async () => {
    mockedApi.validateAction.mockResolvedValue("path does not exist: C:/missing.exe");

    render(
      <ActionEditor
        action={makeAppAction({ path: "C:/missing.exe" })}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(
      await screen.findByText(/path does not exist: C:\/missing\.exe/i),
    ).toBeInTheDocument();
  });

  const PICKED_APP: InstalledApp = {
    name: "VS Code",
    path: "C:/Program Files/Microsoft VS Code/Code.exe",
  };

  it("fills in path and label from the picker when the label is still the default", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue([PICKED_APP]);
    const onChange = vi.fn();

    render(
      <ActionEditor
        action={makeAppAction({ label: "New app" })}
        onChange={onChange}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: /choose app/i }));
    await user.click(await screen.findByText("VS Code"));

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ path: PICKED_APP.path, label: "VS Code" }),
    );
  });

  it("keeps a customized label when picking an app", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue([PICKED_APP]);
    const onChange = vi.fn();

    render(
      <ActionEditor
        action={makeAppAction({ label: "My editor" })}
        onChange={onChange}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: /choose app/i }));
    await user.click(await screen.findByText("VS Code"));

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ path: PICKED_APP.path, label: "My editor" }),
    );
  });

  it("closes the picker after selecting an app", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue([PICKED_APP]);

    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={vi.fn()}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: /choose app/i }));
    await user.click(await screen.findByText("VS Code"));

    expect(screen.queryByPlaceholderText(/search installed apps/i)).not.toBeInTheDocument();
  });
});
