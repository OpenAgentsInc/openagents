import { describe, expect, test } from "bun:test"

import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../src/shared/chat-world-scene"
import type { ChatWorldMultiplayerProjection } from "../src/shared/chat-world-multiplayer"
import {
  PAYMENT_PARTICLE_DIM,
  PAYMENT_PARTICLE_GOLD,
  pylonGrowthTier,
} from "../src/shared/chat-world-scene"
import {
  CHAT_WORLD_STATION_NODE_PREFIX,
  chatWorldMultiplayerLayer,
  chatWorldPaymentEndpointIndex,
  chatWorldPaymentLayer,
  chatWorldVisibleTargetCandidates,
  liveChatWorldNetworkScene,
  resolveChatWorldPaymentEndpoint,
  withChatWorldMultiplayerLayer,
  withChatWorldPaymentLayer,
} from "../src/shared/chat-world-visualization"
import { pylonNetworkVisualizationOptions } from "../src/ui/pylon-network-visualization"

// ── P1: live pylon scene → bezier-graph PylonNetworkScene ─────────────────────

const liveScene = (
  overrides: Partial<ChatWorldPylonScene> = {},
): ChatWorldPylonScene => ({
  empty: false,
  onlineNow: 2,
  nodes: [
    {
      id: "abc123",
      label: "alpha",
      state: "assignment_ready",
      color: 0x4ade80,
      online: true,
      pulseSpeed: 1.8,
      products: [],
    },
    {
      id: "def456",
      label: "bravo",
      state: "wallet_ready",
      color: 0x7dd3fc,
      online: true,
      pulseSpeed: 0.4,
      products: [],
    },
  ],
  growth: pylonGrowthTier(50_000),
  asOfLabel: "moments ago",
  ...overrides,
})

describe("liveChatWorldNetworkScene", () => {
  test("null / empty / zero-node live scenes fall back (return null)", () => {
    expect(liveChatWorldNetworkScene(null)).toBeNull()
    expect(
      liveChatWorldNetworkScene(liveScene({ empty: true, nodes: [] })),
    ).toBeNull()
    expect(liveChatWorldNetworkScene(liveScene({ nodes: [] }))).toBeNull()
  })

  test("maps live nodes onto the three-tone graph (working/online/offline)", () => {
    const scene = liveChatWorldNetworkScene(liveScene())
    expect(scene).not.toBeNull()
    const options = pylonNetworkVisualizationOptions(scene!)
    const nodes = scene!.nodes
    expect(nodes.map((n) => n.id)).toEqual(["abc123", "def456"])
    // assignment_ready → working (earning), wallet_ready (online, not earning) → online
    expect(nodes[0]!.tone).toBe("working")
    expect(nodes[0]!.flowing).toBe(true)
    expect(nodes[1]!.tone).toBe("online")
    expect(nodes[1]!.flowing).toBe(false)
    expect(scene!.onlineNow).toBe(2)
    expect(scene!.sessionsOnlineNow).toBe(1)
    // cumulative settled sats ride through as the growth signal
    expect(scene!.satsSettledTotal).toBe(50_000)
    expect(options.worldLabelDensity).toBe("compact")
    expect(options.nodes?.find(node => node.id === "network")?.position?.[2]).toBeGreaterThan(0)
    expect(options.nodes?.filter(node => node.id !== "network").every(node => (node.position?.[2] ?? 0) !== 0)).toBe(true)
  })

  test("activity intensity is bounded [0,1]", () => {
    const scene = liveChatWorldNetworkScene(liveScene())!
    expect(scene.activityIntensity).toBeGreaterThanOrEqual(0)
    expect(scene.activityIntensity).toBeLessThanOrEqual(1)
  })
})

// ── P2: payment particles → evidence-bound beams/bursts/entities ──────────────

const particle = (overrides: Partial<PaymentParticle> = {}): PaymentParticle => ({
  id: "evt-1",
  fromRef: "pylon:alpha",
  toRef: "pylon:bravo",
  amountSats: 21_000,
  realBitcoinMoved: true,
  color: PAYMENT_PARTICLE_GOLD,
  size: 0.7,
  sourceRefs: ["receipt:nip90:abc"],
  ts: "2026-06-20T00:00:00.000Z",
  text: null,
  ...overrides,
})

