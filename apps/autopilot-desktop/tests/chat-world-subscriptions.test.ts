import { describe, expect, test } from "bun:test"

import {
  createVerseMultiplayerClient,
  parseActivityStreamData,
  publishActiveVerseLocalPose,
  publishSpacetimeAvatarPosition,
  subscribePaymentParticles,
  subscribePylonScene,
  subscribeSpacetimeWorld,
} from "../src/ui/chat-world-subscriptions"
import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../src/shared/chat-world-scene"
import {
  chatWorldRegionRefForRun,
  type ChatWorldMultiplayerProjection,
} from "../src/shared/chat-world-multiplayer"

const jsonResponse = (body: unknown, ok = true): Response =>
  ({
    ok,
    status: ok ? 200 : 500,
    json: async () => body,
  }) as unknown as Response

type TableCallback = (...args: ReadonlyArray<unknown>) => void

class FakeTable {
  rows: unknown[]
  inserts: TableCallback[] = []
  updates: TableCallback[] = []
  deletes: TableCallback[] = []

  constructor(rows: ReadonlyArray<unknown> = []) {
    this.rows = [...rows]
  }

  iter(): Iterable<unknown> {
    return this.rows
  }

  onInsert(cb: TableCallback): void {
    this.inserts.push(cb)
  }

  onUpdate(cb: TableCallback): void {
    this.updates.push(cb)
  }

  onDelete(cb: TableCallback): void {
    this.deletes.push(cb)
  }

  removeOnInsert(cb: TableCallback): void {
    this.inserts = this.inserts.filter((item) => item !== cb)
  }

  removeOnUpdate(cb: TableCallback): void {
    this.updates = this.updates.filter((item) => item !== cb)
  }

  removeOnDelete(cb: TableCallback): void {
    this.deletes = this.deletes.filter((item) => item !== cb)
  }

  insert(row: unknown): void {
    this.rows.push(row)
    for (const cb of this.inserts) cb(row)
  }
}

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

const fakeWorldRows = (
  regionOverrides: Record<string, unknown> = {},
) => ({
  worldRegion: new FakeTable([{
    regionRef,
    runRef,
    label: "Tassadar main",
    minX: -10,
    minY: -2,
    minZ: -10,
    maxX: 10,
    maxY: 8,
    maxZ: 10,
    proximityRadiusMeters: 8,
    avatarPositionMinIntervalMs: 1_000,
    ...regionOverrides,
  }]),
  pylonStation: new FakeTable([{
    pylonRef: "pylon.public.1",
    runRef,
    regionRef,
    label: "Public Pylon",
    positionX: 1,
    positionY: 0,
    positionZ: 2,
  }]),
  agentAvatar: new FakeTable([{
    avatarRef: "avatar.public.1",
    actorRef: "agent.public.1",
    actorKind: "pylon_agent",
    displayName: "Agent One",
  }]),
  avatarPosition: new FakeTable([{
    avatarRef: "avatar.public.1",
    regionRef,
    positionX: 3,
    positionY: 0,
    positionZ: 4,
    yaw: 0,
    movementMode: "walking",
    lastSeenEpochMs: 900,
  }]),
  pylonAttention: new FakeTable([]),
  localChatMessage: new FakeTable([{
    messageRef: "chat.public.1",
    speakerAvatarRef: "avatar.public.1",
    regionRef,
    body: "hello nearby",
    radiusMeters: 8,
    expiresAtEpochMs: 2_000,
  }]),
  chatBubble: new FakeTable([]),
  localEmote: new FakeTable([]),
  agentIntent: new FakeTable([]),
})

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

