import { describe, expect, test } from "bun:test"
import { Effect, Schema } from "effect"

import type {
  ConfirmedAgentTimelineSnapshot,
  KhalaSyncAgentTimeline,
} from "./agent-timeline.js"
import type {
  ConfirmedChatMessage,
  KhalaSyncConversation,
  KhalaSyncConversationChange,
  KhalaSyncConversationStatus,
} from "./conversation.js"
import {
  KhalaConversationLiveEnvelopeSchema,
  openKhalaConversationLive,
  type KhalaConversationLiveUpdate,
} from "./live-conversation.js"

const threadRef = "thread.live.fixture"

const flush = async (): Promise<void> => {
  await new Promise(resolve => setTimeout(resolve, 0))
}

const status = (
  phase: KhalaSyncConversationStatus["phase"],
  cursor: number | null,
  pendingMutationCount = 0,
): KhalaSyncConversationStatus => ({ phase, cursor, pendingMutationCount })

const message = (ref: string, version: number): ConfirmedChatMessage => ({
  messageRef: ref,
  threadRef,
  body: `body ${version}`,
  createdAt: "2026-07-11T00:00:00.000Z",
  updatedAt: "2026-07-11T00:00:00.000Z",
  version,
})

const timeline = (
  cursor: number,
  eventRef = `event.${cursor}`,
): ConfirmedAgentTimelineSnapshot => ({
  status: status("live", cursor),
  run: {
    runRef: "run.live.fixture",
    routeRef: threadRef,
    status: "running",
    createdAt: "2026-07-11T00:00:00.000Z",
    updatedAt: "2026-07-11T00:00:00.000Z",
    startedAt: "2026-07-11T00:00:00.000Z",
    completedAt: null,
    failedAt: null,
    canceledAt: null,
    version: cursor,
  },
  events: [{
    eventRef,
    runRef: "run.live.fixture",
    sequence: cursor,
    eventType: "text.delta",
    summary: "fixture",
    status: null,
    artifactRefs: [],
    item: { kind: "text", messageRef: "assistant.fixture", text: "delta" },
    createdAt: "2026-07-11T00:00:00.000Z",
    version: cursor,
  }],
})

