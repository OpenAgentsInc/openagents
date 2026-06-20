import { describe, expect, test } from "bun:test"

import { summarizeAgentActivity } from "./agent-activity-summary.js"

describe("agent activity summary", () => {
  test("returns a stable empty summary for no events", () => {
    expect(summarizeAgentActivity([])).toEqual({
      toolCalls: 0,
      fileEdits: 0,
      lastAction: "",
      headline: "no activity yet",
    })
  })

  test("counts Claude-style tool calls and file edits", () => {
    expect(summarizeAgentActivity([
      { phase: "agent_message", messageText: "agent: checking the package" },
      { phase: "tool_use", messageText: "Bash: bun test packages/example.test.ts" },
      { phase: "tool_result", messageText: "result: ok" },
      { phase: "tool_use", messageText: "Edit packages/example.ts" },
    ])).toEqual({
      toolCalls: 2,
      fileEdits: 1,
      lastAction: "Edit packages/example.ts",
      headline: "ran 2 commands, edited 1 file",
    })
  })

  test("counts Codex patch file markers when available", () => {
    expect(summarizeAgentActivity([
      {
        phase: "tool_use",
        messageText: "apply_patch: *** Begin Patch *** Update File: src/a.ts *** Add File: src/b.ts",
      },
    ])).toEqual({
      toolCalls: 1,
      fileEdits: 2,
      lastAction: "apply_patch: *** Begin Patch *** Update File: src/a.ts *** Add File: src/b.ts",
      headline: "ran 1 command, edited 2 files",
    })
  })

  test("counts Codex composer command and file-change summaries", () => {
    expect(summarizeAgentActivity([
      { phase: "composer_event", messageText: "completed: bun test exit 0" },
      { phase: "composer_event", messageText: "completed: update src/index.ts, add src/index.test.ts" },
    ])).toEqual({
      toolCalls: 2,
      fileEdits: 2,
      lastAction: "completed: update src/index.ts, add src/index.test.ts",
      headline: "ran 2 commands, edited 2 files",
    })
  })

  test("falls back to a phase for the last action when messages are empty", () => {
    expect(summarizeAgentActivity([
      { phase: "started", messageText: "" },
      { phase: "tool_use", messageText: "" },
    ])).toEqual({
      toolCalls: 1,
      fileEdits: 0,
      lastAction: "tool_use",
      headline: "ran 1 command",
    })
  })

  test("defensively skips malformed rows at runtime", () => {
    const events = [
      null,
      "bad",
      { phase: 123, messageText: ["nope"] },
      { phase: "tool_use", messageText: "Write src/valid.ts" },
    ] as unknown as { phase: string; messageText: string }[]

    expect(summarizeAgentActivity(events)).toEqual({
      toolCalls: 1,
      fileEdits: 1,
      lastAction: "Write src/valid.ts",
      headline: "ran 1 command, edited 1 file",
    })
  })

  test("does not count chat self-reports as observed tool activity", () => {
    expect(summarizeAgentActivity([
      {
        phase: "agent_message",
        messageText: "agent: ran 4 commands and edited 2 files",
      },
    ])).toEqual({
      toolCalls: 0,
      fileEdits: 0,
      lastAction: "agent: ran 4 commands and edited 2 files",
      headline: "no activity yet",
    })
  })

  test("normalizes multiline messages into one-line last actions", () => {
    expect(summarizeAgentActivity([
      { phase: "agent_message", messageText: "agent:\n  finished\tverification" },
    ])).toEqual({
      toolCalls: 0,
      fileEdits: 0,
      lastAction: "agent: finished verification",
      headline: "no activity yet",
    })
  })
})
