import { describe, expect, test } from "bun:test"

import {
  activityEventToParticle,
  activityEventsToParticles,
  heartbeatPulseSpeed,
  isPaymentEvent,
  LIVE_PYLON_STATE_COLOR,
  livePylonState,
  particleSize,
  PAYMENT_PARTICLE_GOLD,
  PAYMENT_PARTICLE_WINDOW_MS,
  paymentParticleTsMs,
  projectChatWorldPylonScene,
  prunePaymentParticlesByRecency,
  pylonGrowthTier,
  type ActivityEvent,
  type LiveRecentPylon,
  type PaymentParticle,
} from "../src/shared/chat-world-scene"
import type { PylonStatsSnapshot } from "../src/shared/pylon-network-scene"

// ── P1: pylon-stats → scene ──────────────────────────────────────────────────

const snapshot = (overrides: Partial<PylonStatsSnapshot> = {}): PylonStatsSnapshot => ({
  available: true,
  status: "live",
  asOfLabel: "moments ago",
  ...overrides,
})

describe("livePylonState (§5 distinct presence encodings)", () => {
  const cases: ReadonlyArray<[LiveRecentPylon, string]> = [
    [{ onlineNow: false }, "offline"],
    [{ onlineNow: true }, "online"],
    [{ onlineNow: true, walletReadyNow: true }, "wallet_ready"],
    [{ onlineNow: true, assignmentReadyNow: true }, "assignment_ready"],
    // assignment-ready outranks wallet-ready
    [{ onlineNow: true, walletReadyNow: true, assignmentReadyNow: true }, "assignment_ready"],
    // wallet/assignment ready but offline → offline (not online → no glow)
    [{ onlineNow: false, assignmentReadyNow: true }, "offline"],
  ]
  for (const [pylon, expected] of cases) {
    test(`${JSON.stringify(pylon)} → ${expected}`, () => {
      expect(livePylonState(pylon)).toBe(expected)
    })
  }

  test("every state has a distinct color", () => {
    const colors = Object.values(LIVE_PYLON_STATE_COLOR)
    expect(new Set(colors).size).toBe(colors.length)
  })
})

describe("heartbeatPulseSpeed (§3.3 heartbeat-age pulse)", () => {
  test("fresh heartbeat pulses fast", () => {
    expect(heartbeatPulseSpeed(2)).toBeGreaterThan(1)
  })
  test("aging heartbeat slows monotonically", () => {
    const fresh = heartbeatPulseSpeed(20)
    const older = heartbeatPulseSpeed(80)
    expect(older).toBeLessThan(fresh)
    expect(older).toBeGreaterThan(0)
  })
  test("stale or unknown age → no pulse (honest stillness)", () => {
    expect(heartbeatPulseSpeed(600)).toBe(0)
    expect(heartbeatPulseSpeed(null)).toBe(0)
    expect(heartbeatPulseSpeed(undefined)).toBe(0)
    expect(heartbeatPulseSpeed(-5)).toBe(0)
  })
})

describe("pylonGrowthTier (#5737 settled-sats → tier)", () => {
  test("zero earnings → tier 0, scale 1", () => {
    const t = pylonGrowthTier(0)
    expect(t.tier).toBe(0)
    expect(t.scale).toBe(1)
    expect(t.brightness).toBe(0)
  })
  test("tier and scale grow monotonically with sats", () => {
    const small = pylonGrowthTier(5_000)
    const big = pylonGrowthTier(5_000_000)
    expect(big.tier).toBeGreaterThan(small.tier)
    expect(big.scale).toBeGreaterThan(small.scale)
    expect(big.facets).toBeGreaterThan(small.facets)
    expect(big.brightness).toBeGreaterThanOrEqual(small.brightness)
  })
  test("carries the settled sats as evidence", () => {
    expect(pylonGrowthTier(12_345).settledSats).toBe(12_345)
    expect(pylonGrowthTier(-1).settledSats).toBe(0)
  })
})

