import { describe, expect, test } from "bun:test"

import {
  forumMessagesByEntityRef,
  projectForumPylonMessages,
  withForumPylonMessages,
  type ChatWorldWorldEventRow,
} from "../src/shared/chat-world-forum-activity"
import {
  chatWorldRegionRefForRun,
  type ChatWorldMultiplayerProjection,
} from "../src/shared/chat-world-multiplayer"
import {
  liveChatWorldNetworkScene,
  chatWorldMultiplayerLayer,
  chatWorldPaymentLayer,
} from "../src/shared/chat-world-visualization"
import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../src/shared/chat-world-scene"
import type { NodeStateMessage } from "../src/shared/rpc"
import { initialRuntimeState } from "../src/ui/initial-state"
import { GotNodeState } from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import {
  createVerseMultiplayerClient,
  publishActiveVerseLocalPose,
  subscribePaymentParticles,
  subscribePylonScene,
  subscribeCloudflareWorld,
} from "../src/ui/chat-world-subscriptions"
import {
  FakeActivityEventSource,
  createFakeChatWorldConnection,
  createFakeChatWorldRows,
  jsonResponse,
  latestWorld,
} from "./harnesses/chat-world-integration-harness"

const runRef = "run.tassadar.executor.20260615"
const regionRef = chatWorldRegionRefForRun(runRef)

const flush = () => new Promise((resolve) => setTimeout(resolve, 0))

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const nodeWithBalance = (balanceSats: number): NodeStateMessage => ({
  ok: true,
  schema: "control.v1",
  sessions: [],
  wallet: {
    configured: true,
    daemonOnline: true,
    balanceSats,
    receiveReady: true,
    sendReady: false,
    readiness: "ready",
  },
})

const pylonStats = (settledSats: number) => ({
  available: true,
  status: "live",
  pylonsOnlineNow: 1,
  recentPylons: [{
    nostrPubkeyShort: "alpha",
    nodeLabel: "Alpha Pylon",
    onlineNow: true,
    walletReadyNow: true,
    assignmentReadyNow: true,
    lastHeartbeatAgeSeconds: 2,
    products: ["forum", "pylon"],
  }],
  nip90MarketSettlementStats: {
    compute: { satsSettledTotal: settledSats },
    data: { satsSettledTotal: 0 },
    labor: { satsSettledTotal: 0 },
  },
  asOfLabel: "moments ago",
})

const forumTipSettledWorldEvent = (): ChatWorldWorldEventRow => ({
  eventRef: "world_event.forum_tip_settled.post_1",
  runRef: "run.public_forum_activity",
  eventKind: "forum_tip_settled",
  entityRef: "agent:forum_author",
  sourceRef: "post_1",
  sourceGeneratedAt: "2026-06-21T18:02:00.000Z",
  summary: JSON.stringify({
    schema: "openagents.world.forum_activity_event_summary.v1",
    text: "Tipped: Forum author received 21 sats",
    topicRef: "topic_1",
  }),
})

