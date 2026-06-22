import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  WORLD_DELTA_SCHEMA_VERSION,
  WORLD_READ_MODEL_SCHEMA_VERSION,
  decodeWorldDelta,
  decodeWorldReadModel,
  type WorldDelta,
  type WorldDiagnostic,
  type WorldErrorTag,
  type WorldReadModel,
  type WorldRow,
  type WorldRef,
  type WorldSourceRef,
  worldRowKey,
} from "@openagentsinc/world-contract"

export const OPENAGENTS_WORLD_WORKER_VERSION = "0.1.0"
export const DEFAULT_WORLD_REGION_REF = "region.run.tassadar.executor.20260615.street"
export const DEFAULT_HANDSHAKE_BUFFER_LIMIT = 16

export type WorldRuntimeConfig = Readonly<{
  envName: string
  schemaVersion: string
  defaultRegionRef: string
  maxHandshakeBuffer: number
  bridgeSource: string
}>

export type RegionSocketSessionAttachment = Readonly<{
  sessionRef: string
  regionRef: string
  actorRef: string
  actorClass: "browser" | "agent" | "operator" | "service"
  connectedAt: string
  hydrated: boolean
  bufferedFrames: ReadonlyArray<string>
  cursor: string
  queuedFrames: ReadonlyArray<string>
  seenRefs: ReadonlyArray<string>
}>

export type WorldTransportFrame =
  | Readonly<{
      frameKind: "snapshot"
      delta: WorldDelta
      readModel: WorldReadModel
    }>
  | Readonly<{
      frameKind: "delta"
      delta: WorldDelta
    }>
  | Readonly<{
      frameKind: "diagnostic"
      delta: WorldDelta
      diagnostic: WorldDiagnostic
    }>

export type RegionClock = Readonly<{
  currentSeq: number
  minReplaySeq: number
}>

export type ReconnectPlan =
  | Readonly<{
      kind: "resume"
      cursor: string
      frames: ReadonlyArray<WorldTransportFrame>
    }>
  | Readonly<{
      kind: "fresh_snapshot"
      cursor: string
      frames: ReadonlyArray<WorldTransportFrame>
      diagnostic?: WorldDiagnostic
    }>

export type SightDeltaPlan = Readonly<{
  fullRefs: ReadonlyArray<string>
  liteRefs: ReadonlyArray<string>
  prunedRefs: ReadonlyArray<string>
  nextSeenRefs: ReadonlyArray<string>
}>

export type BackpressurePolicy = Readonly<{
  maxQueuedFrames: number
}>

export type BackpressureDecision =
  | Readonly<{
      kind: "enqueued"
      attachment: RegionSocketSessionAttachment
    }>
  | Readonly<{
      kind: "disconnect"
      diagnostic: WorldDiagnostic
    }>

export type WorldBridgeQueueMessage = Readonly<{
  kind: "bridge_ingest_requested"
  ingestRef: string
  sourceRef: string
  acceptedAt: string
}>

export const regionDurableObjectMigrationStatements = [
  `CREATE TABLE IF NOT EXISTS _sql_schema_migrations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
  )`,
  `CREATE TABLE IF NOT EXISTS region_projection_checkpoints (
    source_ref TEXT PRIMARY KEY,
    cursor TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    diagnostic_json TEXT NOT NULL DEFAULT '[]'
  )`,
  `CREATE TABLE IF NOT EXISTS region_socket_sessions (
    session_ref TEXT PRIMARY KEY,
    region_ref TEXT NOT NULL,
    actor_ref TEXT NOT NULL,
    actor_class TEXT NOT NULL,
    connected_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    closed_at TEXT,
    metadata_json TEXT NOT NULL
  )`,
  `CREATE INDEX IF NOT EXISTS idx_region_socket_sessions_region_seen
    ON region_socket_sessions(region_ref, last_seen_at)`,
  `CREATE TABLE IF NOT EXISTS region_transport_clock (
    region_ref TEXT PRIMARY KEY,
    current_seq INTEGER NOT NULL,
    min_replay_seq INTEGER NOT NULL,
    updated_at TEXT NOT NULL
  )`,
] as const

export const nowIso = (): string => new Date().toISOString()

