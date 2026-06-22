// Chat-World live subscriptions (P1 #5736 + P2 #5737).
//
// Browser-side feeds for the "agent MMORPG" scene behind chat
// (docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md §3.3):
//   - subscribePylonScene(dispatch)      P1: poll /api/public/pylon-stats →
//                                        ChatWorldPylonScene (live nodes + growth)
//   - subscribePaymentParticles(dispatch) P2: SSE /api/public/activity-timeline/
//                                        stream (poll fallback) → PaymentParticle
//   - subscribeSpacetimeWorld(dispatch)   P2: openagents-world SpacetimeDB
//                                        public rows → multiplayer projection
//
// All feeds are FLAG-GATED and return an unsubscribe() the scene calls
// on teardown. The Foldkit subscription layer owns their lifecycle and the
// hooks themselves stay inert when the matching Verse flags are disabled.
//
// "browser UA": these run inside the Electrobun webview and fetch the public
// openagents.com endpoints directly, so requests carry the webview's browser
// User-Agent (no node/bun UA), matching how the public site is served.
//
// Evidence-bound (§5): subscribePaymentParticles only ever dispatches particles
// that carry a real sourceRef (activityEventToParticle drops the rest).

import {
  createBrowserWorldTransport,
  createWorldClient,
  type WorldClient,
} from "@openagentsinc/world-client"
import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  worldAvatarRefForCharacter,
  type WorldCommandEnvelope,
  type WorldCommandName,
  type WorldIsoTimestamp,
  type WorldRef,
  type WorldRegionRef,
  type WorldSequence,
} from "@openagentsinc/world-contract"
import { Effect } from "effect"
import {
  activityEventToParticle,
  chatWorldFlags,
  projectChatWorldPylonScene,
  type ActivityEvent,
  type ChatWorldPylonScene,
  type PaymentParticle,
} from "../shared/chat-world-scene.js"
import {
  CHAT_WORLD_DESKTOP_AVATAR_REF,
  DEFAULT_OA_CHARACTER_ID,
  DEFAULT_TASSADAR_WORLD_RUN_REF,
  OPENAGENTS_WORLD_DATABASE,
  OPENAGENTS_WORLD_URL,
  chatWorldDesktopAvatarRef,
  chatWorldRegionRefForRun,
  chatWorldMultiplayerSubscriptionQueries,
  sanitizeChatWorldCharacterId,
  type ChatWorldMultiplayerProjection,
} from "../shared/chat-world-multiplayer.js"
import {
  chatWorldDesktopAvatarIdentity,
  defaultChatWorldRegionForRun,
  planChatWorldAvatarPositionWrite,
  projectChatWorldClientWorld,
  projectChatWorldSpacetimeRows,
  type ChatWorldAvatarPositionPlan,
  type ChatWorldAvatarPositionWrite,
  type ChatWorldDesktopAvatarIdentity,
  type ChatWorldRegionRow,
  type ChatWorldSpacetimeRows,
} from "../shared/chat-world-spacetimedb.js"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene.js"

const PUBLIC_BASE_URL = "https://openagents.com"
const PYLON_STATS_PATH = "/api/public/pylon-stats"
const ACTIVITY_POLL_PATH = "/api/public/activity-timeline"
const ACTIVITY_STREAM_PATH = "/api/public/activity-timeline/stream"

const PYLON_POLL_INTERVAL_MS = 4_000
const ACTIVITY_POLL_INTERVAL_MS = 5_000
export type Unsubscribe = () => void

const noop: Unsubscribe = () => {}

// The character id dep may be an eager value or a lazy getter. Resolving lazily
// lets a late globalThis.__OA_CHARACTER injection (Bun host → dom-ready) reach
// join/move-time reducer calls even if the subscription mounted first.
type ChatWorldCharacterIdInput = string | null | (() => string | null)

const resolveChatWorldCharacterId = (
  input: ChatWorldCharacterIdInput | undefined,
): string | null | undefined =>
  typeof input === "function" ? input() : input

// Injectable seams so the subscriptions are testable headlessly.
export type ChatWorldSubscriptionDeps = {
  readonly baseUrl?: string
  readonly fetchFn?: typeof fetch
  /** EventSource ctor (browser global); omit to use the platform one. */
  readonly eventSourceCtor?: typeof EventSource
  readonly setInterval?: (handler: () => void, ms: number) => unknown
  readonly clearInterval?: (handle: unknown) => void
  /** override flags (default: chatWorldFlags() from globalThis.__OA_FLAGS). */
  readonly flags?: {
    readonly CHAT_WORLD_SCENE?: boolean
    readonly CHAT_WORLD_PAYMENTS?: boolean
    readonly CHAT_WORLD_MULTIPLAYER?: boolean
  }
}

const resolveBaseUrl = (deps?: ChatWorldSubscriptionDeps): string =>
  (deps?.baseUrl ?? PUBLIC_BASE_URL).replace(/\/+$/, "")

const resolveSetInterval = (
  deps?: ChatWorldSubscriptionDeps,
): ((handler: () => void, ms: number) => unknown) =>
  deps?.setInterval ??
  ((handler, ms) => (globalThis as unknown as { setInterval: (h: () => void, m: number) => unknown }).setInterval(handler, ms))

