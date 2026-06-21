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
  activityEventToParticle,
  chatWorldFlags,
  projectChatWorldPylonScene,
  type ActivityEvent,
  type ChatWorldPylonScene,
  type PaymentParticle,
} from "../shared/chat-world-scene"
import {
  DEFAULT_TASSADAR_WORLD_RUN_REF,
  OPENAGENTS_WORLD_DATABASE,
  OPENAGENTS_WORLD_URL,
  chatWorldMultiplayerSubscriptionQueries,
  type ChatWorldMultiplayerProjection,
} from "../shared/chat-world-multiplayer"
import {
  chatWorldDesktopAvatarIdentity,
  defaultChatWorldRegionForRun,
  planChatWorldAvatarPositionWrite,
  projectChatWorldSpacetimeRows,
  type ChatWorldAvatarPositionPlan,
  type ChatWorldAvatarPositionWrite,
  type ChatWorldRegionRow,
  type ChatWorldSpacetimeRows,
} from "../shared/chat-world-spacetimedb"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene"
import { DbConnection as GeneratedWorldConnection } from "../../../openagents.com/apps/web/src/scene/spacetimeWorldBindings"

const PUBLIC_BASE_URL = "https://openagents.com"
const PYLON_STATS_PATH = "/api/public/pylon-stats"
const ACTIVITY_POLL_PATH = "/api/public/activity-timeline"
const ACTIVITY_STREAM_PATH = "/api/public/activity-timeline/stream"

const PYLON_POLL_INTERVAL_MS = 4_000
const ACTIVITY_POLL_INTERVAL_MS = 5_000

export type Unsubscribe = () => void

