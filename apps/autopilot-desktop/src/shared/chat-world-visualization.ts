// Chat-world visualization projection (P2.5 wiring · #5730).
//
// PURE, Three.js-free transform that turns the LIVE chat-world state
//   - a ChatWorldPylonScene (from pylon-stats, P1 #5736), and
//   - the active PaymentParticle[] (from the activity stream, P2 #5737)
// into a single `TrainingRunVisualizationOptions` the shared three-effect
// `trainingRunView` element renders behind chat. Tune knobs HERE, not in the
// renderer (mirrors shared/pylon-network-scene.ts + ui/pylon-network-
// visualization.ts discipline).
//
// Two layers, one scene:
//   1. PYLONS  — live nodes ring the "network" hub (online/state color +
//      growth tier on the hub), via the existing pylonNetworkVisualizationOptions
//      bezier graph. A null/empty live scene falls back to the caller's static
//      seed so zero-state / pre-load is calm, never blank.
//   2. PAYMENTS — each PaymentParticle becomes an EVIDENCE-BOUND beam from the
//      actor pylon → target pylon (gold motionKind for real bitcoin, dim for
//      credited), with a settlement burst on the target, plus clickable endpoint
//      entities that carry the receipt sourceRef. motionPolicy.evidence is
//      "required", so the renderer refuses to animate any motion without refs —
//      and activityEventToParticle already drops particles with no sourceRef, so
//      nothing flies without real evidence (§5 evidence-bound motion contract).
//
// The shared renderer animates beams/bursts and registers entity hit targets
// itself; size ∝ amountSats and the gold/dim split are carried on the descriptor
// (particle.size / particle.realBitcoinMoved) so they survive into the inspector
// even where the shared bezier renderer draws a single beam style.

