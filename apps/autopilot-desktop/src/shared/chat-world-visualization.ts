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
//      actor pylon → target pylon (gold real-bitcoin motion), with a settlement
//      burst on the target, plus clickable endpoint
//      entities that carry the receipt sourceRef. motionPolicy.evidence is
//      "required", so the renderer refuses to animate any motion without refs —
//      and activityEventToParticle already drops particles with no sourceRef, so
//      nothing flies without real evidence (§5 evidence-bound motion contract).
//
// The shared renderer animates beams/bursts and registers entity hit targets
// itself; size ∝ amountSats and the gold/dim split are carried on the descriptor
// (particle.size / particle.realBitcoinMoved) so they survive into the inspector
// even where the shared bezier renderer draws a single beam style.

import { verseIconRecipeForId } from "@openagentsinc/three-effect/core"
import type {
  TrainingRunEntityDefinition,
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunRemoteAvatarDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
  VerseIconRecipe,
} from "@openagentsinc/three-effect/core"

import type { ChatWorldPylonScene, PaymentParticle } from "./chat-world-scene.js"
import type { ChatWorldMultiplayerProjection } from "./chat-world-multiplayer.js"
import {
  type PylonNetworkNode,
  type PylonNetworkScene,
} from "./pylon-network-scene.js"
import {
  appendVerseVisualization,
  roundedVerseVector,
} from "./verse-scene-helpers.js"

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
    return {
      id: node.id,
      label: node.label,
      tone,
      flowing: tone === "working",
      growth: node.growth,
    }
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
  return roundedVerseVector([
    Math.cos(angle) * radius,
    Math.sin(angle) * radius * 0.62,
    height,
  ])
}

// An entity status the renderer's status→color map reads. Real bitcoin earns the
// "verified" (gold-family) tone.
const endpointStatus = (_particle: PaymentParticle): string => "verified"

export type ChatWorldPaymentEndpointSource =
  | "avatar"
  | "fallback"
  | "gateway"
  | "station"

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

// ── Public Cloudflare world world → visible station/avatar entities ──────────────

export const CHAT_WORLD_STATION_NODE_PREFIX = "world:station:"
export const CHAT_WORLD_GATEWAY_NODE_PREFIX = "world:gateway:"
export const CHAT_WORLD_INFERENCE_NODE_PREFIX = "world:inference:"
export const CHAT_WORLD_AVATAR_NODE_PREFIX = "world:avatar:"
// The stale FADE is disabled. It used to drop an idle-but-present remote's body
// to 0.35 opacity after 6s (via three-effect's liveness), leaving only the bright
// accent ring — so a player standing still looked like they vanished into a ring.
// An idle avatar sends no position keepalive, so `Date.now() - updatedAtMs`
// crossed 6s while the player was still very much present. A huge stale threshold
// keeps present avatars solid. Despawn (full removal) is kept at 12s as a
// client-side backstop; the server (leave_region / position TTL) is the real
// removal signal that drops avatars from world.agents.
export const CHAT_WORLD_REMOTE_AVATAR_STALE_AFTER_MS = 86_400_000
export const CHAT_WORLD_REMOTE_AVATAR_DESPAWN_AFTER_MS = 12_000
export const CHAT_WORLD_TARGET_DEFAULT_MAX_CANDIDATES = 24
export const CHAT_WORLD_TARGET_DEFAULT_MAX_DISTANCE_METERS = 96

export type ChatWorldMultiplayerLayer = {
  readonly entities: ReadonlyArray<ChatWorldVisualEntityDefinition>
  readonly remoteAvatars: ReadonlyArray<TrainingRunRemoteAvatarDefinition>
}

type ChatWorldGatewayLane =
  ChatWorldMultiplayerProjection["gateways"][number]["lane"]

export type ChatWorldVisualEntityDefinition = TrainingRunEntityDefinition &
  Readonly<{
    gatewayLane?: ChatWorldGatewayLane
    iconRecipe?: VerseIconRecipe
    visualKind?: "default" | "gateway_portal"
  }>

export type ChatWorldMultiplayerLayerOptions = Readonly<{
  localAvatarRef?: string | null
  nowMs?: number
  staleAfterMs?: number
  despawnAfterMs?: number
}>

export type ChatWorldTabTargetKind = "pylon" | "avatar"

export type ChatWorldTargetVisibility = Readonly<{
  id: string
  screenCenterX: number
  screenCenterY: number
  visible: boolean
  occluded?: boolean
}>

