import { DurableObject } from "cloudflare:workers"
import { Context, Effect, Layer } from "effect"

import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  decodeWorldBridgePayload,
  type WorldBridgePayload,
  type WorldDelta,
  type WorldRow,
  type WorldSubscriptionPlan,
} from "@openagentsinc/world-contract"

import {
  bridgePayloadFromPublicActivityTimelineEnvelope,
  bridgeHealthRow,
  decodePublicBridgeRows,
  dedupeBridgeRows,
  publicActivityTimelineBridgePollUrl,
  projectionCursorRow,
  projectionRowMetadata,
} from "./bridge"
import {
  applyWorldCommand,
  commandDeltaFrame,
  makeEmptyHotState,
  type WorldExpiringRef,
  type WorldHotState,
} from "./commands"
import {
  encodeAlarmTimestamp,
  expireWorldHotStateAt,
  expiryDeltaFrame,
  nextExpiryAlarmAt,
} from "./expiry"
import {
  moderationConfigFromEnv,
} from "./moderation"
import {
  OPENAGENTS_WORLD_WORKER_VERSION,
  bufferHandshakeFrame,
  configFromEnv,
  hydrateBufferedSession,
  json,
  makeDiagnostic,
  makeDiagnosticFrame,
  makeDiagnosticResponse,
  makeHeartbeatFrame,
  makeInitialSessionAttachment,
  makeReconnectPlan,
  makeSnapshotFrame,
  normalizeRegionRef,
  regionClockFromStorageRows,
  regionDurableObjectMigrationStatements,
  regionRefFromSocketPath,
  serializeWorldFrame,
  socketUrlForRegion,
  stableWorldRef,
  type ReconnectPlan,
  type RegionSocketSessionAttachment,
  type WorldBridgeQueueMessage,
  type WorldRuntimeConfig,
} from "./protocol"
import {
  type DeltaReplayBufferRow,
  type ReplayBufferConfig,
  makeReplayBufferRow,
  planReplayBufferReconnect,
  regionDeltaReplayBufferMigrationStatements,
  replayBufferConfigFromEnv,
  replayBufferEvictionPlan,
} from "./replay-buffer"
import {
  approveSubscriptionPlan,
  entitiesFromRows,
  planSubscriptionInterestDelta,
  subscriptionInterestStateFromAttachment,
  subscriptionRequestFromUrl,
  type WorldSubscriptionPolicyError,
  type WorldSubscriptionPlanRequest,
} from "./subscriptions"

export interface Env {
  readonly REGION_DURABLE_OBJECT: DurableObjectNamespace<RegionDurableObject>
  readonly WORLD_DB: D1Database
  readonly WORLD_BRIDGE_QUEUE?: Queue<WorldBridgeQueueMessage>
  readonly OPENAGENTS_WORLD_ENV?: string
  readonly OPENAGENTS_WORLD_SCHEMA_VERSION?: string
  readonly OPENAGENTS_WORLD_DEFAULT_REGION?: string
  readonly OPENAGENTS_WORLD_MAX_HANDSHAKE_BUFFER?: string
  readonly OPENAGENTS_WORLD_BRIDGE_SOURCE?: string
  readonly OPENAGENTS_WORLD_ACTIVITY_TIMELINE_SOURCE?: string
  readonly OPENAGENTS_WORLD_ACTIVITY_TIMELINE_LIMIT?: string
  readonly OPENAGENTS_WORLD_MODERATION_HARD_TOKENS_JSON?: string
  readonly OPENAGENTS_WORLD_MODERATION_SOFT_TOKENS_JSON?: string
  readonly OPENAGENTS_WORLD_DELTA_REPLAY?: string
  readonly OPENAGENTS_WORLD_DELTA_REPLAY_MAX_DELTAS?: string
  readonly OPENAGENTS_WORLD_DELTA_REPLAY_MAX_BYTES?: string
}

type RegionSqlRow = Readonly<Record<string, unknown>>

type RegionSqlStorage = Readonly<{
  exec<T = RegionSqlRow>(
    query: string,
    ...params: ReadonlyArray<unknown>
  ): {
    one(): T | null
    toArray(): Array<T>
    readonly rowsRead: number
    readonly rowsWritten: number
  }
}>

type WorldBindingsShape = Readonly<{
  env: Env
  regionObjects: DurableObjectNamespace<RegionDurableObject>
  db: D1Database
  bridgeQueue?: Queue<WorldBridgeQueueMessage>
}>

type WorldWaitUntilShape = Readonly<{
  waitUntil: (effect: Promise<unknown>) => void
}>

const PROJECTION_SNAPSHOT_ROW_LIMIT = 500

export class WorldBindings extends Context.Service<
  WorldBindings,
  WorldBindingsShape
>()("WorldBindings") {}

export class WorldConfigService extends Context.Service<
  WorldConfigService,
  WorldRuntimeConfig
>()("WorldConfigService") {}

export class WorldRequestContext extends Context.Service<
  WorldRequestContext,
  {
    readonly request: Request
    readonly ctx: ExecutionContext
  }
>()("WorldRequestContext") {}

export class WorldWaitUntil extends Context.Service<
  WorldWaitUntil,
  WorldWaitUntilShape
>()("WorldWaitUntil") {}

