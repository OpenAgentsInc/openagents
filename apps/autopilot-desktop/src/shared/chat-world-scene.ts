// Chat-World scene mappers (P1 #5736 + P2 #5737).
//
// PURE, Three.js-free transforms that feed the "agent MMORPG" scene that hangs
// behind the Autopilot chat (docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-
// audit-and-plan.md §3.3 live data, §6 P1/P2). Tune knobs HERE, not in the
// renderer (mirrors the discipline in shared/pylon-network-scene.ts).
//
// Two responsibilities:
//   P1 — map a live /api/public/pylon-stats snapshot's recentPylons[] into
//        richer LIVE scene nodes (online glow, heartbeat-age pulse, wallet/
//        assignment-ready state colors, honest zero-state) + a per-Pylon growth
//        tier from cumulative settled sats.
//   P2 — map a /api/public/activity-timeline event (real_bitcoin_moved /
//        settlement_recorded) into an EVIDENCE-BOUND payment-particle descriptor
//        the scene renders via three-effect flowEffectPrimitives /
//        eventBurstPrimitives. Every particle carries its sourceRefs so it is
//        clickable to its receipt.
//
// Evidence-bound motion contract (§5): nothing moves/pulses/bursts unless it is
// bound to a real public ref or a live state transition. These mappers refuse
// to emit a particle that has no sourceRef.

import {
  recentPylonNetworkId,
  recentPylonNetworkLabel,
  type PylonStatsSnapshot,
} from "./pylon-network-scene.js"

// ─────────────────────────────────────────────────────────────────────────────
// Feature flags (default OFF). The renderer / P0 integration reads these before
// mounting the scene or wiring the subscriptions. Resolved from globalThis so
// the same code works in the webview (window.__OA_FLAGS) and in tests.
// ─────────────────────────────────────────────────────────────────────────────

export type ChatWorldFlags = {
  /** P0/P1: mount the live pylon scene behind chat. */
  readonly CHAT_WORLD_SCENE: boolean
  /** P2: fly Bitcoin payment particles + grow pylons from settled sats. */
  readonly CHAT_WORLD_PAYMENTS: boolean
}

const flagBag = (): Record<string, unknown> => {
  const g = globalThis as unknown as { __OA_FLAGS?: Record<string, unknown> }
  return g.__OA_FLAGS ?? {}
}

const readFlag = (name: keyof ChatWorldFlags): boolean => flagBag()[name] === true

export const chatWorldFlags = (): ChatWorldFlags => ({
  CHAT_WORLD_SCENE: readFlag("CHAT_WORLD_SCENE"),
  CHAT_WORLD_PAYMENTS: readFlag("CHAT_WORLD_PAYMENTS"),
})

// ─────────────────────────────────────────────────────────────────────────────
// P1 — live pylon nodes from pylon-stats
// ─────────────────────────────────────────────────────────────────────────────

// The richer RecentPylon shape the chat-world scene consumes. Superset of the
// home-screen RecentPylon (shared/pylon-network-scene.ts) — the worker owns the
// full schema in apps/openagents.com/workers/api/src/public-pylon-stats.ts. All
// fields optional/nullable so a partial snapshot is safe and never faked.
export type LiveRecentPylon = {
  readonly nodeLabel?: string | null
  readonly nostrPubkeyShort?: string
  readonly runtimeState?: string | null
  readonly onlineNow?: boolean | null
  readonly walletReadyNow?: boolean | null
  readonly assignmentReadyNow?: boolean | null
  readonly cumulativeSettledSats?: number | null
  readonly lastHeartbeatAgeSeconds?: number | null
  readonly products?: ReadonlyArray<string> | null
}

// A live presence state with a distinct visual encoding (§5: online ≠ assigned
// ≠ wallet-ready ≠ offline). offline = seen but not online now.
export type LivePylonState =
  | "offline"
  | "online"
  | "wallet_ready"
  | "assignment_ready"

// Distinct state colors (hex ints for three-effect materials). Gold is reserved
// for real bitcoin motion (P2), so pylon states stay in the blue/green family.
export const LIVE_PYLON_STATE_COLOR: Readonly<Record<LivePylonState, number>> = {
  offline: 0x3a4150, // dim grey — seen but offline
  online: 0xcdd3e0, // off-white — online, idle
  wallet_ready: 0x7dd3fc, // blue — can receive
  assignment_ready: 0x4ade80, // green — working / earning
} as const

