import { DurableObject } from "cloudflare:workers"
import { Context, Effect, Layer } from "effect"

import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  decodeWorldBridgePayload,
  type WorldBridgePayload,
  type WorldRow,
  type WorldSubscriptionPlan,
} from "@openagentsinc/world-contract"

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
  OPENAGENTS_WORLD_WORKER_VERSION,
  bufferHandshakeFrame,
  configFromEnv,
  hydrateBufferedSession,
  json,
  makeDiagnostic,
  makeDiagnosticFrame,
  makeDiagnosticResponse,
  makeInitialSessionAttachment,
  makeReconnectPlan,
  makeSnapshotFrame,
  normalizeRegionRef,
  regionDurableObjectMigrationStatements,
  regionRefFromSocketPath,
  serializeWorldFrame,
  socketUrlForRegion,
  stableWorldRef,
  type RegionSocketSessionAttachment,
  type WorldBridgeQueueMessage,
  type WorldRuntimeConfig,
} from "./protocol"
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
    return decodeWorldBridgePayload(await request.json())
  } catch {
    return null
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
  const ingestRef = stableWorldRef("bridge_ingest.world", `${config.bridgeSource}:${acceptedAt}`)

  if (bindings.bridgeQueue !== undefined) {
    waitUntil.waitUntil(bindings.bridgeQueue.send({
      kind: "bridge_ingest_requested",
      ingestRef,
      sourceRef: payload?.sourceRef ?? config.bridgeSource,
      acceptedAt,
    }))
  }

  return json(
    {
      ok: true,
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      ingestRef,
      acceptedAt,
      rowCount: payload?.rows.length ?? 0,
      diagnostic: makeDiagnostic({
        tag: "bridge",
        severity: "info",
        message: "World bridge ingest accepted by the Effect/Cloudflare scaffold.",
        observedAt: acceptedAt,
        sourceRefs: [payload?.sourceRef ?? config.bridgeSource],
      }),
    },
    { status: 202 },
  )
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

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env)
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
    const reconnectPlan = makeReconnectPlan(regionRef, url.searchParams.get("cursor"), clock, connectedAt)
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
      server.send(serializeWorldFrame(frame))
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
      webSocket.serializeAttachment(hydrated)
      this.upsertSession(hydrated, new Date().toISOString())
      webSocket.send(serializeWorldFrame(makeSnapshotFrame(
        hydrated.regionRef,
        new Date().toISOString(),
        [],
        hydrated.cursor,
      )))
      webSocket.send(serializeWorldFrame(makeDiagnostic({
        tag: "auth",
        severity: "info",
        message: `World socket session hydrated; replayed ${buffered.attachment.bufferedFrames.length} buffered frame(s).`,
        observedAt: new Date().toISOString(),
        sourceRefs: [hydrated.sessionRef],
      })))
      return
    }

    const observedAt = new Date().toISOString()
    this.ensureHotState(attachment.regionRef)
    const hotState = this.hotState ?? makeEmptyHotState(attachment.regionRef)
    try {
      const result = await Effect.runPromise(applyWorldCommand(hotState, JSON.parse(frame) as unknown, observedAt))
      this.hotState = result.state
      this.writeRegionClock(result.state.regionRef, result.state.sequence, result.state.minReplaySeq, observedAt)
      this.persistExpiryRefs(result.state)
      await this.scheduleNextExpiryAlarm(result.state)
      const nextAttachment = this.applySubscriptionStateToAttachment({
        attachment,
        rows: result.delta.rows ?? [],
        deletedRefs: result.delta.deletedRefs ?? [],
        cursor: result.delta.cursor,
      })
      webSocket.serializeAttachment(nextAttachment)
      this.upsertSession(nextAttachment, observedAt)
      webSocket.send(serializeWorldFrame(commandDeltaFrame(result.delta)))
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

  private readRegionClock(regionRef: string): { currentSeq: number; minReplaySeq: number } {
    const clock = this.sql.exec<{ current_seq: number; min_replay_seq: number }>(
      "SELECT current_seq, min_replay_seq FROM region_transport_clock WHERE region_ref = ?",
      regionRef,
    ).one()

    if (clock !== null) {
      return {
        currentSeq: Number(clock.current_seq),
        minReplaySeq: Number(clock.min_replay_seq),
      }
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
}