export const configFromEnv = (env: {
  readonly OPENAGENTS_WORLD_ENV?: string
  readonly OPENAGENTS_WORLD_SCHEMA_VERSION?: string
  readonly OPENAGENTS_WORLD_DEFAULT_REGION?: string
  readonly OPENAGENTS_WORLD_MAX_HANDSHAKE_BUFFER?: string
  readonly OPENAGENTS_WORLD_BRIDGE_SOURCE?: string
}): WorldRuntimeConfig => ({
  envName: env.OPENAGENTS_WORLD_ENV ?? "development",
  schemaVersion: env.OPENAGENTS_WORLD_SCHEMA_VERSION ?? WORLD_CONTRACT_SCHEMA_VERSION,
  defaultRegionRef: env.OPENAGENTS_WORLD_DEFAULT_REGION ?? DEFAULT_WORLD_REGION_REF,
  maxHandshakeBuffer: Number.isFinite(Number(env.OPENAGENTS_WORLD_MAX_HANDSHAKE_BUFFER))
    ? Math.max(1, Math.min(64, Number(env.OPENAGENTS_WORLD_MAX_HANDSHAKE_BUFFER)))
    : DEFAULT_HANDSHAKE_BUFFER_LIMIT,
  bridgeSource: env.OPENAGENTS_WORLD_BRIDGE_SOURCE ?? "https://openagents.com/api/public/tassadar-run-summary",
})

export const json = (value: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(value, null, 2), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...init.headers,
    },
  })

export const normalizeRegionRef = (input: string | null | undefined): string =>
  (input ?? DEFAULT_WORLD_REGION_REF).trim().replace(/[^a-zA-Z0-9._:-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-.]+|[-.]+$/g, "")
    .slice(0, 160) || DEFAULT_WORLD_REGION_REF

export const stableWorldRef = (prefix: string, value: string): string => {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `${prefix}.${(hash >>> 0).toString(16).padStart(8, "0")}`
}

export const makeDiagnostic = (input: {
  readonly tag: WorldErrorTag
  readonly severity: "debug" | "info" | "warn" | "error"
  readonly message: string
  readonly observedAt: string
  readonly sourceRefs?: ReadonlyArray<string>
}): WorldDiagnostic => ({
  diagnosticRef: stableWorldRef("diagnostic.world", `${input.tag}:${input.message}:${input.observedAt}`) as WorldRef,
  tag: input.tag,
  severity: input.severity,
  message: input.message,
  observedAt: input.observedAt as WorldDiagnostic["observedAt"],
  sourceRefs: [...(input.sourceRefs ?? [])] as Array<WorldSourceRef>,
})

export const makeDiagnosticResponse = (
  status: number,
  input: Parameters<typeof makeDiagnostic>[0],
) =>
  json(
    {
      ok: false,
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      diagnostic: makeDiagnostic(input),
    },
    { status },
  )

export const makeZeroSnapshotDelta = (
  regionRef: string,
  generatedAt: string,
): WorldDelta =>
  decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: stableWorldRef("delta.world.snapshot", `${regionRef}:${generatedAt}`),
    kind: "snapshot",
    regionRef,
    cursor: `cursor.${normalizeRegionRef(regionRef)}.0`,
    generatedAt,
    rows: [],
  })

export const cursorForSequence = (regionRef: string, sequence: number): string =>
  `cursor.${normalizeRegionRef(regionRef)}.${Math.max(0, Math.floor(sequence))}`

export const sequenceFromCursor = (cursor: string, regionRef: string): number | null => {
  const prefix = `cursor.${normalizeRegionRef(regionRef)}.`
  if (!cursor.startsWith(prefix)) {
    return null
  }
  const sequence = Number(cursor.slice(prefix.length))
  return Number.isSafeInteger(sequence) && sequence >= 0 ? sequence : null
}

export const makeEmptyReadModel = (
  regionRef: string,
  generatedAt: string,
  cursor = cursorForSequence(regionRef, 0),
): WorldReadModel =>
  decodeWorldReadModel({
    schemaVersion: WORLD_READ_MODEL_SCHEMA_VERSION,
    regionRef,
    cursor,
    generatedAt,
    regions: {},
    pylons: {},
    avatars: {},
    positions: {},
    chatMessages: {},
    chatBubbles: {},
    emotes: {},
    intents: {},
    runs: {},
    entities: {},
    edges: {},
    proofRefs: {},
    settlementRefs: {},
    events: {},
    diagnostics: [],
  })

