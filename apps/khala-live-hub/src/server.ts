// khala-live-hub server — Bun WS/HTTP surface (CFG-5, #8520).
//
// The owned Cloud Run service replacing the Worker's `KhalaSyncHubDO`. It
// serves the SAME per-scope contract on the same relative paths the DO
// served behind the internal Worker routes, so capture and the Worker (or
// the future Cloud Run monolith, CFG-9) only change the base URL:
//
//   GET  /health                          — liveness (no auth)
//   POST /append?scope=…                   — capture batch append
//   GET  /log?scope=…&cursor=…&limit=…     — window log pages
//   POST /access-changed  { scope }        — MustRefetch(access_changed)
//   GET  /connect?scope=…&cursor=…         — live-tail WebSocket upgrade
//
// AUTH: one shared bearer (`KHALA_LIVE_HUB_TOKEN`, Secret Manager) required
// on every route except /health — via `Authorization: Bearer …` or the
// `?token=` query parameter. The query fallback exists because WebSocket
// clients cannot always set upgrade headers (the same reality behind
// `withBearerFromQueryToken` on the Worker's public connect route, commit
// b45071b9b6 — PRESERVED end to end: the public route still promotes the
// CLIENT bearer for its own auth, then the proxy replaces it with THIS
// shared service bearer before forwarding here). The token never reaches
// logs, and the internal hub URL never carries client tokens.
//
// END-USER AUTH DOES NOT LIVE HERE: scope-read authorization (KS-7.1) runs
// in the route layer (`/api/sync/connect` / `/api/sync/log`) BEFORE the
// proxy forwards to this service — identical to the DO deployment shape.
//
// DEPLOY SHAPE (scripts/deploy-cloudrun.sh): exactly ONE instance
// (min=max=1) — the window and socket maps are in-memory per-scope state,
// so every scope's appends and sockets must land on the same instance.
// Session affinity + a high request timeout keep WebSockets stable; Cloud
// Run's 3600s request cap closes long tails eventually and clients resume
// from their durable cursor (reconnect-is-resume is a core protocol
// property, SPEC §6). Scaling past one instance is the sharding extension
// point documented in src/service.ts (`hubFor`).

import { createHash, timingSafeEqual } from "node:crypto"
import type { Server, ServerWebSocket } from "bun"
import { SQL } from "bun"
import { Schema as S } from "effect"

import { SyncScope } from "@openagentsinc/khala-sync"
import {
  DEFAULT_REBUILD_VERSIONS,
  loadNewestWindow,
} from "./rebuild.js"
import {
  liveHubJson,
  liveHubMethodNotAllowed,
  parseNonNegativeInt,
  type HubSocketLike,
} from "./scope-hub.js"
import { LiveHubService } from "./service.js"

const decodeScope = S.decodeUnknownSync(SyncScope)

export const DEFAULT_PING_INTERVAL_MS = 30_000

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export type LiveHubServerConfig = Readonly<{
  /** Shared service bearer; requests without it are 401. */
  token: string
  port?: number | undefined
  /** DIRECT Postgres URL for window rebuilds; absent disables rebuild. */
  databaseUrl?: string | undefined
  rebuildVersions?: number | undefined
  windowMaxEntries?: number | undefined
  windowMaxBytes?: number | undefined
  pingIntervalMs?: number | undefined
  log?: ((line: string) => void) | undefined
}>

const envInt = (raw: string | undefined): number | undefined => {
  if (raw === undefined || raw.trim() === "") return undefined
  const value = Number.parseInt(raw.trim(), 10)
  return Number.isSafeInteger(value) && value >= 1 ? value : undefined
}

/**
 * Build the server config from the environment. Throws with missing
 * variable NAMES only — never echoes values.
 */
export const liveHubConfigFromEnv = (
  env: Record<string, string | undefined> = process.env,
): LiveHubServerConfig => {
  const token = env["KHALA_LIVE_HUB_TOKEN"]
  if (token === undefined || token.trim() === "") {
    throw new Error(
      "khala-live-hub: missing environment variable KHALA_LIVE_HUB_TOKEN",
    )
  }
  return {
    token: token.trim(),
    port: envInt(env["PORT"]) ?? 8080,
    databaseUrl:
      env["KHALA_SYNC_DATABASE_URL"] === undefined ||
      env["KHALA_SYNC_DATABASE_URL"] === ""
        ? undefined
        : env["KHALA_SYNC_DATABASE_URL"],
    rebuildVersions: envInt(env["KHALA_LIVE_HUB_REBUILD_VERSIONS"]),
    windowMaxEntries: envInt(env["KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES"]),
    windowMaxBytes: envInt(env["KHALA_SYNC_HUB_WINDOW_MAX_BYTES"]),
    pingIntervalMs: envInt(env["KHALA_LIVE_HUB_PING_INTERVAL_MS"]),
    log: (line) => console.log(line),
  }
}

