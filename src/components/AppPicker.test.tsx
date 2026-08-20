import { describe, it, expect, beforeEach, vi } from "vitest";
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppPicker } from "./AppPicker";
import type { InstalledApp } from "../types";

vi.mock("../api");

import { mockedApi, resetApiMocks } from "../test/mockApi";

beforeEach(resetApiMocks);

const APPS: InstalledApp[] = [
  { name: "VS Code", path: "C:/Program Files/Microsoft VS Code/Code.exe" },
  { name: "Docker Desktop", path: "C:/Program Files/Docker/Docker/Docker Desktop.exe" },
  { name: "Postman", path: "C:/Users/me/AppData/Local/Postman/Postman.exe" },
];

describe("AppPicker", () => {
  it("shows a scanning state before the list resolves", () => {
    mockedApi.listInstalledApps.mockReturnValue(new Promise(() => {})); // never resolves

    render(<AppPicker onSelect={vi.fn()} onClose={vi.fn()} />);

    expect(screen.getByText(/scanning installed apps/i)).toBeInTheDocument();
  });

  it("lists installed apps once the scan resolves", async () => {
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={vi.fn()} onClose={vi.fn()} />);

    expect(await screen.findByText("VS Code")).toBeInTheDocument();
    expect(screen.getByText("Docker Desktop")).toBeInTheDocument();
    expect(screen.getByText("Postman")).toBeInTheDocument();
  });

  it("filters as you type", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    await screen.findByText("VS Code");

    await user.type(screen.getByPlaceholderText(/search installed apps/i), "post");

    expect(screen.getByText("Postman")).toBeInTheDocument();
    expect(screen.queryByText("VS Code")).not.toBeInTheDocument();
    expect(screen.queryByText("Docker Desktop")).not.toBeInTheDocument();
  });

  it("calls onSelect with the chosen app", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={onSelect} onClose={vi.fn()} />);

    await user.click(await screen.findByText("VS Code"));

    expect(onSelect).toHaveBeenCalledWith(APPS[0]);
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={vi.fn()} onClose={onClose} />);
    await screen.findByText("VS Code");

    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalled();
  });

  it("re-scans with refresh:true on Rescan", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    await screen.findByText("VS Code");

    await user.click(screen.getByRole("button", { name: /rescan/i }));

    expect(mockedApi.listInstalledApps).toHaveBeenLastCalledWith(true);
  });

  // Issue #22: the dialog declares aria-modal="true" but enforced neither
  // half of it — Tab walked out into the editor behind the overlay, and
  // closing dropped focus to <body> instead of back to the trigger.
  it("returns focus to whatever opened it once it closes (#22)", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Choose app…
          </button>
          {open && <AppPicker onSelect={vi.fn()} onClose={() => setOpen(false)} />}
        </>
      );
    }

    render(<Harness />);
    const trigger = screen.getByRole("button", { name: /choose app/i });

    await user.click(trigger);
    await screen.findByText("VS Code");
    expect(trigger).not.toHaveFocus();

    await user.click(screen.getByRole("button", { name: /^close$/i }));

    expect(trigger).toHaveFocus();
  });

  it("wraps Tab at both ends instead of letting focus leave the dialog (#22)", async () => {
    const user = userEvent.setup();
    mockedApi.listInstalledApps.mockResolvedValue(APPS);

    render(<AppPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    await screen.findByText("VS Code");

    // Postman is last in APPS, so its button is the last focusable in the
    // dialog; the search input is the first.
    const lastItem = screen.getByRole("button", { name: /postman/i });
    const search = screen.getByPlaceholderText(/search installed apps/i);

    lastItem.focus();
    await user.tab();
    expect(search).toHaveFocus();

    await user.tab({ shift: true });
    expect(lastItem).toHaveFocus();
  });
});
