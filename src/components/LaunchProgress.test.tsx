import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { LaunchProgress } from "./LaunchProgress";
import type { LaunchReport } from "../types";

describe("LaunchProgress", () => {
  it("renders one row per outcome with the matching status class", () => {
    const report: LaunchReport = {
      outcomes: [
        { actionId: "1", label: "VS Code", status: "started", message: null },
        { actionId: "2", label: "Docker", status: "failed", message: "not found" },
        { actionId: "3", label: "Disabled thing", status: "skipped", message: null },
      ],
    };

    render(<LaunchProgress report={report} />);

    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveClass("outcome-started");
    expect(items[0]).toHaveTextContent("VS Code");
    expect(items[1]).toHaveClass("outcome-failed");
    expect(items[1]).toHaveTextContent("not found");
    expect(items[2]).toHaveClass("outcome-skipped");
  });
});