export class WorldLogger extends Context.Service<
  WorldLogger,
  {
    readonly info: (message: string, fields?: Record<string, unknown>) => Effect.Effect<void>
    readonly warn: (message: string, fields?: Record<string, unknown>) => Effect.Effect<void>
    readonly error: (message: string, fields?: Record<string, unknown>) => Effect.Effect<void>
  }
>()("WorldLogger") {}

export const makeWorldRuntimeLayer = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
) =>
  Layer.mergeAll(
    Layer.succeed(WorldBindings, {
      env,
      regionObjects: env.REGION_DURABLE_OBJECT,
      db: env.WORLD_DB,
      ...(env.WORLD_BRIDGE_QUEUE === undefined ? {} : { bridgeQueue: env.WORLD_BRIDGE_QUEUE }),
    }),
    Layer.succeed(WorldConfigService, configFromEnv(env)),
    Layer.succeed(WorldRequestContext, { request, ctx }),
    Layer.succeed(WorldWaitUntil, { waitUntil: promise => ctx.waitUntil(promise) }),
    Layer.succeed(WorldLogger, {
      info: () => Effect.void,
      warn: () => Effect.void,
      error: () => Effect.void,
    }),
  )

const runWorkerEffect = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  effect: Effect.Effect<Response, never, WorldBindings | WorldConfigService | WorldRequestContext | WorldWaitUntil | WorldLogger>,
): Promise<Response> =>
  Effect.runPromise(Effect.provide(effect, makeWorldRuntimeLayer(request, env, ctx)))

const handleWorkerRequest = (): Effect.Effect<Response, never, WorldBindings | WorldConfigService | WorldRequestContext | WorldWaitUntil | WorldLogger> =>
  Effect.gen(function* () {
    const requestContext = yield* WorldRequestContext
    const bindings = yield* WorldBindings
    const config = yield* WorldConfigService
    const waitUntil = yield* WorldWaitUntil
    const request = requestContext.request
    const url = new URL(request.url)

    if (request.method === "GET" && url.pathname === "/health") {
      return json({
        ok: true,
        service: "openagents-world",
        version: OPENAGENTS_WORLD_WORKER_VERSION,
        schemaVersion: config.schemaVersion,
        env: config.envName,
      })
    }

    if (request.method === "GET" && url.pathname === "/version") {
      return json({
        service: "openagents-world",
        version: OPENAGENTS_WORLD_WORKER_VERSION,
        contractSchemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        configuredSchemaVersion: config.schemaVersion,
      })
    }

    if (request.method === "GET" && url.pathname === "/connect") {
      const regionRef = normalizeRegionRef(url.searchParams.get("region") ?? config.defaultRegionRef)
      const subscriptionPlan = runSubscriptionApproval(subscriptionRequestFromUrl(url, regionRef))
      if (!subscriptionPlan.ok) {
        return makeDiagnosticResponse(400, {
          tag: "validation",
          severity: "warn",
          message: subscriptionPlan.error.reason,
          observedAt: new Date().toISOString(),
          sourceRefs: [subscriptionPlan.error.sourceRef],
        })
      }
      return json({
        ok: true,
        schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        regionRef,
        socketUrl: socketUrlForRegion(request, regionRef),
        subscriptionPlan: subscriptionPlan.plan,
      })
    }

    const socketRegionRef = regionRefFromSocketPath(url.pathname)
    if (socketRegionRef !== null) {
      const objectId = bindings.regionObjects.idFromName(socketRegionRef)
      return yield* Effect.promise(() => bindings.regionObjects.get(objectId).fetch(request))
    }

    if (request.method === "POST" && url.pathname === "/bridge/ingest") {
      const result = yield* Effect.promise(() => handleBridgeIngest(request, bindings, config, waitUntil))
      return result
    }

    if (request.method === "POST" && url.pathname === "/bridge/poll-public-activity-timeline") {
      const result = yield* Effect.promise(() =>
        handlePublicActivityTimelineBridgePoll(bindings, config, waitUntil)
      )
      return json(result, { status: result.ok ? 202 : 502 })
    }

    return makeDiagnosticResponse(404, {
      tag: "validation",
      severity: "warn",
      message: "OpenAgents world route not found.",
      observedAt: new Date().toISOString(),
      sourceRefs: ["source.openagents_world.worker"],
    })
  })

const readBridgePayload = async (request: Request): Promise<WorldBridgePayload | null> => {
  try {
    const payload = decodeWorldBridgePayload(await request.json())
    decodePublicBridgeRows(payload.rows)
    return payload
  } catch {
    return null
  }
}