export const makeReadModelFromRows = (
  regionRef: string,
  cursor: string,
  generatedAt: string,
  rows: ReadonlyArray<WorldRow>,
): WorldReadModel => {
  const readModel = {
    schemaVersion: WORLD_READ_MODEL_SCHEMA_VERSION,
    regionRef,
    cursor,
    generatedAt,
    regions: {} as Record<string, unknown>,
    pylons: {} as Record<string, unknown>,
    avatars: {} as Record<string, unknown>,
    positions: {} as Record<string, unknown>,
    chatMessages: {} as Record<string, unknown>,
    chatBubbles: {} as Record<string, unknown>,
    emotes: {} as Record<string, unknown>,
    intents: {} as Record<string, unknown>,
    runs: {} as Record<string, unknown>,
    entities: {} as Record<string, unknown>,
    edges: {} as Record<string, unknown>,
    proofRefs: {} as Record<string, unknown>,
    settlementRefs: {} as Record<string, unknown>,
    events: {} as Record<string, unknown>,
    diagnostics: [],
  }
  for (const row of rows) {
    const key = worldRowKey(row)
    switch (row.kind) {
      case "world_region":
        readModel.regions[key] = row
        break
      case "pylon_station":
        readModel.pylons[key] = row
        break
      case "agent_avatar":
        readModel.avatars[key] = row
        break
      case "avatar_position":
        readModel.positions[key] = row
        break
      case "local_chat_message":
        readModel.chatMessages[key] = row
        break
      case "chat_bubble":
        readModel.chatBubbles[key] = row
        break
      case "local_emote":
        readModel.emotes[key] = row
        break
      case "agent_intent":
        readModel.intents[key] = row
        break
      case "training_run":
        readModel.runs[key] = row
        break
      case "run_entity":
        readModel.entities[key] = row
        break
      case "world_edge":
        readModel.edges[key] = row
        break
      case "proof_ref":
        readModel.proofRefs[key] = row
        break
      case "settlement_ref":
        readModel.settlementRefs[key] = row
        break
      case "world_event":
        readModel.events[key] = row
        break
      case "projection_cursor":
      case "bridge_health":
        break
    }
  }
  return decodeWorldReadModel(readModel)
}

export const makeSnapshotFrame = (
  regionRef: string,
  generatedAt: string,
  rows: ReadonlyArray<WorldRow> = [],
  cursor = cursorForSequence(regionRef, 0),
): WorldTransportFrame => {
  const delta = decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: stableWorldRef("delta.world.snapshot", `${regionRef}:${cursor}:${generatedAt}`),
    kind: "snapshot",
    regionRef,
    cursor,
    generatedAt,
    rows,
  })
  return {
    frameKind: "snapshot",
    delta,
    readModel: makeReadModelFromRows(regionRef, cursor, generatedAt, rows),
  }
}

export const makeHeartbeatFrame = (
  regionRef: string,
  cursor: string,
  generatedAt: string,
): WorldTransportFrame => ({
  frameKind: "delta",
  delta: decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: stableWorldRef("delta.world.heartbeat", `${regionRef}:${cursor}:${generatedAt}`),
    kind: "heartbeat",
    regionRef,
    cursor,
    generatedAt,
    rows: [],
  }),
})

export const makeDiagnosticFrame = (
  regionRef: string,
  cursor: string,
  diagnostic: WorldDiagnostic,
): WorldTransportFrame => ({
  frameKind: "diagnostic",
  diagnostic,
  delta: decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: stableWorldRef("delta.world.diagnostic", `${regionRef}:${diagnostic.diagnosticRef}`),
    kind: "diagnostic",
    regionRef,
    cursor,
    generatedAt: diagnostic.observedAt,
    diagnostic,
  }),
})

export const makeReconnectPlan = (
  regionRef: string,
  reconnectCursor: string | null,
  clock: RegionClock,
  generatedAt: string,
): ReconnectPlan => {
  const currentCursor = cursorForSequence(regionRef, clock.currentSeq)
  if (reconnectCursor === null || reconnectCursor.length === 0) {
    return {
      kind: "fresh_snapshot",
      cursor: currentCursor,
      frames: [makeSnapshotFrame(regionRef, generatedAt, [], currentCursor)],
    }
  }

  const sequence = sequenceFromCursor(reconnectCursor, regionRef)
  if (sequence !== null && sequence >= clock.minReplaySeq && sequence <= clock.currentSeq) {
    return {
      kind: "resume",
      cursor: currentCursor,
      frames: [makeHeartbeatFrame(regionRef, currentCursor, generatedAt)],
    }
  }

  const diagnostic = makeDiagnostic({
    tag: "cursor",
    severity: "warn",
    message: "World cursor is stale or not valid for this region; sending a fresh snapshot.",
    observedAt: generatedAt,
    sourceRefs: [reconnectCursor],
  })
  return {
    kind: "fresh_snapshot",
    cursor: currentCursor,
    diagnostic,
    frames: [
      makeDiagnosticFrame(regionRef, currentCursor, diagnostic),
      makeSnapshotFrame(regionRef, generatedAt, [], currentCursor),
    ],
  }
}

