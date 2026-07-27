import { vi } from "vitest";
import { api } from "../api";

// Requires the importing test file to have already called `vi.mock("../api")`
// — Vitest's mocking applies per test-file module graph, so this picks up
// the same mocked instance regardless of which file does the importing.
export const mockedApi = vi.mocked(api);

// Default resolved values for calls components fire on mount/effect, so a
// test that doesn't care about hotkeys/validation doesn't have to stub them
// individually to avoid "cannot read properties of undefined (reading 'then')".
export function resetApiMocks() {
  vi.clearAllMocks();
  mockedApi.hotkeyStatus.mockResolvedValue({ kind: "unset" });
  mockedApi.probeHotkey.mockResolvedValue({ kind: "unset" });
  mockedApi.suspendHotkeys.mockResolvedValue(undefined);
  mockedApi.resumeHotkeys.mockResolvedValue(undefined);
  mockedApi.validateAction.mockResolvedValue(null);
}
