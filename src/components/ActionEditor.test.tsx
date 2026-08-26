import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionEditor, VALIDATE_DEBOUNCE_MS } from "./ActionEditor";
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

    expect(screen.getByPlaceholderText(/path to an app, shortcut, or document/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/arguments/i)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/example.com/i)).not.toBeInTheDocument();
  });

  // Issue #8: the args field used to be `action.args.join(" ")` /
  // `.split(" ")`, so a quoted argument containing a space couldn't be
  // expressed and ["a","b"] round-tripped identically to ["a b"].
  it("parses a quoted argument containing a space into one argument (#8)", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={onChange}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    await user.type(screen.getByLabelText("Arguments"), '--dir "C:/My Project"');

    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ args: ["--dir", "C:/My Project"] }),
    );
  });

  it("displays a spacey argument quoted, distinguishing it from two separate arguments (#8)", () => {
    render(
      <ActionEditor
        action={makeAppAction({ args: ["a b"] })}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(screen.getByLabelText("Arguments")).toHaveValue('"a b"');
  });

  it("previews each parsed argument (#8)", () => {
    render(
      <ActionEditor
        action={makeAppAction({ args: ["--dir", "C:/My Project"] })}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(screen.getByText(/parses to 2 arguments/i)).toBeInTheDocument();
    expect(screen.getByText("--dir")).toBeInTheDocument();
    expect(screen.getByText("C:/My Project")).toBeInTheDocument();
  });

  it("does not collapse a double space while typing, unlike the old join/split round-trip (#8)", async () => {
    const user = userEvent.setup();

    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    await user.type(screen.getByLabelText("Arguments"), "a  b");

    expect(screen.getByLabelText("Arguments")).toHaveValue("a  b");
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

  // Issue #11: `action` is a fresh object on every keystroke (every onChange
  // spreads a new one), so the validation effect used to fire one IPC call
  // per character with no ordering guard. These are the only tests in this
  // file using fake timers — scoped to this block so the other tests keep
  // running on real timers.
  describe("validation debounce (#11)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    function renderAt(path: string) {
      return (
        <ActionEditor
          action={makeAppAction({ path })}
          onChange={noop}
          onRemove={noop}
          onMoveUp={noop}
          onMoveDown={noop}
        />
      );
    }

    it("makes zero calls mid-burst, then exactly one after the debounce settles", async () => {
      const { rerender } = render(renderAt("C"));

      for (const path of ["C:", "C:/", "C:/a", "C:/ap", "C:/app"]) {
        rerender(renderAt(path));
      }

      expect(mockedApi.validateAction).not.toHaveBeenCalled();

      await act(() => vi.advanceTimersByTimeAsync(VALIDATE_DEBOUNCE_MS));

      expect(mockedApi.validateAction).toHaveBeenCalledTimes(1);
      expect(mockedApi.validateAction).toHaveBeenCalledWith(
        expect.objectContaining({ path: "C:/app" }),
      );
    });

    it("keeps a newer response over a slower older one that resolves later", async () => {
      let resolveOld!: (value: string | null) => void;
      let resolveNew!: (value: string | null) => void;
      mockedApi.validateAction
        .mockImplementationOnce(
          () =>
            new Promise((resolve) => {
              resolveOld = resolve;
            }),
        )
        .mockImplementationOnce(
          () =>
            new Promise((resolve) => {
              resolveNew = resolve;
            }),
        );

      const { rerender } = render(renderAt("C:/old.exe"));
      await act(() => vi.advanceTimersByTimeAsync(VALIDATE_DEBOUNCE_MS));

      rerender(renderAt("C:/new.exe"));
      await act(() => vi.advanceTimersByTimeAsync(VALIDATE_DEBOUNCE_MS));

      expect(mockedApi.validateAction).toHaveBeenCalledTimes(2);

      // Resolve the newer request first, then the older, slower one — the
      // older response must not overwrite the newer warning once it lands.
      await act(async () => {
        resolveNew("new warning");
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(screen.getByText(/new warning/i)).toBeInTheDocument();

      await act(async () => {
        resolveOld("old warning");
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(screen.getByText(/new warning/i)).toBeInTheDocument();
      expect(screen.queryByText(/old warning/i)).not.toBeInTheDocument();
    });
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

  // Issue #22: these are icon-only. `title` is only a last-resort accname
  // source — the glyph wins — so they announced as "up arrow" / "down arrow"
  // / "multiplication x" rather than as anything actionable.
  it("gives the icon-only row controls real accessible names (#22)", () => {
    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={noop}
        onRemove={noop}
        onMoveUp={noop}
        onMoveDown={noop}
      />,
    );

    expect(screen.getByRole("button", { name: "Move action up" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move action down" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove action" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Enabled" })).toBeInTheDocument();
  });

  it("keeps the icon-only buttons wired to their handlers (#22)", async () => {
    const user = userEvent.setup();
    const onMoveUp = vi.fn();
    const onMoveDown = vi.fn();
    const onRemove = vi.fn();

    render(
      <ActionEditor
        action={makeAppAction()}
        onChange={noop}
        onRemove={onRemove}
        onMoveUp={onMoveUp}
        onMoveDown={onMoveDown}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Move action up" }));
    await user.click(screen.getByRole("button", { name: "Move action down" }));
    await user.click(screen.getByRole("button", { name: "Remove action" }));

    expect(onMoveUp).toHaveBeenCalledTimes(1);
    expect(onMoveDown).toHaveBeenCalledTimes(1);
    expect(onRemove).toHaveBeenCalledTimes(1);
  });
});