describe("forum tipping + direct pylon tips + multiplayer integration harness", () => {
  test("exercises receipt-backed tips, forum activity, world updates, and sats HUD refresh together", async () => {
    FakeActivityEventSource.instances = []
    let nowMs = 1_000
    const rows = createFakeChatWorldRows({
      runRef,
      nowMs,
      stationOverrides: {
        pylonRef: "pylon.public.alpha",
        label: "Alpha Pylon",
        positionX: 6,
        positionY: 0,
        positionZ: -2,
      },
      avatarOverrides: {
        avatarRef: "avatar.forum.author",
        actorRef: "agent:forum_author",
        displayName: "Forum Author",
      },
      positionOverrides: {
        avatarRef: "avatar.forum.author",
        positionX: -4,
        positionY: 0,
        positionZ: 3,
        lastSeenEpochMs: nowMs,
      },
    })

    const publisher = createVerseMultiplayerClient({
      connection: createFakeChatWorldConnection({
        rows,
        avatarRef: "avatar.viewer.publisher",
        fallbackDisplayName: "Publisher",
        nowMs: () => nowMs,
      }),
      database: "openagents-world",
      displayName: "Publisher",
      runRef,
      nowMs: () => nowMs,
      worldUrl: "https://world.openagents.com",
    })
    publisher.joinRegion()

    const worlds: ChatWorldMultiplayerProjection[] = []
    const viewerConnection = createFakeChatWorldConnection({
      rows,
      avatarRef: "avatar.viewer.receiver",
      fallbackDisplayName: "Receiver",
      nowMs: () => nowMs,
    })
    const stopWorld = subscribeCloudflareWorld((world) => worlds.push(world), {
      flags: { CHAT_WORLD_MULTIPLAYER: true },
      identity: { fallbackActorRef: "avatar.viewer.receiver", nodeLabel: "Receiver" },
      maxReconnectAttempts: 0,
      nowMs: () => nowMs,
      runRef,
      connect: () => viewerConnection,
    })

    expect(viewerConnection.capturedQueries[0]).toContain(
      `SELECT * FROM world_event WHERE run_ref = 'run.public_forum_activity'`,
    )
    expect(latestWorld(worlds).agents.map((agent) => agent.avatarRef)).toContain(
      "avatar.forum.author",
    )

    nowMs = 2_000
    expect(
      publisher.publishLocalPose({
        regionRef,
        x: 3,
        y: 0,
        z: 4,
        yaw: 0.5,
        animation: "run",
        capturedAtMs: nowMs,
      }),
    ).toMatchObject({ ok: true })

    const receiverWorld = latestWorld(worlds)
    const receiverLayer = chatWorldMultiplayerLayer(receiverWorld, {
      localAvatarRef: "avatar.viewer.receiver",
      nowMs: receiverWorld.projectedAtMs,
    })
    expect(receiverLayer.remoteAvatars.map((avatar) => avatar.id)).toContain(
      "avatar.viewer.publisher",
    )

    const forumMessages = projectForumPylonMessages([forumTipSettledWorldEvent()])
    const forumAuthor = withForumPylonMessages(
      receiverWorld.agents,
      forumMessagesByEntityRef(forumMessages),
    ).find((agent) => agent.actorRef === "agent:forum_author")
    expect(forumAuthor?.forumMessage).toMatchObject({
      eventKind: "forum_tip_settled",
      sourceUrl: "https://openagents.com/forum/t/topic_1",
    })

    const particles: PaymentParticle[] = []
    const scenes: ChatWorldPylonScene[] = []
    let pylonPoll: (() => void) | null = null
    let statsFetches = 0
    const fetchFn = (async (url: RequestInfo | URL) => {
      const pathname = new URL(String(url)).pathname
      if (pathname === "/api/public/pylon-stats") {
        statsFetches += 1
        return jsonResponse(pylonStats(statsFetches === 1 ? 2_100 : 3_456))
      }
      if (pathname === "/api/public/activity-timeline") {
        return jsonResponse({
          events: [{
            eventRef: "tip.forum.post_1",
            kind: "real_bitcoin_moved",
            actorRef: "agent:tipper",
            targetRef: "agent:forum_author",
            amountSats: 21,
            realBitcoinMoved: true,
            sourceRefs: ["receipt.forum.post_1.bitcoin.21"],
            ts: "2026-06-21T18:02:00.000Z",
            text: "Forum post tip",
          }],
        })
      }
      return jsonResponse({}, false)
    }) as unknown as typeof fetch

    const stopPylons = subscribePylonScene((scene) => scenes.push(scene), {
      flags: { CHAT_WORLD_SCENE: true },
      fetchFn,
      setInterval: (handler) => {
        pylonPoll = handler
        return "pylon-poll"
      },
      clearInterval: () => {},
    })
    const stopPayments = subscribePaymentParticles((particle) => particles.push(particle), {
      flags: { CHAT_WORLD_PAYMENTS: true },
      eventSourceCtor: FakeActivityEventSource as unknown as typeof EventSource,
      fetchFn,
      setInterval: () => "payment-poll",
      clearInterval: () => {},
    })

    await flush()
    expect(scenes.at(-1)?.growth.settledSats).toBe(2_100)
    expect(liveChatWorldNetworkScene(scenes.at(-1)!)?.satsSettledTotal).toBe(2_100)
    expect(particles.map((particle) => particle.id)).toContain("tip.forum.post_1")

    FakeActivityEventSource.instances[0]?.emit("real_bitcoin_moved", {
      event: {
        eventRef: "tip.pylon.alpha",
        kind: "real_bitcoin_moved",
        actorRef: "agent:tipper",
        targetRef: "pylon.public.alpha",
        amountSats: 1_356,
        realBitcoinMoved: true,
        sourceRefs: ["receipt.pylon.alpha.bitcoin.1356"],
        ts: "2026-06-21T18:02:05.000Z",
        text: "Direct pylon tip",
      },
    })

    expect(particles.map((particle) => particle.id)).toEqual([
      "tip.forum.post_1",
      "tip.pylon.alpha",
    ])

    const paymentLayer = chatWorldPaymentLayer(particles, latestWorld(worlds))
    expect(paymentLayer.entities.map((entity) => entity.id)).toContain(
      "pay:tip.forum.post_1:to",
    )
    expect(paymentLayer.entities.map((entity) => entity.id)).toContain(
      "pay:tip.pylon.alpha:to",
    )
    const forumTipTarget = paymentLayer.entities.find(
      (entity) => entity.id === "pay:tip.forum.post_1:to",
    )
    const pylonTipTarget = paymentLayer.entities.find(
      (entity) => entity.id === "pay:tip.pylon.alpha:to",
    )
    expect(forumTipTarget).toBeDefined()
    expect(pylonTipTarget).toBeDefined()
    expect(forumTipTarget?.position).toEqual([-4, 0, 3])
    expect(forumTipTarget?.label).toBe("Forum Author")
    expect(String(forumTipTarget?.label).includes("receipt.forum.post_1")).toBe(false)
    expect(String(forumTipTarget?.detail).includes("receipt.forum.post_1.bitcoin.21")).toBe(true)
    expect(String(forumTipTarget?.detail).includes("avatar")).toBe(true)
    expect(pylonTipTarget?.position).toEqual([6, 0, -2])
    expect(pylonTipTarget?.label).toBe("Alpha Pylon")
    expect(String(pylonTipTarget?.label).includes("receipt.pylon.alpha.bitcoin.1356")).toBe(false)
    expect(String(pylonTipTarget?.detail).includes("receipt.pylon.alpha.bitcoin.1356")).toBe(true)
    expect(String(pylonTipTarget?.detail).includes("station")).toBe(true)
    expect(paymentLayer.beams.every((beam) => beam.motionKind === "real_bitcoin_moved")).toBe(true)
    expect(paymentLayer.beams.flatMap((beam) => beam.sourceRefs)).toEqual([
      "receipt.forum.post_1.bitcoin.21",
      "receipt.pylon.alpha.bitcoin.1356",
    ])

    pylonPoll?.()
    await flush()
    expect(scenes.at(-1)?.growth.settledSats).toBe(3_456)
    expect(liveChatWorldNetworkScene(scenes.at(-1)!)?.satsSettledTotal).toBe(3_456)

    const [initial] = initialRuntimeState()
    const [funded] = update(initial, GotNodeState({ node: nodeWithBalance(2_100) }))
    const [refreshed] = update(funded, GotNodeState({ node: nodeWithBalance(3_456) }))
    const fundedTree = serializeView(view(funded).body)
    const refreshedTree = serializeView(view(refreshed).body)
    expect(fundedTree).toContain("pylon-balance-hud")
    expect(fundedTree).toContain("2,100 sats")
    expect(refreshedTree).toContain("3,456 sats")
    expect(refreshedTree).not.toContain("2,100 sats")

    expect(
      publishActiveVerseLocalPose({
        regionRef,
        x: 1,
        y: 0,
        z: 2,
        yaw: 0,
        animation: "walk",
        capturedAtMs: 3_000,
      }),
    ).toMatchObject({ ok: true })

    stopPayments()
    stopPylons()
    stopWorld()
    expect(FakeActivityEventSource.instances[0]?.closed).toBe(true)
  })
})
