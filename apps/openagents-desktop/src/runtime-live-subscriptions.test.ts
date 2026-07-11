import { describe, expect, test } from "bun:test"
import type {
  KhalaSyncAgentTimeline,
  KhalaSyncConversation,
  KhalaSyncConversationChange,
  KhalaSyncConversationStatus,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import { createDesktopRuntimeLiveSubscriptions } from "./runtime-live-subscriptions.ts"

const live: KhalaSyncConversationStatus = {
  phase: "live",
  cursor: 4,
  pendingMutationCount: 0,
}

const harness = (options: Readonly<{ failOpen?: boolean }> = {}) => {
  const listeners = new Set<(change: KhalaSyncConversationChange) => void>()
  let opens = 0
  let closes = 0
  const conversation: KhalaSyncConversation = {
    personalStatus: () => live,
    threadStatus: () => live,
    listConfirmedThreads: () => Effect.succeed([]),
    listConfirmedMessages: () => Effect.succeed([]),
    openThread: () => options.failOpen
      ? Effect.fail(new Error("unavailable") as never)
      : Effect.sync(() => { opens += 1 }),
    closeThread: () => Effect.sync(() => { closes += 1 }),
    subscribeThread: (_threadRef, listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    createThread: () => Effect.die("unused"),
    appendMessage: () => Effect.die("unused"),
  }
  const timeline: KhalaSyncAgentTimeline = {
    status: () => live,
    open: () => Effect.void,
    snapshot: () => Effect.succeed({ status: live, run: null, events: [] }),
    snapshotForThread: () => Effect.succeed({ status: live, run: null, events: [] }),
  }
  return {
    conversation,
    timeline,
    counts: () => ({ closes, listeners: listeners.size, opens }),
  }
}

describe("Desktop runtime live subscription registry", () => {
  test("replaces a generation before open and fences a stale unsubscribe", async () => {
    const h = harness()
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
    })
    const request = {
      subscriptionRef: "subscription.desktop.fixture",
      generation: 1,
      threadRef: "thread.desktop.fixture",
    }

    expect(await registry.subscribe(request, () => undefined)).toEqual({ status: "subscribed" })
    expect(await registry.subscribe(request, () => undefined)).toEqual({ status: "already_subscribed" })
    expect(await registry.subscribe({ ...request, generation: 2 }, () => undefined)).toEqual({ status: "subscribed" })
    expect(registry.activeCount()).toBe(1)
    expect(h.counts()).toEqual({ closes: 1, listeners: 1, opens: 2 })
    expect(await registry.unsubscribe(request.subscriptionRef, 1)).toBe(false)
    expect(registry.activeCount()).toBe(1)
    expect(await registry.unsubscribe(request.subscriptionRef, 2)).toBe(true)
    expect(h.counts()).toEqual({ closes: 2, listeners: 0, opens: 2 })
  })

  test("rejects a stale subscribe without disturbing the current generation", async () => {
    const h = harness()
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
    })
    const request = {
      subscriptionRef: "subscription.desktop.stale",
      generation: 3,
      threadRef: "thread.desktop.fixture",
    }
    await registry.subscribe(request, () => undefined)

    expect(await registry.subscribe({ ...request, generation: 2 }, () => undefined)).toEqual({
      status: "stale_generation",
      activeGeneration: 3,
    })
    expect(registry.activeCount()).toBe(1)
    expect(h.counts()).toEqual({ closes: 0, listeners: 1, opens: 1 })
    await registry.dispose()
  })

  test("bounds active slots and exposes only exact-generation metrics", async () => {
    const h = harness()
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
      maxSubscriptions: 1,
    })
    await registry.subscribe({
      subscriptionRef: "subscription.desktop.one",
      generation: 1,
      threadRef: "thread.desktop.one",
    }, () => undefined)

    expect(await registry.subscribe({
      subscriptionRef: "subscription.desktop.two",
      generation: 1,
      threadRef: "thread.desktop.two",
    }, () => undefined)).toEqual({ status: "capacity_exceeded" })
    expect(registry.metrics("subscription.desktop.one", 0)).toBeNull()
    expect(registry.metrics("subscription.desktop.one", 1)).toMatchObject({
      maxPendingSnapshots: 1,
      sourceSignals: 1,
    })
    await registry.dispose()
  })

  test("dispose closes every slot once and refuses later work", async () => {
    const h = harness()
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
    })
    for (const ref of ["one", "two"]) {
      await registry.subscribe({
        subscriptionRef: `subscription.desktop.${ref}`,
        generation: 1,
        threadRef: `thread.desktop.${ref}`,
      }, () => undefined)
    }
    await registry.dispose()
    await registry.dispose()

    expect(registry.activeCount()).toBe(0)
    expect(h.counts()).toEqual({ closes: 2, listeners: 0, opens: 2 })
    expect(await registry.subscribe({
      subscriptionRef: "subscription.desktop.late",
      generation: 1,
      threadRef: "thread.desktop.late",
    }, () => undefined)).toEqual({ status: "unavailable" })
  })

  test("reset closes current authority but permits a replacement session", async () => {
    const h = harness()
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
    })
    await registry.subscribe({
      subscriptionRef: "subscription.desktop.reset",
      generation: 1,
      threadRef: "thread.desktop.reset",
    }, () => undefined)
    await registry.reset()
    expect(registry.activeCount()).toBe(0)
    expect(h.counts()).toEqual({ closes: 1, listeners: 0, opens: 1 })
    expect(await registry.subscribe({
      subscriptionRef: "subscription.desktop.reset",
      generation: 2,
      threadRef: "thread.desktop.reset",
    }, () => undefined)).toEqual({ status: "subscribed" })
    await registry.dispose()
    expect(h.counts()).toEqual({ closes: 2, listeners: 0, opens: 2 })
  })

  test("a failed open leaves no active slot or observer", async () => {
    const h = harness({ failOpen: true })
    const registry = createDesktopRuntimeLiveSubscriptions({
      conversation: () => h.conversation,
      timeline: () => h.timeline,
    })

    expect(await registry.subscribe({
      subscriptionRef: "subscription.desktop.failed",
      generation: 1,
      threadRef: "thread.desktop.failed",
    }, () => undefined)).toEqual({ status: "unavailable" })
    expect(registry.activeCount()).toBe(0)
    expect(h.counts()).toEqual({ closes: 0, listeners: 0, opens: 0 })
    expect(await registry.subscribe({
      subscriptionRef: "subscription.desktop.invalid",
      generation: 0,
      threadRef: "thread.desktop.invalid",
    }, () => undefined)).toEqual({ status: "unavailable" })
  })
})
