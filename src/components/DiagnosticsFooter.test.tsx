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
});
