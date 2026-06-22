import { describe, expect, test } from "bun:test"

import {
  createVerseMultiplayerClient,
  parseActivityStreamData,
  publishActiveVerseLocalPose,
  publishCloudflareAvatarPosition,
  subscribePaymentParticles,
  subscribePylonScene,
  subscribeCloudflareWorld,
} from "../src/ui/chat-world-subscriptions"
import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../src/shared/chat-world-scene"
import {
  chatWorldDesktopAvatarRef,
  chatWorldRegionRefForRun,
  type ChatWorldMultiplayerProjection,
} from "../src/shared/chat-world-multiplayer"
import { chatWorldMultiplayerLayer } from "../src/shared/chat-world-visualization"

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

  upsertBy(key: string, row: Record<string, unknown>): void {
    const index = this.rows.findIndex((candidate) =>
      candidate !== null &&
      typeof candidate === "object" &&
      (candidate as Record<string, unknown>)[key] === row[key],
    )
    if (index === -1) {
      this.insert(row)
      return
    }

    const previous = this.rows[index]
    this.rows[index] = row
    for (const cb of this.updates) cb(previous, row)
  }

  deleteBy(key: string, value: unknown): void {
    const index = this.rows.findIndex((candidate) =>
      candidate !== null &&
      typeof candidate === "object" &&
      (candidate as Record<string, unknown>)[key] === value,
    )
    if (index === -1) return
    const [removed] = this.rows.splice(index, 1)
    for (const cb of this.deletes) cb(removed)
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
  avatarPositionNear: new FakeTable([]),
  avatarPositionFar: new FakeTable([]),
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

describe("subscribeCloudflareWorld", () => {
  test("noop when CHAT_WORLD_MULTIPLAYER is off", () => {
    let connected = 0
    const stop = subscribeCloudflareWorld(() => {}, {
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

    const stop = subscribeCloudflareWorld((world) => worlds.push(world), {
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

    expect(tokenRead).toBe("openagents.world.cloudflare.session.v1")
    expect(tokenWritten).toBe("fresh-token")
    expect(capturedQueries).toContain(
      `SELECT * FROM world_region WHERE region_ref = '${regionRef}'`,
    )
    expect(capturedQueries).toContain(
      `SELECT agent_avatar.* FROM avatar_position JOIN agent_avatar ON avatar_position.avatar_ref = agent_avatar.avatar_ref WHERE avatar_position.region_ref = '${regionRef}'`,
    )
    expect(capturedQueries).toContain(
      `SELECT pylon_attention.* FROM pylon_station JOIN pylon_attention ON pylon_station.pylon_ref = pylon_attention.pylon_ref WHERE pylon_station.region_ref = '${regionRef}'`,
    )
    expect(capturedQueries).not.toContain("SELECT * FROM agent_avatar")
    expect(capturedQueries).not.toContain("SELECT * FROM pylon_attention")
    expect(capturedQueries).not.toContain("SELECT * FROM chat_bubble")
    expect(capturedQueries).not.toContain("SELECT * FROM agent_intent")

    applied?.()

    expect(worlds).toHaveLength(1)
    expect(worlds[0]!.connected).toBe(true)
    expect(worlds[0]!.stations[0]?.pylonRef).toBe("pylon.public.1")
    expect(worlds[0]!.agents[0]?.chatMessages).toEqual(["hello nearby"])
    expect(joins).toEqual([{
      regionRef,
      displayName: "Local Pylon",
      characterId: "main",
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
    expect(leaves).toEqual([{ regionRef, characterId: "main" }])
    expect(unsubscribed).toBe(true)
    expect(disconnected).toBe(true)
  })

  test("resolves the characterId LAZILY at join time, so a late OA_CHARACTER injection wins", () => {
    // Regression for the webview-plumbing bug: the Bun host injects
    // globalThis.__OA_CHARACTER (read by chatWorldCharacterId), and that
    // injection may land AFTER the subscription mounts. The subscription must
    // therefore read the character via a getter at join/move time, not capture
    // it once at construction. Here we pass a getter, then set the value AFTER
    // constructing the subscription but BEFORE applied()/join fires, and assert
    // the join (and the self-filter key) use the late value, not a stale capture.
    const joins: unknown[] = []
    const leaves: unknown[] = []
    const rows = fakeWorldRows()
    let applied: (() => void) | null = null
    const worlds: ChatWorldMultiplayerProjection[] = []
    const identityHex = "1122334455667788990011223344556677"

    let liveCharacter: string | null = "main"
    const stop = subscribeCloudflareWorld((world) => worlds.push(world), {
      flags: { CHAT_WORLD_MULTIPLAYER: true },
      runRef,
      nowMs: () => 1_000,
      // Lazy getter, exactly like the real wiring (() => chatWorldCharacterId()).
      characterId: () => liveCharacter,
      storage: { getItem: () => null, setItem: () => {} },
      connect: (input) => {
        input.onConnected("tok", identityHex)
        const builder = {
          onApplied: (cb: () => void) => {
            applied = cb
            return builder
          },
          onError: () => builder,
          subscribe: () => ({ unsubscribe: () => {} }),
        }
        return {
          db: rows,
          reducers: {
            joinRegion: (args: unknown) => joins.push(args),
            leaveRegion: (args: unknown) => leaves.push(args),
          },
          subscriptionBuilder: () => builder,
          disconnect: () => {},
        }
      },
    })

    // The injection lands AFTER mount, BEFORE join. A stale eager capture would
    // still say "main"; the lazy getter must observe "alt".
    liveCharacter = "alt"
    applied?.()

    expect(joins).toEqual([{
      regionRef,
      displayName: "Autopilot Desktop",
      characterId: "alt",
    }])
    // The self-filter key for this instance must use the SAME late value.
    expect(worlds.at(-1)!.localAvatarRef).toBe(
      chatWorldDesktopAvatarRef(identityHex, "alt"),
    )

    stop()
    expect(leaves).toEqual([{ regionRef, characterId: "alt" }])
  })

  test("two distinct resolved character ids produce two distinct avatar / self-filter keys", () => {
    // Two instances on the SAME account but different OA_CHARACTER values must
    // become two distinct, mutually-visible avatars (distinct self-filter keys).
    const identityHex = "00ff112233445566778899aabbccddee"
    const mainRef = chatWorldDesktopAvatarRef(identityHex, "main")
    const altRef = chatWorldDesktopAvatarRef(identityHex, "alt")
    expect(mainRef).not.toBe(altRef)

    const keyFor = (character: string): string | null => {
      const rows = fakeWorldRows()
      let applied: (() => void) | null = null
      const worlds: ChatWorldMultiplayerProjection[] = []
      const stop = subscribeCloudflareWorld((world) => worlds.push(world), {
        flags: { CHAT_WORLD_MULTIPLAYER: true },
        runRef,
        nowMs: () => 1_000,
        characterId: character,
        storage: { getItem: () => null, setItem: () => {} },
        connect: (input) => {
          input.onConnected("tok", identityHex)
          const builder = {
            onApplied: (cb: () => void) => {
              applied = cb
              return builder
            },
            onError: () => builder,
            subscribe: () => ({ unsubscribe: () => {} }),
          }
          return {
            db: rows,
            reducers: { joinRegion: () => {}, leaveRegion: () => {} },
            subscriptionBuilder: () => builder,
            disconnect: () => {},
          }
        },
      })
      applied?.()
      const key = worlds.at(-1)!.localAvatarRef
      stop()
      return key
    }

    expect(keyFor("main")).toBe(mainRef)
    expect(keyFor("alt")).toBe(altRef)
    expect(keyFor("main")).not.toBe(keyFor("alt"))
  })

  test("self-filters only the local character once the identity is known, rendering other characters of the same account", () => {
    // MMO model: one account (identity) fields many characters. This instance
    // is OA_CHARACTER=main; the SAME account also runs OA_CHARACTER=alt in a
    // second instance. Both characters' avatars exist in the world. The scene
    // must hide ONLY this instance's own character (main) and render every other
    // avatar — including the same account's "alt" character and unrelated
    // remote avatars.
    const identityHex = "00ff112233445566778899aabbccddee"
    const localRef = chatWorldDesktopAvatarRef(identityHex, "main")
    const sameAccountAltRef = chatWorldDesktopAvatarRef(identityHex, "alt")
    const remoteRef = chatWorldDesktopAvatarRef("99887766554433221100", "main")

    const rows = fakeWorldRows()
    rows.agentAvatar = new FakeTable([
      { avatarRef: localRef, actorRef: "a", actorKind: "guest", displayName: "Me (main)" },
      { avatarRef: sameAccountAltRef, actorRef: "b", actorKind: "guest", displayName: "Me (alt)" },
      { avatarRef: remoteRef, actorRef: "c", actorKind: "guest", displayName: "Someone else" },
    ])
    rows.avatarPosition = new FakeTable([
      { avatarRef: localRef, regionRef, positionX: 0, positionY: 0, positionZ: 0, yaw: 0, movementMode: "idle", lastSeenEpochMs: 1_000 },
      { avatarRef: sameAccountAltRef, regionRef, positionX: 2, positionY: 0, positionZ: 2, yaw: 0, movementMode: "idle", lastSeenEpochMs: 1_000 },
      { avatarRef: remoteRef, regionRef, positionX: 4, positionY: 0, positionZ: 4, yaw: 0, movementMode: "idle", lastSeenEpochMs: 1_000 },
    ])

    const worlds: ChatWorldMultiplayerProjection[] = []
    let applied: (() => void) | null = null

    const stop = subscribeCloudflareWorld((world) => worlds.push(world), {
      flags: { CHAT_WORLD_MULTIPLAYER: true },
      runRef,
      nowMs: () => 1_000,
      characterId: "main",
      storage: { getItem: () => null, setItem: () => {} },
      connect: (input) => {
        // onConnect yields BOTH the token and the live account identity.
        input.onConnected("tok", identityHex)
        const builder = {
          onApplied: (cb: () => void) => {
            applied = cb
            return builder
          },
          onError: () => builder,
          subscribe: () => ({ unsubscribe: () => {} }),
        }
        return {
          db: rows,
          reducers: { joinRegion: () => {}, leaveRegion: () => {} },
          subscriptionBuilder: () => builder,
          disconnect: () => {},
        }
      },
    })

    applied?.()
    const latest = worlds.at(-1)!
    // Projection carries this instance's own per-character key.
    expect(latest.localAvatarRef).toBe(localRef)

    // The scene self-filters on that key: local character hidden, the same
    // account's other character and the unrelated remote both rendered.
    const layer = chatWorldMultiplayerLayer(latest, {
      localAvatarRef: latest.localAvatarRef,
      nowMs: 1_000,
    })
    const rendered = layer.remoteAvatars.map((avatar) => avatar.id)
    expect(rendered).not.toContain(localRef)
    expect(rendered).toContain(sameAccountAltRef)
    expect(rendered).toContain(remoteRef)

    stop()
  })

  test("dispatches disconnected fallback and removes stale token when connect fails", () => {
    const worlds: ChatWorldMultiplayerProjection[] = []
    let removed: string | null = null
    let scheduled = false

    const stop = subscribeCloudflareWorld((world) => worlds.push(world), {
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
    expect(worlds[0]!.stations).toEqual([])
    expect(worlds[0]!.agents).toEqual([])
    expect(
      publishActiveVerseLocalPose({
        regionRef,
        x: 1,
        y: 0,
        z: 2,
        yaw: 0,
        animation: "walk",
        capturedAtMs: 1_100,
      }),
    ).toMatchObject({
      ok: false,
      reason: "multiplayer client unavailable",
    })
    expect(removed).toBe("openagents.world.cloudflare.session.v1")
    expect(scheduled).toBe(false)
  })

  test("two-client smoke joins one region, propagates movement, and filters the local duplicate", () => {
    const rows = fakeWorldRows({ avatarPositionMinIntervalMs: 100 })
    rows.agentAvatar = new FakeTable([])
    rows.avatarPosition = new FakeTable([])
    rows.avatarPositionNear = new FakeTable([])
    rows.avatarPositionFar = new FakeTable([])

    let nowMs = 1_000
    const joins: Array<{
      avatarRef: string
      displayName: string
      regionRef: string
    }> = []
    const capturedQueries: string[][] = []

    const upsertAvatar = (avatarRef: string, displayName: string): void => {
      rows.agentAvatar.upsertBy("avatarRef", {
        avatarRef,
        actorRef: `identity.${avatarRef}`,
        actorKind: "guest",
        displayName,
      })
    }

    const upsertPosition = (
      avatarRef: string,
      write: {
        readonly movementMode: string
        readonly positionX: number
        readonly positionY: number
        readonly positionZ: number
        readonly yaw: number
      },
    ): void => {
      rows.avatarPosition.upsertBy("avatarRef", {
        avatarRef,
        regionRef,
        positionX: write.positionX,
        positionY: write.positionY,
        positionZ: write.positionZ,
        yaw: write.yaw,
        movementMode: write.movementMode,
        lastSeenEpochMs: nowMs,
      })
    }

    const connectionFor = (avatarRef: string, fallbackDisplayName: string) => {
      const builder = {
        onApplied: (cb: () => void) => {
          builder.applied = cb
          return builder
        },
        onError: () => builder,
        subscribe: (queries: ReadonlyArray<string>) => {
          capturedQueries.push([...queries])
          builder.applied?.()
          return { unsubscribe: () => {} }
        },
        applied: null as (() => void) | null,
      }

      return {
        db: rows,
        reducers: {
          joinRegion: (args: { readonly displayName: string; readonly regionRef: string }) => {
            const displayName = args.displayName || fallbackDisplayName
            joins.push({ avatarRef, displayName, regionRef: args.regionRef })
            upsertAvatar(avatarRef, displayName)
            rows.avatarPosition.upsertBy("avatarRef", {
              avatarRef,
              regionRef: args.regionRef,
              positionX: 0,
              positionY: 0,
              positionZ: 0,
              yaw: 0,
              movementMode: "idle",
              lastSeenEpochMs: nowMs,
            })
          },
          leaveRegion: (args: { readonly regionRef: string }) => {
            if (args.regionRef === regionRef) rows.avatarPosition.deleteBy("avatarRef", avatarRef)
          },
          setAvatarPosition: (args: {
            readonly movementMode: string
            readonly positionX: number
            readonly positionY: number
            readonly positionZ: number
            readonly yaw: number
          }) => upsertPosition(avatarRef, args),
        },
        subscriptionBuilder: () => builder,
      }
    }

    const senderAvatarRef = "avatar.identity.sender"
    const receiverAvatarRef = "avatar.identity.receiver"
    const sender = createVerseMultiplayerClient({
      connection: connectionFor(senderAvatarRef, "Sender"),
      database: "openagents-world",
      displayName: "Sender",
      runRef,
      nowMs: () => nowMs,
      worldUrl: "https://openagents-world.openagents.workers.dev",
    })
    sender.joinRegion()

    const receiverWorlds: ChatWorldMultiplayerProjection[] = []
    const stopReceiver = subscribeCloudflareWorld(
      (world) => receiverWorlds.push(world),
      {
        flags: { CHAT_WORLD_MULTIPLAYER: true },
        identity: { fallbackActorRef: receiverAvatarRef, nodeLabel: "Receiver" },
        maxReconnectAttempts: 0,
        nowMs: () => nowMs,
        runRef,
        connect: () => connectionFor(receiverAvatarRef, "Receiver"),
      },
    )

    expect(capturedQueries[0]).toContain(
      `SELECT * FROM avatar_position WHERE avatar_position.region_ref = '${regionRef}'`,
    )
    expect(joins).toEqual([
      { avatarRef: senderAvatarRef, displayName: "Sender", regionRef },
      { avatarRef: receiverAvatarRef, displayName: "Receiver", regionRef },
    ])

    nowMs = 2_000
    expect(
      sender.publishLocalPose({
        regionRef,
        x: 3,
        y: 0,
        z: 4,
        yaw: 0.5,
        animation: "run",
        capturedAtMs: nowMs,
      }),
    ).toMatchObject({ ok: true })

    const receiverProjection = receiverWorlds.at(-1)!
    const movedSender = receiverProjection.agents.find(
      agent => agent.avatarRef === senderAvatarRef,
    )
    const localReceiver = receiverProjection.agents.find(
      agent => agent.avatarRef === receiverAvatarRef,
    )

    expect(movedSender).toMatchObject({
      avatarRef: senderAvatarRef,
      label: "Sender",
      x: 3,
      y: 0,
      z: 4,
      movementMode: "running",
      presenceFeed: "high",
    })
    expect(localReceiver).toMatchObject({
      avatarRef: receiverAvatarRef,
      label: "Receiver",
    })
    expect(receiverProjection.projectedAtMs - movedSender!.lastSeenEpochMs).toBeLessThanOrEqual(1_000)

    const receiverLayer = chatWorldMultiplayerLayer(receiverProjection, {
      localAvatarRef: receiverAvatarRef,
      nowMs: receiverProjection.projectedAtMs,
    })
    expect(receiverLayer.remoteAvatars.map(avatar => avatar.id)).toEqual([senderAvatarRef])

    stopReceiver()
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
      publishCloudflareAvatarPosition(connection, {
        ok: false,
        reason: "position outside region bounds",
      }),
    ).toBe(false)
    expect(writes).toEqual([])

    expect(
      publishCloudflareAvatarPosition(connection, {
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
      worldUrl: "https://openagents-world.openagents.workers.dev",
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

    expect(joins).toEqual([{ regionRef, displayName: "Local Pylon", characterId: "main" }])
    expect(plan).toMatchObject({ ok: true })
    expect(writes).toEqual([{
      regionRef,
      positionX: 1.235,
      positionY: 0,
      positionZ: 2.346,
      yaw: 0.123,
      pitch: 0,
      movementMode: "walking",
      characterId: "main",
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
      worldUrl: "https://openagents-world.openagents.workers.dev",
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
      worldUrl: "https://openagents-world.openagents.workers.dev",
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