describe("subscribeSpacetimeWorld", () => {
  test("noop when CHAT_WORLD_MULTIPLAYER is off", () => {
    let connected = 0
    const stop = subscribeSpacetimeWorld(() => {}, {
      flags: { CHAT_WORLD_MULTIPLAYER: false },
      connect: () => {
        connected += 1
        return {}
      },
    })

    stop()
    expect(connected).toBe(0)
    expect(
      publishActiveVerseLocalPose({
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "idle",
        capturedAtMs: 1_000,
      }),
    ).toMatchObject({
      ok: false,
      reason: "multiplayer client unavailable",
    })
  })

  test("subscribes to public world rows, dispatches a projection, and joins region", () => {
    const worlds: ChatWorldMultiplayerProjection[] = []
    const joins: unknown[] = []
    const leaves: unknown[] = []
    const rows = fakeWorldRows()
    let applied: (() => void) | null = null
    let capturedQueries: ReadonlyArray<string> = []
    let disconnected = false
    let unsubscribed = false
    let tokenRead: string | null = null
    let tokenWritten: string | null = null

    const stop = subscribeSpacetimeWorld((world) => worlds.push(world), {
      flags: { CHAT_WORLD_MULTIPLAYER: true },
      runRef,
      nowMs: () => 1_000,
      identity: { pylonRef: "pylon.public.1", nodeLabel: "Local Pylon" },
      storage: {
        getItem: (key) => {
          tokenRead = key
          return "stored-token"
        },
        setItem: (_key, value) => {
          tokenWritten = value
        },
      },
      connect: (input) => {
        input.onConnected("fresh-token")
        const builder = {
          onApplied: (cb: () => void) => {
            applied = cb
            return builder
          },
          onError: () => builder,
          subscribe: (queries: ReadonlyArray<string>) => {
            capturedQueries = queries
            return {
              unsubscribe: () => {
                unsubscribed = true
              },
            }
          },
        }
        return {
          db: rows,
          reducers: {
            joinRegion: (args: unknown) => {
              joins.push(args)
            },
            leaveRegion: (args: unknown) => {
              leaves.push(args)
            },
          },
          subscriptionBuilder: () => builder,
          disconnect: () => {
            disconnected = true
          },
        }
      },
    })

    expect(tokenRead).toBe("openagents.world.spacetimedb.token.v1")
    expect(tokenWritten).toBe("fresh-token")
    expect(capturedQueries).toContain(
      `SELECT * FROM world_region WHERE region_ref = '${regionRef}'`,
    )

    applied?.()

    expect(worlds).toHaveLength(1)
    expect(worlds[0]!.connected).toBe(true)
    expect(worlds[0]!.stations[0]?.pylonRef).toBe("pylon.public.1")
    expect(worlds[0]!.agents[0]?.chatMessages).toEqual(["hello nearby"])
    expect(joins).toEqual([{
      regionRef,
      displayName: "Local Pylon",
    }])

    rows.avatarPosition.insert({
      avatarRef: "avatar.public.1",
      regionRef,
      positionX: 4,
      positionY: 0,
      positionZ: 5,
      yaw: 0,
      movementMode: "running",
      lastSeenEpochMs: 1_000,
    })
    expect(worlds.length).toBeGreaterThanOrEqual(2)

    stop()
    expect(leaves).toEqual([{ regionRef }])
    expect(unsubscribed).toBe(true)
    expect(disconnected).toBe(true)
  })

  test("dispatches disconnected fallback and removes stale token when connect fails", () => {
    const worlds: ChatWorldMultiplayerProjection[] = []
    let removed: string | null = null
    let scheduled = false

    const stop = subscribeSpacetimeWorld((world) => worlds.push(world), {
      flags: { CHAT_WORLD_MULTIPLAYER: true },
      runRef,
      nowMs: () => 1_000,
      maxReconnectAttempts: 0,
      storage: {
        getItem: () => "stale-token",
        setItem: () => {},
        removeItem: (key) => {
          removed = key
        },
      },
      setTimeout: () => {
        scheduled = true
        return 0
      },
      clearTimeout: () => {},
      connect: () => {
        throw new Error("offline")
      },
    })

    stop()
    expect(worlds).toHaveLength(1)
    expect(worlds[0]!.connected).toBe(false)
    expect(removed).toBe("openagents.world.spacetimedb.token.v1")
    expect(scheduled).toBe(false)
  })

  test("publishes local avatar position only after a safe movement plan", () => {
    const writes: unknown[] = []
    const connection = {
      reducers: {
        setAvatarPosition: (args: unknown) => {
          writes.push(args)
        },
      },
    }

    expect(
      publishSpacetimeAvatarPosition(connection, {
        ok: false,
        reason: "position outside region bounds",
      }),
    ).toBe(false)
    expect(writes).toEqual([])

    expect(
      publishSpacetimeAvatarPosition(connection, {
        ok: true,
        write: {
          regionRef,
          positionX: 1,
          positionY: 0,
          positionZ: 2,
          yaw: 0,
          pitch: 0,
          movementMode: "walking",
        },
      }),
    ).toBe(true)
    expect(writes).toEqual([{
      regionRef,
      positionX: 1,
      positionY: 0,
      positionZ: 2,
      yaw: 0,
      pitch: 0,
      movementMode: "walking",
    }])
  })

  test("publishes Verse controller poses through the multiplayer client", () => {
    const writes: unknown[] = []
    const joins: unknown[] = []
    const rows = fakeWorldRows({ avatarPositionMinIntervalMs: 100 })
    const client = createVerseMultiplayerClient({
      connection: {
        db: rows,
        reducers: {
          joinRegion: (args: unknown) => joins.push(args),
          setAvatarPosition: (args: unknown) => writes.push(args),
        },
      },
      database: "openagents-world",
      displayName: "Local Pylon",
      runRef,
      nowMs: () => 2_000,
      worldUrl: "https://spacetime.openagents.com",
    })

    client.joinRegion()
    const plan = client.publishLocalPose({
      regionRef,
      x: 1.23456,
      y: 0,
      z: 2.34567,
      yaw: 0.12345,
      animation: "walk",
      capturedAtMs: 2_000,
    })

    expect(joins).toEqual([{ regionRef, displayName: "Local Pylon" }])
    expect(plan).toMatchObject({ ok: true })
    expect(writes).toEqual([{
      regionRef,
      positionX: 1.235,
      positionY: 0,
      positionZ: 2.346,
      yaw: 0.123,
      pitch: 0,
      movementMode: "walking",
    }])
  })

  test("suppresses unsafe Verse pose writes before reducer calls", () => {
    const writes: unknown[] = []
    const client = createVerseMultiplayerClient({
      connection: {
        db: fakeWorldRows({ avatarPositionMinIntervalMs: 100 }),
        reducers: {
          setAvatarPosition: (args: unknown) => writes.push(args),
        },
      },
      database: "openagents-world",
      displayName: "Local Pylon",
      runRef,
      nowMs: () => 1_000,
      worldUrl: "https://spacetime.openagents.com",
    })
    client.joinRegion()

    expect(
      client.publishLocalPose({
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "run",
        capturedAtMs: 1_000,
      }),
    ).toMatchObject({ ok: true })

    expect(
      client.publishLocalPose({
        regionRef,
        x: 99,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "run",
        capturedAtMs: 2_000,
      }),
    ).toMatchObject({ ok: false, reason: "position outside region bounds" })

    expect(
      client.publishLocalPose({
        regionRef,
        x: 1,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "run",
        capturedAtMs: 1_050,
      }),
    ).toMatchObject({ ok: false, reason: "position update rate limited" })

    expect(
      client.publishLocalPose({
        regionRef,
        x: 10,
        y: 8,
        z: 10,
        yaw: 0,
        animation: "run",
        capturedAtMs: 2_000,
      }),
    ).toMatchObject({
      ok: false,
      reason: "position jump exceeds movement limit",
    })

    expect(writes).toHaveLength(1)
  })

  test("rate-limits stationary idle pose keepalives", () => {
    const writes: unknown[] = []
    const client = createVerseMultiplayerClient({
      connection: {
        db: fakeWorldRows({ avatarPositionMinIntervalMs: 100 }),
        reducers: {
          setAvatarPosition: (args: unknown) => writes.push(args),
        },
      },
      database: "openagents-world",
      displayName: "Local Pylon",
      runRef,
      nowMs: () => 1_000,
      worldUrl: "https://spacetime.openagents.com",
    })
    client.joinRegion()

    expect(
      client.publishLocalPose({
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "idle",
        capturedAtMs: 1_000,
      }),
    ).toMatchObject({ ok: true })
    expect(
      client.publishLocalPose({
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "idle",
        capturedAtMs: 2_000,
      }),
    ).toMatchObject({
      ok: false,
      reason: "idle pose keepalive rate limited",
    })
    expect(
      client.publishLocalPose({
        regionRef,
        x: 0,
        y: 0,
        z: 0,
        yaw: 0,
        animation: "idle",
        capturedAtMs: 7_000,
      }),
    ).toMatchObject({ ok: true })
    expect(writes).toHaveLength(2)
  })
})
