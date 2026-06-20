import { describe, expect, test } from "bun:test"

import { groupTimeline } from "./session-timeline-group.js"

describe("session timeline grouping", () => {
  test("returns no groups and zero total for an empty timeline", () => {
    expect(groupTimeline([])).toEqual({
      groups: [],
      total: 0,
    })
  })

  test("wraps a single event in one group", () => {
    expect(groupTimeline([
      {
        phase: "started",
        messageText: "Started session",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
    ])).toEqual({
      groups: [
        {
          phase: "started",
          count: 1,
          items: [
            {
              messageText: "Started session",
              observedAt: "2026-06-13T14:00:00.000Z",
            },
          ],
        },
      ],
      total: 1,
    })
  })

  test("groups consecutive events with the same phase", () => {
    expect(groupTimeline([
      {
        phase: "thinking",
        messageText: "Reading files",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Checking tests",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Planning change",
        observedAt: "2026-06-13T14:03:00.000Z",
      },
    ])).toEqual({
      groups: [
        {
          phase: "thinking",
          count: 3,
          items: [
            {
              messageText: "Reading files",
              observedAt: "2026-06-13T14:01:00.000Z",
            },
            {
              messageText: "Checking tests",
              observedAt: "2026-06-13T14:02:00.000Z",
            },
            {
              messageText: "Planning change",
              observedAt: "2026-06-13T14:03:00.000Z",
            },
          ],
        },
      ],
      total: 3,
    })
  })

  test("starts a new group when the phase changes", () => {
    expect(groupTimeline([
      {
        phase: "started",
        messageText: "Started session",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Reading context",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "completed",
        messageText: "Finished",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
    ])).toEqual({
      groups: [
        {
          phase: "started",
          count: 1,
          items: [
            {
              messageText: "Started session",
              observedAt: "2026-06-13T14:00:00.000Z",
            },
          ],
        },
        {
          phase: "thinking",
          count: 1,
          items: [
            {
              messageText: "Reading context",
              observedAt: "2026-06-13T14:01:00.000Z",
            },
          ],
        },
        {
          phase: "completed",
          count: 1,
          items: [
            {
              messageText: "Finished",
              observedAt: "2026-06-13T14:02:00.000Z",
            },
          ],
        },
      ],
      total: 3,
    })
  })

  test("keeps non-consecutive repeats in separate groups", () => {
    expect(groupTimeline([
      {
        phase: "thinking",
        messageText: "Read first file",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "executing",
        messageText: "Ran test",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Reviewed result",
        observedAt: "2026-06-13T14:03:00.000Z",
      },
    ])).toEqual({
      groups: [
        {
          phase: "thinking",
          count: 1,
          items: [
            {
              messageText: "Read first file",
              observedAt: "2026-06-13T14:01:00.000Z",
            },
          ],
        },
        {
          phase: "executing",
          count: 1,
          items: [
            {
              messageText: "Ran test",
              observedAt: "2026-06-13T14:02:00.000Z",
            },
          ],
        },
        {
          phase: "thinking",
          count: 1,
          items: [
            {
              messageText: "Reviewed result",
              observedAt: "2026-06-13T14:03:00.000Z",
            },
          ],
        },
      ],
      total: 3,
    })
  })

  test("preserves mixed group order and per-item order", () => {
    expect(groupTimeline([
      {
        phase: "started",
        messageText: "Started",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Step one",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Step two",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
      {
        phase: "executing",
        messageText: "Command one",
        observedAt: "2026-06-13T14:03:00.000Z",
      },
      {
        phase: "executing",
        messageText: "Command two",
        observedAt: "2026-06-13T14:04:00.000Z",
      },
      {
        phase: "completed",
        messageText: "Done",
        observedAt: "2026-06-13T14:05:00.000Z",
      },
    ])).toEqual({
      groups: [
        {
          phase: "started",
          count: 1,
          items: [
            {
              messageText: "Started",
              observedAt: "2026-06-13T14:00:00.000Z",
            },
          ],
        },
        {
          phase: "thinking",
          count: 2,
          items: [
            {
              messageText: "Step one",
              observedAt: "2026-06-13T14:01:00.000Z",
            },
            {
              messageText: "Step two",
              observedAt: "2026-06-13T14:02:00.000Z",
            },
          ],
        },
        {
          phase: "executing",
          count: 2,
          items: [
            {
              messageText: "Command one",
              observedAt: "2026-06-13T14:03:00.000Z",
            },
            {
              messageText: "Command two",
              observedAt: "2026-06-13T14:04:00.000Z",
            },
          ],
        },
        {
          phase: "completed",
          count: 1,
          items: [
            {
              messageText: "Done",
              observedAt: "2026-06-13T14:05:00.000Z",
            },
          ],
        },
      ],
      total: 6,
    })
  })

  test("does not mutate the input events", () => {
    const events = [
      {
        phase: "thinking",
        messageText: "Read context",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Drafted change",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
    ]

    groupTimeline(events)

    expect(events).toEqual([
      {
        phase: "thinking",
        messageText: "Read context",
        observedAt: "2026-06-13T14:01:00.000Z",
      },
      {
        phase: "thinking",
        messageText: "Drafted change",
        observedAt: "2026-06-13T14:02:00.000Z",
      },
    ])
  })
})