const resolveClearInterval = (
  deps?: ChatWorldSubscriptionDeps,
): ((handle: unknown) => void) =>
  deps?.clearInterval ??
  ((handle) => (globalThis as unknown as { clearInterval: (h: unknown) => void }).clearInterval(handle))

// ─────────────────────────────────────────────────────────────────────────────
// P1 — live pylons
// ─────────────────────────────────────────────────────────────────────────────

export type PylonSceneDispatch = (scene: ChatWorldPylonScene) => void

// Poll pylon-stats and push a projected scene to `dispatch`. Fail-soft: a fetch
// error pushes the honest zero-state (projectChatWorldPylonScene(null)) rather
// than throwing or freezing the last snapshot. Returns unsubscribe().
export const subscribePylonScene = (
  dispatch: PylonSceneDispatch,
  deps?: ChatWorldSubscriptionDeps,
): Unsubscribe => {
  const flags = deps?.flags ?? chatWorldFlags()
  if (flags.CHAT_WORLD_SCENE !== true) return noop

  const baseUrl = resolveBaseUrl(deps)
  const fetchFn = deps?.fetchFn ?? fetch
  const url = `${baseUrl}${PYLON_STATS_PATH}`
  let stopped = false

  const poll = async (): Promise<void> => {
    try {
      const response = await fetchFn(url, { headers: { accept: "application/json" } })
      if (stopped) return
      if (!response.ok) {
        dispatch(projectChatWorldPylonScene(null))
        return
      }
      const snapshot = (await response.json()) as PylonStatsSnapshot
      if (stopped) return
      dispatch(projectChatWorldPylonScene(snapshot))
    } catch {
      if (!stopped) dispatch(projectChatWorldPylonScene(null))
    }
  }

  void poll()
  const handle = resolveSetInterval(deps)(() => void poll(), PYLON_POLL_INTERVAL_MS)

  return () => {
    stopped = true
    resolveClearInterval(deps)(handle)
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// P2 — payment particles
// ─────────────────────────────────────────────────────────────────────────────

export type PaymentParticleDispatch = (particle: PaymentParticle) => void

// Parse one SSE `data:` payload ({ event: <ActivityEvent> }) into a particle.
// Tolerant of either { event: {...} } (the worker's frame shape) or a bare event
// object. Returns null when it is not an honestly-renderable payment.
export const parseActivityStreamData = (raw: string): PaymentParticle | null => {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  const candidate =
    parsed && typeof parsed === "object" && "event" in (parsed as Record<string, unknown>)
      ? (parsed as { event: unknown }).event
      : parsed
  if (!candidate || typeof candidate !== "object") return null
  const event = candidate as ActivityEvent
  if (typeof event.eventRef !== "string" || typeof event.kind !== "string") return null
  return activityEventToParticle(event)
}

// Backfill / poll: pull the activity-timeline envelope once and push particles
// for every payment event it carries (used for "backfill last N on connect" and
// as the no-EventSource fallback).
const pollActivityOnce = async (
  baseUrl: string,
  fetchFn: typeof fetch,
  dispatch: PaymentParticleDispatch,
  isStopped: () => boolean,
): Promise<void> => {
  try {
    const response = await fetchFn(`${baseUrl}${ACTIVITY_POLL_PATH}`, {
      headers: { accept: "application/json" },
    })
    if (isStopped() || !response.ok) return
    const envelope = (await response.json()) as { events?: ReadonlyArray<ActivityEvent> }
    if (isStopped()) return
    for (const event of envelope.events ?? []) {
      const particle = activityEventToParticle(event)
      if (particle) dispatch(particle)
    }
  } catch {
    // fail-soft: no particles rather than throwing
  }
}

// Subscribe to the activity SSE stream and push a PaymentParticle per real
// money event. Prefers EventSource (live SSE); if EventSource is unavailable,
// falls back to polling the envelope. Always does one backfill poll on connect
// so the scene starts populated. Returns unsubscribe().
export const subscribePaymentParticles = (
  dispatch: PaymentParticleDispatch,
  deps?: ChatWorldSubscriptionDeps,
): Unsubscribe => {
  const flags = deps?.flags ?? chatWorldFlags()
  if (flags.CHAT_WORLD_PAYMENTS !== true) return noop

  const baseUrl = resolveBaseUrl(deps)
  const fetchFn = deps?.fetchFn ?? fetch
  let stopped = false
  const isStopped = (): boolean => stopped

  // Backfill last N events on connect (evidence-bound; non-payments dropped).
  void pollActivityOnce(baseUrl, fetchFn, dispatch, isStopped)

  const EventSourceCtor =
    deps?.eventSourceCtor ??
    (globalThis as unknown as { EventSource?: typeof EventSource }).EventSource

  // No EventSource (headless / older webview): poll the envelope on an interval.
  if (typeof EventSourceCtor !== "function") {
    const handle = resolveSetInterval(deps)(
      () => void pollActivityOnce(baseUrl, fetchFn, dispatch, isStopped),
      ACTIVITY_POLL_INTERVAL_MS,
    )
    return () => {
      stopped = true
      resolveClearInterval(deps)(handle)
    }
  }

  const source = new EventSourceCtor(`${baseUrl}${ACTIVITY_STREAM_PATH}`)
  // The worker frames each event with `event: <kind>` and data { event }. We
  // listen to the two payment kinds by name, plus the default `message` handler
  // as a safety net for servers that do not set the SSE event field.
  const onMessage = (raw: unknown): void => {
    if (stopped) return
    const data = (raw as { data?: string }).data
    if (typeof data !== "string") return
    const particle = parseActivityStreamData(data)
    if (particle) dispatch(particle)
  }
  source.addEventListener("real_bitcoin_moved", onMessage as EventListener)
  source.addEventListener("settlement_recorded", onMessage as EventListener)
  source.addEventListener("message", onMessage as EventListener)

  return () => {
    stopped = true
    source.removeEventListener("real_bitcoin_moved", onMessage as EventListener)
    source.removeEventListener("settlement_recorded", onMessage as EventListener)
    source.removeEventListener("message", onMessage as EventListener)
    source.close()
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// P2 — Cloudflare world rows
// ─────────────────────────────────────────────────────────────────────────────

type SpacetimeTableRef = Readonly<{
  iter?: () => Iterable<unknown>
  onInsert?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
  onUpdate?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
  onDelete?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
  removeOnInsert?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
  removeOnUpdate?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
  removeOnDelete?: (cb: (...args: ReadonlyArray<unknown>) => void) => void
}>

export type SpacetimeWorldConnection = Readonly<{
  db?: Record<string, SpacetimeTableRef | undefined>
  reducers?: {
    readonly joinRegion?: (args: {
      readonly regionRef: string
      readonly displayName: string
      readonly characterId: string
    }) => unknown
    readonly setAvatarPosition?: (args: {
      readonly regionRef: string
      readonly positionX: number
      readonly positionY: number
      readonly positionZ: number
      readonly yaw: number
      readonly pitch: number
      readonly movementMode: string
      readonly characterId: string
    }) => unknown
    readonly leaveRegion?: (args: {
      readonly regionRef: string
      readonly characterId: string
    }) => unknown
  }
  subscriptionBuilder?: () => {
    onApplied?: (cb: (...args: ReadonlyArray<unknown>) => void) => unknown
    onError?: (cb: (...args: ReadonlyArray<unknown>) => void) => unknown
    subscribe?: (queries: ReadonlyArray<string>) => { unsubscribe?: () => void }
  }
  disconnect?: () => void
}>

type SpacetimeWorldConnectInput = Readonly<{
  database: string
  token: string | null
  worldUrl: string
  // `identity` is the live SpacetimeDB account identity (hex). It is the
  // account axis of the MMO model and is needed to build this instance's own
  // per-character avatar key for the scene self-filter.
  onConnected: (token: string | null, identity: string | null) => void
  onDisconnected: () => void
  onUnavailable: () => void
}>

export type ChatWorldSpacetimeSubscriptionDeps = ChatWorldSubscriptionDeps & {
  readonly connect?: (input: SpacetimeWorldConnectInput) => SpacetimeWorldConnection
  readonly database?: string
  readonly worldUrl?: string
  readonly runRef?: string
  readonly nowMs?: () => number
  readonly storage?: {
    readonly getItem: (key: string) => string | null
    readonly setItem: (key: string, value: string) => void
    readonly removeItem?: (key: string) => void
  } | null
  readonly setTimeout?: (handler: () => void, ms: number) => unknown
  readonly clearTimeout?: (handle: unknown) => void
  readonly maxReconnectAttempts?: number
  readonly reconnectBaseMs?: number
  readonly identity?: {
    readonly pylonRef?: string | null
    readonly nodeLabel?: string | null
    readonly fallbackActorRef?: string | null
  }
  // The character this app launch fields (from OA_CHARACTER). Defaults to
  // "main" so a single instance behaves exactly as before. Accepts a getter so
  // the value can be resolved LAZILY at join/move time — the Bun host injects
  // globalThis.__OA_CHARACTER (read by chatWorldCharacterId) and that injection
  // may land after this subscription mounts, so an eager capture could miss it.
  readonly characterId?: string | null | (() => string | null)
}

export type SpacetimeWorldDispatch = (
  world: ChatWorldMultiplayerProjection,
) => void

const CLOUD_WORLD_SESSION_STORAGE_KEY = "openagents.world.cloudflare.session.v1"
const IDLE_POSE_KEEPALIVE_MS = 5_000
const POSE_STATIONARY_EPSILON_METERS = 0.02

export type VerseAvatarPose = Readonly<{
  regionRef: string
  x: number
  y: number
  z: number
  yaw: number
  animation: "idle" | "walk" | "run"
  capturedAtMs: number
}>

export type VerseMultiplayerClient = Readonly<{
  publishLocalPose: (pose: VerseAvatarPose) => ChatWorldAvatarPositionPlan
  joinRegion: () => void
  leaveRegion: () => void
}>

let activeVerseMultiplayerClient: VerseMultiplayerClient | null = null

const resolveStorage = (
  deps?: ChatWorldSpacetimeSubscriptionDeps,
): ChatWorldSpacetimeSubscriptionDeps["storage"] => {
  if (deps?.storage !== undefined) return deps.storage
  const storage = (globalThis as { localStorage?: Storage }).localStorage
  return storage ?? null
}

const resolveTimeout = (
  deps?: ChatWorldSpacetimeSubscriptionDeps,
): ((handler: () => void, ms: number) => unknown) =>
  deps?.setTimeout ??
  ((handler, ms) =>
    (globalThis as unknown as { setTimeout: (h: () => void, m: number) => unknown })
      .setTimeout(handler, ms))

const resolveClearTimeout = (
  deps?: ChatWorldSpacetimeSubscriptionDeps,
): ((handle: unknown) => void) =>
  deps?.clearTimeout ??
  ((handle) =>
    (globalThis as unknown as { clearTimeout: (h: unknown) => void })
      .clearTimeout(handle))

const collectRows = (table: SpacetimeTableRef | undefined): ReadonlyArray<unknown> =>
  table?.iter === undefined ? [] : [...table.iter()]

// The SpacetimeDB SDK exposes connection.db / connection.reducers accessors by
// the table/reducer SCHEMA NAME, which is snake_case (e.g. `world_region`,
// `set_avatar_position`). Earlier code read camelCase (`worldRegion`,
// `setAvatarPosition`), which silently resolved to undefined -> empty world and
// no published avatars. Resolve snake_case first, fall back to camelCase so this
// is correct regardless of SDK casing.
const pickTable = (
  db: SpacetimeWorldConnection["db"] | undefined,
  snake: string,
  camel: string,
): SpacetimeTableRef | undefined => {
  const bag = db as unknown as Record<string, SpacetimeTableRef | undefined> | undefined
  return bag?.[snake] ?? bag?.[camel]
}

const pickReducer = (
  reducers: SpacetimeWorldConnection["reducers"] | undefined,
  snake: string,
  camel: string,
): ((args: unknown) => unknown) | undefined => {
  const bag = reducers as unknown as
    | Record<string, ((args: unknown) => unknown) | undefined>
    | undefined
  const candidate = bag?.[snake] ?? bag?.[camel]
  return typeof candidate === "function" ? candidate : undefined
}

const collectPositionRows = (
  table: SpacetimeTableRef | undefined,
  presenceFeed: "high" | "low",
): ReadonlyArray<unknown> =>
  collectRows(table).map((row) =>
    row !== null && typeof row === "object"
      ? { ...(row as Record<string, unknown>), presenceFeed }
      : row,
  )

const worldRowsFromConnection = (
  connection: SpacetimeWorldConnection,
): ChatWorldSpacetimeRows => ({
  regions: collectRows(pickTable(connection.db, "world_region", "worldRegion")),
  stations: collectRows(pickTable(connection.db, "pylon_station", "pylonStation")),
  avatars: collectRows(pickTable(connection.db, "agent_avatar", "agentAvatar")),
  positions: [
    ...collectPositionRows(pickTable(connection.db, "avatar_position", "avatarPosition"), "high"),
    ...collectPositionRows(pickTable(connection.db, "avatar_position_near", "avatarPositionNear"), "high"),
    ...collectPositionRows(pickTable(connection.db, "avatar_position_far", "avatarPositionFar"), "low"),
  ],
  messages: collectRows(pickTable(connection.db, "local_chat_message", "localChatMessage")),
  attention: collectRows(pickTable(connection.db, "pylon_attention", "pylonAttention")),
})

const invokeMaybePromise = (value: unknown): void => {
  if (
    value !== null &&
    typeof value === "object" &&
    typeof (value as { catch?: unknown }).catch === "function"
  ) {
    void (value as Promise<unknown>).catch(() => null)
  }
}

export const publishSpacetimeAvatarPosition = (
  connection: SpacetimeWorldConnection,
  plan: ChatWorldAvatarPositionPlan,
): boolean => {
  if (plan.ok !== true) return false
  const reducer = pickReducer(connection.reducers, "set_avatar_position", "setAvatarPosition")
  if (reducer === undefined) return false
  invokeMaybePromise(reducer(plan.write))
  return true
}

const movementModeForVerseAnimation = (
  animation: VerseAvatarPose["animation"],
): ChatWorldAvatarPositionWrite["movementMode"] => {
  if (animation === "run") return "running"
  if (animation === "walk") return "walking"
  return "idle"
}

const regionByRef = (
  regions: ReadonlyArray<ChatWorldRegionRow>,
  regionRef: string,
): ChatWorldRegionRow | null =>
  regions.find((region) => region.regionRef === regionRef) ?? null

const localPreviousFromWrite = (
  write: ChatWorldAvatarPositionWrite,
  capturedAtMs: number,
) => ({
  avatarRef: CHAT_WORLD_DESKTOP_AVATAR_REF,
  regionRef: write.regionRef,
  x: write.positionX,
  y: write.positionY,
  z: write.positionZ,
  yaw: write.yaw,
  movementMode: write.movementMode,
  lastSeenEpochMs: capturedAtMs,
})

const poseDistance = (
  pose: VerseAvatarPose,
  previous: ReturnType<typeof localPreviousFromWrite>,
): number => {
  const dx = pose.x - previous.x
  const dy = pose.y - previous.y
  const dz = pose.z - previous.z
  return Math.sqrt(dx * dx + dy * dy + dz * dz)
}

export const createVerseMultiplayerClient = (input: {
  readonly connection: SpacetimeWorldConnection
  readonly database: string
  readonly displayName: string
  readonly runRef: string
  readonly nowMs: () => number
  readonly worldUrl: string
  // The character this instance fields. One account can run many instances,
  // each with a distinct characterId, and every distinct character is its own
  // visible avatar in the world. Omitted/blank → "main". Accepts a getter so the
  // value is resolved LAZILY at join/move time, after the Bun host has injected
  // globalThis.__OA_CHARACTER into the webview.
  readonly characterId?: string | null | (() => string | null)
}): VerseMultiplayerClient => {
  const lastAcceptedByRegion = new Map<string, ReturnType<typeof localPreviousFromWrite>>()
  let joinedRegionRef: string | null = null
  // Resolve lazily on every reducer call so a late globalThis.__OA_CHARACTER
  // injection (dom-ready) is reflected by the time join/move actually fire.
  const characterId = (): string =>
    sanitizeChatWorldCharacterId(resolveChatWorldCharacterId(input.characterId))

  const projection = () =>
    projectChatWorldSpacetimeRows({
      flagEnabled: true,
      runRef: input.runRef,
      rows: worldRowsFromConnection(input.connection),
      nowMs: input.nowMs(),
      worldUrl: input.worldUrl,
      database: input.database,
    })

  const joinRegion = (): void => {
    const region = defaultChatWorldRegionForRun(projection().regions, input.runRef)
    // world_region can arrive a tick after onApplied, so this is retried from
    // onRowsChanged; bail quietly until the region row is present.
    if (region === null) return
    if (joinedRegionRef === region.regionRef) return
    joinedRegionRef = region.regionRef
    const reducer = pickReducer(input.connection.reducers, "join_region", "joinRegion")
    invokeMaybePromise(reducer?.({
      regionRef: region.regionRef,
      displayName: input.displayName,
      characterId: characterId(),
    }))
  }

  const leaveRegion = (): void => {
    if (joinedRegionRef === null) return
    const reducer = pickReducer(input.connection.reducers, "leave_region", "leaveRegion")
    invokeMaybePromise(reducer?.({
      regionRef: joinedRegionRef,
      characterId: characterId(),
    }))
    joinedRegionRef = null
  }

  const publishLocalPose = (pose: VerseAvatarPose): ChatWorldAvatarPositionPlan => {
    const regionRef = pose.regionRef.trim()
    if (joinedRegionRef !== regionRef) {
      return { ok: false, reason: "avatar must join region before moving" }
    }
    const previous = lastAcceptedByRegion.get(regionRef) ?? null
    if (
      previous !== null &&
      pose.animation === "idle" &&
      poseDistance(pose, previous) <= POSE_STATIONARY_EPSILON_METERS &&
      pose.capturedAtMs - previous.lastSeenEpochMs < IDLE_POSE_KEEPALIVE_MS
    ) {
      return { ok: false, reason: "idle pose keepalive rate limited" }
    }

    const plan = planChatWorldAvatarPositionWrite({
      region: regionByRef(projection().regions, regionRef),
      previous,
      nowMs: pose.capturedAtMs,
      x: pose.x,
      y: pose.y,
      z: pose.z,
      yaw: pose.yaw,
      pitch: 0,
      movementMode: movementModeForVerseAnimation(pose.animation),
      characterId: characterId(),
    })
    if (plan.ok !== true) return plan
    if (!publishSpacetimeAvatarPosition(input.connection, plan)) {
      return { ok: false, reason: "position reducer unavailable" }
    }
    lastAcceptedByRegion.set(
      plan.write.regionRef,
      localPreviousFromWrite(plan.write, pose.capturedAtMs),
    )
    return plan
  }

  return { publishLocalPose, joinRegion, leaveRegion }
}

export const publishActiveVerseLocalPose = (
  pose: VerseAvatarPose,
): ChatWorldAvatarPositionPlan =>
  activeVerseMultiplayerClient?.publishLocalPose(pose) ?? {
    ok: false,
    reason: "multiplayer client unavailable",
  }

const attachWorldConnection = (input: {
  readonly connection: SpacetimeWorldConnection
  readonly dispatch: SpacetimeWorldDispatch
  readonly database: string
  readonly runRef: string
  readonly nowMs: () => number
  readonly worldUrl: string
  readonly displayName: string
  // Lazy character resolver (see resolveChatWorldCharacterId): read at join/move
  // time so a late globalThis.__OA_CHARACTER injection is honored.
  readonly characterId: ChatWorldCharacterIdInput
  // Latest resolved per-character self-filter key, or null before the live
  // identity is known. Read lazily so snapshots dispatched after onConnect
  // carry the real key.
  readonly getLocalAvatarRef: () => string | null
  readonly onUnavailable: () => void
  readonly onClientReady?: (client: VerseMultiplayerClient) => void
  readonly onClientGone?: (client: VerseMultiplayerClient) => void
  // Exposes this connection's snapshot dispatcher so the outer subscription can
  // force a re-paint once the live identity (and thus the self-filter key)
  // becomes available after onConnect.
  readonly onSnapshotReady?: (dispatchSnapshot: () => void) => void
}): Unsubscribe => {
  const tables = [
    pickTable(input.connection.db, "world_region", "worldRegion"),
    pickTable(input.connection.db, "pylon_station", "pylonStation"),
    pickTable(input.connection.db, "agent_avatar", "agentAvatar"),
    pickTable(input.connection.db, "avatar_position", "avatarPosition"),
    pickTable(input.connection.db, "avatar_position_near", "avatarPositionNear"),
    pickTable(input.connection.db, "avatar_position_far", "avatarPositionFar"),
    pickTable(input.connection.db, "pylon_attention", "pylonAttention"),
    pickTable(input.connection.db, "local_chat_message", "localChatMessage"),
    pickTable(input.connection.db, "chat_bubble", "chatBubble"),
    pickTable(input.connection.db, "local_emote", "localEmote"),
    pickTable(input.connection.db, "agent_intent", "agentIntent"),
  ]
  let subscriptionHandle: { unsubscribe?: () => void } | null = null
  const client = createVerseMultiplayerClient({
    connection: input.connection,
    database: input.database,
    displayName: input.displayName,
    runRef: input.runRef,
    nowMs: input.nowMs,
    worldUrl: input.worldUrl,
    characterId: input.characterId,
  })
  input.onClientReady?.(client)

  const dispatchSnapshot = (): void => {
    const projection = projectChatWorldSpacetimeRows({
      flagEnabled: true,
      runRef: input.runRef,
      rows: worldRowsFromConnection(input.connection),
      nowMs: input.nowMs(),
      worldUrl: input.worldUrl,
      database: input.database,
      localAvatarRef: input.getLocalAvatarRef(),
    })
    input.dispatch(projection.world)
  }
  input.onSnapshotReady?.(dispatchSnapshot)

  const onRowsChanged = (): void => {
    dispatchSnapshot()
    // Retry the region join: world_region may arrive a tick AFTER onApplied, so
    // the onApplied join can bail on an empty cache. joinRegion is guarded to a
    // single join per region, so calling it on every row change is safe.
    client.joinRegion()
  }

  for (const table of tables) {
    table?.onInsert?.(onRowsChanged)
    table?.onUpdate?.(onRowsChanged)
    table?.onDelete?.(onRowsChanged)
  }

  try {
    const builder = input.connection.subscriptionBuilder?.()
    builder?.onApplied?.(() => {
      dispatchSnapshot()
      client.joinRegion()
    })
    builder?.onError?.(() => input.onUnavailable())
    subscriptionHandle = builder?.subscribe?.(
      chatWorldMultiplayerSubscriptionQueries(input.runRef),
    ) ?? null
  } catch {
    input.onUnavailable()
  }

  return () => {
    input.onClientGone?.(client)
    client.leaveRegion()
    for (const table of tables) {
      table?.removeOnInsert?.(onRowsChanged)
      table?.removeOnUpdate?.(onRowsChanged)
      table?.removeOnDelete?.(onRowsChanged)
    }
    try {
      subscriptionHandle?.unsubscribe?.()
    } catch {
      // The SDK only allows unsubscribe after a subscription is active. Teardown
      // may happen during connect/reconnect; disconnect below is the hard stop.
    }
    input.connection.disconnect?.()
  }
}

const commandAnimationForVerse = (
  animation: VerseAvatarPose["animation"],
): "idle" | "walk" | "run" =>
  animation === "run" ? "run" : animation === "walk" ? "walk" : "idle"

const makeWorldCommand = (input: {
  readonly actorRef: string
  readonly command: WorldCommandName
  readonly commandRef: string
  readonly payload: unknown
  readonly regionRef: string
  readonly seq: number
  readonly issuedAt: string
}): WorldCommandEnvelope => ({
  schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
  actorClass: "browser",
  actorRef: input.actorRef as WorldRef,
  command: input.command,
  commandRef: input.commandRef as WorldRef,
  issuedAt: input.issuedAt as WorldIsoTimestamp,
  payload: input.payload,
  regionRef: input.regionRef as WorldRegionRef,
  seq: input.seq as WorldSequence,
})

const createCloudflareVerseMultiplayerClient = (input: {
  readonly actorRef: string
  readonly client: WorldClient
  readonly displayName: string
  readonly regionRef: string
  readonly runRef: string
  readonly characterId: () => string
  readonly nowMs: () => number
}): VerseMultiplayerClient => {
  let joined = false
  let seq = 0
  const nextSeq = (): number => {
    seq += 1
    return seq
  }
  const commandRef = (command: string, issuedAt: string): string =>
    `command.desktop.${command}.${input.actorRef}.${nextSeq()}.${issuedAt}`
  const call = (command: WorldCommandName, payload: unknown): void => {
    const issuedAt = new Date(input.nowMs()).toISOString()
    void Effect.runPromise(input.client.callCommand(makeWorldCommand({
      actorRef: input.actorRef,
      command,
      commandRef: commandRef(command, issuedAt),
      issuedAt,
      payload,
      regionRef: input.regionRef,
      seq,
    }))).catch(() => null)
  }

  return {
    joinRegion: () => {
      if (joined) return
      joined = true
      call("join_region", {
        characterId: input.characterId(),
        label: input.displayName,
        runRef: input.runRef,
      })
    },
    leaveRegion: () => {
      if (!joined) return
      joined = false
      call("leave_region", {
        characterId: input.characterId(),
      })
    },
    publishLocalPose: (pose) => {
      if (!joined || pose.regionRef !== input.regionRef) {
        return { ok: false, reason: "avatar must join region before moving" }
      }
      call("set_avatar_position", {
        characterId: input.characterId(),
        position: {
          x: Number(pose.x.toFixed(3)),
          y: Number(pose.y.toFixed(3)),
          z: Number(pose.z.toFixed(3)),
        },
        rotationY: Number(pose.yaw.toFixed(3)),
        animation: commandAnimationForVerse(pose.animation),
      })
      return {
        ok: true,
        write: {
          regionRef: pose.regionRef,
          positionX: Number(pose.x.toFixed(3)),
          positionY: Number(pose.y.toFixed(3)),
          positionZ: Number(pose.z.toFixed(3)),
          yaw: Number(pose.yaw.toFixed(3)),
          pitch: 0,
          movementMode: movementModeForVerseAnimation(pose.animation),
          characterId: input.characterId(),
        },
      }
    },
  }
}

const subscribeCloudflareWorld = (
  dispatch: SpacetimeWorldDispatch,
  input: {
    readonly database: string
    readonly identity: ChatWorldDesktopAvatarIdentity
    readonly runRef: string
    readonly nowMs: () => number
    readonly worldUrl: string
    readonly characterId: () => string
    readonly fetchFn?: typeof fetch
    readonly onClientReady?: (client: VerseMultiplayerClient) => void
    readonly onClientGone?: (client: VerseMultiplayerClient) => void
  },
): Unsubscribe => {
  const regionRef = chatWorldRegionRefForRun(input.runRef)
  const localAvatarRef = (): string =>
    worldAvatarRefForCharacter(input.identity.actorRef, input.characterId())
  let stopped = false
  let multiplayerClient: VerseMultiplayerClient | null = null

  const dispatchProjection = (client: WorldClient): void => {
    void Effect.runPromise(client.readModel()).then(readModel => {
      if (stopped) return
      dispatch(projectChatWorldClientWorld({
        flagEnabled: true,
        runRef: input.runRef,
        readModel,
        nowMs: input.nowMs(),
        worldUrl: input.worldUrl,
        database: input.database,
        localAvatarRef: localAvatarRef(),
      }).world)
    }).catch(() => dispatchUnavailable())
  }

  const dispatchUnavailable = (): void => {
    if (stopped) return
    dispatch(projectChatWorldClientWorld({
      flagEnabled: false,
      runRef: input.runRef,
      readModel: null,
      nowMs: input.nowMs(),
      worldUrl: input.worldUrl,
      database: input.database,
      localAvatarRef: localAvatarRef(),
    }).world)
  }

  const client = createWorldClient({
    initialRegionRef: regionRef,
    now: () => new Date(input.nowMs()).toISOString(),
    transport: createBrowserWorldTransport({
      worldUrl: input.worldUrl,
      actorRef: input.identity.actorRef,
      actorClass: "browser",
      ...(input.fetchFn === undefined ? {} : { fetchFn: input.fetchFn }),
      onDelta: () => dispatchProjection(client),
      onDiagnostic: () => dispatchProjection(client),
    }),
  })

  void Effect.runPromise(client.connect({
    characterId: input.characterId(),
    regionRef,
    runRef: input.runRef,
    scope: "region",
  })).then(() => {
    if (stopped) return
    multiplayerClient = createCloudflareVerseMultiplayerClient({
      actorRef: input.identity.actorRef,
      client,
      displayName: input.identity.displayName,
      regionRef,
      runRef: input.runRef,
      characterId: input.characterId,
      nowMs: input.nowMs,
    })
    input.onClientReady?.(multiplayerClient)
    multiplayerClient.joinRegion()
    dispatchProjection(client)
  }).catch(() => dispatchUnavailable())

  return () => {
    stopped = true
    if (multiplayerClient !== null) {
      input.onClientGone?.(multiplayerClient)
      multiplayerClient.leaveRegion()
    }
    void Effect.runPromise(client.disconnect()).catch(() => null)
  }
}

export const subscribeSpacetimeWorld = (
  dispatch: SpacetimeWorldDispatch,
  deps?: ChatWorldSpacetimeSubscriptionDeps,
): Unsubscribe => {
  const flags = { ...chatWorldFlags(), ...(deps?.flags ?? {}) }
  if (flags.CHAT_WORLD_MULTIPLAYER !== true) return noop

  const database = deps?.database ?? OPENAGENTS_WORLD_DATABASE
  const worldUrl = deps?.worldUrl ?? OPENAGENTS_WORLD_URL
  const runRef = deps?.runRef ?? DEFAULT_TASSADAR_WORLD_RUN_REF
  const nowMs = deps?.nowMs ?? (() => Date.now())
  const storage = resolveStorage(deps)
  const setTimeoutFn = resolveTimeout(deps)
  const clearTimeoutFn = resolveClearTimeout(deps)
  const maxReconnectAttempts = deps?.maxReconnectAttempts ?? Number.POSITIVE_INFINITY
  const reconnectBaseMs = deps?.reconnectBaseMs ?? 2_000
  const identity = chatWorldDesktopAvatarIdentity(deps?.identity ?? {})
  // Resolve lazily: the value is read at join/move and self-filter time, after
  // the Bun host has injected globalThis.__OA_CHARACTER into the webview.
  const characterId = (): string =>
    sanitizeChatWorldCharacterId(
      resolveChatWorldCharacterId(deps?.characterId) ?? DEFAULT_OA_CHARACTER_ID,
    )

  if (deps?.connect === undefined) {
    storage?.setItem(CLOUD_WORLD_SESSION_STORAGE_KEY, JSON.stringify({
      actorRef: identity.actorRef,
      characterId: characterId(),
      connectedAtMs: nowMs(),
    }))
    return subscribeCloudflareWorld(dispatch, {
      database,
      identity,
      runRef,
      nowMs,
      worldUrl,
      characterId,
      ...(deps?.fetchFn === undefined ? {} : { fetchFn: deps.fetchFn }),
      onClientReady: (client) => {
        activeVerseMultiplayerClient = client
      },
      onClientGone: (client) => {
        if (activeVerseMultiplayerClient === client) {
          activeVerseMultiplayerClient = null
        }
      },
    })
  }

  const connect = deps.connect

  let stopped = false
  let retryHandle: unknown = null
  let cleanupConnection: Unsubscribe | null = null
  let reconnectAttempt = 0
  // The live SpacetimeDB account identity, once onConnect yields it. Combined
  // with characterId it gives this instance's own avatar key for self-filter.
  let liveIdentityHex: string | null = null
  let redispatchSnapshot: (() => void) | null = null
  const localAvatarRef = (): string | null =>
    liveIdentityHex !== null && liveIdentityHex.length > 0
      ? chatWorldDesktopAvatarRef(liveIdentityHex, characterId())
      : null

  const dispatchUnavailable = (): void => {
    dispatch(projectChatWorldSpacetimeRows({
      flagEnabled: false,
      runRef,
      rows: null,
      nowMs: nowMs(),
      worldUrl,
      database,
    }).world)
  }

  const clearRetry = (): void => {
    if (retryHandle !== null) {
      clearTimeoutFn(retryHandle)
      retryHandle = null
    }
  }

  const scheduleReconnect = (): void => {
    if (stopped || reconnectAttempt >= maxReconnectAttempts) return
    clearRetry()
    const delay = Math.min(30_000, reconnectBaseMs * (2 ** reconnectAttempt))
    reconnectAttempt += 1
    retryHandle = setTimeoutFn(() => connectOnce(), delay)
  }

  const unavailable = (): void => {
    if (stopped) return
    storage?.removeItem?.(CLOUD_WORLD_SESSION_STORAGE_KEY)
    dispatchUnavailable()
    scheduleReconnect()
  }

  function connectOnce(): void {
    if (stopped) return
    cleanupConnection?.()
    cleanupConnection = null
    try {
      redispatchSnapshot = null
      const connection = connect({
        database,
        token: storage?.getItem(CLOUD_WORLD_SESSION_STORAGE_KEY) ?? null,
        worldUrl,
        onConnected: (token, connectedIdentity) => {
          reconnectAttempt = 0
          if (token !== null) storage?.setItem(CLOUD_WORLD_SESSION_STORAGE_KEY, token)
          if (
            typeof connectedIdentity === "string" &&
            connectedIdentity.length > 0 &&
            connectedIdentity !== liveIdentityHex
          ) {
            liveIdentityHex = connectedIdentity
            // Re-paint so the scene self-filter switches from the pre-connect
            // fallback to this instance's real per-character avatar key.
            redispatchSnapshot?.()
          }
        },
        onDisconnected: unavailable,
        onUnavailable: unavailable,
      })
      cleanupConnection = attachWorldConnection({
        connection,
        database,
        dispatch,
        displayName: identity.displayName,
        characterId,
        getLocalAvatarRef: localAvatarRef,
        nowMs,
        onUnavailable: unavailable,
        runRef,
        worldUrl,
        onSnapshotReady: (dispatchSnapshot) => {
          redispatchSnapshot = dispatchSnapshot
        },
        onClientReady: (client) => {
          activeVerseMultiplayerClient = client
        },
        onClientGone: (client) => {
          if (activeVerseMultiplayerClient === client) {
            activeVerseMultiplayerClient = null
          }
        },
      })
    } catch {
      unavailable()
    }
  }

  connectOnce()

  return () => {
    stopped = true
    clearRetry()
    cleanupConnection?.()
    cleanupConnection = null
  }
}