export type ChatWorldTabTargetCandidate = Readonly<{
  id: string
  ref: string
  kind: ChatWorldTabTargetKind
  label: string
  position: TrainingRunVector
  screenCenterDistance: number
  worldDistanceMeters: number
}>

export type ChatWorldTabTargetOptions = Readonly<{
  despawnAfterMs?: number
  localAvatarRef?: string | null
  maxCandidates?: number
  maxDistanceMeters?: number
  nowMs?: number
  viewerPosition: TrainingRunVector
  visibility: ReadonlyArray<ChatWorldTargetVisibility>
}>

const movementModeToRemoteAvatarAnimation = (
  movementMode: string,
): TrainingRunRemoteAvatarDefinition["animation"] => {
  const normalized = movementMode.trim().toLowerCase()
  if (normalized === "running" || normalized === "run") return "run"
  if (normalized === "walking" || normalized === "walk") return "walk"
  return "idle"
}

const vectorDistanceMeters = (
  a: TrainingRunVector,
  b: TrainingRunVector,
): number => {
  const dx = a[0] - b[0]
  const dy = a[1] - b[1]
  const dz = a[2] - b[2]
  return Math.sqrt(dx * dx + dy * dy + dz * dz)
}

const finiteVector = (vector: TrainingRunVector): boolean =>
  finite(vector[0]) && finite(vector[1]) && finite(vector[2])

const targetKindRank = (kind: ChatWorldTabTargetKind): number =>
  kind === "pylon" ? 0 : 1

const visibilityMapFromRows = (
  rows: ReadonlyArray<ChatWorldTargetVisibility>,
): ReadonlyMap<string, ChatWorldTargetVisibility> => {
  const out = new Map<string, ChatWorldTargetVisibility>()
  for (const row of rows) {
    if (row.id.trim().length === 0) continue
    out.set(row.id, row)
  }
  return out
}

const firstVisibleTarget = (
  visibility: ReadonlyMap<string, ChatWorldTargetVisibility>,
  ids: ReadonlyArray<string>,
): ChatWorldTargetVisibility | null => {
  for (const id of ids) {
    const target = visibility.get(id)
    if (
      target !== undefined &&
      target.visible === true &&
      target.occluded !== true &&
      finite(target.screenCenterX) &&
      finite(target.screenCenterY)
    ) {
      return target
    }
  }
  return null
}

export const chatWorldVisibleTargetCandidates = (
  world: ChatWorldMultiplayerProjection | null | undefined,
  options: ChatWorldTabTargetOptions,
): ReadonlyArray<ChatWorldTabTargetCandidate> => {
  if (world?.connected !== true || !finiteVector(options.viewerPosition)) return []
  const maxCandidates = Math.max(
    0,
    Math.floor(options.maxCandidates ?? CHAT_WORLD_TARGET_DEFAULT_MAX_CANDIDATES),
  )
  if (maxCandidates === 0) return []
  const maxDistanceMeters = Math.max(
    0,
    options.maxDistanceMeters ?? CHAT_WORLD_TARGET_DEFAULT_MAX_DISTANCE_METERS,
  )
  const visibility = visibilityMapFromRows(options.visibility)
  const localAvatarRef = options.localAvatarRef?.trim() ?? ""
  const nowMs = options.nowMs ?? world.projectedAtMs
  const despawnAfterMs = options.despawnAfterMs ?? CHAT_WORLD_REMOTE_AVATAR_DESPAWN_AFTER_MS
  const candidates: ChatWorldTabTargetCandidate[] = []

  const maybePush = (input: {
    readonly id: string
    readonly ref: string
    readonly label: string
    readonly kind: ChatWorldTabTargetKind
    readonly position: TrainingRunVector
    readonly visibilityIds: ReadonlyArray<string>
  }): void => {
    if (!finiteVector(input.position)) return
    const visible = firstVisibleTarget(visibility, input.visibilityIds)
    if (visible === null) return
    const worldDistanceMeters = vectorDistanceMeters(options.viewerPosition, input.position)
    if (worldDistanceMeters > maxDistanceMeters) return
    candidates.push({
      id: input.id,
      ref: input.ref,
      kind: input.kind,
      label: input.label,
      position: input.position,
      screenCenterDistance: Number(
        Math.hypot(visible.screenCenterX, visible.screenCenterY).toFixed(3),
      ),
      worldDistanceMeters: Number(worldDistanceMeters.toFixed(3)),
    })
  }

  for (const station of world.stations) {
    const position = vectorFromXYZ(station.x, station.y, station.z)
    if (position === null) continue
    const id = `${CHAT_WORLD_STATION_NODE_PREFIX}${station.pylonRef}`
    maybePush({
      id,
      ref: station.pylonRef,
      kind: "pylon",
      label: station.label,
      position,
      visibilityIds: [id, station.pylonRef],
    })
  }

  for (const agent of world.agents) {
    if (localAvatarRef.length > 0 && agent.avatarRef === localAvatarRef) continue
    if (Math.max(0, nowMs - agent.lastSeenEpochMs) >= despawnAfterMs) continue
    const position = vectorFromXYZ(agent.x, agent.y, agent.z)
    if (position === null) continue
    maybePush({
      id: agent.avatarRef,
      ref: agent.avatarRef,
      kind: "avatar",
      label: agent.label,
      position,
      visibilityIds: [
        agent.avatarRef,
        `${CHAT_WORLD_AVATAR_NODE_PREFIX}${agent.avatarRef}`,
        agent.actorRef,
      ],
    })
  }

  return candidates
    .sort((a, b) =>
      a.screenCenterDistance - b.screenCenterDistance ||
      a.worldDistanceMeters - b.worldDistanceMeters ||
      targetKindRank(a.kind) - targetKindRank(b.kind) ||
      a.label.localeCompare(b.label),
    )
    .slice(0, maxCandidates)
}

