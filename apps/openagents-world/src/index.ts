import { DurableObject } from "cloudflare:workers"
import { Context, Effect, Layer } from "effect"

import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  decodeWorldBridgePayload,
  type WorldBridgePayload,
} from "@openagentsinc/world-contract"

import {
  OPENAGENTS_WORLD_WORKER_VERSION,
  bufferHandshakeFrame,
  configFromEnv,
  hydrateBufferedSession,
  json,
  makeDiagnostic,
  makeDiagnosticResponse,
  makeInitialSessionAttachment,
  makeZeroSnapshotDelta,
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
      return json({
        ok: true,
        schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        regionRef,
        socketUrl: socketUrlForRegion(request, regionRef),
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

export class RegionDurableObject extends DurableObject<Env> {
  private initialized: Promise<void>

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
    const actorRef = url.searchParams.get("actorRef")
    const actorClass = url.searchParams.get("actorClass")
    const attachment = makeInitialSessionAttachment({
      regionRef,
      ...(actorRef === null ? {} : { actorRef }),
      ...(actorClass === null ? {} : { actorClass: actorClass as RegionSocketSessionAttachment["actorClass"] }),
      connectedAt,
    })

    server.serializeAttachment(attachment)
    this.ctx.acceptWebSocket(server)
    this.upsertSession(attachment, connectedAt)
    server.send(serializeWorldFrame(makeZeroSnapshotDelta(regionRef, connectedAt)))

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
      webSocket.send(serializeWorldFrame(makeDiagnostic({
        tag: "auth",
        severity: "info",
        message: `World socket session hydrated; replayed ${buffered.attachment.bufferedFrames.length} buffered frame(s).`,
        observedAt: new Date().toISOString(),
        sourceRefs: [hydrated.sessionRef],
      })))
      return
    }

    this.upsertSession(attachment, new Date().toISOString())
    webSocket.send(serializeWorldFrame(makeDiagnostic({
      tag: "command",
      severity: "info",
      message: "World command intake scaffold received a frame; command application lands in the next cutover issue.",
      observedAt: new Date().toISOString(),
      sourceRefs: [attachment.sessionRef],
    })))
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
  }

  private restoreHibernatedSessions(): void {
    for (const webSocket of this.ctx.getWebSockets()) {
      const attachment = this.readAttachment(webSocket)
      this.upsertSession(attachment, new Date().toISOString())
    }
  }

  private readAttachment(webSocket: WebSocket): RegionSocketSessionAttachment {
    const attachment = webSocket.deserializeAttachment() as RegionSocketSessionAttachment | undefined
    return attachment ?? makeInitialSessionAttachment({
      regionRef: "region.unknown",
      connectedAt: new Date().toISOString(),
    })
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
}

export default {
  fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    return runWorkerEffect(request, env, ctx, handleWorkerRequest())
  },
}
