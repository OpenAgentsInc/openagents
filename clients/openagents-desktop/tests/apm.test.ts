import { describe, expect, test } from "bun:test"

import { calculateApmStats, formatApm } from "../src/shared/apm.js"
import type { CodingCodexSession } from "../src/shared/coding-status.js"

const session = (
  messages: CodingCodexSession["messages"],
): CodingCodexSession => ({
  accountRef: "codex",
  active: true,
  assignmentRef: "assignment.test",
  closeout: {
    blockerRefs: [],
    closeoutRef: null,
    status: null,
  },
  cwd: "/tmp/work",
  elapsed: null,
  issueRef: "7609",
  lastEvent: {
    ageSeconds: null,
    name: null,
    timestamp: null,
  },
  leaseRef: null,
  pullRequestRef: null,
  messageCount: messages.length,
  messages,
  modifiedAt: "2026-06-29T12:00:00.000Z",
  path: "/tmp/session.jsonl",
  pid: 123,
  sessionId: "session.test",
  source: "exec",
  status: "active",
  title: "Implement APM",
})

describe("desktop APM calculation", () => {
  test("counts user, assistant, and tool-call messages as actions", () => {
    const stats = calculateApmStats(
      [
        session([
          {
            detail: null,
            kind: "message",
            role: "user",
            status: "info",
            text: "Build it",
            timestamp: "2026-06-29T11:01:00.000Z",
            title: "user",
          },
          {
            detail: "call-1",
            kind: "tool call",
            role: "tool",
            status: "running",
            text: "{\"cmd\":\"git status\"}",
            timestamp: "2026-06-29T11:02:00.000Z",
            title: "shell",
          },
          {
            detail: "call-1",
            kind: "tool output",
            role: "tool",
            status: "ok",
            text: "clean",
            timestamp: "2026-06-29T11:02:01.000Z",
            title: "tool result",
          },
          {
            detail: null,
            kind: "message",
            role: "assistant",
            status: "info",
            text: "Done",
            timestamp: "2026-06-29T11:03:00.000Z",
            title: "assistant",
          },
        ]),
      ],
      Date.parse("2026-06-29T12:00:00.000Z"),
    )

    expect(stats.actionCount).toBe(3)
    expect(formatApm(stats.currentApm)).toBe("1.5")
    expect(formatApm(stats.recentApm)).toBe("0.1")
    expect(stats.series.some(point => point.actionCount > 0)).toBe(true)
  })

  test("returns an empty graph model when no timestamped actions exist", () => {
    const stats = calculateApmStats(
      [
        session([
          {
            detail: null,
            kind: "reasoning",
            role: "reasoning",
            status: "info",
            text: "thinking",
            timestamp: null,
            title: "reasoning",
          },
        ]),
      ],
      Date.parse("2026-06-29T12:00:00.000Z"),
    )

    expect(stats.actionCount).toBe(0)
    expect(stats.currentApm).toBe(0)
    expect(stats.series).toHaveLength(24)
    expect(stats.series.every(point => point.actionCount === 0)).toBe(true)
  })
})