const persistAcceptedBridgePayload = async (
  payload: WorldBridgePayload,
  bindings: WorldBindingsShape,
  acceptedAt: string,
  waitUntil: WorldWaitUntilShape,
) => {
  const ingestRef = stableWorldRef(
    "bridge_ingest.world",
    `${payload.sourceRef}:${payload.cursor ?? payload.observedAt}:${acceptedAt}`,
  )
  const rows = dedupeBridgeRows([
    ...payload.rows,
    bridgeHealthRow({
      sourceRef: payload.sourceRef,
      status: "current",
      observedAt: acceptedAt,
    }),
    ...(payload.cursor === undefined
      ? []
      : [projectionCursorRow({
          sourceRef: payload.sourceRef,
          cursor: payload.cursor,
          observedAt: payload.observedAt,
        })]),
  ])

  await persistProjectionRows(bindings.db, {
    ingestRef,
    sourceRef: payload.sourceRef,
    observedAt: payload.observedAt,
    acceptedAt,
    ...(payload.cursor === undefined ? {} : { cursor: payload.cursor }),
    rows,
    payload,
  })

  if (bindings.bridgeQueue !== undefined) {
    waitUntil.waitUntil(bindings.bridgeQueue.send({
      kind: "bridge_ingest_requested",
      ingestRef,
      sourceRef: payload.sourceRef,
      acceptedAt,
    }))
  }

  return {
    acceptedAt,
    diagnostic: makeDiagnostic({
      tag: "bridge",
      severity: "info",
      message: "World bridge ingest persisted public projection rows.",
      observedAt: acceptedAt,
      sourceRefs: [payload.sourceRef],
    }),
    ingestRef,
    rowCount: rows.length,
    rows,
  }
}

const persistFailedBridgeRead = async (
  input: {
    readonly acceptedAt: string
    readonly bindings: WorldBindingsShape
    readonly message: string
    readonly observedAt?: string
    readonly payload?: unknown
    readonly sourceRef: string
  },
) => {
  const diagnostic = makeDiagnostic({
    tag: "bridge",
    severity: "warn",
    message: input.message,
    observedAt: input.acceptedAt,
    sourceRefs: [input.sourceRef],
  })
  const ingestRef = stableWorldRef(
    "bridge_ingest.world",
    `${input.sourceRef}:failed:${input.acceptedAt}`,
  )
  const rows = [bridgeHealthRow({
    sourceRef: input.sourceRef,
    status: "failed",
    observedAt: input.acceptedAt,
    diagnosticRefs: [diagnostic.diagnosticRef],
  })]
  await persistProjectionRows(input.bindings.db, {
    ingestRef,
    sourceRef: input.sourceRef,
    observedAt: input.observedAt ?? input.acceptedAt,
    acceptedAt: input.acceptedAt,
    rows,
    payload: input.payload ?? {
      rejected: true,
      diagnosticRef: diagnostic.diagnosticRef,
    },
  })
  return {
    acceptedAt: input.acceptedAt,
    diagnostic,
    ingestRef,
    rowCount: rows.length,
  }
}

const handleBridgeIngest = async (
  request: Request,
  bindings: WorldBindingsShape,
  config: WorldRuntimeConfig,
  waitUntil: WorldWaitUntilShape,
): Promise<Response> => {
  const acceptedAt = new Date().toISOString()
  const payload = await readBridgePayload(request)
  const sourceRef = payload?.sourceRef ?? config.bridgeSource

  if (payload === null) {
    const failed = await persistFailedBridgeRead({
      acceptedAt,
      bindings,
      message: "World bridge ingest rejected: payload failed schema or public-safe validation.",
      sourceRef,
    })
    return json(
      {
        ok: false,
        schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        ingestRef: failed.ingestRef,
        acceptedAt: failed.acceptedAt,
        rowCount: failed.rowCount,
        diagnostic: failed.diagnostic,
      },
      { status: 400 },
    )
  }

  const persisted = await persistAcceptedBridgePayload(payload, bindings, acceptedAt, waitUntil)

  return json(
    {
      ok: true,
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      ingestRef: persisted.ingestRef,
      acceptedAt: persisted.acceptedAt,
      rowCount: persisted.rowCount,
      diagnostic: persisted.diagnostic,
    },
    { status: 202 },
  )
}

const readProjectionCursor = async (
  db: D1Database,
  sourceRef: string,
): Promise<string | undefined> => {
  const result = await db.prepare(
    "SELECT cursor FROM world_projection_cursors WHERE source_ref = ? LIMIT 1",
  ).bind(sourceRef).all<{ cursor: string }>()
  const cursor = result.results?.[0]?.cursor
  return typeof cursor === "string" && cursor.trim().length > 0
    ? cursor.trim()
    : undefined
}

const publicBridgeFailureReason = (error: unknown): string => {
  const message = error instanceof Error ? error.message : String(error)
  return message
    .replace(/raw_prompt|raw_payload|provider_payload|secret|token|sk-[a-z0-9_-]+/gi, "redacted")
    .slice(0, 240)
}

