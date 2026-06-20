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
  chatWorldPaymentEndpointIndex,
  chatWorldPaymentLayer,
  liveChatWorldNetworkScene,
  resolveChatWorldPaymentEndpoint,
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
    // the target endpoint label carries the receipt ref (click → inspector)
    const toEntity = layer.entities.find((e) => e.id === "pay:evt-1:to")!
    const fromEntity = layer.entities.find((e) => e.id === "pay:evt-1:from")!
    expect(fromEntity.label).toContain("receipt:nip90:abc")
    expect(toEntity.label).toContain("receipt:nip90:abc")
    expect(toEntity.label).toContain("21000 sats")
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
    expect(fromEntity.label).toContain("Alpha Pylon")
    expect(fromEntity.label).toContain("station")
    expect(toEntity.position).toEqual([-3, 1, 2.75])
    expect(toEntity.label).toContain("Tassadar")
    expect(toEntity.label).toContain("avatar")
  })

  test("labels unresolved endpoints as fallback instead of claiming a world location", () => {
    const layer = chatWorldPaymentLayer([
      particle({ fromRef: "pylon:alpha", toRef: "pylon:missing" }),
    ], worldProjection())

    const fromEntity = layer.entities.find((e) => e.id === "pay:evt-1:from")!
    const toEntity = layer.entities.find((e) => e.id === "pay:evt-1:to")!
    expect(fromEntity.label).toContain("station")
    expect(toEntity.label).toContain("unresolved pylon:missing")
    expect(toEntity.label).toContain("fallback")
    expect(toEntity.label).toContain("receipt:nip90:abc")
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
})