import type {
  TrainingRunEntityDefinition,
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import type { ChatWorldPylonScene, PaymentParticle } from "./chat-world-scene"
import type { ChatWorldMultiplayerProjection } from "./chat-world-multiplayer"
import {
  type PylonNetworkNode,
  type PylonNetworkScene,
} from "./pylon-network-scene"

// ── Live pylon scene → PylonNetworkScene (the existing bezier graph model) ────

// Map a live presence state to the three-tone graph node language the bezier
// graph already understands (working = earning, online = idle, offline = seen).
const liveNodeTone = (node: ChatWorldPylonScene["nodes"][number]): PylonNetworkNode["tone"] => {
  if (node.state === "assignment_ready") return "working"
  if (node.online) return "online"
  return "offline"
}

// Project the live ChatWorldPylonScene onto the PylonNetworkScene the existing
// renderer consumes. Returns null when there is nothing live to show (no
// snapshot yet, or an honest zero-state) so the caller keeps its static seed.
export const liveChatWorldNetworkScene = (
  live: ChatWorldPylonScene | null,
): PylonNetworkScene | null => {
  if (live === null || live.empty || live.nodes.length === 0) return null

  const nodes: PylonNetworkNode[] = live.nodes.map((node) => {
    const tone = liveNodeTone(node)
    return { id: node.id, label: node.label, tone, flowing: tone === "working" }
  })

  const sessionsOnline = nodes.filter((n) => n.tone === "working").length
  // Activity glow rides the share of online pylons that are actually working,
  // lifted by fleet growth so a richer network reads as livelier. Bounded [0,1].
  const workingShare = live.onlineNow > 0 ? sessionsOnline / live.onlineNow : 0
  const activityIntensity = Math.max(
    0,
    Math.min(1, Number((workingShare * 0.7 + live.growth.brightness * 0.3).toFixed(3))),
  )

  return {
    activityIntensity,
    dormant: live.onlineNow === 0,
    onlineNow: live.onlineNow,
    sessionsOnlineNow: sessionsOnline,
    sellableOnlineNow: nodes.filter((n) => n.tone !== "offline").length,
    walletReadyNow: live.nodes.filter((n) => n.state === "wallet_ready").length,
    assignmentReadyNow: sessionsOnline,
    seen24h: nodes.length,
    registeredTotal: nodes.length,
    satsSettled24h: 0,
    satsSettledTotal: live.growth.settledSats,
    trainingAssignedContributors: 0,
    trainingAcceptedContributors: 0,
    trainingProgressContributors: 0,
    nodes,
    asOfLabel: live.asOfLabel,
  }
}

// ── Payment particles → evidence-bound entities / beams / bursts ──────────────

// Deterministic ring layout for payment endpoints, matched to the pylon graph's
// own ring (ui/pylon-network-visualization.ts ringPosition) so a beam flies
// between the visible pylon positions rather than floating off-graph.
const PAYMENT_RING_RADIUS = 2.4
const endpointRingPosition = (index: number, count: number): TrainingRunVector => {
  const radius = PAYMENT_RING_RADIUS + Math.min(1.6, count / 40)
  const angle = count <= 0 ? 0 : (2 * Math.PI * index) / count
  const height = 0.52 + Math.sin(index * 1.61803398875 + count * 0.23) * 0.64
  return [
    Number((Math.cos(angle) * radius).toFixed(3)),
    Number((Math.sin(angle) * radius * 0.62).toFixed(3)),
    Number(height.toFixed(3)),
  ]
}

// An entity status the renderer's status→color map reads. real_bitcoin earns the
// "verified" (gold-family) tone; credited settlement reads as "active". These are
// the public statuses the shared training-run entity color map honors.
const endpointStatus = (particle: PaymentParticle): string =>
  particle.realBitcoinMoved ? "verified" : "active"

export type ChatWorldPaymentEndpointSource = "station" | "avatar" | "fallback"

export type ChatWorldPaymentEndpoint = Readonly<{
  ref: string
  label: string
  position: TrainingRunVector
  source: ChatWorldPaymentEndpointSource
}>

export type ChatWorldPaymentEndpointIndex =
  ReadonlyMap<string, ChatWorldPaymentEndpoint>

const finite = (value: number): boolean =>
  Number.isFinite(value) && !Number.isNaN(value)

const vectorFromXYZ = (
  x: number,
  y: number,
  z: number,
): TrainingRunVector | null =>
  finite(x) && finite(y) && finite(z)
    ? [
        Number(x.toFixed(3)),
        Number(y.toFixed(3)),
        Number(z.toFixed(3)),
      ]
    : null

// ── Public SpacetimeDB world → visible station/avatar entities ──────────────

export const CHAT_WORLD_STATION_NODE_PREFIX = "world:station:"
export const CHAT_WORLD_AVATAR_NODE_PREFIX = "world:avatar:"

export type ChatWorldMultiplayerLayer = {
  readonly entities: ReadonlyArray<TrainingRunEntityDefinition>
}

export const chatWorldMultiplayerLayer = (
  world: ChatWorldMultiplayerProjection | null | undefined,
): ChatWorldMultiplayerLayer => {
  if (world?.connected !== true) return { entities: [] }

  const entities: TrainingRunEntityDefinition[] = []

  for (const station of world.stations) {
    const position = vectorFromXYZ(station.x, station.y, station.z)
    if (position === null) continue
    entities.push({
      id: `${CHAT_WORLD_STATION_NODE_PREFIX}${station.pylonRef}`,
      label: station.label,
      status: "verified",
      position,
    })
  }

  for (const agent of world.agents) {
    const position = vectorFromXYZ(agent.x, agent.y, agent.z)
    if (position === null) continue
    const chats =
      agent.chatMessages.length > 0
        ? ` · ${agent.chatMessages.length} chat`
        : ""
    entities.push({
      id: `${CHAT_WORLD_AVATAR_NODE_PREFIX}${agent.avatarRef}`,
      label: `${agent.label} · ${agent.movementMode}${chats}`,
      status: agent.attentionRefs.length > 0 ? "sync" : "active",
      position,
    })
  }

  return { entities }
}

const endpointRefKeys = (ref: string): ReadonlyArray<string> => {
  const trimmed = ref.trim()
  if (trimmed.length === 0) return []
  const lower = trimmed.toLowerCase()
  return [
    trimmed,
    lower,
    lower.replace(/:/g, "."),
    lower.replace(/\./g, ":"),
  ].filter((key, index, keys) => key.length > 0 && keys.indexOf(key) === index)
}

const indexEndpoint = (
  index: Map<string, ChatWorldPaymentEndpoint>,
  ref: string,
  endpoint: ChatWorldPaymentEndpoint,
): void => {
  for (const key of endpointRefKeys(ref)) {
    if (!index.has(key)) index.set(key, endpoint)
  }
}

// Build an exact-ref endpoint index from the already-projected public world rows.
// Stations map by pylonRef; avatars map by both actorRef and avatarRef. The
// fallback ring below remains explicitly labeled, so unknown endpoints never
// pretend to have a real SpacetimeDB position.
export const chatWorldPaymentEndpointIndex = (
  world: ChatWorldMultiplayerProjection | null | undefined,
): ChatWorldPaymentEndpointIndex => {
  const index = new Map<string, ChatWorldPaymentEndpoint>()
  if (world?.connected !== true) return index

  for (const station of world.stations) {
    const position = vectorFromXYZ(station.x, station.y, station.z)
    if (position === null) continue
    const endpoint: ChatWorldPaymentEndpoint = {
      ref: station.pylonRef,
      label: station.label,
      position,
      source: "station",
    }
    indexEndpoint(index, station.pylonRef, endpoint)
  }

  for (const agent of world.agents) {
    const position = vectorFromXYZ(agent.x, agent.y, agent.z)
    if (position === null) continue
    const endpoint: ChatWorldPaymentEndpoint = {
      ref: agent.actorRef,
      label: agent.label,
      position,
      source: "avatar",
    }
    indexEndpoint(index, agent.actorRef, endpoint)
    indexEndpoint(index, agent.avatarRef, endpoint)
  }

  return index
}

export const resolveChatWorldPaymentEndpoint = (
  ref: string,
  fallbackPosition: TrainingRunVector,
  endpoints: ChatWorldPaymentEndpointIndex,
): ChatWorldPaymentEndpoint => {
  for (const key of endpointRefKeys(ref)) {
    const endpoint = endpoints.get(key)
    if (endpoint !== undefined) return endpoint
  }
  return {
    ref,
    label: `unresolved ${ref}`,
    position: fallbackPosition,
    source: "fallback",
  }
}

// A compact, inspector-friendly label that survives into the node-selection
// detail. It starts with the receipt/source ref so clicking either endpoint
// opens evidence first, then names the real station/avatar or fallback context.
const endpointLabel = (
  sourceRef: string,
  endpoint: ChatWorldPaymentEndpoint,
  particle: PaymentParticle,
  role: "from" | "to",
): string => {
  const amount = role === "to" && particle.amountSats > 0
    ? `${particle.amountSats} sats`
    : role
  return `${sourceRef} · ${amount} · ${endpoint.label} · ${endpoint.source}`
}

export type ChatWorldPaymentLayer = {
  readonly entities: ReadonlyArray<TrainingRunEntityDefinition>
  readonly beams: ReadonlyArray<TrainingRunBeamDefinition>
  readonly bursts: ReadonlyArray<TrainingRunBurstDefinition>
}

// Build the evidence-bound payment layer from the active particles. Stable ids
// per particle keep positions steady across re-renders; the sourceRefs ride both
// the beam (motion evidence) and the endpoint entities (click → receipt). Every
// particle here is already evidence-bound (activityEventToParticle dropped the
// rest), and we additionally refuse any that somehow lost their refs.
export const chatWorldPaymentLayer = (
  particles: ReadonlyArray<PaymentParticle>,
  world?: ChatWorldMultiplayerProjection | null,
): ChatWorldPaymentLayer => {
  const renderable = particles.filter((p) => p.sourceRefs.length > 0)
  const endpoints = chatWorldPaymentEndpointIndex(world)

  const entityById = new Map<string, TrainingRunEntityDefinition>()
  const beams: TrainingRunBeamDefinition[] = []
  const bursts: TrainingRunBurstDefinition[] = []

  renderable.forEach((particle, index) => {
    const primaryRef = particle.sourceRefs[0] as string
    const fromId = `pay:${particle.id}:from`
    const toId = `pay:${particle.id}:to`
    const fromEndpoint = resolveChatWorldPaymentEndpoint(
      particle.fromRef,
      endpointRingPosition(index * 2, renderable.length * 2),
      endpoints,
    )
    const toEndpoint = resolveChatWorldPaymentEndpoint(
      particle.toRef,
      endpointRingPosition(index * 2 + 1, renderable.length * 2),
      endpoints,
    )

    if (!entityById.has(fromId)) {
      entityById.set(fromId, {
        id: fromId,
        status: endpointStatus(particle),
        label: endpointLabel(primaryRef, fromEndpoint, particle, "from"),
        position: fromEndpoint.position,
      })
    }
    if (!entityById.has(toId)) {
      entityById.set(toId, {
        id: toId,
        status: endpointStatus(particle),
        label: endpointLabel(primaryRef, toEndpoint, particle, "to"),
        position: toEndpoint.position,
      })
    }

    const evidence = {
      motionId: particle.id,
      motionKind: particle.realBitcoinMoved ? "real_bitcoin_moved" : "settlement_recorded",
      sourceRefs: particle.sourceRefs,
      generatedAt: particle.ts ?? undefined,
      simulated: false,
    } as const

    beams.push({ fromId, toId, ...evidence })
    bursts.push({ atId: toId, ...evidence })
  })

  return { entities: [...entityById.values()], beams, bursts }
}

// ── Compose pylons + multiplayer + payments into one visualization object ────

export const withChatWorldMultiplayerLayer = (
  base: TrainingRunVisualizationOptions,
  world: ChatWorldMultiplayerProjection | null | undefined,
): TrainingRunVisualizationOptions => {
  const layer = chatWorldMultiplayerLayer(world)
  if (layer.entities.length === 0) return base
  return {
    ...base,
    entities: [...(base.entities ?? []), ...layer.entities],
  }
}

// Take the pylon-graph options (already built from the live-or-seed network) and
// overlay the evidence-bound payment layer. motionPolicy.evidence is forced to
// "required" so the shared renderer animates a beam/burst ONLY when it carries
// sourceRefs — a hard backstop behind the pure mappers' own evidence checks.
export const withChatWorldPaymentLayer = (
  base: TrainingRunVisualizationOptions,
  particles: ReadonlyArray<PaymentParticle>,
  world?: ChatWorldMultiplayerProjection | null,
): TrainingRunVisualizationOptions => {
  const layer = chatWorldPaymentLayer(particles, world)
  if (layer.entities.length === 0) return base
  return {
    ...base,
    entities: [...(base.entities ?? []), ...layer.entities],
    beams: [...(base.beams ?? []), ...layer.beams],
    bursts: [...(base.bursts ?? []), ...layer.bursts],
    motionPolicy: { ...(base.motionPolicy ?? {}), evidence: "required" },
  }
}