// Pylon growth tiers (#5737): cumulative settled sats → crystal scale/facets/
// brightness. Thresholds in sats; tier index grows monotonically. Returned as a
// descriptor so the renderer maps tier → scale/facet/brightness consistently.
export const PYLON_GROWTH_TIER_THRESHOLDS_SATS: ReadonlyArray<number> = [
  0, // tier 0 — no settled earnings yet (honest still crystal)
  1_000,
  10_000,
  100_000,
  1_000_000,
  10_000_000,
] as const

export type PylonGrowthTier = {
  /** 0..N tier index. */
  readonly tier: number
  /** [1,_] crystal scale multiplier the renderer applies. */
  readonly scale: number
  /** facet count hint for the crystal mesh. */
  readonly facets: number
  /** [0,1] extra glow brightness from accumulated earnings. */
  readonly brightness: number
  /** the cumulative settled sats that produced this tier (evidence). */
  readonly settledSats: number
}

export const pylonGrowthTier = (settledSats: number): PylonGrowthTier => {
  const sats = Number.isFinite(settledSats) && settledSats > 0 ? settledSats : 0
  let tier = 0
  for (let i = 1; i < PYLON_GROWTH_TIER_THRESHOLDS_SATS.length; i += 1) {
    if (sats >= (PYLON_GROWTH_TIER_THRESHOLDS_SATS[i] ?? Infinity)) tier = i
  }
  const max = PYLON_GROWTH_TIER_THRESHOLDS_SATS.length - 1
  const frac = max <= 0 ? 0 : tier / max
  return {
    tier,
    scale: Number((1 + 0.18 * tier).toFixed(3)),
    facets: 6 + tier * 2,
    brightness: Number(frac.toFixed(3)),
    settledSats: sats,
  }
}

// Heartbeat-age pulse: a fresh heartbeat pulses fast, an aging one slows toward
// a slow idle breath, and a stale node (no recent heartbeat) stops pulsing.
// Returns pulses/sec in [0, fast]; 0 means "do not pulse" (honest stillness).
const HEARTBEAT_FRESH_SECONDS = 15
const HEARTBEAT_STALE_SECONDS = 120
const PULSE_FAST = 1.8
const PULSE_SLOW = 0.4

export const heartbeatPulseSpeed = (
  lastHeartbeatAgeSeconds: number | null | undefined,
): number => {
  if (
    typeof lastHeartbeatAgeSeconds !== "number" ||
    !Number.isFinite(lastHeartbeatAgeSeconds) ||
    lastHeartbeatAgeSeconds < 0
  ) {
    return 0 // unknown age → no pulse (do not fake liveness)
  }
  if (lastHeartbeatAgeSeconds >= HEARTBEAT_STALE_SECONDS) return 0
  if (lastHeartbeatAgeSeconds <= HEARTBEAT_FRESH_SECONDS) return PULSE_FAST
  const span = HEARTBEAT_STALE_SECONDS - HEARTBEAT_FRESH_SECONDS
  const aged = (lastHeartbeatAgeSeconds - HEARTBEAT_FRESH_SECONDS) / span
  return Number((PULSE_FAST - (PULSE_FAST - PULSE_SLOW) * aged).toFixed(3))
}

export const livePylonState = (pylon: LiveRecentPylon): LivePylonState => {
  if (pylon.onlineNow !== true) return "offline"
  if (pylon.assignmentReadyNow === true) return "assignment_ready"
  if (pylon.walletReadyNow === true) return "wallet_ready"
  return "online"
}

export type LivePylonNode = {
  /** stable id (the short pubkey) so positions are stable across polls. */
  readonly id: string
  readonly label: string
  readonly state: LivePylonState
  /** state color the renderer applies to glow/material. */
  readonly color: number
  /** online nodes glow; offline ones sit dim. */
  readonly online: boolean
  /** pulses/sec from heartbeat age (0 = still). */
  readonly pulseSpeed: number
  /** per-Pylon crystal growth from public cumulative settled sats. */
  readonly growth: PylonGrowthTier
  /** public capability tags (products) for the inspector. */
  readonly products: ReadonlyArray<string>
}

export type ChatWorldPylonScene = {
  /** true when the fleet snapshot is unavailable/empty — honest zero-state. */
  readonly empty: boolean
  readonly onlineNow: number
  /** per-pylon live nodes (the named, inspectable ones). */
  readonly nodes: ReadonlyArray<LivePylonNode>
  /** fleet-wide growth tier from cumulative settled sats (drives center pylon). */
  readonly growth: PylonGrowthTier
  readonly asOfLabel: string | null
}

const cumulativeSettledSatsTotal = (snapshot: PylonStatsSnapshot): number => {
  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  const total =
    (m.compute?.satsSettledTotal ?? 0) +
    (m.data?.satsSettledTotal ?? 0) +
    (m.labor?.satsSettledTotal ?? 0)
  return Number.isFinite(total) && total > 0 ? total : 0
}

