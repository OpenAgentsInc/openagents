import { describe, expect, test } from "bun:test"

import { reduceRuntimeTranscript } from "../src/sync/khala-runtime-transcript-core"

const base = {
  causalityRefs: [] as ReadonlyArray<string>,
  observedAt: "2026-01-01T00:00:00Z",
  redactionClass: "private_ref" as const,
  schema: "openagents.khala_runtime_event.v1" as const,
  sequence: 0,
  source: { lane: "codex_app_server" as const },
  threadId: "t1",
  turnId: "turn1",
  visibility: "private" as const
}

describe("reduceRuntimeTranscript", () => {
  test("merges consecutive text.delta chunks for the same message into one growing part", () => {
    const parts = reduceRuntimeTranscript([
      { ...base, eventId: "e1", kind: "text.delta", messageId: "m1", chunkId: "c1", text: "Hel" },
      { ...base, eventId: "e2", kind: "text.delta", messageId: "m1", chunkId: "c2", text: "lo" }
    ])
    expect(parts).toEqual([{ id: "m1", kind: "text", text: "Hello" }])
  })

  test("interleaves text and tool parts in temporal order (not grouped by id)", () => {
    const parts = reduceRuntimeTranscript([
      { ...base, eventId: "e1", kind: "text.delta", messageId: "m1", chunkId: "c1", text: "checking" },
      {
        ...base,
        authority: {} as never,
        eventId: "e2",
        kind: "tool.call",
        toolCallId: "call1",
        toolName: "search"
      },
      {
        ...base,
        authority: {} as never,
        eventId: "e3",
        kind: "tool.result",
        resultRef: "ref1",
        toolCallId: "call1",
        toolName: "search"
      },
      { ...base, eventId: "e4", kind: "text.delta", messageId: "m2", chunkId: "c2", text: "found it" }
    ])
    expect(parts.map(p => p.kind)).toEqual(["text", "tool", "text"])
    expect(parts[1]).toMatchObject({ status: "completed", toolCallId: "call1" })
  })

  test("turn lifecycle events become turn-status parts", () => {
    const parts = reduceRuntimeTranscript([
      { ...base, eventId: "e1", kind: "turn.started" },
      { ...base, eventId: "e2", kind: "turn.finished", finishReason: "stop" }
    ])
    expect(parts.map(p => p.kind)).toEqual(["turn-status", "turn-status"])
    expect(parts[0]).toMatchObject({ status: "running" })
    expect(parts[1]).toMatchObject({ status: "completed" })
  })

  test("turn.finished with error finishReason maps to failed status", () => {
    const parts = reduceRuntimeTranscript([
      { ...base, eventId: "e1", kind: "turn.finished", finishReason: "error" }
    ])
    expect(parts[0]).toMatchObject({ status: "failed" })
  })

  test("appending new events preserves the id (and position) of previously-produced parts", () => {
    // This id-stability property is what lets the mobile transcript FlatList
    // (`app/thread/[threadId].tsx`) safely rely on Reanimated's `entering=`
    // FadeIn to animate ONLY newly-appended parts as a turn streams in: since
    // `reduceRuntimeTranscript` re-folds the full event log from scratch on
    // every call, only appending strictly-new events at the end must leave
    // every earlier part's `id` (React/FlatList's `keyExtractor` key)
    // unchanged, so React reuses the same row component instance instead of
    // remounting it and re-triggering the entrance animation.
    const events: Array<Parameters<typeof reduceRuntimeTranscript>[0][number]> = [
      { ...base, eventId: "e1", kind: "turn.started" },
      { ...base, eventId: "e2", kind: "text.delta", messageId: "m1", chunkId: "c1", text: "checking" },
      {
        ...base,
        authority: {} as never,
        eventId: "e3",
        kind: "tool.call",
        toolCallId: "call1",
        toolName: "search"
      }
    ]

    const before = reduceRuntimeTranscript(events)

    const after = reduceRuntimeTranscript([
      ...events,
      {
        ...base,
        authority: {} as never,
        eventId: "e4",
        kind: "tool.result",
        resultRef: "ref1",
        toolCallId: "call1",
        toolName: "search"
      },
      { ...base, eventId: "e5", kind: "text.delta", messageId: "m2", chunkId: "c2", text: "found it" }
    ])

    // Every part id from the earlier fold still appears, in the same order,
    // as a prefix of the new fold's ids.
    expect(after.slice(0, before.length).map(p => p.id)).toEqual(before.map(p => p.id))
    // Exactly one new part id was appended (the text.delta for m2) — the
    // tool.call part was updated in place (same id) rather than duplicated.
    expect(after.length).toEqual(before.length + 1)
  })

  test("usage.recorded becomes a usage part carrying token counts", () => {
    const parts = reduceRuntimeTranscript([
      {
        ...base,
        eventId: "e1",
        kind: "usage.recorded",
        usage: { inputTokens: 100, outputTokens: 20, totalTokens: 120, usageRef: "usage.1" }
      }
    ])
    expect(parts[0]).toEqual({
      id: "usage-0",
      inputTokens: 100,
      kind: "usage",
      outputTokens: 20,
      totalTokens: 120
    })
  })
})
