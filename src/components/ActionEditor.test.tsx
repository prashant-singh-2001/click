import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ActionEditor } from "./ActionEditor";
import type { AppAction, UrlAction } from "../types";

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
});