// Map a live pylon-stats snapshot into the chat-world pylon scene. Honest
// zero-state: an unavailable/empty snapshot yields empty:true with no nodes.
export const projectChatWorldPylonScene = (
  snapshot: PylonStatsSnapshot | null,
): ChatWorldPylonScene => {
  const unavailable =
    snapshot === null ||
    snapshot.available === false ||
    snapshot.status === "unavailable"

  if (unavailable) {
    return {
      empty: true,
      onlineNow: 0,
      nodes: [],
      growth: pylonGrowthTier(0),
      asOfLabel: snapshot?.asOfLabel ?? null,
    }
  }

  const recent = (snapshot.recentPylons ?? []) as ReadonlyArray<LiveRecentPylon>
  const nodes: LivePylonNode[] = recent.map((pylon, index) => {
    const state = livePylonState(pylon)
    const settledSats =
      typeof pylon.cumulativeSettledSats === "number"
        ? pylon.cumulativeSettledSats
        : 0
    return {
      id: recentPylonNetworkId(pylon, index),
      label: recentPylonNetworkLabel(pylon, index),
      state,
      color: LIVE_PYLON_STATE_COLOR[state],
      online: pylon.onlineNow === true,
      pulseSpeed:
        pylon.onlineNow === true
          ? heartbeatPulseSpeed(pylon.lastHeartbeatAgeSeconds)
          : 0,
      growth: pylonGrowthTier(settledSats),
      products: (pylon.products ?? []).filter((p): p is string => typeof p === "string"),
    }
  })

  const onlineNow =
    typeof snapshot.pylonsOnlineNow === "number" && snapshot.pylonsOnlineNow > 0
      ? snapshot.pylonsOnlineNow
      : 0

  return {
    empty: nodes.length === 0 && onlineNow === 0,
    onlineNow,
    nodes,
    growth: pylonGrowthTier(cumulativeSettledSatsTotal(snapshot)),
    asOfLabel: snapshot.asOfLabel ?? null,
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// P2 — payment particles from activity-timeline events
// ─────────────────────────────────────────────────────────────────────────────

// The minimal activity-timeline event shape we consume. The worker / package
// @openagentsinc/public-activity-timeline owns the full schema; we keep a
// structural subset so this stays Three.js- and Effect-Schema-free and
// unit-testable. All money fields optional so a partial event is safe.
export type ActivityEvent = {
  readonly eventRef: string
  readonly kind: string
  readonly ts?: string
  readonly actorRef?: string
  readonly targetRef?: string
  readonly amountSats?: number
  readonly realBitcoinMoved?: boolean
  readonly sourceRefs?: ReadonlyArray<string>
  readonly text?: string
}

export type PaymentParticleKind = "real_bitcoin_moved" | "settlement_recorded"

// A renderer-agnostic descriptor for one payment particle. The P0 scene maps
// fromRef/toRef → node positions and feeds size/color/evidence into
// three-effect createFlowBeam + createEvidenceBackedEventBurst. EVIDENCE-BOUND:
// sourceRefs is required and non-empty, so a particle is always clickable to a
// real receipt/event.
export type PaymentParticle = {
  readonly id: string
  /** originating public activity event kind. */
  readonly kind: PaymentParticleKind
  /** sender node ref (actorRef). */
  readonly fromRef: string
  /** recipient node ref (targetRef). */
  readonly toRef: string
  readonly amountSats: number
  /** accepted payment particles are real-bitcoin settlement evidence. */
  readonly realBitcoinMoved: boolean
  /** gold for real bitcoin movement. */
  readonly color: number
  /** [0.2,1] visual size ∝ amountSats (log-scaled, clamped). */
  readonly size: number
  /** receipt/event refs — what a click resolves to (≥1, evidence-bound). */
  readonly sourceRefs: ReadonlyArray<string>
  readonly ts: string | null
  readonly text: string | null
}

export const PAYMENT_PARTICLE_GOLD = 0xf5b73a // real bitcoin moved

// Kinds that produce a sender→recipient money particle.
const PAYMENT_EVENT_KINDS = new Set(["real_bitcoin_moved", "settlement_recorded"])

export const isPaymentEvent = (event: ActivityEvent): boolean =>
  PAYMENT_EVENT_KINDS.has(event.kind)

const paymentParticleKind = (kind: string): PaymentParticleKind | null =>
  kind === "real_bitcoin_moved" || kind === "settlement_recorded" ? kind : null

// Log-scale size in [0.2, 1] so a 10-sat tip and a 10M-sat settlement both read,
// with bigger amounts visibly larger. 0 sats → smallest visible particle.
const PARTICLE_SIZE_MIN = 0.2
const PARTICLE_SIZE_MAX = 1
const PARTICLE_SIZE_LOG_CAP = Math.log10(10_000_000) // 10M sats reads as max

export const particleSize = (amountSats: number): number => {
  const sats = Number.isFinite(amountSats) && amountSats > 0 ? amountSats : 0
  if (sats <= 0) return PARTICLE_SIZE_MIN
  const frac = Math.min(1, Math.log10(sats + 1) / PARTICLE_SIZE_LOG_CAP)
  return Number(
    (PARTICLE_SIZE_MIN + (PARTICLE_SIZE_MAX - PARTICLE_SIZE_MIN) * frac).toFixed(3),
  )
}

// Map a single activity event into a payment-particle descriptor, or null when
// it cannot be honestly rendered. We REFUSE to emit a particle that lacks:
//   - a payment kind,
//   - both endpoints (actorRef → targetRef), or
//   - at least one sourceRef (evidence-bound contract §5).
export const activityEventToParticle = (
  event: ActivityEvent,
): PaymentParticle | null => {
  const kind = paymentParticleKind(event.kind)
  if (kind === null) return null
  if (event.realBitcoinMoved !== true) return null

  const fromRef = event.actorRef
  const toRef = event.targetRef
  if (!fromRef || !toRef) return null

  const sourceRefs = (event.sourceRefs ?? []).filter(
    (r): r is string => typeof r === "string" && r.length > 0,
  )
  if (sourceRefs.length === 0) return null // never fake a clickable receipt

  const amountSats =
    typeof event.amountSats === "number" && event.amountSats > 0
      ? event.amountSats
      : 0

  return {
    id: event.eventRef,
    kind,
    fromRef,
    toRef,
    amountSats,
    realBitcoinMoved: true,
    color: PAYMENT_PARTICLE_GOLD,
    size: particleSize(amountSats),
    sourceRefs,
    ts: event.ts ?? null,
    text: event.text ?? null,
  }
}

// Map a batch (e.g. an activity-timeline poll / backfill) into particles,
// dropping any that are not honestly renderable.
export const activityEventsToParticles = (
  events: ReadonlyArray<ActivityEvent>,
): ReadonlyArray<PaymentParticle> =>
  events
    .map(activityEventToParticle)
    .filter((p): p is PaymentParticle => p !== null)

// ─────────────────────────────────────────────────────────────────────────────
// P2.5 — payment-particle recency window (#5730 live wiring)
// ─────────────────────────────────────────────────────────────────────────────

// How long a payment beam stays in the live scene after its event timestamp.
// Beyond this, a beam no longer represents *recent* activity and is pruned, so a
// quiet network does not keep flying a stale beam forever — the active-set count
// cap alone would let an old beam linger until enough newer payments push it out.
// Pruning is bound to the event's OWN ts (not a wall clock), so it stays pure and
// deterministic: replaying the same stream always yields the same active set.
export const PAYMENT_PARTICLE_WINDOW_MS = 90_000

// Parse a particle's ISO ts into epoch ms, or null when absent/unparseable. We
// never invent a timestamp — a particle the worker emitted without a ts simply
// cannot be recency-pruned (the count cap still bounds it).
export const paymentParticleTsMs = (particle: PaymentParticle): number | null => {
  const ts = particle.ts
  if (typeof ts !== "string") return null
  const ms = Date.parse(ts)
  return Number.isFinite(ms) ? ms : null
}

// Drop payment particles whose event ts is older than `referenceTsMs - windowMs`.
// Particles with no parseable ts are KEPT (we never fabricate a time to expire
// them; the caller's count cap bounds them). Input order is preserved. Pure: the
// caller passes the reference time — typically the newest incoming particle's ts
// (paymentParticleTsMs) — so there is no hidden clock and the result is testable.
// A non-finite referenceTsMs is a no-op (we never prune against an unknown clock).
export const prunePaymentParticlesByRecency = (
  particles: ReadonlyArray<PaymentParticle>,
  referenceTsMs: number,
  windowMs: number = PAYMENT_PARTICLE_WINDOW_MS,
): ReadonlyArray<PaymentParticle> => {
  if (!Number.isFinite(referenceTsMs)) return particles
  const cutoff = referenceTsMs - Math.max(0, windowMs)
  return particles.filter((p) => {
    const ms = paymentParticleTsMs(p)
    return ms === null || ms >= cutoff
  })
}
