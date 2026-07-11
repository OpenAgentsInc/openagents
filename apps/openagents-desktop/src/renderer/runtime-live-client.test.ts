import { describe, expect, test } from "bun:test"

import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayRequest,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import {
  openDesktopRuntimeLiveThread,
  type DesktopRuntimeLiveUpdate,
} from "./runtime-live-client.ts"

const update = (input: Readonly<{
  subscriptionRef?: string
  generation?: number
  threadRef?: string
  sequence?: number
  cursor?: number
}> = {}): DesktopRuntimeLiveUpdate => ({
  kind: "conversation.live.update",
  envelope: {
    kind: "conversation.live",
    delivery: "confirmed",
    subscriptionRef: input.subscriptionRef ?? "subscription.renderer.1",
    generation: input.generation ?? 1,
    sequence: input.sequence ?? 1,
    threadRef: input.threadRef ?? "thread.renderer.1",
    cursor: input.cursor ?? 4,
    recovery: "resumed",
    messageRefs: [],
    eventRefs: [],
  },
  snapshot: {
    status: { phase: "live", cursor: input.cursor ?? 4, pendingMutationCount: 0 },
    thread: null,
    messages: [],
    timeline: null,
  },
})

const harness = (subscribeStatus: "subscribed" | "unavailable" = "subscribed") => {
  const listeners = new Set<(event: DesktopRuntimeGatewayEvent) => void>()
  const requests: Array<DesktopRuntimeGatewayRequest> = []
  let emitBeforeResponse: DesktopRuntimeLiveUpdate | null = null
  return {
    requests,
    listeners,
    emit: (event: DesktopRuntimeGatewayEvent) => {
      for (const listener of [...listeners]) listener(event)
    },
    beforeResponse: (event: DesktopRuntimeLiveUpdate) => { emitBeforeResponse = event },
    bridge: {
      subscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => {
        listeners.add(listener)
        return () => listeners.delete(listener)
      },
      request: async (value: unknown): Promise<DesktopRuntimeGatewayResponse> => {
        const request = value as DesktopRuntimeGatewayRequest
        if (request.kind !== "command") throw new Error("expected command")
        requests.push(request)
        if (request.command.id === "conversation.subscribe") {
          if (emitBeforeResponse !== null) {
            for (const listener of [...listeners]) listener(emitBeforeResponse)
          }
          return {
            kind: "conversation_subscription_outcome" as const,
            commandId: request.commandId,
            subscriptionRef: request.command.subscriptionRef,
            generation: request.command.generation,
            status: subscribeStatus,
          }
        }
        if (request.command.id !== "conversation.unsubscribe") {
          throw new Error("expected unsubscribe command")
        }
        return {
          kind: "conversation_subscription_outcome" as const,
          commandId: request.commandId,
          subscriptionRef: request.command.subscriptionRef,
          generation: request.command.generation,
          status: "unsubscribed" as const,
        }
      },
    },
  }
}

describe("Desktop Runtime Gateway live renderer client", () => {
  test("registers before subscribe and delivers the raced initial snapshot once", async () => {
    const h = harness()
    const received: Array<DesktopRuntimeLiveUpdate> = []
    h.beforeResponse(update())
    const handle = await openDesktopRuntimeLiveThread({
      bridge: h.bridge,
      subscriptionRef: "subscription.renderer.1",
      generation: 1,
      threadRef: "thread.renderer.1",
      afterCursor: 3,
      onUpdate: value => { received.push(value) },
    })

    expect(handle).not.toBeNull()
    expect(received).toEqual([update()])
    expect(handle?.cursor()).toBe(4)
    expect(handle?.sequence()).toBe(1)
    expect(h.requests[0]).toMatchObject({
      command: { id: "conversation.subscribe", afterCursor: 3 },
    })
    await handle?.close()
  })

  test("fences foreign generations, refs, stale sequence, and cursor regression", async () => {
    const h = harness()
    const received: Array<DesktopRuntimeLiveUpdate> = []
    const handle = await openDesktopRuntimeLiveThread({
      bridge: h.bridge,
      subscriptionRef: "subscription.renderer.1",
      generation: 1,
      threadRef: "thread.renderer.1",
      afterCursor: 3,
      onUpdate: value => { received.push(value) },
    })
    h.emit(update({ generation: 2 }))
    h.emit(update({ subscriptionRef: "subscription.renderer.other" }))
    h.emit(update({ threadRef: "thread.renderer.other" }))
    h.emit(update({ sequence: 2, cursor: 5 }))
    h.emit(update({ sequence: 1, cursor: 6 }))
    h.emit(update({ sequence: 3, cursor: 4 }))
    h.emit(update({ sequence: 4, cursor: 6 }))

    expect(received.map(value => [value.envelope.sequence, value.envelope.cursor])).toEqual([
      [2, 5],
      [4, 6],
    ])
    expect(handle?.sequence()).toBe(4)
    expect(handle?.cursor()).toBe(6)
    await handle?.close()
  })

  test("close is exact and idempotent and rejects late delivery", async () => {
    const h = harness()
    const received: Array<DesktopRuntimeLiveUpdate> = []
    const handle = await openDesktopRuntimeLiveThread({
      bridge: h.bridge,
      subscriptionRef: "subscription.renderer.1",
      generation: 7,
      threadRef: "thread.renderer.1",
      onUpdate: value => { received.push(value) },
    })
    await handle?.close()
    await handle?.close()
    h.emit(update({ generation: 7 }))

    expect(handle?.closed()).toBe(true)
    expect(received).toEqual([])
    expect(h.listeners.size).toBe(0)
    expect(h.requests).toHaveLength(2)
    expect(h.requests[1]).toMatchObject({
      command: {
        id: "conversation.unsubscribe",
        subscriptionRef: "subscription.renderer.1",
        generation: 7,
      },
    })
  })

  test("unavailable or invalid subscriptions release the event listener", async () => {
    const h = harness("unavailable")
    const errors: Array<unknown> = []
    expect(await openDesktopRuntimeLiveThread({
      bridge: h.bridge,
      subscriptionRef: "subscription.renderer.1",
      generation: 1,
      threadRef: "thread.renderer.1",
      onUpdate: () => undefined,
      onError: error => { errors.push(error) },
    })).toBeNull()
    expect(h.listeners.size).toBe(0)
    expect(errors).toEqual([])
    expect(await openDesktopRuntimeLiveThread({
      bridge: h.bridge,
      subscriptionRef: "subscription.renderer.invalid",
      generation: 0,
      threadRef: "thread.renderer.1",
      onUpdate: () => undefined,
    })).toBeNull()
  })
})