const handlePublicActivityTimelineBridgePoll = async (
  bindings: WorldBindingsShape,
  config: WorldRuntimeConfig,
  waitUntil: WorldWaitUntilShape,
) => {
  const acceptedAt = new Date().toISOString()
  const sourceRef = config.activityTimelineBridgeSource
  const cursor = await readProjectionCursor(bindings.db, sourceRef)
  const pollUrl = publicActivityTimelineBridgePollUrl({
    sourceRef,
    ...(cursor === undefined ? {} : { cursor }),
    limit: config.activityTimelineBridgeLimit,
  })

  try {
    const response = await fetch(pollUrl, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      const failed = await persistFailedBridgeRead({
        acceptedAt,
        bindings,
        message: `Public activity timeline bridge poll failed with HTTP ${response.status}.`,
        payload: {
          status: response.status,
          sourceRef,
        },
        sourceRef,
      })
      return {
        ok: false,
        schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        sourceRef,
        pollUrl,
        previousCursor: cursor ?? null,
        ...failed,
      }
    }

    const body = await response.json()
    const payload = bridgePayloadFromPublicActivityTimelineEnvelope(body, sourceRef)
    const persisted = await persistAcceptedBridgePayload(payload, bindings, acceptedAt, waitUntil)
    const { rows: _rows, ...publicPersisted } = persisted
    return {
      ok: true,
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      sourceRef,
      pollUrl,
      previousCursor: cursor ?? null,
      cursor: payload.cursor ?? null,
      projectedRowCount: payload.rows.length,
      ...publicPersisted,
    }
  } catch (error) {
    const failed = await persistFailedBridgeRead({
      acceptedAt,
      bindings,
      message: `Public activity timeline bridge poll failed: ${publicBridgeFailureReason(error)}`,
      payload: {
        sourceRef,
      },
      sourceRef,
    })
    return {
      ok: false,
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      sourceRef,
      pollUrl,
      previousCursor: cursor ?? null,
      ...failed,
    }
  }
}

const persistProjectionRows = async (
  db: D1Database,
  input: {
    readonly ingestRef: string
    readonly sourceRef: string
    readonly observedAt: string
    readonly acceptedAt: string
    readonly cursor?: string
    readonly rows: ReadonlyArray<WorldRow>
    readonly payload: unknown
  },
): Promise<void> => {
  for (const row of input.rows) {
    const metadata = projectionRowMetadata(row)
    await db.prepare(
      `INSERT INTO world_projection_rows (
        row_ref,
        row_kind,
        region_ref,
        run_ref,
        source_ref,
        cursor,
        payload_json,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(row_ref) DO UPDATE SET
        row_kind = excluded.row_kind,
        region_ref = excluded.region_ref,
        run_ref = excluded.run_ref,
        source_ref = excluded.source_ref,
        cursor = excluded.cursor,
        payload_json = excluded.payload_json,
        updated_at = excluded.updated_at`,
    ).bind(
      metadata.rowRef,
      metadata.rowKind,
      metadata.regionRef,
      metadata.runRef,
      metadata.sourceRef,
      metadata.cursor,
      JSON.stringify(row),
      metadata.updatedAt,
    ).run()
  }

  if (input.cursor !== undefined) {
    await db.prepare(
      `INSERT INTO world_projection_cursors (
        source_ref,
        cursor,
        observed_at,
        diagnostic_json
      ) VALUES (?, ?, ?, ?)
      ON CONFLICT(source_ref) DO UPDATE SET
        cursor = excluded.cursor,
        observed_at = excluded.observed_at,
        diagnostic_json = excluded.diagnostic_json`,
    ).bind(
      input.sourceRef,
      input.cursor,
      input.observedAt,
      JSON.stringify([]),
    ).run()
  }

  await db.prepare(
    `INSERT INTO world_bridge_ingest_log (
      ingest_ref,
      source_ref,
      observed_at,
      accepted_at,
      cursor,
      row_count,
      payload_json
    ) VALUES (?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(ingest_ref) DO UPDATE SET
      source_ref = excluded.source_ref,
      observed_at = excluded.observed_at,
      accepted_at = excluded.accepted_at,
      cursor = excluded.cursor,
      row_count = excluded.row_count,
      payload_json = excluded.payload_json`,
  ).bind(
    input.ingestRef,
    input.sourceRef,
    input.observedAt,
    input.acceptedAt,
    input.cursor ?? null,
    input.rows.length,
    JSON.stringify(input.payload),
  ).run()
}

const readSnapshotProjectionRows = async (
  db: D1Database,
  regionRef: string,
  limit = PROJECTION_SNAPSHOT_ROW_LIMIT,
): Promise<ReadonlyArray<WorldRow>> => {
  try {
    const result = await db.prepare(
      `SELECT payload_json
      FROM world_projection_rows
      WHERE region_ref IS NULL OR region_ref = ?
      ORDER BY updated_at DESC, row_ref ASC
      LIMIT ?`,
    ).bind(regionRef, limit).all<{ payload_json: string }>()
    const rows: Array<WorldRow> = []
    for (const row of result.results ?? []) {
      if (typeof row.payload_json !== "string") continue
      try {
        const decoded = decodePublicBridgeRows([JSON.parse(row.payload_json)])[0]
        if (decoded !== undefined) rows.push(decoded)
      } catch {
        // Bad persisted projection rows should not break a WebSocket upgrade.
      }
    }
    return dedupeBridgeRows(rows)
  } catch {
    return []
  }
}

const runSubscriptionApproval = (
  request: WorldSubscriptionPlanRequest,
):
  | { readonly ok: true; readonly plan: WorldSubscriptionPlan }
  | { readonly ok: false; readonly error: WorldSubscriptionPolicyError } => {
  try {
    return {
      ok: true,
      plan: Effect.runSync(approveSubscriptionPlan(request)),
    }
  } catch (error) {
    return {
      ok: false,
      error: normalizeSubscriptionPolicyError(error),
    }
  }
}

