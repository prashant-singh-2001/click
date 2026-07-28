import { describe, it, expect, beforeEach, vi } from "vitest";
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
});