describe("projectChatWorldPylonScene", () => {
  test("null/unavailable snapshot → honest empty zero-state", () => {
    expect(projectChatWorldPylonScene(null).empty).toBe(true)
    expect(projectChatWorldPylonScene(null).nodes).toHaveLength(0)
    expect(projectChatWorldPylonScene(snapshot({ status: "unavailable" })).empty).toBe(true)
    expect(projectChatWorldPylonScene(snapshot({ available: false })).empty).toBe(true)
  })

  test("maps recentPylons into live nodes with state colors + products", () => {
    const scene = projectChatWorldPylonScene(
      snapshot({
        pylonsOnlineNow: 2,
        recentPylons: [
          {
            nostrPubkeyShort: "abc123",
            nodeLabel: "orrery",
            onlineNow: true,
            assignmentReadyNow: true,
            lastHeartbeatAgeSeconds: 5,
            products: ["compute", "labor"],
          } satisfies LiveRecentPylon,
          {
            nostrPubkeyShort: "def456",
            onlineNow: false,
          } satisfies LiveRecentPylon,
        ] as never,
      }),
    )
    expect(scene.empty).toBe(false)
    expect(scene.onlineNow).toBe(2)
    expect(scene.nodes).toHaveLength(2)
    const [orrery, offline] = scene.nodes
    expect(orrery!.id).toBe("abc123")
    expect(orrery!.label).toBe("orrery")
    expect(orrery!.state).toBe("assignment_ready")
    expect(orrery!.color).toBe(LIVE_PYLON_STATE_COLOR.assignment_ready)
    expect(orrery!.online).toBe(true)
    expect(orrery!.pulseSpeed).toBeGreaterThan(0)
    expect(orrery!.products).toEqual(["compute", "labor"])
    // offline node sits still (no pulse, no glow)
    expect(offline!.id).toBe("def456")
    expect(offline!.label).toBe("def456")
    expect(offline!.state).toBe("offline")
    expect(offline!.online).toBe(false)
    expect(offline!.pulseSpeed).toBe(0)
  })

  test("labels live pylons from network names or refs, never generic pylon text", () => {
    const scene = projectChatWorldPylonScene(
      snapshot({
        pylonsOnlineNow: 3,
        recentPylons: [
          {
            nostrPubkeyShort: "pylon.public.named",
            nodeLabel: "North Dock",
            onlineNow: true,
          } satisfies LiveRecentPylon,
          {
            nostrPubkeyShort: "pylon.public.generic",
            nodeLabel: "pylon",
            onlineNow: true,
          } satisfies LiveRecentPylon,
          {
            nostrPubkeyShort: "pylon.public.unnamed",
            onlineNow: false,
          } satisfies LiveRecentPylon,
        ] as never,
      }),
    )

    expect(scene.nodes.map((node) => node.label)).toEqual([
      "North Dock",
      "pylon.public.generic",
      "pylon.public.unnamed",
    ])
    expect(scene.nodes.some((node) => node.label.toLowerCase() === "pylon")).toBe(false)
  })

  test("fleet growth tier comes from cumulative settled sats total", () => {
    const scene = projectChatWorldPylonScene(
      snapshot({
        pylonsOnlineNow: 1,
        recentPylons: [{ nostrPubkeyShort: "x", onlineNow: true }] as never,
        nip90MarketSettlementStats: {
          compute: { satsSettledTotal: 600_000 },
          data: { satsSettledTotal: 300_000 },
          labor: { satsSettledTotal: 200_000 },
        },
      }),
    )
    // 1.1M total → tier ≥ the 1M threshold
    expect(scene.growth.settledSats).toBe(1_100_000)
    expect(scene.growth.tier).toBeGreaterThanOrEqual(4)
  })

  test("per-Pylon growth tiers come from public cumulative settled sats", () => {
    const scene = projectChatWorldPylonScene(
      snapshot({
        pylonsOnlineNow: 3,
        recentPylons: [
          {
            nostrPubkeyShort: "zero",
            onlineNow: true,
            cumulativeSettledSats: 0,
          } satisfies LiveRecentPylon,
          {
            nostrPubkeyShort: "missing",
            onlineNow: true,
          } satisfies LiveRecentPylon,
          {
            nostrPubkeyShort: "earned",
            onlineNow: true,
            cumulativeSettledSats: 1_000_000,
          } satisfies LiveRecentPylon,
        ] as never,
      }),
    )

    expect(scene.nodes.map((node) => node.growth.settledSats)).toEqual([
      0,
      0,
      1_000_000,
    ])
    expect(scene.nodes[0]!.growth.tier).toBe(0)
    expect(scene.nodes[0]!.growth.scale).toBe(1)
    expect(scene.nodes[0]!.growth.brightness).toBe(0)
    expect(scene.nodes[1]!.growth.tier).toBe(0)
    expect(scene.nodes[2]!.growth.tier).toBeGreaterThan(
      scene.nodes[0]!.growth.tier,
    )
    expect(scene.nodes[2]!.growth.scale).toBeGreaterThan(
      scene.nodes[0]!.growth.scale,
    )
    expect(scene.nodes[2]!.growth.facets).toBeGreaterThan(
      scene.nodes[0]!.growth.facets,
    )
    expect(scene.nodes[2]!.growth.brightness).toBeGreaterThan(
      scene.nodes[0]!.growth.brightness,
    )
  })

  test("empty recentPylons with zero online → empty zero-state", () => {
    expect(projectChatWorldPylonScene(snapshot({ pylonsOnlineNow: 0 })).empty).toBe(true)
  })
})