const normalizeSubscriptionPolicyError = (error: unknown): WorldSubscriptionPolicyError => {
  if (
    typeof error === "object" &&
    error !== null &&
    "_tag" in error &&
    error._tag === "WorldSubscriptionPolicyError" &&
    "reason" in error &&
    "sourceRef" in error
  ) {
    return error as WorldSubscriptionPolicyError
  }
  return {
    _tag: "WorldSubscriptionPolicyError",
    reason: error instanceof Error ? error.message : "Subscription plan rejected.",
    sourceRef: "subscription.world.policy",
  } as WorldSubscriptionPolicyError
}

export class RegionDurableObject extends DurableObject<Env> {
  private initialized: Promise<void>
  private hotState: WorldHotState | null = null
  private readonly replayConfig: ReplayBufferConfig

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env)
    this.replayConfig = replayBufferConfigFromEnv(env)
    this.initialized = ctx.blockConcurrencyWhile(async () => {
      this.migrateRegionStorage()
      this.restoreHibernatedSessions()
    })
  }

  override async fetch(request: Request): Promise<Response> {
    await this.initialized
    const url = new URL(request.url)
    const regionRef = regionRefFromSocketPath(url.pathname) ?? normalizeRegionRef(url.searchParams.get("region"))
    this.ensureHotState(regionRef)

    if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
      return makeDiagnosticResponse(426, {
        tag: "validation",
        severity: "warn",
        message: "World region socket route requires a WebSocket upgrade.",
        observedAt: new Date().toISOString(),
        sourceRefs: [regionRef],
      })
    }

    const pair = new WebSocketPair()
    const [client, server] = Object.values(pair)
    const connectedAt = new Date().toISOString()
    const clock = this.readRegionClock(regionRef)
    const reconnectPlan = this.resolveReconnectPlan(
      regionRef,
      url.searchParams.get("cursor"),
      clock,
      connectedAt,
    )
    const snapshotRows = reconnectPlan.kind === "fresh_snapshot"
      ? await readSnapshotProjectionRows(this.env.WORLD_DB, regionRef)
      : []
    const subscriptionPlan = runSubscriptionApproval(subscriptionRequestFromUrl(url, regionRef))
    if (!subscriptionPlan.ok) {
      return makeDiagnosticResponse(400, {
        tag: "validation",
        severity: "warn",
        message: subscriptionPlan.error.reason,
        observedAt: connectedAt,
        sourceRefs: [subscriptionPlan.error.sourceRef],
      })
    }
    const actorRef = url.searchParams.get("actorRef")
    const actorClass = url.searchParams.get("actorClass")
    const attachment = makeInitialSessionAttachment({
      regionRef,
      ...(actorRef === null ? {} : { actorRef }),
      ...(actorClass === null ? {} : { actorClass: actorClass as RegionSocketSessionAttachment["actorClass"] }),
      connectedAt,
      subscriptionPlan: subscriptionPlan.plan,
    })
    const attachedSession = {
      ...attachment,
      cursor: reconnectPlan.cursor,
    }

    server.serializeAttachment(attachedSession)
    this.ctx.acceptWebSocket(server)
    this.upsertSession(attachedSession, connectedAt)
    for (const frame of reconnectPlan.frames) {
      server.send(serializeWorldFrame(
        frame.frameKind === "snapshot"
          ? makeSnapshotFrame(regionRef, connectedAt, snapshotRows, reconnectPlan.cursor)
          : frame,
      ))
    }

    return new Response(null, {
      status: 101,
      webSocket: client,
    })
  }

  override async webSocketMessage(webSocket: WebSocket, message: string | ArrayBuffer): Promise<void> {
    await this.initialized
    const attachment = this.readAttachment(webSocket)
    const frame = typeof message === "string" ? message : new TextDecoder().decode(message)
    const config = configFromEnv(this.env)

    if (!attachment.hydrated) {
      const buffered = bufferHandshakeFrame(attachment, frame, config.maxHandshakeBuffer)
      if (!buffered.ok) {
        webSocket.send(serializeWorldFrame(buffered.diagnostic))
        return
      }

      const hydrated = hydrateBufferedSession(buffered.attachment)
      const generatedAt = new Date().toISOString()
      const snapshotRows = await readSnapshotProjectionRows(this.env.WORLD_DB, hydrated.regionRef)
      webSocket.serializeAttachment(hydrated)
      this.upsertSession(hydrated, generatedAt)
      webSocket.send(serializeWorldFrame(makeSnapshotFrame(
        hydrated.regionRef,
        generatedAt,
        snapshotRows,
        hydrated.cursor,
      )))
      webSocket.send(serializeWorldFrame(makeDiagnostic({
        tag: "auth",
        severity: "info",
        message: `World socket session hydrated; replayed ${buffered.attachment.bufferedFrames.length} buffered frame(s).`,
        observedAt: generatedAt,
        sourceRefs: [hydrated.sessionRef],
      })))
      return
    }

    const observedAt = new Date().toISOString()
    this.ensureHotState(attachment.regionRef)
    const hotState = this.hotState ?? makeEmptyHotState(attachment.regionRef)
    try {
      const result = await Effect.runPromise(applyWorldCommand(
        hotState,
        JSON.parse(frame) as unknown,
        observedAt,
        {
          sessionRef: attachment.sessionRef,
          moderationConfig: moderationConfigFromEnv(this.env),
        },
      ))
      this.hotState = result.state
      this.writeRegionClock(result.state.regionRef, result.state.sequence, result.state.minReplaySeq, observedAt)
      this.persistExpiryRefs(result.state)
      await this.scheduleNextExpiryAlarm(result.state)
      // Buffer only sequence-advancing deltas (accepted commands). Rejected
      // commands emit a `diagnostic` delta that keeps the prior sequence and
      // must NOT collide with the accepted payload already at that offset.
      if (result.delta.kind !== "diagnostic") {
        this.appendReplayBufferDelta(result.delta, result.state.sequence, observedAt)
      }
      this.broadcastCommandDelta(result.delta, observedAt)
    } catch (error) {
      const diagnostic = makeDiagnostic({
        tag: "validation",
        severity: "warn",
        message: `World command decode failed: ${error instanceof Error ? error.message : String(error)}`,
        observedAt,
        sourceRefs: [attachment.sessionRef],
      })
      webSocket.send(serializeWorldFrame(makeDiagnosticFrame(
        attachment.regionRef,
        attachment.cursor,
        diagnostic,
      )))
    }
  }

  private applySubscriptionStateToAttachment(input: {
    readonly attachment: RegionSocketSessionAttachment
    readonly rows: ReadonlyArray<WorldRow>
    readonly deletedRefs: ReadonlyArray<string>
    readonly cursor: string
  }): RegionSocketSessionAttachment {
    const plan = input.attachment.subscriptionPlan
    if (plan === undefined) {
      return {
        ...input.attachment,
        cursor: input.cursor,
        seenRefs: input.attachment.seenRefs.filter(ref => !input.deletedRefs.includes(ref)),
        interestTierByRef: Object.fromEntries(
          Object.entries(input.attachment.interestTierByRef).filter(([ref]) => !input.deletedRefs.includes(ref)),
        ),
      }
    }
    const entities = entitiesFromRows(input.rows)
    if (entities.length === 0) {
      return {
        ...input.attachment,
        cursor: input.cursor,
        seenRefs: input.attachment.seenRefs.filter(ref => !input.deletedRefs.includes(ref)),
        interestTierByRef: Object.fromEntries(
          Object.entries(input.attachment.interestTierByRef).filter(([ref]) => !input.deletedRefs.includes(ref)),
        ),
      }
    }
    const interest = planSubscriptionInterestDelta({
      plan,
      previous: subscriptionInterestStateFromAttachment({
        seenRefs: input.attachment.seenRefs,
        tierByRef: input.attachment.interestTierByRef,
      }),
      entities,
    })
    const deletedRefs = new Set(input.deletedRefs)
    return {
      ...input.attachment,
      cursor: input.cursor,
      seenRefs: interest.nextState.seenRefs.filter(ref => !deletedRefs.has(ref)),
      interestTierByRef: Object.fromEntries(
        Object.entries(interest.nextState.tierByRef).filter(([ref]) => !deletedRefs.has(ref)),
      ),
    }
  }

  private broadcastCommandDelta(delta: WorldDelta, observedAt: string): void {
    const frame = serializeWorldFrame(commandDeltaFrame(delta))
    for (const socket of this.ctx.getWebSockets()) {
      const attachment = this.readAttachment(socket)
      const nextAttachment = this.applySubscriptionStateToAttachment({
        attachment,
        rows: delta.rows ?? [],
        deletedRefs: delta.deletedRefs ?? [],
        cursor: delta.cursor,
      })
      socket.serializeAttachment(nextAttachment)
      this.upsertSession(nextAttachment, observedAt)
      socket.send(frame)
    }
  }

  override async webSocketClose(webSocket: WebSocket): Promise<void> {
    await this.initialized
    const attachment = this.readAttachment(webSocket)
    this.sql.exec(
      "UPDATE region_socket_sessions SET closed_at = ?, last_seen_at = ? WHERE session_ref = ?",
      new Date().toISOString(),
      new Date().toISOString(),
      attachment.sessionRef,
    )
  }

  override async webSocketError(webSocket: WebSocket): Promise<void> {
    await this.initialized
    const attachment = this.readAttachment(webSocket)
    this.upsertSession(attachment, new Date().toISOString())
  }

  override async alarm(): Promise<void> {
    await this.initialized
    const regionRef = this.hotState?.regionRef ?? "region.run.tassadar.executor.20260615.street"
    this.ensureHotState(regionRef)
    const hotState = this.hotState ?? makeEmptyHotState(regionRef)
    const now = new Date().toISOString()
    const plan = expireWorldHotStateAt(hotState, now)
    this.hotState = plan.state
    this.writeRegionClock(plan.state.regionRef, plan.state.sequence, plan.state.minReplaySeq, now)
    this.persistExpiryRefs(plan.state)
    if (plan.delta !== null) {
      // Expiry advances the sequence and emits a `delete` delta; buffer it so a
      // reconnect across an expiry boundary still gets gap replay.
      this.appendReplayBufferDelta(plan.delta, plan.state.sequence, now)
      const frame = serializeWorldFrame(expiryDeltaFrame(plan.delta))
      for (const webSocket of this.ctx.getWebSockets()) {
        webSocket.send(frame)
      }
    }
    await this.scheduleNextExpiryAlarm(plan.state)
  }

  private get sql(): RegionSqlStorage {
    return this.ctx.storage.sql as RegionSqlStorage
  }

  private migrateRegionStorage(): void {
    for (const statement of regionDurableObjectMigrationStatements) {
      this.sql.exec(statement)
    }
    for (const statement of regionDeltaReplayBufferMigrationStatements) {
      this.sql.exec(statement)
    }

    const existing = this.sql.exec<{ count: number }>(
      "SELECT COUNT(*) AS count FROM _sql_schema_migrations WHERE id = ?",
      1,
    ).one()

    if ((existing?.count ?? 0) === 0) {
      this.sql.exec(
        "INSERT INTO _sql_schema_migrations (id, name, applied_at) VALUES (?, ?, ?)",
        1,
        "region durable object bootstrap",
        new Date().toISOString(),
      )
    }

    this.sql.exec(
      `INSERT INTO region_transport_clock (
        region_ref,
        current_seq,
        min_replay_seq,
        updated_at
      ) VALUES (?, ?, ?, ?)
      ON CONFLICT(region_ref) DO NOTHING`,
      "region.run.tassadar.executor.20260615.street",
      0,
      0,
      new Date().toISOString(),
    )
  }

  private restoreHibernatedSessions(): void {
    for (const webSocket of this.ctx.getWebSockets()) {
      const attachment = this.readAttachment(webSocket)
      this.upsertSession(attachment, new Date().toISOString())
    }
  }

  private readAttachment(webSocket: WebSocket): RegionSocketSessionAttachment {
    const attachment = webSocket.deserializeAttachment() as RegionSocketSessionAttachment | undefined
    const fallback = makeInitialSessionAttachment({
      regionRef: "region.unknown",
      connectedAt: new Date().toISOString(),
    })
    return {
      ...fallback,
      ...(attachment ?? {}),
      seenRefs: [...(attachment?.seenRefs ?? fallback.seenRefs)],
      interestTierByRef: { ...(attachment?.interestTierByRef ?? fallback.interestTierByRef) },
    }
  }

  private upsertSession(
    attachment: RegionSocketSessionAttachment,
    lastSeenAt: string,
  ): void {
    this.sql.exec(
      `INSERT INTO region_socket_sessions (
        session_ref,
        region_ref,
        actor_ref,
        actor_class,
        connected_at,
        last_seen_at,
        metadata_json
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(session_ref) DO UPDATE SET
        last_seen_at = excluded.last_seen_at,
        metadata_json = excluded.metadata_json`,
      attachment.sessionRef,
      attachment.regionRef,
      attachment.actorRef,
      attachment.actorClass,
      attachment.connectedAt,
      lastSeenAt,
      JSON.stringify(attachment),
    )
  }

  /**
   * Resolve the reconnect plan, preferring TRUE gap replay from the bounded
   * delta-replay buffer when the cursor is within the buffered window. Falls
   * back to the existing heartbeat / fresh-snapshot reconnect plan when the
   * feature flag is off, the buffer is unavailable, or the cursor is older than
   * the buffered window (evicted). Fail-safe by construction: any uncertainty
   * degrades to today's snapshot behavior.
   */
  private resolveReconnectPlan(
    regionRef: string,
    reconnectCursor: string | null,
    clock: { currentSeq: number; minReplaySeq: number },
    connectedAt: string,
  ): ReconnectPlan {
    if (!this.replayConfig.enabled) {
      return makeReconnectPlan(regionRef, reconnectCursor, clock, connectedAt)
    }

    const gap = planReplayBufferReconnect({
      regionRef,
      reconnectCursor,
      currentSeq: clock.currentSeq,
      bufferedRows: this.readReplayBufferRows(regionRef),
      generatedAt: connectedAt,
      makeHeartbeatFrame: (cursor, generatedAt) => makeHeartbeatFrame(regionRef, cursor, generatedAt),
    })

    if (gap.kind === "gap_replay") {
      // TRUE gap replay: the exact buffered delta suffix, no snapshot.
      return { kind: "resume", cursor: gap.cursor, frames: gap.frames }
    }
    if (gap.kind === "at_tail") {
      return { kind: "resume", cursor: gap.cursor, frames: gap.frames }
    }

    // snapshot_fallback (evicted / empty / no cursor): defer to the existing
    // reconnect planner so stale-cursor diagnostics + the fresh snapshot are
    // produced exactly as before.
    return makeReconnectPlan(regionRef, reconnectCursor, clock, connectedAt)
  }

  private readReplayBufferRows(regionRef: string): ReadonlyArray<DeltaReplayBufferRow> {
    const rows = this.sql.exec<{
      sequence: number
      byte_len: number
      delta_json: string
      generated_at: string
    }>(
      "SELECT sequence, byte_len, delta_json, generated_at FROM region_delta_replay_buffer WHERE region_ref = ? ORDER BY sequence ASC",
      regionRef,
    ).toArray()
    return rows.map(row => ({
      regionRef,
      sequence: Number(row.sequence),
      byteLen: Number(row.byte_len),
      deltaJson: row.delta_json,
      generatedAt: row.generated_at,
    }))
  }

  /**
   * Append a broadcast delta into the bounded replay buffer and evict the oldest
   * rows so it respects both the count cap and the byte cap. Bounded growth: the
   * retained set is always a contiguous suffix ending at the live sequence.
   */
  private appendReplayBufferDelta(delta: WorldDelta, sequence: number, generatedAt: string): void {
    if (!this.replayConfig.enabled) {
      return
    }
    const regionRef = delta.regionRef
    const row = makeReplayBufferRow({ regionRef, sequence, delta, generatedAt })
    const existing = this.readReplayBufferRows(regionRef).filter(r => r.sequence !== row.sequence)
    const plan = replayBufferEvictionPlan(existing, row, this.replayConfig)

    this.sql.exec(
      `INSERT INTO region_delta_replay_buffer (
        region_ref,
        sequence,
        byte_len,
        delta_json,
        generated_at
      ) VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(region_ref, sequence) DO UPDATE SET
        byte_len = excluded.byte_len,
        delta_json = excluded.delta_json,
        generated_at = excluded.generated_at`,
      regionRef,
      row.sequence,
      row.byteLen,
      row.deltaJson,
      row.generatedAt,
    )
    for (const evicted of plan.evictedSequences) {
      this.sql.exec(
        "DELETE FROM region_delta_replay_buffer WHERE region_ref = ? AND sequence = ?",
        regionRef,
        evicted,
      )
    }
  }

  private readRegionClock(regionRef: string): { currentSeq: number; minReplaySeq: number } {
    const clock = regionClockFromStorageRows(
      this.sql.exec<{ current_seq: number; min_replay_seq: number }>(
        "SELECT current_seq, min_replay_seq FROM region_transport_clock WHERE region_ref = ?",
        regionRef,
      ).toArray(),
    )

    if (clock !== null) {
      return clock
    }

    this.sql.exec(
      `INSERT INTO region_transport_clock (
        region_ref,
        current_seq,
        min_replay_seq,
        updated_at
      ) VALUES (?, ?, ?, ?)`,
      regionRef,
      0,
      0,
      new Date().toISOString(),
    )
    return { currentSeq: 0, minReplaySeq: 0 }
  }

  private writeRegionClock(
    regionRef: string,
    currentSeq: number,
    minReplaySeq: number,
    updatedAt: string,
  ): void {
    this.sql.exec(
      `INSERT INTO region_transport_clock (
        region_ref,
        current_seq,
        min_replay_seq,
        updated_at
      ) VALUES (?, ?, ?, ?)
      ON CONFLICT(region_ref) DO UPDATE SET
        current_seq = excluded.current_seq,
        min_replay_seq = excluded.min_replay_seq,
        updated_at = excluded.updated_at`,
      regionRef,
      currentSeq,
      minReplaySeq,
      updatedAt,
    )
  }

  private ensureHotState(regionRef: string): void {
    if (this.hotState === null || this.hotState.regionRef !== regionRef) {
      const clock = this.readRegionClock(regionRef)
      this.hotState = {
        ...makeEmptyHotState(regionRef),
        sequence: clock.currentSeq,
        minReplaySeq: clock.minReplaySeq,
        expiringRefs: this.readExpiryRefs(regionRef),
      }
    }
  }

  private readExpiryRefs(regionRef: string): Readonly<Record<string, WorldExpiringRef>> {
    const rows = this.sql.exec<{ ref: string; metadata_json: string }>(
      "SELECT ref, metadata_json FROM region_hot_expiry_refs WHERE region_ref = ?",
      regionRef,
    ).toArray()
    return Object.fromEntries(rows.map(row => [row.ref, JSON.parse(row.metadata_json) as WorldExpiringRef]))
  }

  private persistExpiryRefs(state: WorldHotState): void {
    this.sql.exec("DELETE FROM region_hot_expiry_refs WHERE region_ref = ?", state.regionRef)
    for (const expiry of Object.values(state.expiringRefs)) {
      this.sql.exec(
        `INSERT INTO region_hot_expiry_refs (
          ref,
          region_ref,
          kind,
          expires_at,
          metadata_json
        ) VALUES (?, ?, ?, ?, ?)`,
        expiry.ref,
        state.regionRef,
        expiry.kind,
        expiry.expiresAt,
        JSON.stringify(expiry),
      )
    }
  }

  private async scheduleNextExpiryAlarm(state: WorldHotState): Promise<void> {
    const alarmAt = nextExpiryAlarmAt(state)
    if (alarmAt === null) {
      await this.ctx.storage.deleteAlarm()
      return
    }
    await this.ctx.storage.setAlarm(encodeAlarmTimestamp(alarmAt))
  }
}

export default {
  fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    return runWorkerEffect(request, env, ctx, handleWorkerRequest())
  },
  async scheduled(_controller: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    await handlePublicActivityTimelineBridgePoll(
      {
        env,
        regionObjects: env.REGION_DURABLE_OBJECT,
        db: env.WORLD_DB,
        ...(env.WORLD_BRIDGE_QUEUE === undefined ? {} : { bridgeQueue: env.WORLD_BRIDGE_QUEUE }),
      },
      configFromEnv(env),
      { waitUntil: promise => ctx.waitUntil(promise) },
    )
  },
} satisfies ExportedHandler<Env>
