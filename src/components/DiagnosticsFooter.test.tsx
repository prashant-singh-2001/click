import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DiagnosticsFooter } from "./DiagnosticsFooter";

vi.mock("../api");

import { mockedApi, resetApiMocks } from "../test/mockApi";

beforeEach(resetApiMocks);

describe("DiagnosticsFooter", () => {
  it("shows the log directory once it resolves", async () => {
    mockedApi.logDir.mockResolvedValue("C:\\Users\\test\\AppData\\Local\\com.launchpad.app\\logs");

    render(<DiagnosticsFooter />);

    expect(
      await screen.findByText("C:\\Users\\test\\AppData\\Local\\com.launchpad.app\\logs"),
    ).toBeInTheDocument();
  });

  it("opens the logs folder when clicked", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    mockedApi.openLogDir.mockResolvedValue(undefined);

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /open logs folder/i }));

    expect(mockedApi.openLogDir).toHaveBeenCalledTimes(1);
  });

  // Issue #25: no auto-update existed at all — this is the manual trigger
  // alongside the startup check and the tray's "Check for updates..." item.
  it("shows the up-to-date status after checking (#25)", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    mockedApi.checkForUpdates.mockResolvedValue({ kind: "upToDate", currentVersion: "0.2.4" });

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(await screen.findByText(/you're up to date \(v0\.2\.4\)/i)).toBeInTheDocument();
  });

  it("shows a declined message when the user dismisses the native prompt (#25)", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    mockedApi.checkForUpdates.mockResolvedValue({ kind: "declined", availableVersion: "0.3.0" });

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(await screen.findByText(/update to v0\.3\.0 declined/i)).toBeInTheDocument();
  });

  it("shows an installing message when the user confirms (#25)", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    mockedApi.checkForUpdates.mockResolvedValue({ kind: "installing", availableVersion: "0.3.0" });

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(await screen.findByText(/installing v0\.3\.0/i)).toBeInTheDocument();
  });

  it("shows an unavailable message when the check fails (#25)", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    mockedApi.checkForUpdates.mockRejectedValue("network error");

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(await screen.findByText(/update check unavailable: network error/i)).toBeInTheDocument();
  });

  it("disables the button while a check is in progress (#25)", async () => {
    const user = userEvent.setup();
    mockedApi.logDir.mockResolvedValue("C:\\logs");
    let resolveCheck!: (value: { kind: "upToDate"; currentVersion: string }) => void;
    mockedApi.checkForUpdates.mockReturnValue(
      new Promise((resolve) => {
        resolveCheck = resolve;
      }),
    );

    render(<DiagnosticsFooter />);
    await screen.findByText("C:\\logs");

    await user.click(screen.getByRole("button", { name: /check for updates/i }));

    const button = screen.getByRole("button", { name: /checking/i });
    expect(button).toBeDisabled();

    resolveCheck({ kind: "upToDate", currentVersion: "0.2.4" });
    await screen.findByText(/you're up to date/i);
    expect(screen.getByRole("button", { name: /check for updates/i })).not.toBeDisabled();
  });
});
