import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  WORLD_DELTA_SCHEMA_VERSION,
  decodeWorldDelta,
  type WorldDelta,
  type WorldDiagnostic,
  type WorldErrorTag,
  type WorldRef,
  type WorldSourceRef,
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