// ---------------------------------------------------------------------------
// Auth (constant-time compare on digests; never log tokens)
// ---------------------------------------------------------------------------

const sha256 = (value: string): Buffer =>
  createHash("sha256").update(value, "utf8").digest()

export const bearerFromRequest = (request: Request): string | undefined => {
  const header = request.headers.get("authorization")
  if (header !== null) {
    const match = /^Bearer\s+(.+)$/i.exec(header.trim())
    if (match !== null && match[1]!.trim() !== "") return match[1]!.trim()
    return undefined
  }
  const token = new URL(request.url).searchParams.get("token")?.trim()
  return token === undefined || token === "" ? undefined : token
}

const authorized = (request: Request, expectedDigest: Buffer): boolean => {
  const presented = bearerFromRequest(request)
  if (presented === undefined) return false
  return timingSafeEqual(sha256(presented), expectedDigest)
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

type SocketData = {
  scope: SyncScope
  cursor: number
  adapter?: HubSocketLike
  closed?: boolean
}

export type LiveHubServer = Readonly<{
  server: Server<SocketData>
  service: LiveHubService
  port: number
  stop: () => Promise<void>
}>

export const startLiveHubServer = (
  config: LiveHubServerConfig,
): LiveHubServer => {
  if (config.token.trim() === "") {
    throw new Error("khala-live-hub: token must not be empty")
  }
  const tokenDigest = sha256(config.token.trim())
  const log = config.log ?? (() => {})

  const sql =
    config.databaseUrl === undefined
      ? undefined
      : new SQL({ url: config.databaseUrl, max: 3 })
  const rebuildVersions = config.rebuildVersions ?? DEFAULT_REBUILD_VERSIONS

  const service = new LiveHubService({
    bounds: {
      maxEntries: config.windowMaxEntries,
      maxBytes: config.windowMaxBytes,
    },
    loadWindow:
      sql === undefined
        ? undefined
        : (scope) => loadNewestWindow(sql, scope, rebuildVersions),
    log,
  })

  const server = Bun.serve<SocketData>({
    port: config.port ?? 8080,
    // Generous socket idle timeout (seconds); the ping interval below keeps
    // healthy sockets far under it.
    idleTimeout: 240,
    async fetch(request, srv) {
      const url = new URL(request.url)

      if (url.pathname === "/health") {
        return liveHubJson({
          ok: true,
          scopes: service.scopeCount(),
          sockets: service.socketCount(),
        })
      }

      if (!authorized(request, tokenDigest)) {
        return liveHubJson({ error: "unauthorized" }, { status: 401 })
      }

      if (url.pathname === "/append") {
        if (request.method !== "POST") return liveHubMethodNotAllowed(["POST"])
        const body = (await request.json().catch(() => undefined)) as unknown
        const scopeRaw = (body as Record<string, unknown> | undefined)?.[
          "scope"
        ]
        let scope: SyncScope
        try {
          scope = decodeScope(scopeRaw)
        } catch {
          return liveHubJson(
            { error: "khala_sync_hub_append_invalid", reason: "missing scope" },
            { status: 400 },
          )
        }
        const hub = await service.hubFor(scope)
        return hub.append(body)
      }

      if (url.pathname === "/log") {
        if (request.method !== "GET") return liveHubMethodNotAllowed(["GET"])
        let scope: SyncScope
        try {
          scope = decodeScope(url.searchParams.get("scope"))
        } catch {
          return liveHubJson(
            { error: "khala_sync_hub_log_invalid", reason: "invalid scope" },
            { status: 400 },
          )
        }
        const hub = await service.hubFor(scope)
        return hub.log(url.searchParams)
      }

      if (url.pathname === "/access-changed") {
        if (request.method !== "POST") return liveHubMethodNotAllowed(["POST"])
        let scopeRaw: string | null = url.searchParams.get("scope")
        if (scopeRaw === null) {
          const body = (await request.json().catch(() => undefined)) as
            | Record<string, unknown>
            | undefined
          scopeRaw = typeof body?.["scope"] === "string" ? body["scope"] : null
        }
        let scope: SyncScope
        try {
          scope = decodeScope(scopeRaw)
        } catch {
          return liveHubJson(
            {
              error: "khala_sync_hub_access_changed_invalid",
              reason: "invalid scope",
            },
            { status: 400 },
          )
        }
        const hub = await service.hubFor(scope)
        const notified = hub.accessChanged()
        return liveHubJson({ notified, ok: true, scope })
      }

      if (url.pathname === "/connect") {
        if (request.method !== "GET") return liveHubMethodNotAllowed(["GET"])
        if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") {
          return liveHubJson(
            { error: "khala_sync_hub_upgrade_required" },
            { status: 426 },
          )
        }
        let scope: SyncScope
        try {
          scope = decodeScope(url.searchParams.get("scope"))
        } catch {
          return liveHubJson(
            {
              error: "khala_sync_hub_connect_invalid",
              reason: "invalid scope",
            },
            { status: 400 },
          )
        }
        const cursor = parseNonNegativeInt(
          url.searchParams.get("cursor") ?? "0",
        )
        if (cursor === undefined) {
          return liveHubJson(
            {
              error: "khala_sync_hub_connect_invalid",
              reason: "invalid cursor",
            },
            { status: 400 },
          )
        }
        // Rebuild BEFORE the upgrade so attach catches up from a warm
        // window (single-flight per scope).
        await service.hubFor(scope)
        const upgraded = srv.upgrade(request, {
          data: { scope, cursor } satisfies SocketData,
        })
        if (upgraded) return undefined as unknown as Response
        return liveHubJson(
          { error: "khala_sync_hub_websocket_unavailable" },
          { status: 500 },
        )
      }

      return liveHubJson({ error: "not_found" }, { status: 404 })
    },
    websocket: {
      // Delta channel: server→client frames are small JSON texts.
      sendPings: true,
      async open(ws: ServerWebSocket<SocketData>) {
        const adapter: HubSocketLike = {
          send: (message) => {
            if (ws.send(message) === -1) {
              throw new Error("khala_live_hub_socket_backpressure_closed")
            }
          },
          close: (code, reason) => {
            // Flush-then-close: a send immediately followed by close can
            // drop the final frame through the Cloud Run HTTP proxy
            // (observed: MustRefetchFrame lost, client saw 1006). Losing
            // it is safe (revocation/refetch is re-enforced at reconnect
            // by the route layer) but delivering it is prompt — give the
            // socket one macrotask to flush before the close frame.
            setTimeout(() => {
              try {
                ws.close(code, reason)
              } catch {
                // already closed
              }
            }, 50)
          },
        }
        ws.data.adapter = adapter
        const hub = await service.hubFor(ws.data.scope)
        if (ws.data.closed === true) return
        hub.attachSocket(adapter, ws.data.cursor)
      },
      async message(ws: ServerWebSocket<SocketData>, message) {
        const adapter = ws.data.adapter
        if (adapter === undefined) return
        const hub = await service.hubFor(ws.data.scope)
        hub.onSocketMessage(
          adapter,
          typeof message === "string" ? message : Buffer.from(message),
        )
      },
      async close(ws: ServerWebSocket<SocketData>) {
        ws.data.closed = true
        const adapter = ws.data.adapter
        if (adapter === undefined) return
        const hub = await service.hubFor(ws.data.scope)
        hub.detachSocket(adapter)
      },
    },
  })

  const pingTimer = setInterval(
    () => service.pingAll(),
    config.pingIntervalMs ?? DEFAULT_PING_INTERVAL_MS,
  )

  const stop = async (): Promise<void> => {
    clearInterval(pingTimer)
    service.dispose()
    // Bun quirk (observed on 1.3.11): `server.stop(true)`'s promise can
    // fail to settle while a client-initiated WebSocket close handshake is
    // still in flight. Every server-side socket was force-closed by
    // dispose() above, so bound the wait — shutdown must always complete.
    await Promise.race([
      server.stop(true),
      new Promise<void>((resolve) => setTimeout(resolve, 1_000)),
    ])
    if (sql !== undefined) await sql.end().catch(() => {})
  }

  log(
    `khala-live-hub listening on :${server.port} ` +
      `(rebuild ${sql === undefined ? "disabled" : `enabled, ${rebuildVersions} versions`})`,
  )

  return { server, service, port: server.port ?? config.port ?? 8080, stop }
}

if (import.meta.main) {
  const running = startLiveHubServer(liveHubConfigFromEnv())
  const shutdown = () => {
    running
      .stop()
      .catch(() => {})
      .finally(() => process.exit(0))
  }
  process.on("SIGTERM", shutdown)
  process.on("SIGINT", shutdown)
}
