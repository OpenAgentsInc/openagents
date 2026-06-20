import { describe, expect, test } from "bun:test"

import {
  parseActivityStreamData,
  subscribePaymentParticles,
  subscribePylonScene,
} from "../src/ui/chat-world-subscriptions"
import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../src/shared/chat-world-scene"

const jsonResponse = (body: unknown, ok = true): Response =>
  ({
    ok,
    status: ok ? 200 : 500,
    json: async () => body,
  }) as unknown as Response

describe("subscribePylonScene (flag-gated poll)", () => {
  test("returns noop and does not fetch when CHAT_WORLD_SCENE is off", () => {
    let fetched = 0
    const stop = subscribePylonScene(() => {}, {
      flags: { CHAT_WORLD_SCENE: false },
      fetchFn: (async () => {
        fetched += 1
        return jsonResponse({})
      }) as unknown as typeof fetch,
    })
    stop()
    expect(fetched).toBe(0)
  })

  test("polls pylon-stats and dispatches a projected scene when flag on", async () => {
    const scenes: ChatWorldPylonScene[] = []
    const stop = subscribePylonScene((s) => scenes.push(s), {
      flags: { CHAT_WORLD_SCENE: true },
      setInterval: () => 0, // suppress repeat; we test the immediate poll
      clearInterval: () => {},
      fetchFn: (async () =>
        jsonResponse({
          available: true,
          status: "live",
          pylonsOnlineNow: 1,
          recentPylons: [{ nostrPubkeyShort: "abc", onlineNow: true }],
        })) as unknown as typeof fetch,
    })
    await new Promise((r) => setTimeout(r, 5))
    stop()
    expect(scenes.length).toBeGreaterThanOrEqual(1)
    expect(scenes[0]!.onlineNow).toBe(1)
    expect(scenes[0]!.nodes[0]!.id).toBe("abc")
  })

  test("dispatches honest zero-state on a failed fetch", async () => {
    const scenes: ChatWorldPylonScene[] = []
    const stop = subscribePylonScene((s) => scenes.push(s), {
      flags: { CHAT_WORLD_SCENE: true },
      setInterval: () => 0,
      clearInterval: () => {},
      fetchFn: (async () => jsonResponse({}, false)) as unknown as typeof fetch,
    })
    await new Promise((r) => setTimeout(r, 5))
    stop()
    expect(scenes[0]!.empty).toBe(true)
  })
})

describe("parseActivityStreamData", () => {
  const payment = {
    eventRef: "evt-1",
    kind: "real_bitcoin_moved",
    actorRef: "a",
    targetRef: "b",
    amountSats: 100,
    realBitcoinMoved: true,
    sourceRefs: ["receipt.x"],
  }

  test("parses the worker { event } frame shape", () => {
    const p = parseActivityStreamData(JSON.stringify({ event: payment }))
    expect(p?.id).toBe("evt-1")
    expect(p?.realBitcoinMoved).toBe(true)
  })

  test("parses a bare event object too", () => {
    const p = parseActivityStreamData(JSON.stringify(payment))
    expect(p?.id).toBe("evt-1")
  })

  test("returns null for malformed / non-payment data", () => {
    expect(parseActivityStreamData("not json")).toBeNull()
    expect(parseActivityStreamData(JSON.stringify({ event: { eventRef: "x", kind: "forum_posted" } }))).toBeNull()
    expect(parseActivityStreamData(JSON.stringify({ nope: 1 }))).toBeNull()
  })
})

describe("subscribePaymentParticles (flag-gated, evidence-bound)", () => {
  test("noop when CHAT_WORLD_PAYMENTS off", () => {
    let fetched = 0
    const stop = subscribePaymentParticles(() => {}, {
      flags: { CHAT_WORLD_PAYMENTS: false },
      fetchFn: (async () => {
        fetched += 1
        return jsonResponse({})
      }) as unknown as typeof fetch,
    })
    stop()
    expect(fetched).toBe(0)
  })

  test("backfills payment particles from the envelope poll on connect", async () => {
    const particles: PaymentParticle[] = []
    const stop = subscribePaymentParticles((p) => particles.push(p), {
      flags: { CHAT_WORLD_PAYMENTS: true },
      // no EventSource → poll path; suppress repeat interval
      eventSourceCtor: undefined,
      setInterval: () => 0,
      clearInterval: () => {},
      fetchFn: (async () =>
        jsonResponse({
          events: [
            {
              eventRef: "ok",
              kind: "real_bitcoin_moved",
              actorRef: "a",
              targetRef: "b",
              amountSats: 5,
              realBitcoinMoved: true,
              sourceRefs: ["receipt.ok"],
            },
            // dropped: no sourceRef
            {
              eventRef: "bad",
              kind: "real_bitcoin_moved",
              actorRef: "a",
              targetRef: "b",
              sourceRefs: [],
            },
          ],
        })) as unknown as typeof fetch,
    })
    await new Promise((r) => setTimeout(r, 5))
    stop()
    expect(particles).toHaveLength(1)
    expect(particles[0]!.id).toBe("ok")
    expect(particles[0]!.sourceRefs).toContain("receipt.ok")
  })
})
