import { describe, expect, test } from "bun:test"

import { exportSessionMarkdown } from "./session-markdown-export.js"

describe("session markdown export", () => {
  test("renders a custom title and each event as a timeline bullet", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.alpha",
      title: "Forum intake session",
      events: [
        {
          phase: "started",
          messageText: "opened the workspace",
          observedAt: "2026-06-13T13:00:00.000Z",
        },
        {
          phase: "verification",
          messageText: "bun test passed",
          observedAt: "2026-06-13T13:04:00.000Z",
        },
      ],
    })).toBe([
      "# Forum intake session",
      "",
      "- 2026-06-13T13:00:00.000Z [started] opened the workspace",
      "- 2026-06-13T13:04:00.000Z [verification] bun test passed",
      "",
    ].join("\n"))
  })

  test("falls back to the session ref when no title is supplied", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.no-title",
      events: [
        {
          phase: "queued",
          messageText: "waiting for an agent",
          observedAt: "2026-06-13T14:00:00.000Z",
        },
      ],
    })).toBe([
      "# session.no-title",
      "",
      "- 2026-06-13T14:00:00.000Z [queued] waiting for an agent",
      "",
    ].join("\n"))
  })

  test("falls back to the session ref for blank titles", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.blank-title",
      title: "   ",
      events: [],
    })).toBe("# session.blank-title\n")
  })

  test("renders an empty timeline as a title-only markdown document", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.empty",
      title: "Empty session",
      events: [],
    })).toBe("# Empty session\n")
  })

  test("normalizes multiline event fields into a single bullet", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.multiline",
      events: [
        {
          phase: "agent\nmessage",
          messageText: "first line\r\nsecond\tline\n- not a new event",
          observedAt: "2026-06-13T15:00:00.000Z\nextra",
        },
      ],
    })).toBe([
      "# session.multiline",
      "",
      "- 2026-06-13T15:00:00.000Z extra [agent message] first line second line - not a new event",
      "",
    ].join("\n"))
  })

  test("preserves markdown significant text without escaping", () => {
    expect(exportSessionMarkdown({
      sessionRef: "session.markdown",
      title: "Timeline *draft*",
      events: [
        {
          phase: "agent_message",
          messageText: "**done** <https://openagents.com>",
          observedAt: "2026-06-13T16:00:00.000Z",
        },
      ],
    })).toBe([
      "# Timeline *draft*",
      "",
      "- 2026-06-13T16:00:00.000Z [agent_message] **done** <https://openagents.com>",
      "",
    ].join("\n"))
  })

  test("does not mutate source event objects", () => {
    const events = [
      {
        phase: "running",
        messageText: "checking state",
        observedAt: "2026-06-13T17:00:00.000Z",
      },
    ]

    exportSessionMarkdown({
      sessionRef: "session.immutable",
      events,
    })

    expect(events).toEqual([
      {
        phase: "running",
        messageText: "checking state",
        observedAt: "2026-06-13T17:00:00.000Z",
      },
    ])
  })
})