// ── P2: activity event → payment particle ────────────────────────────────────

const receiptRef = "receipt.settlement.abc"

const paymentEvent = (overrides: Partial<ActivityEvent> = {}): ActivityEvent => ({
  eventRef: "evt-1",
  kind: "real_bitcoin_moved",
  ts: "2026-06-20T00:00:00Z",
  actorRef: "pylon:payer",
  targetRef: "pylon:payee",
  amountSats: 21_000,
  realBitcoinMoved: true,
  sourceRefs: [receiptRef],
  text: "settled",
  ...overrides,
})

describe("isPaymentEvent / particleSize", () => {
  test("only payment kinds are payment events", () => {
    expect(isPaymentEvent(paymentEvent())).toBe(true)
    expect(isPaymentEvent(paymentEvent({ kind: "settlement_recorded" }))).toBe(true)
    expect(isPaymentEvent(paymentEvent({ kind: "forum_posted" }))).toBe(false)
    expect(isPaymentEvent(paymentEvent({ kind: "artanis_tick" }))).toBe(false)
  })

  test("size grows with amount and is clamped to [0.2,1]", () => {
    const small = particleSize(10)
    const big = particleSize(10_000_000)
    expect(small).toBeGreaterThanOrEqual(0.2)
    expect(big).toBeLessThanOrEqual(1)
    expect(big).toBeGreaterThan(small)
    expect(particleSize(0)).toBe(0.2)
    expect(particleSize(1_000_000_000)).toBeLessThanOrEqual(1)
  })
})

describe("activityEventToParticle (evidence-bound §5)", () => {
  test("real bitcoin → gold particle carrying sourceRefs", () => {
    const p = activityEventToParticle(paymentEvent())!
    expect(p).not.toBeNull()
    expect(p.id).toBe("evt-1")
    expect(p.kind).toBe("real_bitcoin_moved")
    expect(p.fromRef).toBe("pylon:payer")
    expect(p.toRef).toBe("pylon:payee")
    expect(p.realBitcoinMoved).toBe(true)
    expect(p.color).toBe(PAYMENT_PARTICLE_GOLD)
    expect(p.amountSats).toBe(21_000)
    expect(p.sourceRefs).toContain(receiptRef)
  })

  test("settlement records with real bitcoin → gold particle carrying sourceRefs", () => {
    const p = activityEventToParticle(paymentEvent({ kind: "settlement_recorded" }))!
    expect(p).not.toBeNull()
    expect(p.kind).toBe("settlement_recorded")
    expect(p.realBitcoinMoved).toBe(true)
    expect(p.color).toBe(PAYMENT_PARTICLE_GOLD)
    expect(p.sourceRefs).toContain(receiptRef)
  })

  test("refuses non-real settlement records", () => {
    expect(
      activityEventToParticle(
        paymentEvent({ kind: "settlement_recorded", realBitcoinMoved: false }),
      ),
    ).toBeNull()
    expect(
      activityEventToParticle(
        paymentEvent({ kind: "settlement_recorded", realBitcoinMoved: undefined }),
      ),
    ).toBeNull()
  })

  test("refuses non-payment kinds", () => {
    expect(activityEventToParticle(paymentEvent({ kind: "forum_posted" }))).toBeNull()
  })

  test("refuses an event missing an endpoint", () => {
    expect(activityEventToParticle(paymentEvent({ actorRef: undefined }))).toBeNull()
    expect(activityEventToParticle(paymentEvent({ targetRef: undefined }))).toBeNull()
  })

  test("refuses an event with no sourceRef (never fakes a clickable receipt)", () => {
    expect(activityEventToParticle(paymentEvent({ sourceRefs: [] }))).toBeNull()
    expect(activityEventToParticle(paymentEvent({ sourceRefs: undefined }))).toBeNull()
  })

  test("batch maps drop non-renderable events", () => {
    const particles = activityEventsToParticles([
      paymentEvent({ eventRef: "ok" }),
      paymentEvent({ eventRef: "bad", sourceRefs: [] }),
      paymentEvent({ eventRef: "skip", kind: "forum_posted" }),
    ])
    expect(particles).toHaveLength(1)
    expect(particles[0]!.id).toBe("ok")
  })
})

