import { describe, expect, it } from "vitest";

import { formatToolUseHeader, summarizeToolCall } from "../src/coder-tool-summary.js";

describe("a tool call in one line", () => {
  it("turns an argument vector back into the command line it was", () => {
    // The case this exists for. Eight of these in a row was a screen of
    // punctuation with the answers pushed off the bottom.
    expect(
      summarizeToolCall(`{"args":["issue","view","212","-R","OpenAgentsInc/openagents.com"]}`),
    ).toBe("issue view 212 -R OpenAgentsInc/openagents.com");
  });

  it("formats tool headers in Claude Code style", () => {
    expect(
      formatToolUseHeader(
        "openagents",
        `{"args":["issue","view","212","-R","OpenAgentsInc/openagents.com"]}`,
      ),
    ).toBe("openagents(issue view 212 -R OpenAgentsInc/openagents.com)");
    expect(formatToolUseHeader("shell", `{"command":"git status"}`)).toBe("shell(git status)");
    expect(formatToolUseHeader("read", `{"file_path":"src/main.ts"}`)).toBe("read(src/main.ts)");
  });

  it("quotes an argument with a space, so it still reads as one", () => {
    expect(summarizeToolCall(`{"args":["issue","create","--title","two words"]}`)).toBe(
      `issue create --title "two words"`,
    );
  });

  it("shows a command as the command", () => {
    expect(summarizeToolCall(`{"command":"ls -la docs/"}`)).toBe("ls -la docs/");
  });

  it("prefers the subject over the tool's other settings", () => {
    // `timeout_seconds` is a knob; the command is what the reader is reading
    // for.
    expect(summarizeToolCall(`{"timeout_seconds":600,"command":"pnpm test"}`)).toBe("pnpm test");
  });

  it("names a skill, a path, and a pattern", () => {
    expect(summarizeToolCall(`{"name":"superdelegate"}`)).toBe("superdelegate");
    expect(summarizeToolCall(`{"path":"lib/a.ex"}`)).toBe("lib/a.ex");
    expect(summarizeToolCall(`{"pattern":"needle"}`)).toBe("needle");
  });

  it("drops the punctuation when nothing is named", () => {
    expect(summarizeToolCall(`{"count":3,"description":"audit the providers"}`)).toBe(
      "audit the providers",
    );
  });

  it("says nothing for a call with no arguments", () => {
    expect(summarizeToolCall("{}")).toBe("");
    expect(summarizeToolCall("")).toBe("");
  });

  it("keeps the raw text while the arguments are still arriving", () => {
    // A call streams in a fragment at a time, so half of it is not JSON yet.
    // The row shows what there is rather than going blank and back.
    expect(summarizeToolCall(`{"command":"ls -`)).toBe(`{"command":"ls -`);
  });
});