export const chatWorldMultiplayerLayer = (
  world: ChatWorldMultiplayerProjection | null | undefined,
  options: ChatWorldMultiplayerLayerOptions = {},
): ChatWorldMultiplayerLayer => {
  if (world?.connected !== true) return { entities: [], remoteAvatars: [] }

  const entities: ChatWorldVisualEntityDefinition[] = []
  const remoteAvatars: TrainingRunRemoteAvatarDefinition[] = []
  const nowMs = options.nowMs ?? world.projectedAtMs
  const staleAfterMs = options.staleAfterMs ?? CHAT_WORLD_REMOTE_AVATAR_STALE_AFTER_MS
  const despawnAfterMs = options.despawnAfterMs ?? CHAT_WORLD_REMOTE_AVATAR_DESPAWN_AFTER_MS
  const localAvatarRef = options.localAvatarRef?.trim() ?? ""

  for (const station of world.stations) {
    const position = vectorFromXYZ(station.x, station.y, station.z)
    if (position === null) continue
    entities.push({
      id: `${CHAT_WORLD_STATION_NODE_PREFIX}${station.pylonRef}`,
      label: station.label,
      status: "verified",
      position,
      iconRecipe: verseIconRecipeForId(station.pylonRef || "pylon"),
    })
  }

  for (const agent of world.agents) {
    const position = vectorFromXYZ(agent.x, agent.y, agent.z)
    if (position === null) continue
    if (localAvatarRef.length > 0 && agent.avatarRef === localAvatarRef) continue
    const ageMs = Math.max(0, nowMs - agent.lastSeenEpochMs)
    if (ageMs >= despawnAfterMs) continue
    const animation = movementModeToRemoteAvatarAnimation(agent.movementMode)
    remoteAvatars.push({
      id: agent.avatarRef,
      label: agent.label,
      position,
      yaw: agent.yaw,
      ...(animation !== undefined ? { animation } : {}),
      updatedAtMs: agent.lastSeenEpochMs,
      stale: ageMs >= staleAfterMs,
      color: agent.color,
      avatarKind: agent.avatarKind,
      actorRef: agent.actorRef,
      labelVisibility: "hidden",
    })
  }

  return { entities, remoteAvatars }
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
// pretend to have a real Cloudflare world position.
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

  for (const gateway of world.gateways) {
    const position = vectorFromXYZ(gateway.x, gateway.y, gateway.z)
    if (position === null) continue
    const endpoint: ChatWorldPaymentEndpoint = {
      ref: gateway.gatewayRef,
      label: gateway.label,
      position,
      source: "gateway",
    }
    indexEndpoint(index, gateway.gatewayRef, endpoint)
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

// Short scene labels keep the 3D world readable. Full receipt/source detail
// stays on `detail`, which the inspector opens after selection.
const endpointLabel = (
  endpoint: ChatWorldPaymentEndpoint,
  role: "from" | "to",
): string => {
  if (endpoint.source !== "fallback" && endpoint.label.trim().length > 0) {
    return endpoint.label
  }
  return role === "from" ? "Tip sender" : "Payment target"
}

const endpointDetail = (
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
  readonly entities: ReadonlyArray<ChatWorldVisualEntityDefinition>
  readonly beams: ReadonlyArray<TrainingRunBeamDefinition>
  readonly bursts: ReadonlyArray<TrainingRunBurstDefinition>
}

export type ChatWorldInferenceLayer = ChatWorldPaymentLayer

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

  const entityById = new Map<string, ChatWorldVisualEntityDefinition>()
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
        label: endpointLabel(fromEndpoint, "from"),
        detail: endpointDetail(primaryRef, fromEndpoint, particle, "from"),
        position: fromEndpoint.position,
        iconRecipe: verseIconRecipeForId(fromEndpoint.ref),
      })
    }
    if (!entityById.has(toId)) {
      entityById.set(toId, {
        id: toId,
        status: endpointStatus(particle),
        label: endpointLabel(toEndpoint, "to"),
        detail: endpointDetail(primaryRef, toEndpoint, particle, "to"),
        position: toEndpoint.position,
        iconRecipe: verseIconRecipeForId("zap"),
      })
    }

    const evidence = {
      motionId: particle.id,
      motionKind: particle.kind,
      sourceRefs: particle.sourceRefs,
      ...(typeof particle.ts === "string" ? { generatedAt: particle.ts } : {}),
      simulated: false,
    } as const

    beams.push({ fromId, toId, ...evidence })
    bursts.push({ atId: toId, ...evidence })
  })

  return { entities: [...entityById.values()], beams, bursts }
}

