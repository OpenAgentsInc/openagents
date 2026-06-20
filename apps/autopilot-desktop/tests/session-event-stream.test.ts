import { describe, expect, test } from "bun:test"
import {
  createSessionEventStreamer,
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

  test("watch opens a session event stream before the next node poll", async () => {
    const urls: string[] = []
    const rows: Array<{ sessionRef: string; detail: string }> = []
    const streamer = createSessionEventStreamer({
      baseUrl: "http://127.0.0.1:4716",
      tokenProvider: async () => "tok",
      fetchFn: (async (url: string) => {
        urls.push(url)
        return new Response(
          `data: ${JSON.stringify({
            eventIndex: 1,
            phase: "composer_event",
            state: "running",
            observedAt: "t",
            messageText: "agent: live",
          })}\n\n`,
          { status: 200 },
        )
      }) as unknown as typeof fetch,
      onEvent: (sessionRef, event) => rows.push({ sessionRef, detail: event.detail }),
    })

    streamer.watch("session.pylon.control.live")
    for (let i = 0; rows.length === 0 && i < 20; i += 1) {
      await new Promise(resolve => setTimeout(resolve, 1))
    }
    streamer.stop()

    expect(urls[0]).toBe("http://127.0.0.1:4716/sessions/session.pylon.control.live/events")
    expect(rows).toEqual([{ sessionRef: "session.pylon.control.live", detail: "agent: live" }])
  })
})
