import {
  FleetPublicRef,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  SyncScope,
  SyncVersionWatermark,
  decodeLiveFrame,
  encodeLiveFrame,
  fleetRunScope,
  PingFrame,
  type LiveFrame,
  type MustRefetchReason,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import type { SarahFleetOwnerProjection } from "../contracts/fleet-owner-projection.ts"
import {
  SarahFleetSyncClientError,
  type SarahFleetSyncClient,
} from "./fleet-sync-client.ts"
import type { SarahFleetBrowserPersistence } from "./fleet-sync-browser-persistence.ts"
import {
  SarahFleetProjectionReducerError,
  makeSarahFleetProjectionStore,
  projectSarahFleetProjectionState,
  reduceSarahFleetLogPages,
  type SarahFleetProjectionOpenResult,
} from "./fleet-sync-projection-store.ts"

export const SARAH_FLEET_LIVE_CONNECT_PATH = "/api/sync/connect"
export const MAX_SARAH_FLEET_LIVE_FRAME_BYTES = 1_048_576
export const MAX_SARAH_FLEET_LIVE_FRAME_ENTRIES = 1_000
export const MAX_SARAH_FLEET_LIVE_QUEUED_FRAMES = 64

export const SarahFleetConnectionErrorReason = Schema.Literals([
  "invalid_scope",
  "network_unavailable",
  "socket_closed",
  "socket_stale",
  "protocol_failure",
  "cursor_no_progress",
  "storage_corrupt",
  "foreign_state",
  "storage_unavailable",
  "must_refetch",
  "retry_exhausted",
  "aborted",
  "disposed",
])
export type SarahFleetConnectionErrorReason =
  typeof SarahFleetConnectionErrorReason.Type

const CONNECTION_ERROR_MESSAGES = {
  invalid_scope: "Fleet connection scope is invalid.",
  network_unavailable: "Fleet connection is temporarily unavailable.",
  socket_closed: "Fleet live connection closed; catching up.",
  socket_stale: "Fleet live connection became stale; catching up.",
  protocol_failure: "Fleet live data failed its typed contract.",
  cursor_no_progress: "Fleet live cursor could not advance safely.",
  storage_corrupt: "Saved fleet state is corrupt and was not opened.",
  foreign_state: "Saved fleet state belongs to another scope.",
  storage_unavailable: "Fleet state storage is unavailable.",
  must_refetch: "Fleet server requires a fresh scoped snapshot.",
  retry_exhausted: "Fleet reconnect attempts were exhausted.",
  aborted: "Fleet connection was aborted.",
  disposed: "Fleet connection was disposed.",
} as const satisfies Record<SarahFleetConnectionErrorReason, string>

export class SarahFleetLiveSessionError extends Error {
  readonly _tag = "SarahFleetLiveSessionError"
  override readonly name = "SarahFleetLiveSessionError"

  constructor(
    readonly reason: SarahFleetConnectionErrorReason,
    readonly retryable: boolean,
  ) {
    super(CONNECTION_ERROR_MESSAGES[reason])
  }
}

const NonNegativeMilliseconds = Schema.Number.check(
  Schema.isFinite(),
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

export const SarahFleetConnectionIssue = Schema.Struct({
  reason: SarahFleetConnectionErrorReason,
  messageSafe: Schema.String,
  retryable: Schema.Boolean,
})
export type SarahFleetConnectionIssue = typeof SarahFleetConnectionIssue.Type

export const SarahFleetConnectionState = Schema.Union([
  Schema.Struct({ phase: Schema.Literal("idle") }),
  Schema.Struct({
    phase: Schema.Literal("catching_up"),
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    attempt: NonNegativeMilliseconds,
  }),
  Schema.Struct({
    phase: Schema.Literal("connecting"),
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    attempt: NonNegativeMilliseconds,
  }),
  Schema.Struct({
    phase: Schema.Literal("live"),
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    connectedAtMs: NonNegativeMilliseconds,
    lastActivityAtMs: NonNegativeMilliseconds,
  }),
  Schema.Struct({
    phase: Schema.Literal("must_refetch"),
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    reason: Schema.Literals([
      "cursor_behind_retained_window",
      "schema_version_unsupported",
      "access_changed",
      "scope_reset",
    ]),
  }),
  Schema.Struct({
    phase: Schema.Literal("reconnecting"),
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    attempt: NonNegativeMilliseconds,
    retryAtMs: NonNegativeMilliseconds,
    mustRefetchReason: Schema.NullOr(
      Schema.Literals([
        "cursor_behind_retained_window",
        "schema_version_unsupported",
        "access_changed",
        "scope_reset",
      ]),
    ),
    error: SarahFleetConnectionIssue,
  }),
  Schema.Struct({
    phase: Schema.Literal("failed"),
    scope: Schema.NullOr(SyncScope),
    cursor: Schema.NullOr(SyncVersionWatermark),
    error: SarahFleetConnectionIssue,
  }),
  Schema.Struct({
    phase: Schema.Literal("stopped"),
    scope: Schema.NullOr(SyncScope),
    reason: Schema.Literals(["aborted", "disposed"]),
  }),
])
export type SarahFleetConnectionState = typeof SarahFleetConnectionState.Type

export type SarahFleetWebSocketLike = Readonly<{
  send: (data: string) => void
  close: (code?: number, reason?: string) => void
}> & {
  onopen: ((event: unknown) => void) | null
  onmessage: ((event: { readonly data: unknown }) => void) | null
  onerror: ((event: unknown) => void) | null
  onclose: ((event: { readonly code?: number }) => void) | null
}

export type SarahFleetWebSocketConstructor = new (
  url: string,
) => SarahFleetWebSocketLike

export type SarahFleetSchedule = (
  task: () => void,
  delayMs: number,
) => () => void

const defaultSchedule: SarahFleetSchedule = (task, delayMs) => {
  const handle = setTimeout(task, delayMs)
  return () => clearTimeout(handle)
}

const issue = (
  reason: SarahFleetConnectionErrorReason,
  retryable: boolean,
): SarahFleetConnectionIssue => ({
  reason,
  messageSafe: CONNECTION_ERROR_MESSAGES[reason],
  retryable,
})

const sessionError = (
  reason: SarahFleetConnectionErrorReason,
  retryable: boolean,
): SarahFleetLiveSessionError =>
  new SarahFleetLiveSessionError(reason, retryable)

const exactFleetScope = (raw: string): typeof SyncScope.Type => {
  const match =
    /^scope\.fleet_run\.([A-Za-z0-9](?:[A-Za-z0-9._:-]*[A-Za-z0-9])?)$/.exec(
      raw,
    )
  if (match?.[1] === undefined) throw sessionError("invalid_scope", false)
  try {
    const runRef = Schema.decodeUnknownSync(FleetPublicRef)(match[1])
    const scope = Schema.decodeUnknownSync(SyncScope)(raw)
    if (fleetRunScope(runRef) !== scope) {
      throw sessionError("invalid_scope", false)
    }
    return scope
  } catch (error) {
    if (error instanceof SarahFleetLiveSessionError) throw error
    throw sessionError("invalid_scope", false)
  }
}

export const buildSarahFleetLiveConnectUrl = (
  rawOrigin: string,
  rawScope: string,
  rawCursor: number,
): string => {
  const scope = exactFleetScope(rawScope)
  let cursor: typeof SyncVersionWatermark.Type
  let origin: URL
  try {
    cursor = SyncVersionWatermark.make(rawCursor)
    origin = new URL(rawOrigin)
  } catch {
    throw sessionError("invalid_scope", false)
  }
  if (
    (origin.protocol !== "http:" && origin.protocol !== "https:") ||
    origin.username !== "" ||
    origin.password !== "" ||
    origin.origin !== rawOrigin.replace(/\/$/, "")
  ) {
    throw sessionError("invalid_scope", false)
  }
  const url = new URL(SARAH_FLEET_LIVE_CONNECT_PATH, origin.origin)
  url.protocol = origin.protocol === "https:" ? "wss:" : "ws:"
  url.searchParams.set("scope", scope)
  url.searchParams.set("cursor", String(cursor))
  return url.toString()
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const exactKeys = (value: unknown, allowed: ReadonlyArray<string>): void => {
  if (!isRecord(value)) throw sessionError("protocol_failure", true)
  const keys = new Set(allowed)
  if (Object.keys(value).some((key) => !keys.has(key))) {
    throw sessionError("protocol_failure", true)
  }
}

const decodeFleetLiveFrame = (data: unknown): LiveFrame => {
  if (typeof data !== "string") throw sessionError("protocol_failure", true)
  if (
    new TextEncoder().encode(data).byteLength >
    MAX_SARAH_FLEET_LIVE_FRAME_BYTES
  ) {
    throw sessionError("protocol_failure", true)
  }
  let raw: unknown
  try {
    raw = JSON.parse(data) as unknown
  } catch {
    throw sessionError("protocol_failure", true)
  }
  if (!isRecord(raw) || typeof raw._tag !== "string") {
    throw sessionError("protocol_failure", true)
  }
  switch (raw._tag) {
    case "PingFrame":
      exactKeys(raw, ["_tag"])
      break
    case "MutationAckFrame":
      exactKeys(raw, ["_tag", "clientId", "lastMutationId"])
      break
    case "MustRefetchFrame":
      exactKeys(raw, ["_tag", "scope", "reason"])
      break
    case "DeltaFrame":
      exactKeys(raw, ["_tag", "scope", "entries", "cursor"])
      if (
        !Array.isArray(raw.entries) ||
        raw.entries.length > MAX_SARAH_FLEET_LIVE_FRAME_ENTRIES
      ) {
        throw sessionError("protocol_failure", true)
      }
      break
    default:
      throw sessionError("protocol_failure", true)
  }
  try {
    return decodeLiveFrame(raw)
  } catch {
    throw sessionError("protocol_failure", true)
  }
}

const classify = (error: unknown): SarahFleetLiveSessionError => {
  if (error instanceof SarahFleetLiveSessionError) return error
  if (error instanceof SarahFleetSyncClientError) {
    if (error.reason === "request_aborted") {
      return sessionError("aborted", false)
    }
    if (error.reason === "network_unavailable") {
      return sessionError("network_unavailable", true)
    }
    if (
      error.reason === "cursor_no_progress" ||
      error.serverCode === "cursor_behind_retained_window"
    ) {
      return error.serverCode === "cursor_behind_retained_window"
        ? sessionError("must_refetch", true)
        : sessionError("cursor_no_progress", true)
    }
    if (
      error.reason === "foreign_scope" ||
      error.serverCode === "unauthorized_scope" ||
      error.serverCode === "unknown_scope"
    ) {
      return sessionError("foreign_state", false)
    }
    if (error.reason === "server_rejected" && error.retryable === true) {
      return sessionError("network_unavailable", true)
    }
    return sessionError("protocol_failure", true)
  }
  if (error instanceof SarahFleetProjectionReducerError) {
    if (error.reason === "invalid_state") {
      return sessionError("storage_corrupt", false)
    }
    if (error.reason === "foreign_scope") {
      return sessionError("foreign_state", false)
    }
    if (error.reason === "persistence_failed") {
      return sessionError("storage_unavailable", false)
    }
    if (
      error.reason === "cursor_mismatch" ||
      error.reason === "version_regression" ||
      error.reason === "duplicate_conflict"
    ) {
      return sessionError("cursor_no_progress", true)
    }
    if (error.reason === "invalid_scope") {
      return sessionError("invalid_scope", false)
    }
    if (error.reason === "projection_failed") {
      return sessionError("protocol_failure", false)
    }
    return sessionError("protocol_failure", true)
  }
  return sessionError("network_unavailable", true)
}

const computeBackoff = (
  attempt: number,
  baseMs: number,
  maxMs: number,
  randomSample: number,
): number => {
  const cap = Math.min(maxMs, baseMs * 2 ** Math.max(0, attempt - 1))
  return Math.floor(cap / 2 + randomSample * (cap / 2))
}

export type SarahFleetLiveSession = Readonly<{
  start: (scope: string, signal?: AbortSignal) => Promise<void>
  refresh: () => Promise<void>
  dispose: () => void
  snapshot: () => SarahFleetConnectionState
  projection: () => SarahFleetOwnerProjection | null
  subscribe: (
    listener: (state: SarahFleetConnectionState) => void,
  ) => () => void
  subscribeProjection: (
    listener: (projection: SarahFleetOwnerProjection) => void,
  ) => () => void
}>

export const makeSarahFleetLiveSession = (input: Readonly<{
  client: Pick<SarahFleetSyncClient, "bootstrap" | "resume">
  persistence: SarahFleetBrowserPersistence
  origin: string
  webSocket?: SarahFleetWebSocketConstructor
  now?: () => number
  schedule?: SarahFleetSchedule
  random?: () => number
  cadenceMs?: number
  staleAfterMs?: number
  connectTimeoutMs?: number
  backoffBaseMs?: number
  backoffMaxMs?: number
  maxReconnectAttempts?: number
  maxQueuedFrames?: number
}>): SarahFleetLiveSession => {
  const now = input.now ?? Date.now
  const schedule = input.schedule ?? defaultSchedule
  const random = input.random ?? Math.random
  const WebSocketImpl =
    input.webSocket ??
    (globalThis.WebSocket as unknown as SarahFleetWebSocketConstructor)
  const cadenceMs = input.cadenceMs ?? 10_000
  const staleAfterMs = input.staleAfterMs ?? 30_000
  const connectTimeoutMs = input.connectTimeoutMs ?? 10_000
  const backoffBaseMs = input.backoffBaseMs ?? 500
  const backoffMaxMs = input.backoffMaxMs ?? 10_000
  const maxReconnectAttempts = input.maxReconnectAttempts ?? 6
  const maxQueuedFrames =
    input.maxQueuedFrames ?? MAX_SARAH_FLEET_LIVE_QUEUED_FRAMES

  if (
    WebSocketImpl === undefined ||
    !Number.isSafeInteger(cadenceMs) ||
    cadenceMs < 10 ||
    !Number.isSafeInteger(staleAfterMs) ||
    staleAfterMs < cadenceMs ||
    staleAfterMs > 300_000 ||
    !Number.isSafeInteger(connectTimeoutMs) ||
    connectTimeoutMs < 10 ||
    !Number.isSafeInteger(backoffBaseMs) ||
    backoffBaseMs < 1 ||
    !Number.isSafeInteger(backoffMaxMs) ||
    backoffMaxMs < backoffBaseMs ||
    backoffMaxMs > 60_000 ||
    !Number.isSafeInteger(maxReconnectAttempts) ||
    maxReconnectAttempts < 1 ||
    maxReconnectAttempts > 16 ||
    !Number.isSafeInteger(maxQueuedFrames) ||
    maxQueuedFrames < 1 ||
    maxQueuedFrames > MAX_SARAH_FLEET_LIVE_QUEUED_FRAMES
  ) {
    throw sessionError("protocol_failure", false)
  }

  const projectionStore = makeSarahFleetProjectionStore({
    client: input.client,
    persistence: input.persistence,
    now,
  })
  const stateListeners = new Set<
    (state: SarahFleetConnectionState) => void
  >()
  const projectionListeners = new Set<
    (projection: SarahFleetOwnerProjection) => void
  >()
  let connectionState: SarahFleetConnectionState = { phase: "idle" }
  let current: SarahFleetProjectionOpenResult | null = null
  let scope: typeof SyncScope.Type | null = null
  let disposed = false
  let stopped = false
  let connectedAtMs = 0
  let lastActivityAtMs = 0
  let reconnectAttempts = 0
  let socketGeneration = 0
  let socket: SarahFleetWebSocketLike | null = null
  let requestAbort: AbortController | null = null
  let cyclePromise: Promise<void> | null = null
  let frameChain: Promise<void> = Promise.resolve()
  let queuedFrames = 0
  let cancelReconnect: (() => void) | null = null
  let cancelHeartbeat: (() => void) | null = null
  let cancelConnectTimeout: (() => void) | null = null
  let removeAbortListener: (() => void) | null = null
  let pendingMustRefetchReason: MustRefetchReason | null = null
  let ownerSignal: AbortSignal | null | undefined

  const readNow = (): number => {
    let value: number
    try {
      value = now()
    } catch {
      throw sessionError("protocol_failure", false)
    }
    if (!Number.isSafeInteger(value) || value < 0) {
      throw sessionError("protocol_failure", false)
    }
    return value
  }

  const readRandom = (): number => {
    let value: number
    try {
      value = random()
    } catch {
      throw sessionError("protocol_failure", false)
    }
    if (!Number.isFinite(value) || value < 0 || value >= 1) {
      throw sessionError("protocol_failure", false)
    }
    return value
  }

  const setState = (raw: SarahFleetConnectionState): void => {
    connectionState = Schema.decodeUnknownSync(SarahFleetConnectionState)(raw)
    for (const listener of [...stateListeners]) {
      try {
        listener(connectionState)
      } catch {
        // A view callback has no authority over the durable sync loop.
      }
    }
  }

  const emitProjection = (projection: SarahFleetOwnerProjection): void => {
    for (const listener of [...projectionListeners]) {
      try {
        listener(projection)
      } catch {
        // A view callback has no authority over persisted projection state.
      }
    }
  }

  const cursor = (): typeof SyncVersionWatermark.Type =>
    current?.state.cursor ?? SyncVersionWatermark.make(0)

  const closeSocket = (): void => {
    socketGeneration += 1
    cancelHeartbeat?.()
    cancelHeartbeat = null
    cancelConnectTimeout?.()
    cancelConnectTimeout = null
    const active = socket
    socket = null
    try {
      active?.close(1000, "sarah_fleet_session_close")
    } catch {
      // The generation guard already detached this socket.
    }
  }

  const stop = (reason: "aborted" | "disposed"): void => {
    if (stopped) return
    stopped = true
    disposed = reason === "disposed"
    cancelReconnect?.()
    cancelReconnect = null
    requestAbort?.abort()
    requestAbort = null
    closeSocket()
    removeAbortListener?.()
    removeAbortListener = null
    setState({ phase: "stopped", scope, reason })
  }

  const terminal = (error: SarahFleetLiveSessionError): void => {
    if (stopped) return
    cancelReconnect?.()
    cancelReconnect = null
    closeSocket()
    setState({
      phase: "failed",
      scope,
      cursor: current?.state.cursor ?? null,
      error: issue(error.reason, false),
    })
  }

  const readNowOrTerminal = (): number | null => {
    try {
      return readNow()
    } catch (error) {
      terminal(classify(error))
      return null
    }
  }

  let requestCycle: () => Promise<void>

  const scheduleFailure = async (rawError: unknown): Promise<void> => {
    if (stopped) return
    const error = classify(rawError)
    let reconnectRefetchReason: MustRefetchReason | null = null
    if (error.reason === "aborted") {
      stop("aborted")
      return
    }
    if (error.reason === "must_refetch") {
      if (scope === null) return terminal(sessionError("invalid_scope", false))
      const refetchReason =
        pendingMustRefetchReason ?? "cursor_behind_retained_window"
      reconnectRefetchReason = refetchReason
      pendingMustRefetchReason = null
      setState({
        phase: "must_refetch",
        scope,
        cursor: cursor(),
        reason: refetchReason,
      })
      try {
        await input.persistence.clear(scope)
      } catch (clearError) {
        terminal(classify(clearError))
        return
      }
      current = null
      reconnectAttempts = 0
    } else if (!error.retryable) {
      terminal(error)
      return
    }

    reconnectAttempts += 1
    if (reconnectAttempts > maxReconnectAttempts) {
      terminal(sessionError("retry_exhausted", false))
      return
    }
    let delayMs: number
    try {
      delayMs =
        error.reason === "must_refetch"
          ? 0
          : computeBackoff(
              reconnectAttempts,
              backoffBaseMs,
              backoffMaxMs,
              readRandom(),
            )
    } catch (randomError) {
      terminal(classify(randomError))
      return
    }
    const observedAtMs = readNowOrTerminal()
    if (observedAtMs === null) return
    const retryAtMs = observedAtMs + delayMs
    if (!Number.isSafeInteger(retryAtMs)) {
      terminal(sessionError("protocol_failure", false))
      return
    }
    if (scope === null) return terminal(sessionError("invalid_scope", false))
    setState({
      phase: "reconnecting",
      scope,
      cursor: cursor(),
      attempt: reconnectAttempts,
      retryAtMs,
      mustRefetchReason: reconnectRefetchReason,
      error: issue(error.reason, true),
    })
    cancelReconnect?.()
    cancelReconnect = schedule(() => {
      cancelReconnect = null
      void requestCycle()
    }, delayMs)
  }

  const onSocketLoss = (
    generation: number,
    error: SarahFleetLiveSessionError,
  ): void => {
    if (generation !== socketGeneration || stopped) return
    closeSocket()
    void frameChain.then(() => scheduleFailure(error))
  }

  const touchLive = (): boolean => {
    if (scope === null || current === null || stopped) return false
    const observedAtMs = readNowOrTerminal()
    if (observedAtMs === null) return false
    lastActivityAtMs = observedAtMs
    reconnectAttempts = 0
    setState({
      phase: "live",
      scope,
      cursor: current.state.cursor,
      connectedAtMs,
      lastActivityAtMs,
    })
    return true
  }

  const scheduleHeartbeat = (generation: number): void => {
    cancelHeartbeat?.()
    cancelHeartbeat = schedule(() => {
      cancelHeartbeat = null
      if (generation !== socketGeneration || socket === null || stopped) return
      const observedAt = readNowOrTerminal()
      if (observedAt === null) return
      if (observedAt - lastActivityAtMs >= staleAfterMs) {
        onSocketLoss(generation, sessionError("socket_stale", true))
        return
      }
      try {
        socket.send(JSON.stringify(encodeLiveFrame(new PingFrame())))
      } catch {
        onSocketLoss(generation, sessionError("socket_closed", true))
        return
      }
      scheduleHeartbeat(generation)
    }, cadenceMs)
  }

  const enqueueFrame = (
    generation: number,
    task: () => Promise<void>,
  ): void => {
    if (queuedFrames >= maxQueuedFrames) {
      onSocketLoss(generation, sessionError("protocol_failure", true))
      return
    }
    queuedFrames += 1
    frameChain = frameChain
      .then(async () => {
        if (generation !== socketGeneration || stopped) return
        await task()
      })
      .catch((error: unknown) => {
        if (generation !== socketGeneration || stopped) return
        closeSocket()
        void scheduleFailure(classify(error))
      })
      .finally(() => {
        queuedFrames -= 1
      })
  }

  const applyFrame = (
    generation: number,
    frame: LiveFrame,
  ): void => {
    if (scope === null || current === null) {
      onSocketLoss(generation, sessionError("protocol_failure", true))
      return
    }
    if (
      (frame._tag === "DeltaFrame" || frame._tag === "MustRefetchFrame") &&
      frame.scope !== scope
    ) {
      onSocketLoss(generation, sessionError("foreign_state", false))
      return
    }
    if (!touchLive()) return
    if (frame._tag === "PingFrame" || frame._tag === "MutationAckFrame") {
      return
    }
    if (frame._tag === "MustRefetchFrame") {
      enqueueFrame(generation, async () => {
        pendingMustRefetchReason = frame.reason
        throw sessionError("must_refetch", true)
      })
      return
    }

    enqueueFrame(generation, async () => {
      if (current === null) throw sessionError("protocol_failure", true)
      const savedCursor = current.state.cursor
      if (frame.cursor < savedCursor) {
        throw sessionError("cursor_no_progress", true)
      }
      if (
        frame.cursor > savedCursor &&
        frame.entries[0]?.version !== savedCursor + 1
      ) {
        throw sessionError("cursor_no_progress", true)
      }
      const page = new LogPage({
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope: frame.scope,
        entries: [...frame.entries],
        nextCursor: SyncVersionWatermark.make(frame.cursor),
        upToDate: true,
      })
      const state = reduceSarahFleetLogPages(current.state, [page])
      const projection = projectSarahFleetProjectionState(state, readNow())
      await input.persistence.save(state)
      current = { source: "resume", state, projection }
      emitProjection(projection)
      touchLive()
    })
  }

  const connect = (
    expectedScope: typeof SyncScope.Type,
    expectedCursor: typeof SyncVersionWatermark.Type,
  ): Promise<void> =>
    new Promise<void>((resolve, reject) => {
      closeSocket()
      const generation = socketGeneration
      let opened = false
      let settled = false
      let candidate: SarahFleetWebSocketLike
      try {
        candidate = new WebSocketImpl(
          buildSarahFleetLiveConnectUrl(
            input.origin,
            expectedScope,
            expectedCursor,
          ),
        )
        socket = candidate
      } catch (error) {
        reject(classify(error))
        return
      }

      const rejectBeforeOpen = (error: SarahFleetLiveSessionError): void => {
        if (settled) return
        settled = true
        closeSocket()
        reject(error)
      }

      cancelConnectTimeout = schedule(() => {
        cancelConnectTimeout = null
        if (!opened) rejectBeforeOpen(sessionError("network_unavailable", true))
      }, connectTimeoutMs)

      candidate.onopen = () => {
        if (generation !== socketGeneration || stopped) {
          try {
            candidate.close(1000, "sarah_fleet_session_stale")
          } catch {
            // Already closed.
          }
          return
        }
        try {
          connectedAtMs = readNow()
        } catch (error) {
          rejectBeforeOpen(classify(error))
          return
        }
        opened = true
        settled = true
        cancelConnectTimeout?.()
        cancelConnectTimeout = null
        lastActivityAtMs = connectedAtMs
        setState({
          phase: "live",
          scope: expectedScope,
          cursor: expectedCursor,
          connectedAtMs,
          lastActivityAtMs,
        })
        scheduleHeartbeat(generation)
        resolve()
      }
      candidate.onmessage = (event) => {
        if (generation !== socketGeneration || stopped) return
        let frame: LiveFrame
        try {
          frame = decodeFleetLiveFrame(event.data)
        } catch (error) {
          onSocketLoss(generation, classify(error))
          return
        }
        applyFrame(generation, frame)
      }
      candidate.onerror = () => {
        if (generation !== socketGeneration || stopped) return
        if (!opened) {
          rejectBeforeOpen(sessionError("network_unavailable", true))
          return
        }
        onSocketLoss(generation, sessionError("socket_closed", true))
      }
      candidate.onclose = () => {
        if (generation !== socketGeneration || stopped) return
        if (!opened) {
          rejectBeforeOpen(sessionError("network_unavailable", true))
          return
        }
        onSocketLoss(generation, sessionError("socket_closed", true))
      }
    })

  const performCycle = async (): Promise<void> => {
    if (stopped || scope === null) return
    readNow()
    cancelReconnect?.()
    cancelReconnect = null
    closeSocket()
    await frameChain
    if (stopped) return
    const aborter = new AbortController()
    requestAbort = aborter
    setState({
      phase: "catching_up",
      scope,
      cursor: cursor(),
      attempt: reconnectAttempts,
    })
    let opened: SarahFleetProjectionOpenResult
    try {
      opened = await projectionStore.open(scope, { signal: aborter.signal })
    } finally {
      if (requestAbort === aborter) requestAbort = null
    }
    if (stopped) return
    current = opened
    emitProjection(opened.projection)
    setState({
      phase: "connecting",
      scope,
      cursor: opened.state.cursor,
      attempt: reconnectAttempts,
    })
    await connect(scope, opened.state.cursor)
  }

  requestCycle = () => {
    if (cyclePromise !== null) return cyclePromise
    cyclePromise = performCycle()
      .catch((error: unknown) => scheduleFailure(error))
      .finally(() => {
        cyclePromise = null
      })
    return cyclePromise
  }

  return {
    start: (rawScope, signal) => {
      if (disposed) return Promise.resolve()
      const firstStart = scope === null
      if (scope === null) {
        try {
          scope = exactFleetScope(rawScope)
          buildSarahFleetLiveConnectUrl(input.origin, scope, 0)
        } catch (error) {
          terminal(classify(error))
          return Promise.resolve()
        }
      } else if (scope !== rawScope) {
        terminal(sessionError("foreign_state", false))
        return Promise.resolve()
      }
      if (!firstStart) return cyclePromise ?? Promise.resolve()
      if (signal !== undefined) {
        if (ownerSignal === undefined) {
          ownerSignal = signal
          if (signal.aborted) {
            stop("aborted")
            return Promise.resolve()
          }
          const abort = () => stop("aborted")
          signal.addEventListener("abort", abort, { once: true })
          removeAbortListener = () => signal.removeEventListener("abort", abort)
        }
      } else if (ownerSignal === undefined) {
        ownerSignal = null
      }
      return requestCycle()
    },
    refresh: () => {
      if (stopped || scope === null) return Promise.resolve()
      reconnectAttempts = 0
      return requestCycle()
    },
    dispose: () => stop("disposed"),
    snapshot: () => connectionState,
    projection: () => current?.projection ?? null,
    subscribe: (listener) => {
      stateListeners.add(listener)
      listener(connectionState)
      return () => stateListeners.delete(listener)
    },
    subscribeProjection: (listener) => {
      projectionListeners.add(listener)
      if (current !== null) listener(current.projection)
      return () => projectionListeners.delete(listener)
    },
  }
}