const inferenceStatus = (
  verification: ChatWorldMultiplayerProjection["inferenceEvents"][number]["verification"],
): string => {
  if (verification === "failed") return "blocked"
  if (
    verification === "exact_trace_replay" ||
    verification === "test_passed"
  ) {
    return "verified"
  }
  if (verification === "seeded") return "active"
  return "queued"
}

const inferenceEndpointFor = (
  refs: ReadonlyArray<string>,
  fallback: ChatWorldPaymentEndpoint,
  endpoints: ChatWorldPaymentEndpointIndex,
): ChatWorldPaymentEndpoint => {
  for (const ref of refs) {
    const endpoint = resolveChatWorldPaymentEndpoint(ref, fallback.position, endpoints)
    if (endpoint.source !== "fallback") return endpoint
  }
  return fallback
}

const inferenceFallbackPosition = (index: number, total: number): TrainingRunVector =>
  endpointRingPosition(index + total * 2, Math.max(1, total * 3))

const inferenceSourceRefs = (
  event: ChatWorldMultiplayerProjection["inferenceEvents"][number],
): ReadonlyArray<string> => {
  const out: string[] = []
  for (const ref of event.sourceRefs) {
    const trimmed = ref.trim()
    if (trimmed.length > 0 && !out.includes(trimmed)) out.push(trimmed)
  }
  return out
}

const inferenceMotionSimulated = (refs: ReadonlyArray<string>): boolean =>
  refs.some((ref) => /\bfixture\b|fixture:|scaffold/i.test(ref))

