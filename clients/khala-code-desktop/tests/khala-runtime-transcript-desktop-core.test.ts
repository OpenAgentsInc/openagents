import {
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  type RuntimeEventEntity,
  type RuntimeTurnEntity,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import {
  buildDesktopRuntimeTurnMessages,
  foldRuntimeTurnReplyText,
  runtimeTurnMessageBody,
  runtimeTurnSortKey,
  sortRuntimeEventEntitiesBySequence,
} from "../src/bun/khala-runtime-transcript-desktop-core"

/**
 * Desktop-side coverage for the #8425 render-gap closeout: a turn dispatched
 * from mobile never produces a `chat_message` for the assistant's reply —
 * only `runtime_event`/`runtime_turn` rows carry it. This mirrors
 * `clients/khala-mobile/tests/khala-runtime-transcript-core.test.ts`'s fold
 * coverage, scoped to desktop's simpler one-message-per-turn model.
 */

const THREAD_ID = "thread.public.fixture.desktop.1"
const OWNER_ID = "user_desktop_test"

const turnEntity = (
  patch: Partial<RuntimeTurnEntity> = {},
): RuntimeTurnEntity =>
  decodeRuntimeTurnEntity({
    createdAt: "2026-07-04T20:00:00.000Z",
    eventCount: 1,
    lane: "codex_app_server",
    latestIntentId: null,
    ownerUserId: OWNER_ID,
    settledAt: null,
    startedAt: "2026-07-04T20:00:01.000Z",
    status: "completed",
    threadId: THREAD_ID,
    turnId: "turn.public.fixture.desktop.1",
    updatedAt: "2026-07-04T20:00:02.000Z",
    ...patch,
  })

const textDeltaEvent = (input: {
  readonly eventId: string
  readonly turnId: string
  readonly sequence: number
  readonly text: string
  readonly messageId?: string
}): RuntimeEventEntity =>
  decodeRuntimeEventEntity({
    createdAt: "2026-07-04T20:00:01.500Z",
    event: {
      causalityRefs: [],
      chunkId: `chunk.${input.eventId}`,
      eventId: input.eventId,
      kind: "text.delta",
      messageId: input.messageId ?? "message.public.fixture.desktop.1",
      observedAt: "2026-07-04T20:00:01.500Z",
      redactionClass: "private_ref",
      schema: "openagents.khala_runtime_event.v1",
      sequence: input.sequence,
      source: { lane: "codex_app_server", surface: "server" },
      text: input.text,
      threadId: THREAD_ID,
      turnId: input.turnId,
      visibility: "private",
    },
    eventId: input.eventId,
    kind: "text.delta",
    observedAt: "2026-07-04T20:00:01.500Z",
    ownerUserId: OWNER_ID,
    sequence: input.sequence,
    threadId: THREAD_ID,
    turnId: input.turnId,
  })

describe("sortRuntimeEventEntitiesBySequence", () => {
  test("orders out-of-order entities by their sequence number", () => {
    const a = textDeltaEvent({ eventId: "e1", sequence: 2, text: "b", turnId: "t1" })
    const b = textDeltaEvent({ eventId: "e2", sequence: 1, text: "a", turnId: "t1" })
    expect(sortRuntimeEventEntitiesBySequence([a, b]).map(entity => entity.eventId)).toEqual([
      "e2",
      "e1",
    ])
  })
})

describe("foldRuntimeTurnReplyText", () => {
  test("concatenates text.delta bodies in order", () => {
    const events = [
      textDeltaEvent({ eventId: "e1", sequence: 1, text: "codex ", turnId: "t1" }),
      textDeltaEvent({ eventId: "e2", sequence: 2, text: "mobile-to-desktop-test-ok", turnId: "t1" }),
    ]
    expect(foldRuntimeTurnReplyText(events.map(entity => entity.event))).toBe(
      "codex mobile-to-desktop-test-ok",
    )
  })

  test("returns an empty string when there are no text.delta events", () => {
    expect(foldRuntimeTurnReplyText([])).toBe("")
  })
})

describe("runtimeTurnMessageBody", () => {
  test("a completed turn's body is the reply text with no suffix", () => {
    expect(runtimeTurnMessageBody("hello", "completed")).toBe("hello")
  })

  test("a still-running turn gets an honest status suffix appended", () => {
    expect(runtimeTurnMessageBody("partial reply", "running")).toBe("partial reply (running…)")
  })

  test("a queued turn with no text yet still shows something", () => {
    expect(runtimeTurnMessageBody("", "queued")).toBe("(queued…)")
  })

  test("a failed turn keeps its partial text and marks it failed", () => {
    expect(runtimeTurnMessageBody("oops", "failed")).toBe("oops (failed)")
  })

  test("an interrupted turn keeps partial output, matching runtime.interruptTurn semantics", () => {
    expect(runtimeTurnMessageBody("partial", "interrupted")).toBe("partial (interrupted)")
  })

  test("closed carries no suffix, same as completed", () => {
    expect(runtimeTurnMessageBody("done", "closed")).toBe("done")
  })
})

describe("runtimeTurnSortKey", () => {
  test("prefers startedAt over createdAt when both are present", () => {
    const turn = turnEntity({ createdAt: "2026-07-04T19:00:00.000Z", startedAt: "2026-07-04T20:00:00.000Z" })
    expect(runtimeTurnSortKey(turn)).toBe("2026-07-04T20:00:00.000Z")
  })

  test("falls back to createdAt when a turn has not started yet", () => {
    const turn = turnEntity({ createdAt: "2026-07-04T19:00:00.000Z", startedAt: null, status: "queued" })
    expect(runtimeTurnSortKey(turn)).toBe("2026-07-04T19:00:00.000Z")
  })
})

describe("buildDesktopRuntimeTurnMessages", () => {
  test("folds a completed turn's events into one assistant message — the exact #8425 proof shape", () => {
    // This mirrors the real production proof from
    // docs/khala-code/2026-07-04-mobile-tailnet-handshake.md: a Codex turn
    // started on mobile (target.lane: codex_app_server) whose runtime_turn
    // settled `completed` with a real text.delta body
    // "codex mobile-to-desktop-test-ok".
    const turn = turnEntity({ status: "completed", turnId: "turn.mc7.codex.1" })
    const events = [
      textDeltaEvent({
        eventId: "event.mc7.1",
        sequence: 1,
        text: "codex mobile-to-desktop-test-ok",
        turnId: "turn.mc7.codex.1",
      }),
    ]

    const messages = buildDesktopRuntimeTurnMessages([turn], events)
    expect(messages).toEqual([
      {
        body: "codex mobile-to-desktop-test-ok",
        role: "assistant",
        sortKey: "2026-07-04T20:00:01.000Z",
        turnId: "turn.mc7.codex.1",
      },
    ])
  })

  test("interleaves multiple turns in chronological order by startedAt", () => {
    const earlier = turnEntity({
      startedAt: "2026-07-04T20:00:00.000Z",
      turnId: "turn.a",
    })
    const later = turnEntity({
      startedAt: "2026-07-04T20:05:00.000Z",
      turnId: "turn.b",
    })
    const events = [
      textDeltaEvent({ eventId: "e.b", sequence: 1, text: "second reply", turnId: "turn.b" }),
      textDeltaEvent({ eventId: "e.a", sequence: 1, text: "first reply", turnId: "turn.a" }),
    ]

    const messages = buildDesktopRuntimeTurnMessages([later, earlier], events)
    expect(messages.map(message => message.turnId)).toEqual(["turn.a", "turn.b"])
    expect(messages.map(message => message.body)).toEqual(["first reply", "second reply"])
  })

  test("a running turn with a partial reply renders with its status suffix, not as a finished bubble", () => {
    const turn = turnEntity({ status: "running", turnId: "turn.running.1" })
    const events = [
      textDeltaEvent({ eventId: "e1", sequence: 1, text: "still working", turnId: "turn.running.1" }),
    ]
    const [message] = buildDesktopRuntimeTurnMessages([turn], events)
    expect(message?.body).toBe("still working (running…)")
  })

  test("drops a completed turn that produced no text at all (e.g. tool-calls-only) rather than an empty bubble", () => {
    const turn = turnEntity({ status: "completed", turnId: "turn.empty.1" })
    expect(buildDesktopRuntimeTurnMessages([turn], [])).toEqual([])
  })

  test("keeps a queued turn with zero events as an honest placeholder bubble", () => {
    const turn = turnEntity({ startedAt: null, status: "queued", turnId: "turn.queued.1" })
    const messages = buildDesktopRuntimeTurnMessages([turn], [])
    expect(messages).toEqual([
      {
        body: "(queued…)",
        role: "assistant",
        sortKey: "2026-07-04T20:00:00.000Z",
        turnId: "turn.queued.1",
      },
    ])
  })

  test("only folds events belonging to the matching turnId", () => {
    const turn = turnEntity({ turnId: "turn.only.1" })
    const events = [
      textDeltaEvent({ eventId: "e1", sequence: 1, text: "mine", turnId: "turn.only.1" }),
      textDeltaEvent({ eventId: "e2", sequence: 2, text: "not mine", turnId: "turn.other.1" }),
    ]
    const [message] = buildDesktopRuntimeTurnMessages([turn], events)
    expect(message?.body).toBe("mine")
  })
})