const worldProjection = (
  overrides: Partial<ChatWorldMultiplayerProjection> = {},
): ChatWorldMultiplayerProjection => ({
  connected: true,
  database: "openagents-world",
  worldUrl: "https://spacetime.openagents.com",
  regionRef: "region.run.public",
  projectedAtMs: 10_000,
  stations: [{
    pylonRef: "pylon.alpha",
    label: "Alpha Pylon",
    x: 1.25,
    y: 0.5,
    z: -2,
  }],
  agents: [{
    avatarRef: "avatar.bravo",
    actorRef: "agent.bravo",
    avatarKind: "tassadar",
    label: "Tassadar",
    color: "#f5b73a",
    x: -3,
    y: 1,
    z: 2.75,
    yaw: 0,
    movementMode: "walk",
    lastSeenEpochMs: 9_500,
    presenceFeed: "high",
    chatMessages: [],
    attentionRefs: [],
  }],
  proximityChatCount: 0,
  ...overrides,
})

describe("chatWorldPaymentLayer (evidence-bound motion)", () => {
  test("each particle yields a beam + burst + two clickable endpoints", () => {
    const layer = chatWorldPaymentLayer([particle()])
    expect(layer.beams).toHaveLength(1)
    expect(layer.bursts).toHaveLength(1)
    expect(layer.entities).toHaveLength(2)
    const beam = layer.beams[0]!
    expect(beam.fromId).toBe("pay:evt-1:from")
    expect(beam.toId).toBe("pay:evt-1:to")
    // gold real-bitcoin motion is tagged real_bitcoin_moved; dim is settlement
    expect(beam.motionKind).toBe("real_bitcoin_moved")
    expect(beam.sourceRefs).toEqual(["receipt:nip90:abc"])
    expect(beam.simulated).toBe(false)
    // Scene labels stay short; full receipt refs live in detail + beam evidence
    // for selection/inspector paths.
    const toEntity = layer.entities.find((e) => e.id === "pay:evt-1:to")!
    const fromEntity = layer.entities.find((e) => e.id === "pay:evt-1:from")!
    expect(fromEntity.label).toBe("Tip sender")
    expect(toEntity.label).toBe("Payment target")
    expect(fromEntity.detail).toContain("receipt:nip90:abc")
    expect(toEntity.detail).toContain("receipt:nip90:abc")
    expect(toEntity.detail).toContain("21000 sats")
    expect(fromEntity.position?.[2]).not.toBe(0)
    expect(toEntity.position?.[2]).not.toBe(0)
  })

  test("credited (non-bitcoin) particles tag settlement_recorded", () => {
    const layer = chatWorldPaymentLayer([
      particle({ realBitcoinMoved: false, color: PAYMENT_PARTICLE_DIM }),
    ])
    expect(layer.beams[0]!.motionKind).toBe("settlement_recorded")
  })

  test("refuses particles with no sourceRef (never fakes a receipt)", () => {
    const layer = chatWorldPaymentLayer([particle({ sourceRefs: [] })])
    expect(layer.beams).toHaveLength(0)
    expect(layer.bursts).toHaveLength(0)
    expect(layer.entities).toHaveLength(0)
  })

  test("indexes public world endpoints by station, actor, and avatar refs", () => {
    const endpoints = chatWorldPaymentEndpointIndex(worldProjection())
    expect(
      resolveChatWorldPaymentEndpoint("pylon:alpha", [9, 9, 9], endpoints),
    ).toMatchObject({
      label: "Alpha Pylon",
      position: [1.25, 0.5, -2],
      source: "station",
    })
    expect(
      resolveChatWorldPaymentEndpoint("agent.bravo", [9, 9, 9], endpoints),
    ).toMatchObject({
      label: "Tassadar",
      position: [-3, 1, 2.75],
      source: "avatar",
    })
    expect(
      resolveChatWorldPaymentEndpoint("avatar.bravo", [9, 9, 9], endpoints),
    ).toMatchObject({
      label: "Tassadar",
      position: [-3, 1, 2.75],
      source: "avatar",
    })
  })

  test("uses real station/avatar positions when public world endpoints exist", () => {
    const layer = chatWorldPaymentLayer([
      particle({ fromRef: "pylon:alpha", toRef: "agent.bravo" }),
    ], worldProjection())

    const fromEntity = layer.entities.find((e) => e.id === "pay:evt-1:from")!
    const toEntity = layer.entities.find((e) => e.id === "pay:evt-1:to")!
    expect(fromEntity.position).toEqual([1.25, 0.5, -2])
    expect(fromEntity.label).toBe("Alpha Pylon")
    expect(fromEntity.detail).toContain("station")
    expect(toEntity.position).toEqual([-3, 1, 2.75])
    expect(toEntity.label).toBe("Tassadar")
    expect(toEntity.detail).toContain("avatar")
  })

  test("labels unresolved endpoints as fallback instead of claiming a world location", () => {
    const layer = chatWorldPaymentLayer([
      particle({ fromRef: "pylon:alpha", toRef: "pylon:missing" }),
    ], worldProjection())

    const fromEntity = layer.entities.find((e) => e.id === "pay:evt-1:from")!
    const toEntity = layer.entities.find((e) => e.id === "pay:evt-1:to")!
    expect(fromEntity.label).toBe("Alpha Pylon")
    expect(fromEntity.detail).toContain("station")
    expect(toEntity.label).toBe("Payment target")
    expect(toEntity.detail).toContain("unresolved pylon:missing")
    expect(toEntity.detail).toContain("fallback")
    expect(toEntity.detail).toContain("receipt:nip90:abc")
  })
})