const noop: Unsubscribe = () => {}

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
// P2 — SpacetimeDB world rows
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
    }) => unknown
    readonly setAvatarPosition?: (args: {
      readonly regionRef: string
      readonly positionX: number
      readonly positionY: number
      readonly positionZ: number
      readonly yaw: number
      readonly pitch: number
      readonly movementMode: string
    }) => unknown
    readonly leaveRegion?: (args: {
      readonly regionRef: string
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
  onConnected: (token: string | null) => void
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
}

export type SpacetimeWorldDispatch = (
  world: ChatWorldMultiplayerProjection,
) => void

const SPACETIME_TOKEN_STORAGE_KEY = "openagents.world.spacetimedb.token.v1"
const LOCAL_AVATAR_PUBLISH_REF = "avatar.desktop.local"
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

const defaultSpacetimeConnect = (
  input: SpacetimeWorldConnectInput,
): SpacetimeWorldConnection => {
  let builder = GeneratedWorldConnection
    .builder()
    .withUri(input.worldUrl)
    .withDatabaseName(input.database)
    .onConnect((_ctx: unknown, _identity: unknown, token: string) =>
      input.onConnected(typeof token === "string" ? token : null),
    )
    .onConnectError(() => input.onUnavailable())
    .onDisconnect(() => input.onDisconnected())

  if (input.token !== null && input.token.length > 0) {
    builder = builder.withToken(input.token)
  }
  return builder.build() as unknown as SpacetimeWorldConnection
}

const collectRows = (table: SpacetimeTableRef | undefined): ReadonlyArray<unknown> =>
  table?.iter === undefined ? [] : [...table.iter()]

const worldRowsFromConnection = (
  connection: SpacetimeWorldConnection,
): ChatWorldSpacetimeRows => ({
  regions: collectRows(connection.db?.worldRegion),
  stations: collectRows(connection.db?.pylonStation),
  avatars: collectRows(connection.db?.agentAvatar),
  positions: collectRows(connection.db?.avatarPosition),
  messages: collectRows(connection.db?.localChatMessage),
  attention: collectRows(connection.db?.pylonAttention),
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
  const reducer = connection.reducers?.setAvatarPosition
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
  avatarRef: LOCAL_AVATAR_PUBLISH_REF,
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
}): VerseMultiplayerClient => {
  const lastAcceptedByRegion = new Map<string, ReturnType<typeof localPreviousFromWrite>>()
  let joinedRegionRef: string | null = null

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
    if (region === null) return
    joinedRegionRef = region.regionRef
    invokeMaybePromise(input.connection.reducers?.joinRegion?.({
      regionRef: region.regionRef,
      displayName: input.displayName,
    }))
  }

  const leaveRegion = (): void => {
    if (joinedRegionRef === null) return
    invokeMaybePromise(input.connection.reducers?.leaveRegion?.({
      regionRef: joinedRegionRef,
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
  readonly onUnavailable: () => void
  readonly onClientReady?: (client: VerseMultiplayerClient) => void
  readonly onClientGone?: (client: VerseMultiplayerClient) => void
}): Unsubscribe => {
  const tables = [
    input.connection.db?.worldRegion,
    input.connection.db?.pylonStation,
    input.connection.db?.agentAvatar,
    input.connection.db?.avatarPosition,
    input.connection.db?.pylonAttention,
    input.connection.db?.localChatMessage,
    input.connection.db?.chatBubble,
    input.connection.db?.localEmote,
    input.connection.db?.agentIntent,
  ]
  let subscriptionHandle: { unsubscribe?: () => void } | null = null
  const client = createVerseMultiplayerClient({
    connection: input.connection,
    database: input.database,
    displayName: input.displayName,
    runRef: input.runRef,
    nowMs: input.nowMs,
    worldUrl: input.worldUrl,
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
    })
    input.dispatch(projection.world)
  }

  const onRowsChanged = (): void => dispatchSnapshot()

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

export const subscribeSpacetimeWorld = (
  dispatch: SpacetimeWorldDispatch,
  deps?: ChatWorldSpacetimeSubscriptionDeps,
): Unsubscribe => {
  const flags = deps?.flags ?? chatWorldFlags()
  if (flags.CHAT_WORLD_MULTIPLAYER !== true) return noop

  const database = deps?.database ?? OPENAGENTS_WORLD_DATABASE
  const worldUrl = deps?.worldUrl ?? OPENAGENTS_WORLD_URL
  const runRef = deps?.runRef ?? DEFAULT_TASSADAR_WORLD_RUN_REF
  const nowMs = deps?.nowMs ?? (() => Date.now())
  const storage = resolveStorage(deps)
  const connect = deps?.connect ?? defaultSpacetimeConnect
  const setTimeoutFn = resolveTimeout(deps)
  const clearTimeoutFn = resolveClearTimeout(deps)
  const maxReconnectAttempts = deps?.maxReconnectAttempts ?? Number.POSITIVE_INFINITY
  const reconnectBaseMs = deps?.reconnectBaseMs ?? 2_000
  const identity = chatWorldDesktopAvatarIdentity(deps?.identity ?? {})

  let stopped = false
  let retryHandle: unknown = null
  let cleanupConnection: Unsubscribe | null = null
  let reconnectAttempt = 0

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
    storage?.removeItem?.(SPACETIME_TOKEN_STORAGE_KEY)
    dispatchUnavailable()
    scheduleReconnect()
  }

  function connectOnce(): void {
    if (stopped) return
    cleanupConnection?.()
    cleanupConnection = null
    try {
      const connection = connect({
        database,
        token: storage?.getItem(SPACETIME_TOKEN_STORAGE_KEY) ?? null,
        worldUrl,
        onConnected: (token) => {
          reconnectAttempt = 0
          if (token !== null) storage?.setItem(SPACETIME_TOKEN_STORAGE_KEY, token)
        },
        onDisconnected: unavailable,
        onUnavailable: unavailable,
      })
      cleanupConnection = attachWorldConnection({
        connection,
        database,
        dispatch,
        displayName: identity.displayName,
        nowMs,
        onUnavailable: unavailable,
        runRef,
        worldUrl,
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