// ── P2.5: payment-particle recency window (#5730 live wiring) ─────────────────

const particleAt = (id: string, ts: string | null): PaymentParticle => ({
  id,
  kind: "real_bitcoin_moved",
  fromRef: "pylon:payer",
  toRef: "pylon:payee",
  amountSats: 1_000,
  realBitcoinMoved: true,
  color: PAYMENT_PARTICLE_GOLD,
  size: 0.5,
  sourceRefs: [`receipt:${id}`],
  ts,
  text: null,
})

const MINUTE_MS = 60_000

describe("paymentParticleTsMs", () => {
  test("parses an ISO ts into epoch ms", () => {
    expect(paymentParticleTsMs(particleAt("a", "2026-06-20T00:00:00Z"))).toBe(
      Date.parse("2026-06-20T00:00:00Z"),
    )
  })

  test("returns null for a missing or unparseable ts (never invents a time)", () => {
    expect(paymentParticleTsMs(particleAt("a", null))).toBeNull()
    expect(paymentParticleTsMs(particleAt("a", "not-a-date"))).toBeNull()
  })
})

describe("prunePaymentParticlesByRecency (P2.5: stale beams expire)", () => {
  const t0 = Date.parse("2026-06-20T00:00:00Z")
  const isoAt = (offsetMs: number): string => new Date(t0 + offsetMs).toISOString()

  test("keeps particles inside the window and drops older ones", () => {
    const particles = [
      particleAt("old", isoAt(-5 * MINUTE_MS)), // well outside the 90s window
      particleAt("recent", isoAt(-MINUTE_MS)), // inside the window
      particleAt("now", isoAt(0)),
    ]
    const kept = prunePaymentParticlesByRecency(particles, t0)
    expect(kept.map((p) => p.id)).toEqual(["recent", "now"])
  })

  test("preserves input order", () => {
    const particles = [
      particleAt("a", isoAt(-1_000)),
      particleAt("b", isoAt(-2_000)),
      particleAt("c", isoAt(0)),
    ]
    expect(prunePaymentParticlesByRecency(particles, t0).map((p) => p.id)).toEqual([
      "a",
      "b",
      "c",
    ])
  })

  test("keeps a particle exactly on the cutoff boundary", () => {
    const onCutoff = particleAt("edge", isoAt(-PAYMENT_PARTICLE_WINDOW_MS))
    expect(prunePaymentParticlesByRecency([onCutoff], t0)).toHaveLength(1)
  })

  test("keeps particles with no parseable ts (count cap bounds them instead)", () => {
    const particles = [
      particleAt("no-ts", null),
      particleAt("old", isoAt(-5 * MINUTE_MS)),
    ]
    expect(prunePaymentParticlesByRecency(particles, t0).map((p) => p.id)).toEqual([
      "no-ts",
    ])
  })

  test("a non-finite reference time is a no-op (never prunes against an unknown clock)", () => {
    const particles = [particleAt("old", isoAt(-5 * MINUTE_MS))]
    expect(prunePaymentParticlesByRecency(particles, Number.NaN)).toEqual(particles)
  })

  test("a custom window widens what stays renderable", () => {
    const particles = [particleAt("old", isoAt(-5 * MINUTE_MS))]
    expect(prunePaymentParticlesByRecency(particles, t0, 10 * MINUTE_MS)).toHaveLength(1)
  })
})