describe("chatWorldMultiplayerLayer", () => {
  test("renders stations as entities and users as remote avatar instances", () => {
    const layer = chatWorldMultiplayerLayer(worldProjection())
    expect(layer.entities).toHaveLength(1)
    expect(layer.remoteAvatars).toHaveLength(1)
    expect(layer.entities[0]).toMatchObject({
      id: "world:station:pylon.alpha",
      label: "Alpha Pylon",
      status: "verified",
      position: [1.25, 0.5, -2],
    })
    expect(layer.remoteAvatars[0]).toMatchObject({
      id: "avatar.bravo",
      label: "Tassadar",
      position: [-3, 1, 2.75],
      animation: "walk",
      updatedAtMs: 9_500,
      stale: false,
      labelVisibility: "hidden",
    })
  })

  test("filters the local desktop avatar and stale-despawns old remotes", () => {
    const layer = chatWorldMultiplayerLayer(
      worldProjection({
        projectedAtMs: 20_000,
        agents: [
          {
            avatarRef: "avatar.desktop.local",
            actorRef: "agent.local",
            avatarKind: "tassadar",
            label: "Local",
            color: "#ffffff",
            x: 0,
            y: 0,
            z: 0,
            yaw: 0,
            movementMode: "idle",
            lastSeenEpochMs: 20_000,
            presenceFeed: "high",
            chatMessages: [],
            attentionRefs: [],
          },
          {
            avatarRef: "avatar.stale",
            actorRef: "agent.stale",
            avatarKind: "tassadar",
            label: "Stale",
            color: "#9ca3af",
            x: 1,
            y: 0,
            z: 1,
            yaw: 0,
            movementMode: "running",
            lastSeenEpochMs: 13_500,
            presenceFeed: "high",
            chatMessages: [],
            attentionRefs: [],
          },
          {
            avatarRef: "avatar.gone",
            actorRef: "agent.gone",
            avatarKind: "tassadar",
            label: "Gone",
            color: "#9ca3af",
            x: 2,
            y: 0,
            z: 2,
            yaw: 0,
            movementMode: "walk",
            lastSeenEpochMs: 7_000,
            presenceFeed: "high",
            chatMessages: [],
            attentionRefs: [],
          },
        ],
      }),
      { localAvatarRef: "avatar.desktop.local" },
    )

    expect(layer.remoteAvatars.map(avatar => avatar.id)).toEqual(["avatar.stale"])
    expect(layer.remoteAvatars[0]).toMatchObject({
      animation: "run",
      stale: true,
    })
  })

  test("stays inert while disconnected", () => {
    const layer = chatWorldMultiplayerLayer(worldProjection({ connected: false }))
    expect(layer.entities).toEqual([])
    expect(layer.remoteAvatars).toEqual([])
  })
})