const harness = (initialStatus = status("live", 4)) => {
  let currentStatus = initialStatus
  let messages: ReadonlyArray<ConfirmedChatMessage> = [message("message.1", 1)]
  let currentTimeline: ConfirmedAgentTimelineSnapshot | null = timeline(4)
  const listeners = new Set<(change: KhalaSyncConversationChange) => void>()
  let openCount = 0
  let closeCount = 0

  const conversation: KhalaSyncConversation = {
    personalStatus: () => currentStatus,
    threadStatus: () => currentStatus,
    listConfirmedThreads: () => Effect.succeed([]),
    openThread: () => Effect.sync(() => { openCount += 1 }),
    closeThread: () => Effect.sync(() => { closeCount += 1 }),
    subscribeThread: (_ref, listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    listConfirmedMessages: () => Effect.succeed(messages),
    createThread: () => Effect.die("unused"),
    appendMessage: () => Effect.die("unused"),
  }
  const agentTimeline: KhalaSyncAgentTimeline = {
    status: () => currentTimeline?.status ?? currentStatus,
    open: () => Effect.void,
    snapshot: () => Effect.succeed(currentTimeline ?? {
      status: currentStatus,
      run: null,
      events: [],
    }),
    snapshotForThread: () => Effect.succeed(currentTimeline ?? {
      status: currentStatus,
      run: null,
      events: [],
    }),
  }

  return {
    conversation,
    timeline: agentTimeline,
    counts: () => ({ closeCount, listeners: listeners.size, openCount }),
    change: (next: Readonly<{
      status: KhalaSyncConversationStatus
      messages?: ReadonlyArray<ConfirmedChatMessage>
      timeline?: ConfirmedAgentTimelineSnapshot | null
      kind?: KhalaSyncConversationChange["kind"]
    }>) => {
      currentStatus = next.status
      if (next.messages !== undefined) messages = next.messages
      if (next.timeline !== undefined) currentTimeline = next.timeline
      for (const listener of [...listeners]) listener({
        kind: next.kind ?? "content",
        status: currentStatus,
        threadRef,
      })
    },
  }
}

describe("cursor-aware live conversation subscription", () => {
  test("emits one schema-valid confirmed resume snapshot with correlation refs", async () => {
    const h = harness()
    const updates: Array<KhalaConversationLiveUpdate> = []
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.fixture",
      generation: 2,
      threadRef,
      afterCursor: 3,
    }, update => { updates.push(update) })
    await flush()

    expect(updates).toHaveLength(1)
    expect(Schema.decodeUnknownSync(KhalaConversationLiveEnvelopeSchema)(updates[0]!.envelope)).toEqual(updates[0]!.envelope)
    expect(updates[0]!.envelope).toMatchObject({
      cursor: 4,
      delivery: "confirmed",
      eventRefs: ["event.4"],
      generation: 2,
      messageRefs: ["message.1"],
      recovery: "resumed",
      runRef: "run.live.fixture",
      sequence: 1,
    })
    await subscription.close()
    expect(h.counts()).toEqual({ closeCount: 1, listeners: 0, openCount: 1 })
  })

  test("classifies a proven cursor gap as one bounded authoritative refetch", async () => {
    const h = harness(status("live", 9))
    h.change({ status: status("live", 9), timeline: timeline(9) })
    const updates: Array<KhalaConversationLiveUpdate> = []
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.gap",
      generation: 1,
      threadRef,
      afterCursor: 4,
    }, update => { updates.push(update) })
    await flush()

    expect(updates[0]!.envelope).toMatchObject({
      cursor: 9,
      delivery: "confirmed",
      recovery: "authoritative_refetch",
    })
    expect(updates[0]!.snapshot?.timeline?.events).toHaveLength(1)
    await subscription.close()
  })

  test("orders provisional, confirmed, and interrupted delivery without polling", async () => {
    const h = harness(status("live", 4, 1))
    const updates: Array<KhalaConversationLiveUpdate> = []
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.order",
      generation: 1,
      threadRef,
    }, update => { updates.push(update) })
    await flush()
    h.change({
      status: status("live", 5),
      messages: [message("message.1", 1), message("message.2", 5)],
      timeline: timeline(5),
    })
    await flush()
    h.change({ status: status("must_refetch", null), timeline: null, kind: "state" })
    await flush()

    expect(updates.map(update => update.envelope.delivery)).toEqual([
      "provisional",
      "confirmed",
      "interrupted",
    ])
    expect(updates.map(update => update.envelope.sequence)).toEqual([1, 2, 3])
    const interrupted = updates[2]!.envelope
    expect(interrupted.delivery).toBe("interrupted")
    if (interrupted.delivery !== "interrupted") throw new Error("expected interrupted delivery")
    expect(interrupted.reason).toBe("must_refetch")
    expect(updates[2]!.snapshot).toBeNull()
    await subscription.close()
  })

  test("coalesces arbitrary source churn behind one slow consumer", async () => {
    const h = harness()
    const updates: Array<KhalaConversationLiveUpdate> = []
    let release!: () => void
    const blocked = new Promise<void>(resolve => { release = resolve })
    let first = true
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.slow",
      generation: 1,
      threadRef,
    }, async update => {
      updates.push(update)
      if (first) {
        first = false
        await blocked
      }
    })
    await flush()

    for (let cursor = 5; cursor <= 25; cursor += 1) {
      h.change({ status: status("live", cursor), timeline: timeline(cursor) })
    }
    release()
    await flush()
    await flush()

    expect(updates).toHaveLength(2)
    expect(updates[1]!.envelope.cursor).toBe(25)
    expect(updates[1]!.envelope.sequence).toBe(2)
    await subscription.close()
  })

  test("rejects a future resume cursor and disposes idempotently", async () => {
    const h = harness()
    const updates: Array<KhalaConversationLiveUpdate> = []
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.future",
      generation: 3,
      threadRef,
      afterCursor: 10,
    }, update => { updates.push(update) })
    await flush()

    expect(updates[0]!.envelope).toMatchObject({
      cursor: 4,
      delivery: "interrupted",
      reason: "cursor_ahead",
    })
    expect(updates[0]!.snapshot).toBeNull()
    await subscription.close()
    await subscription.close()
    h.change({ status: status("live", 5), timeline: timeline(5) })
    await flush()
    expect(updates).toHaveLength(1)
    expect(h.counts()).toEqual({ closeCount: 1, listeners: 0, openCount: 1 })
  })

  test("an already-aborted subscription never publishes and closes its scope", async () => {
    const h = harness()
    const controller = new AbortController()
    controller.abort()
    const updates: Array<KhalaConversationLiveUpdate> = []
    const subscription = await openKhalaConversationLive({
      conversation: h.conversation,
      timeline: h.timeline,
      subscriptionRef: "subscription.aborted",
      generation: 1,
      threadRef,
      signal: controller.signal,
    }, update => { updates.push(update) })
    await flush()

    expect(subscription.closed()).toBe(true)
    expect(updates).toEqual([])
    expect(h.counts()).toEqual({ closeCount: 1, listeners: 0, openCount: 1 })
  })
})
