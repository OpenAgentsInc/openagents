import { describe, expect, test } from "bun:test"

import { projectDecisionFeed } from "./decision-feed-view.js"

describe("decision feed view projection", () => {
  test("projects sessions with pending decision phases", () => {
    expect(
      projectDecisionFeed([
        {
          sessionRef: "session-1",
          state: "running",
          events: [
            {
              phase: "needs_decision",
              messageText: "Pick the deploy target.",
            },
          ],
        },
        {
          sessionRef: "session-2",
          state: "running",
          events: [
            {
              phase: "working",
              messageText: "Still running.",
            },
          ],
        },
      ]),
    ).toEqual({
      pending: [
        {
          sessionRef: "session-1",
          prompt: "Pick the deploy target.",
        },
      ],
      count: 1,
    })
  })

  test("matches pending phases case-insensitively", () => {
    expect(
      projectDecisionFeed([
        {
          sessionRef: "session-approval",
          state: "waiting",
          events: [
            {
              phase: "NEEDS_APPROVAL",
              messageText: "Approve file edits?",
            },
          ],
        },
        {
          sessionRef: "session-input",
          state: "waiting",
          events: [
            {
              phase: "Awaiting_Input",
              messageText: "Provide the branch name.",
            },
          ],
        },
      ]),
    ).toEqual({
      pending: [
        {
          sessionRef: "session-approval",
          prompt: "Approve file edits?",
        },
        {
          sessionRef: "session-input",
          prompt: "Provide the branch name.",
        },
      ],
      count: 2,
    })
  })

  test("uses the latest matching event as the pending prompt", () => {
    expect(
      projectDecisionFeed([
        {
          sessionRef: "session-latest",
          state: "waiting",
          latestActivity: "2026-06-13T12:00:00.000Z",
          events: [
            {
              phase: "needs_decision",
              messageText: "Old prompt.",
            },
            {
              phase: "working",
              messageText: "Intermediate progress.",
            },
            {
              phase: "needs_approval",
              messageText: "New prompt.",
            },
          ],
        },
      ]),
    ).toEqual({
      pending: [
        {
          sessionRef: "session-latest",
          prompt: "New prompt.",
        },
      ],
      count: 1,
    })
  })

  test("finds pending decisions in recent events even when the latest event is not pending", () => {
    expect(
      projectDecisionFeed([
        {
          sessionRef: "session-recent",
          state: "running",
          events: [
            {
              phase: "awaiting_input",
              messageText: "Need an answer.",
            },
            {
              phase: "working",
              messageText: "Polling for reply.",
            },
          ],
        },
      ]),
    ).toEqual({
      pending: [
        {
          sessionRef: "session-recent",
          prompt: "Need an answer.",
        },
      ],
      count: 1,
    })
  })

  test("returns an empty feed for sessions without pending events", () => {
    expect(
      projectDecisionFeed([
        {
          sessionRef: "session-done",
          state: "done",
          events: [
            {
              phase: "completed",
              messageText: "Finished.",
            },
          ],
        },
        {
          sessionRef: "session-empty",
          state: "running",
          events: [],
        },
        {
          sessionRef: "session-missing-events",
          state: "running",
        },
      ]),
    ).toEqual({ pending: [], count: 0 })
  })

  test("skips malformed sessions and events defensively", () => {
    expect(
      projectDecisionFeed([
        null,
        {
          state: "waiting",
          events: [
            {
              phase: "needs_decision",
              messageText: "Missing session ref.",
            },
          ],
        },
        {
          sessionRef: "session-good",
          state: "waiting",
          events: [
            null,
            {
              phase: 42,
              messageText: "Bad phase.",
            },
            {
              phase: "needs_decision",
              messageText: "Valid prompt.",
            },
          ],
        },
      ] as never),
    ).toEqual({
      pending: [
        {
          sessionRef: "session-good",
          prompt: "Valid prompt.",
        },
      ],
      count: 1,
    })
  })

  test("returns an empty feed for non-array input", () => {
    expect(projectDecisionFeed(null as never)).toEqual({ pending: [], count: 0 })
    expect(projectDecisionFeed({ pending: [] } as never)).toEqual({ pending: [], count: 0 })
  })

  test("returns a snapshot that is not affected by later source mutation", () => {
    const sessions = [
      {
        sessionRef: "session-snapshot",
        state: "waiting",
        events: [
          {
            phase: "needs_approval",
            messageText: "Approve restart?",
          },
        ],
      },
    ]

    const projected = projectDecisionFeed(sessions)
    sessions[0]!.events![0]!.messageText = "Changed after projection."

    expect(projected).toEqual({
      pending: [
        {
          sessionRef: "session-snapshot",
          prompt: "Approve restart?",
        },
      ],
      count: 1,
    })
  })
})