export const chatWorldInferenceLayer = (
  world: ChatWorldMultiplayerProjection | null | undefined,
): ChatWorldInferenceLayer => {
  if (world?.connected !== true) return { entities: [], beams: [], bursts: [] }
  const endpoints = chatWorldPaymentEndpointIndex(world)
  const entityById = new Map<string, ChatWorldVisualEntityDefinition>()
  const beams: TrainingRunBeamDefinition[] = []
  const bursts: TrainingRunBurstDefinition[] = []

  for (const gateway of world.gateways) {
    const position = vectorFromXYZ(gateway.x, gateway.y, gateway.z)
    if (position === null) continue
    entityById.set(`${CHAT_WORLD_GATEWAY_NODE_PREFIX}${gateway.gatewayRef}`, {
      id: `${CHAT_WORLD_GATEWAY_NODE_PREFIX}${gateway.gatewayRef}`,
      label: gateway.label,
      detail: `${gateway.providerLabel} · ${gateway.lane} · ${gateway.gatewayRef}`,
      status: gateway.status === "working" ? "active" : gateway.status,
      position,
      iconRecipe: verseIconRecipeForId(`gateway:${gateway.lane}:${gateway.gatewayRef}`),
      visualKind: "gateway_portal",
      gatewayLane: gateway.lane,
    })
  }

  world.inferenceEvents.forEach((event, index) => {
    const refs = inferenceSourceRefs(event)
    if (refs.length === 0) return
    const fromId = `${CHAT_WORLD_INFERENCE_NODE_PREFIX}${event.eventRef}:from`
    const toId = `${CHAT_WORLD_INFERENCE_NODE_PREFIX}${event.eventRef}:to`
    const status = inferenceStatus(event.verification)
    const fallbackFrom: ChatWorldPaymentEndpoint = {
      ref: event.requestRef,
      label: "Khala request",
      position: inferenceFallbackPosition(index * 2, world.inferenceEvents.length),
      source: "fallback",
    }
    const fallbackTo: ChatWorldPaymentEndpoint = {
      ref: event.gatewayRef ?? event.route,
      label: event.gatewayRef === null ? "Khala route" : "Gateway route",
      position: inferenceFallbackPosition(index * 2 + 1, world.inferenceEvents.length),
      source: "fallback",
    }
    const fromEndpoint = inferenceEndpointFor(event.workerRefs, fallbackFrom, endpoints)
    const toEndpoint = inferenceEndpointFor(
      event.gatewayRef === null ? event.workerRefs : [event.gatewayRef, ...event.workerRefs],
      fallbackTo,
      endpoints,
    )
    const costDetail = event.costMsat === null
      ? "cost not public"
      : `${event.costMsat} msat`

    entityById.set(fromId, {
      id: fromId,
      label: endpointLabel(fromEndpoint, "from"),
      detail: `${event.receiptRef} · ${event.model} · ${event.route} · ${fromEndpoint.source}`,
      status,
      position: fromEndpoint.position,
      iconRecipe: verseIconRecipeForId(fromEndpoint.ref),
    })
    const toGatewayLane = toEndpoint.source === "gateway"
      ? world.gateways.find(gateway => gateway.gatewayRef === toEndpoint.ref)?.lane
      : undefined

    entityById.set(toId, {
      id: toId,
      label: endpointLabel(toEndpoint, "to"),
      detail: `${event.receiptRef} · ${costDetail} · ${toEndpoint.label} · ${toEndpoint.source}`,
      status,
      position: toEndpoint.position,
      iconRecipe: verseIconRecipeForId(
        toEndpoint.source === "gateway" ? `gateway:${toEndpoint.ref}` : "receipt",
      ),
      ...(toGatewayLane === undefined
        ? {}
        : {
            gatewayLane: toGatewayLane,
            visualKind: "gateway_portal" as const,
          }),
      ...(toEndpoint.source === "gateway" && toGatewayLane === undefined
        ? {
            visualKind: "gateway_portal" as const,
          }
        : {}),
    })

    const evidence = {
      motionId: event.eventRef,
      motionKind: event.gatewayRef === null
        ? "khala_in_world_inference"
        : "khala_gateway_inference",
      sourceRefs: refs,
      generatedAt: event.generatedAt,
      simulated: inferenceMotionSimulated(refs),
    } as const
    beams.push({ fromId, toId, style: "crackling_arc", ...evidence })
    if (
      event.settled ||
      event.verification === "exact_trace_replay" ||
      event.verification === "test_passed"
    ) {
      bursts.push({ atId: toId, ...evidence })
    }
  })

  return { entities: [...entityById.values()], beams, bursts }
}

// ── Compose pylons + multiplayer + inference + payments into one visualization object ────

export const withChatWorldMultiplayerLayer = (
  base: TrainingRunVisualizationOptions,
  world: ChatWorldMultiplayerProjection | null | undefined,
  options: ChatWorldMultiplayerLayerOptions = {},
): TrainingRunVisualizationOptions => {
  const layer = chatWorldMultiplayerLayer(world, options)
  if (layer.entities.length === 0 && layer.remoteAvatars.length === 0) return base
  return {
    ...appendVerseVisualization(base, {
      entities: layer.entities,
      remoteAvatars: layer.remoteAvatars,
    }),
    remoteAvatarInterpolation: {
      ...(base.remoteAvatarInterpolation ?? {}),
      despawnAfterMs: options.despawnAfterMs ?? CHAT_WORLD_REMOTE_AVATAR_DESPAWN_AFTER_MS,
      staleAfterMs: options.staleAfterMs ?? CHAT_WORLD_REMOTE_AVATAR_STALE_AFTER_MS,
    },
  }
}

export const withChatWorldInferenceLayer = (
  base: TrainingRunVisualizationOptions,
  world: ChatWorldMultiplayerProjection | null | undefined,
): TrainingRunVisualizationOptions => {
  const layer = chatWorldInferenceLayer(world)
  if (layer.entities.length === 0) return base
  return {
    ...appendVerseVisualization(base, {
      entities: layer.entities,
      beams: layer.beams,
      bursts: layer.bursts,
    }),
    motionPolicy: { ...(base.motionPolicy ?? {}), evidence: "required" },
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
    ...appendVerseVisualization(base, {
      entities: layer.entities,
      beams: layer.beams,
      bursts: layer.bursts,
    }),
    motionPolicy: { ...(base.motionPolicy ?? {}), evidence: "required" },
  }
}