describe("chatWorldVisibleTargetCandidates", () => {
  test("cycles only visible nearby pylon/avatar targets in screen order", () => {
    const world = worldProjection({
      projectedAtMs: 10_000,
      stations: [
        {
          pylonRef: "pylon.alpha",
          label: "Alpha Pylon",
          x: 5,
          y: 0,
          z: 0,
        },
        {
          pylonRef: "pylon.beta",
          label: "Beta Pylon",
          x: 2,
          y: 0,
          z: 0,
        },
        {
          pylonRef: "pylon.far",
          label: "Far Pylon",
          x: 120,
          y: 0,
          z: 0,
        },
      ],
      agents: [
        {
          avatarRef: "avatar.bravo",
          actorRef: "agent.bravo",
          avatarKind: "tassadar",
          label: "Bravo",
          color: "#f5b73a",
          x: -3,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "walk",
          lastSeenEpochMs: 9_900,
          presenceFeed: "high",
          chatMessages: [],
          attentionRefs: [],
        },
        {
          avatarRef: "avatar.desktop.local",
          actorRef: "agent.local",
          avatarKind: "tassadar",
          label: "Local",
          color: "#ffffff",
          x: 0,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "idle",
          lastSeenEpochMs: 10_000,
          presenceFeed: "high",
          chatMessages: [],
          attentionRefs: [],
        },
        {
          avatarRef: "avatar.offscreen",
          actorRef: "agent.offscreen",
          avatarKind: "tassadar",
          label: "Offscreen",
          color: "#9ca3af",
          x: 4,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "walk",
          lastSeenEpochMs: 9_900,
          presenceFeed: "high",
          chatMessages: [],
          attentionRefs: [],
        },
        {
          avatarRef: "avatar.stale",
          actorRef: "agent.stale",
          avatarKind: "tassadar",
          label: "Stale",
          color: "#9ca3af",
          x: 1,
          y: 0,
          z: 0,
          yaw: 0,
          movementMode: "walk",
          lastSeenEpochMs: -5_000,
          presenceFeed: "high",
          chatMessages: [],
          attentionRefs: [],
        },
      ],
    })

    const candidates = chatWorldVisibleTargetCandidates(world, {
      localAvatarRef: "avatar.desktop.local",
      maxDistanceMeters: 10,
      viewerPosition: [0, 0, 0],
      visibility: [
        {
          id: "avatar.desktop.local",
          screenCenterX: 0,
          screenCenterY: 0,
          visible: true,
        },
        {
          id: "avatar.bravo",
          screenCenterX: 0.1,
          screenCenterY: 0.1,
          visible: true,
        },
        {
          id: `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.alpha`,
          screenCenterX: 0.2,
          screenCenterY: 0,
          visible: true,
        },
        {
          id: `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.beta`,
          screenCenterX: 0.2,
          screenCenterY: 0,
          visible: true,
        },
        {
          id: `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.far`,
          screenCenterX: 0.01,
          screenCenterY: 0,
          visible: true,
        },
        {
          id: "avatar.offscreen",
          screenCenterX: 0,
          screenCenterY: 0,
          visible: false,
        },
        {
          id: "avatar.stale",
          screenCenterX: 0,
          screenCenterY: 0,
          visible: true,
        },
      ],
    })

    expect(candidates.map(candidate => candidate.id)).toEqual([
      "avatar.bravo",
      `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.beta`,
      `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.alpha`,
    ])
    expect(candidates.map(candidate => candidate.kind)).toEqual([
      "avatar",
      "pylon",
      "pylon",
    ])
    expect(candidates.some(candidate => candidate.id.startsWith("pay:"))).toBe(false)
  })

  test("caps crowded region targets after sorting", () => {
    const candidates = chatWorldVisibleTargetCandidates(
      worldProjection({
        stations: Array.from({ length: 8 }, (_, index) => ({
          pylonRef: `pylon.${index}`,
          label: `Pylon ${index}`,
          x: index + 1,
          y: 0,
          z: 0,
        })),
        agents: [],
      }),
      {
        maxCandidates: 3,
        viewerPosition: [0, 0, 0],
        visibility: Array.from({ length: 8 }, (_, index) => ({
          id: `${CHAT_WORLD_STATION_NODE_PREFIX}pylon.${index}`,
          screenCenterX: index / 100,
          screenCenterY: 0,
          visible: true,
        })),
      },
    )

    expect(candidates).toHaveLength(3)
    expect(candidates.map(candidate => candidate.ref)).toEqual([
      "pylon.0",
      "pylon.1",
      "pylon.2",
    ])
  })
})

describe("withChatWorldPaymentLayer", () => {
  const base = pylonNetworkVisualizationOptions(
    liveChatWorldNetworkScene(liveScene())!,
  )

  test("no particles → base options unchanged (no motion overlay)", () => {
    const out = withChatWorldPaymentLayer(base, [])
    expect(out).toBe(base)
  })

  test("particles overlay beams/bursts/entities and force evidence=required", () => {
    const out = withChatWorldPaymentLayer(base, [particle()])
    expect(out.beams).toHaveLength(1)
    expect(out.bursts).toHaveLength(1)
    expect((out.entities ?? []).length).toBe(2)
    // hard backstop: the renderer must require evidence before animating motion
    expect(out.motionPolicy?.evidence).toBe("required")
    // the base pylon graph nodes survive the overlay
    expect(out.nodes).toBe(base.nodes)
  })

  test("world layer composes before payment layer so beams can resolve endpoints", () => {
    const withWorld = withChatWorldMultiplayerLayer(base, worldProjection())
    const out = withChatWorldPaymentLayer(
      withWorld,
      [particle({ fromRef: "pylon:alpha", toRef: "agent.bravo" })],
      worldProjection(),
    )

    expect((out.entities ?? []).some((entity) => entity.id === "world:station:pylon.alpha")).toBe(true)
    expect((out.remoteAvatars ?? []).some((avatar) => avatar.id === "avatar.bravo")).toBe(true)
    expect(out.entities?.find((entity) => entity.id === "pay:evt-1:from")?.position).toEqual([1.25, 0.5, -2])
    expect(out.entities?.find((entity) => entity.id === "pay:evt-1:to")?.position).toEqual([-3, 1, 2.75])
  })
})