export const planSightDelta = (
  seenRefs: ReadonlyArray<string>,
  currentInterestRefs: ReadonlyArray<string>,
): SightDeltaPlan => {
  const seen = new Set(seenRefs)
  const interest = new Set(currentInterestRefs)
  const fullRefs = currentInterestRefs.filter(ref => !seen.has(ref))
  const liteRefs = currentInterestRefs.filter(ref => seen.has(ref))
  const prunedRefs = seenRefs.filter(ref => !interest.has(ref))

  return {
    fullRefs,
    liteRefs,
    prunedRefs,
    nextSeenRefs: [...interest],
  }
}

export const enqueueTransportFrame = (
  attachment: RegionSocketSessionAttachment,
  frame: WorldTransportFrame,
  policy: BackpressurePolicy,
): BackpressureDecision => {
  if (attachment.queuedFrames.length >= policy.maxQueuedFrames) {
    return {
      kind: "disconnect",
      diagnostic: makeDiagnostic({
        tag: "command",
        severity: "warn",
        message: "World session backpressure limit exceeded; disconnecting the slow subscriber.",
        observedAt: nowIso(),
        sourceRefs: [attachment.sessionRef],
      }),
    }
  }

  return {
    kind: "enqueued",
    attachment: {
      ...attachment,
      cursor: frame.delta.cursor,
      queuedFrames: [...attachment.queuedFrames, serializeWorldFrame(frame)],
    },
  }
}

export class WorldWireCache {
  private readonly encoded = new Map<string, string>()

  encodeEntity(row: WorldRow, cursor: string): string {
    const key = `${cursor}:${row.kind}:${worldRowKey(row)}`
    const existing = this.encoded.get(key)
    if (existing !== undefined) {
      return existing
    }
    const value = JSON.stringify(row)
    this.encoded.set(key, value)
    return value
  }

  get size(): number {
    return this.encoded.size
  }
}

export const serializeWorldFrame = (value: unknown): string =>
  JSON.stringify(value)

export const makeInitialSessionAttachment = (input: {
  readonly regionRef: string
  readonly actorRef?: string
  readonly actorClass?: RegionSocketSessionAttachment["actorClass"]
  readonly connectedAt: string
}): RegionSocketSessionAttachment => ({
  sessionRef: stableWorldRef("world_session", `${input.regionRef}:${input.actorRef ?? "anonymous"}:${input.connectedAt}`),
  regionRef: normalizeRegionRef(input.regionRef),
  actorRef: input.actorRef ?? "actor.public.anonymous",
  actorClass: input.actorClass ?? "browser",
  connectedAt: input.connectedAt,
  hydrated: false,
  bufferedFrames: [],
  cursor: cursorForSequence(input.regionRef, 0),
  queuedFrames: [],
  seenRefs: [],
})

export const bufferHandshakeFrame = (
  attachment: RegionSocketSessionAttachment,
  frame: string,
  maxBufferedFrames: number,
):
  | { readonly ok: true; readonly attachment: RegionSocketSessionAttachment }
  | { readonly ok: false; readonly diagnostic: WorldDiagnostic } => {
  if (attachment.bufferedFrames.length >= maxBufferedFrames) {
    return {
      ok: false,
      diagnostic: makeDiagnostic({
        tag: "auth",
        severity: "warn",
        message: "World socket handshake buffer is full; frame rejected before session hydration completed.",
        observedAt: nowIso(),
        sourceRefs: [attachment.sessionRef],
      }),
    }
  }

  return {
    ok: true,
    attachment: {
      ...attachment,
      bufferedFrames: [...attachment.bufferedFrames, frame],
    },
  }
}

export const hydrateBufferedSession = (
  attachment: RegionSocketSessionAttachment,
): RegionSocketSessionAttachment => ({
  ...attachment,
  hydrated: true,
  bufferedFrames: [],
})

export const regionRefFromSocketPath = (pathname: string): string | null => {
  const match = pathname.match(/^\/regions\/([^/]+)\/socket$/)
  return match?.[1] === undefined ? null : normalizeRegionRef(decodeURIComponent(match[1]))
}

export const socketUrlForRegion = (request: Request, regionRef: string): string => {
  const url = new URL(request.url)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  url.pathname = `/regions/${encodeURIComponent(normalizeRegionRef(regionRef))}/socket`
  url.search = ""
  return url.toString()
}
