import { describe, expect, test } from "bun:test"
import {
  mergeSessionEventRows,
  sessionRefsToStream,
} from "../src/bun/session-event-stream"

describe("desktop session event stream helpers", () => {
  test("sessionRefsToStream includes running sessions and discovered external aliases", () => {
    expect(sessionRefsToStream({
      ok: true,
      schema: "test.node",
      sessions: [
        { sessionRef: "session.pylon.control.live", state: "running" } as any,
        { sessionRef: "session.pylon.control.done", state: "completed" } as any,
      ],
      events: {
        "session.pylon.control.live": [
          {
            eventIndex: 1,
            phase: "composer_event",
            state: "running",
            observedAt: "t",
            detail: "external session: session.pylon.codex_composer.live",
          },
        ],
      },
      artifacts: {
        "session.pylon.control.done": {
          kind: "proof",
          outcome: "completed",
          editedFileCount: 0,
          commandCount: 0,
          totalTokens: 0,
          detail: {
            schema: null,
            objectiveDigestRef: null,
            verifyRef: null,
            responseDigestRef: null,
            externalSessionRef: "session.pylon.codex_composer.done",
            executionPathRef: null,
            executionMode: null,
            sandboxMode: null,
            permissionMode: null,
            devCheckState: null,
            deviationRefs: [],
            redactionState: null,
            errorClass: null,
            errorDigestRef: null,
            workspaceRef: null,
          },
        },
      },
    })).toEqual([
      "session.pylon.control.live",
      "session.pylon.codex_composer.live",
      "session.pylon.codex_composer.done",
    ])
  })

  test("mergeSessionEventRows replaces duplicate indices and keeps order", () => {
    const merged = mergeSessionEventRows(
      [
        { eventIndex: 2, phase: "reasoning", state: "running", observedAt: "t2", detail: "old" },
        { eventIndex: 1, phase: "started", state: "running", observedAt: "t1", detail: "start" },
      ],
      { eventIndex: 2, phase: "reasoning", state: "running", observedAt: "t3", detail: "new" },
    )

    expect(merged.map((event) => event.detail)).toEqual(["start", "new"])
  })
})
