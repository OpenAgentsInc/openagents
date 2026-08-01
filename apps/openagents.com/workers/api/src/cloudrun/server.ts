import {
  PostgresSarahLiveKitRoomAuthorityStore,
  makeSarahRealtimeVoiceStore,
} from '@openagentsinc/khala-sync-server'
import { Runtime } from '@openagentsinc/runtime-platform'
import { createHash } from 'node:crypto'

/**
 * CFG-9 (#8524): the openagents.com monolith on Google Cloud Run.
 *
 * A Node HTTP entrypoint wrapping the existing application handler
 * handler (`src/index.ts`) with a process.env-backed Env (env.ts):
 *
 * - every HTTP route the application serves, same hostname-based issuer routing
 *   (openagents.com vs auth.openagents.com), same SPA asset fallback
 * - `POST /internal/cron` (bearer-protected) invokes the application's
 *   `scheduled()` task table — driven by Cloud Scheduler every minute
 * - `GET /internal/healthz` liveness probe
 * - queue delivery arrives over HTTP from the oa-queue-worker pump (CFG-7)
 * - `ctx.waitUntil` work is tracked and drained on SIGTERM
 *
 * Run with `node --import tsx ./src/cloudrun/server.ts`.
 */

import worker from '../index'
import { acquireSharedPostgresClient } from '../khala-sync-postgres-pool'
import { defaultMakeKhalaSyncSqlClient } from '../khala-sync-push-routes'
import {
  makeSarahLiveKitRoomBroker,
  parseSarahLiveKitRoomBrokerConfig,
} from '../sarah-livekit-room-broker'
import {
  SARAH_REALTIME_VOICE_SESSION_HEADER,
  SARAH_REALTIME_VOICE_TICKET_HEADER,
  parseSarahRealtimeVoiceRouteConfig,
  reconcileSarahLiveKitProvisioningIntents,
  reconcileSarahLiveKitTerminalRooms,
  sha256Hex,
} from '../sarah-realtime-voice-routes'
import { assertAssetsDirExists } from './assets'
import {
  isBindingUnavailableError,
  responseForBindingUnavailable,
} from './binding-unavailable'
import { buildCloudRunRuntime } from './env'
import { makeBackgroundTasks, makeExecutionContext } from './execution-context'
import { gateForgeDocumentRequest } from './forge-ui-gate'
// #8634/#8635 scope 5: retained /forum* serves the Effect Native conversion.
import { handleForumUiRequest } from './forum-ui'
import {
  cronAuthorized,
  withForwardedHost,
  withForwardedProto,
} from './http-utils'
// #8652 PORTAL-1: client portal mounts at openagents.com/portal (EN surface).
import { handlePortalUiRequest } from './portal-ui'
import { awaitPostgresReady } from './postgres-readiness'
import { isPublicSiteRootRequest } from './public-site-host'
import {
  type SarahRealtimeBridgeData,
  isSarahRealtimeVoiceUpgrade,
  makeSarahRealtimeBridgeData,
  makeSarahRealtimeWebSocketHandlers,
} from './sarah-realtime-bridge'
import { runSarahVoiceScheduledMaintenance } from './sarah-voice-maintenance'
import {
  assertStartUiArtifactsExist,
  handleStartUiRequest,
  isStartDocumentRequestPath,
  isStartServerRequestPath,
} from './start-ui'
import {
  type SyncBridgeData,
  isSyncConnectUpgrade,
  liveHubConnectTarget,
  makeSyncBridgeWebSocketHandlers,
  withoutUpgradeHeaders,
} from './sync-connect-bridge'

type CloudRunWebSocketData = SyncBridgeData | SarahRealtimeBridgeData

const log = (event: string, detail: Record<string, unknown> = {}): void => {
  console.log(
    JSON.stringify({
      event: `cloudrun.${event}`,
      ...detail,
      at: new Date().toISOString(),
    }),
  )
}

const main = async (): Promise<void> => {
  const runtime = buildCloudRunRuntime(process.env)
  assertAssetsDirExists(runtime.webDistDir)
  assertStartUiArtifactsExist()

  const tasks = makeBackgroundTasks((event, error) => {
    log(event, {
      error: error instanceof Error ? error.message : String(error),
    })
  })
  const ctx = makeExecutionContext(tasks)
  const cronToken = process.env['CLOUD_RUN_CRON_TOKEN']
  const port = Number(process.env['PORT'] ?? 8080)
  const trustForwardedHost =
    process.env['OPENAGENTS_TRUST_FORWARDED_HOST'] === '1'

  // Queue consumption is the separate apps/oa-queue-worker Cloud Run pump
  // (CFG-7): it leases oa_infra_jobs and POSTs to this app's
  // /api/internal/queue/deliver route — no in-process consumer here.

  const runSarahVoiceMaintenance = async (): Promise<void> => {
    const voiceConfig = parseSarahRealtimeVoiceRouteConfig(runtime.env)
    const connectionString = runtime.env.KHALA_SYNC_DB?.connectionString?.trim()
    if (
      voiceConfig === undefined ||
      connectionString === undefined ||
      connectionString === ''
    ) {
      return
    }
    const liveKitConfig = parseSarahLiveKitRoomBrokerConfig(runtime.env)
    const makeLifecycleDependencies =
      liveKitConfig === undefined
        ? undefined
        : () => ({
            broker: makeSarahLiveKitRoomBroker(liveKitConfig),
            creditMsatPerMillionTokens: voiceConfig.creditMsatPerMillionTokens,
            openStore: async () => {
              const opened =
                await defaultMakeKhalaSyncSqlClient(connectionString)
              return {
                store: makeSarahRealtimeVoiceStore(opened.sql),
                close: () => opened.end(),
              }
            },
          })
    await runSarahVoiceScheduledMaintenance(
      {
        sweepExpired: voiceConfig.enabled
          ? async () => {
              const client =
                await defaultMakeKhalaSyncSqlClient(connectionString)
              try {
                return await makeSarahRealtimeVoiceStore(
                  client.sql,
                ).sweepExpired(new Date().toISOString())
              } finally {
                await client.end()
              }
            }
          : undefined,
        escalateStuckAccountingHolds: voiceConfig.enabled
          ? async () => {
              const client =
                await defaultMakeKhalaSyncSqlClient(connectionString)
              try {
                return await makeSarahRealtimeVoiceStore(
                  client.sql,
                ).escalateStuckAccountingUncertainHolds({
                  nowIso: new Date().toISOString(),
                })
              } finally {
                await client.end()
              }
            }
          : undefined,
        // An escalated hold still needs exact provider reconciliation. Keep it
        // visible even after it no longer denies the owner's voice slot.
        reportStuckAccountingHolds: voiceConfig.enabled
          ? async () => {
              const client =
                await defaultMakeKhalaSyncSqlClient(connectionString)
              try {
                return await makeSarahRealtimeVoiceStore(
                  client.sql,
                ).readStuckAccountingUncertainHolds({
                  nowIso: new Date().toISOString(),
                })
              } finally {
                await client.end()
              }
            }
          : undefined,
        reconcileProvisioning:
          makeLifecycleDependencies === undefined
            ? undefined
            : () =>
                reconcileSarahLiveKitProvisioningIntents(
                  makeLifecycleDependencies(),
                  runtime.env,
                ),
        reconcileTerminalRooms:
          makeLifecycleDependencies === undefined
            ? undefined
            : () =>
                reconcileSarahLiveKitTerminalRooms(
                  makeLifecycleDependencies(),
                  runtime.env,
                ),
        // EP263-LK H5 (#9282): rooms that died before member retirement
        // existed have no close event left to fire, so the sweep is the only
        // path by which those rows can converge.
        retireStaleRoomMembers: voiceConfig.enabled
          ? async () => {
              const client =
                await defaultMakeKhalaSyncSqlClient(connectionString)
              try {
                return await new PostgresSarahLiveKitRoomAuthorityStore(
                  client.sql,
                ).retireExpiredRoomMembers({ now: new Date().toISOString() })
              } finally {
                await client.end()
              }
            }
          : undefined,
      },
      log,
    )
  }

  const runScheduled = async (source: string): Promise<void> => {
    const scheduledTime = Date.now()
    const controller = {
      cron: '* * * * *',
      noRetry: () => undefined,
      scheduledTime,
    } as ScheduledController
    log('scheduled_tick_start', { source })
    try {
      await runSarahVoiceMaintenance()
    } catch (error) {
      log('sarah_voice_scheduled_maintenance_failed', {
        error: error instanceof Error ? error.message : String(error),
      })
    }
    await worker.scheduled!(controller, runtime.env, ctx)
    log('scheduled_tick_done', {
      elapsedMs: Date.now() - scheduledTime,
      source,
    })
  }

  const liveHub = (() => {
    const baseUrl = process.env['KHALA_SYNC_LIVE_HUB_URL']?.trim()
    const token = process.env['KHALA_SYNC_LIVE_HUB_TOKEN']?.trim()
    return baseUrl !== undefined &&
      baseUrl.length > 0 &&
      token !== undefined &&
      token.length > 0
      ? { baseUrl, token }
      : undefined
  })()

  const syncWebSocketHandlers = makeSyncBridgeWebSocketHandlers()
  const sarahWebSocketHandlers = makeSarahRealtimeWebSocketHandlers()

  // Withhold traffic until Postgres actually answers on this instance.
  // Cloud Run treats "the port is open" as "ready", and the Cloud SQL Auth
  // Proxy sidecar needs 10-70s after a new revision starts before
  // `/cloudsql/<instance>/.s.PGSQL.5432` accepts a connection. Opening the
  // port first is what let a cold instance answer authenticated requests with
  // a 30-second 503 (incident 2026-07-31). See ./postgres-readiness.ts for the
  // budget and why the gate is fail-open.
  const readinessConnectionString =
    runtime.env.KHALA_SYNC_DB?.connectionString?.trim()
  if (
    readinessConnectionString !== undefined &&
    readinessConnectionString !== ''
  ) {
    const outcome = await awaitPostgresReady(
      async () => {
        // The SAME pool the auth path uses ('sync'), so a successful probe
        // also leaves a warm connection behind for the first real request.
        const { sql } = await acquireSharedPostgresClient<{
          unsafe: (
            text: string,
            params: ReadonlyArray<string>,
          ) => Promise<ReadonlyArray<Record<string, unknown>>>
          end: (options?: { timeout?: number }) => Promise<void>
        }>({
          connectionString: readinessConnectionString,
          options: { connect_timeout: 10, prepare: false },
          variant: 'sync',
        })
        await sql.unsafe('select 1', [])
      },
      {
        onAttemptFailed: info => {
          log('postgres_readiness_attempt_failed', {
            attempt: info.attempt,
            elapsedMs: info.elapsedMs,
            error:
              info.error instanceof Error
                ? info.error.message
                : String(info.error),
          })
        },
      },
    )
    log(
      outcome.ready
        ? 'postgres_readiness_ready'
        : 'postgres_readiness_budget_exhausted',
      {
        attempts: outcome.attempts,
        elapsedMs: outcome.elapsedMs,
        ready: outcome.ready,
        severity: outcome.ready ? 'INFO' : 'ERROR',
      },
    )
  }

  const server = Runtime.serve<CloudRunWebSocketData>({
    fetch: async (incoming, bunServer): Promise<Response | undefined> => {
      const request = withForwardedHost(
        withForwardedProto(incoming),
        trustForwardedHost,
      )
      const url = new URL(request.url)
      const isPublicSiteRoot = isPublicSiteRootRequest(url)

      if (url.pathname === '/internal/healthz') {
        return Response.json({ ok: true, service: 'openagents-monolith' })
      }

      // Start owns a deliberately tiny set of server endpoints in addition to
      // its documents. Dispatch them explicitly before any Worker API route so
      // production cannot silently turn a healthy Start handler into the
      // Worker's generic not_found response.
      if (
        !isStartDocumentRequestPath(url.pathname) &&
        isStartServerRequestPath(url.pathname)
      ) {
        const response = await handleStartUiRequest(
          request,
          runtime.env as unknown as Readonly<Record<string, unknown>>,
          ctx,
        )
        if (response === undefined) {
          return Response.json(
            { error: 'start_server_route_unavailable', path: url.pathname },
            { status: 500 },
          )
        }
        const headers = new Headers(response.headers)
        headers.set('x-openagents-route-owner', 'start')
        return new Response(response.body, {
          headers,
          status: response.status,
          statusText: response.statusText,
        })
      }

      // Sarah removed at owner direction 2026-07-10 (epic #8610; supersedes
      // the #8594 SM-5 path mount): the web surface AND every /sarah/api/*
      // route are gone (apps/sarah deleted). Explicit 404 tombstone so the
      // application's unknown-document 302-to-home never resurrects a /sarah
      // page and stale clients get a typed not_found instead of HTML.
      if (url.pathname === '/sarah' || url.pathname.startsWith('/sarah/')) {
        return Response.json(
          { error: 'not_found', path: url.pathname },
          { status: 404 },
        )
      }

      // The public root is apex-only. Its document now comes from the same
      // TanStack Start build as /astro and /download;
      // auth.openagents.com/ must continue into the auth handler below.
      if (isPublicSiteRoot) {
        const rootResponse = await handleStartUiRequest(
          request,
          runtime.env as unknown as Readonly<Record<string, unknown>>,
          ctx,
          true,
        )
        if (rootResponse !== undefined) return rootResponse
      }

      // #8652 PORTAL-1: client portal page + bundle at openagents.com/portal.
      // API authority stays with the Worker's /api/portal/* routes.
      if (url.pathname === '/portal' || url.pathname.startsWith('/portal/')) {
        const portalResponse = await handlePortalUiRequest(request)
        if (portalResponse !== undefined) {
          return portalResponse
        }
      }

      // #8634/#8635 scope 5: the four converted /forum* document routes serve
      // the Effect Native forum (shell + /forum/app.js) instead of the legacy
      // Foldkit SPA shell. Worker /api/forum* authority and unconverted
      // /forum/* paths fall straight through to the Worker below.
      if (url.pathname === '/forum' || url.pathname.startsWith('/forum/')) {
        const forumResponse = await handleForumUiRequest(request)
        if (forumResponse !== undefined) {
          return forumResponse
        }
      }

      // #8813: apps/start owns retained documents after the EN mounts above.
      // API/auth/unknown
      // paths continue into the application handler unchanged.
      const startResponse = await gateForgeDocumentRequest(
        request,
        async (documentRequest, tenantRef) => {
          const membershipUrl = new URL(
            '/api/forge/membership',
            documentRequest.url,
          )
          membershipUrl.searchParams.set('tenantRef', tenantRef)
          return worker.fetch!(
            new Request(membershipUrl, {
              headers: documentRequest.headers,
              method: 'GET',
            }),
            runtime.env,
            ctx,
          )
        },
        () =>
          handleStartUiRequest(
            request,
            runtime.env as unknown as Readonly<Record<string, unknown>>,
            ctx,
          ),
      )
      if (startResponse !== undefined) {
        return startResponse
      }

      if (isSarahRealtimeVoiceUpgrade(request)) {
        const routeConfig = parseSarahRealtimeVoiceRouteConfig(runtime.env)
        const apiKey = runtime.env.OPENAI_API_KEY?.trim()
        const connectionString =
          runtime.env.KHALA_SYNC_DB?.connectionString?.trim()
        const sessionRef = request.headers
          .get(SARAH_REALTIME_VOICE_SESSION_HEADER)
          ?.trim()
        const ticket = request.headers
          .get(SARAH_REALTIME_VOICE_TICKET_HEADER)
          ?.trim()
        if (
          routeConfig === undefined ||
          !routeConfig.enabled ||
          apiKey === undefined ||
          apiKey === '' ||
          connectionString === undefined ||
          connectionString === '' ||
          sessionRef === undefined ||
          ticket === undefined
        ) {
          return Response.json(
            { error: 'sarah_voice_upgrade_rejected' },
            { status: 401, headers: { 'cache-control': 'no-store' } },
          )
        }

        const client = await defaultMakeKhalaSyncSqlClient(connectionString)
        const store = makeSarahRealtimeVoiceStore(client.sql)
        try {
          const session = await store.connect({
            sessionRef,
            ticketDigest: await sha256Hex(ticket),
            nowIso: new Date().toISOString(),
          })
          const safetyIdentifier = createHash('sha256')
            .update(`openagents:user:${session.ownerUserId}`)
            .digest('hex')
          const liveKitConfig = parseSarahLiveKitRoomBrokerConfig(runtime.env)
          const liveKitBroker =
            liveKitConfig === undefined
              ? undefined
              : makeSarahLiveKitRoomBroker(liveKitConfig)
          const upgraded = bunServer.upgrade(incoming, {
            data: makeSarahRealtimeBridgeData({
              session,
              apiKey,
              safetyIdentifier,
              creditMsatPerMillionTokens:
                routeConfig.creditMsatPerMillionTokens,
              store,
              interruptLiveKit: liveKitBroker?.interrupt,
              closeStore: client.end,
              tasks,
            }),
          })
          if (upgraded) return undefined
          await store.settle({
            sessionRef,
            closeReason: 'upgrade_failed',
            nowIso: new Date().toISOString(),
          })
          await client.end()
          return Response.json(
            { error: 'sarah_voice_upgrade_failed' },
            { status: 500, headers: { 'cache-control': 'no-store' } },
          )
        } catch {
          await client.end()
          return Response.json(
            { error: 'sarah_voice_upgrade_rejected' },
            { status: 401, headers: { 'cache-control': 'no-store' } },
          )
        }
      }

      // CFG-5 LiveHub WS bridge: Bun fetch cannot carry a WebSocket upgrade,
      // so run the route's full pre-upgrade pipeline (upgrade headers
      // stripped) and bridge the socket only on its documented 426 success
      // sentinel — see sync-connect-bridge.ts.
      if (liveHub !== undefined && isSyncConnectUpgrade(request)) {
        const preflight = await worker.fetch!(
          withoutUpgradeHeaders(request),
          runtime.env,
          ctx,
        )
        if (preflight.status !== 426) {
          return preflight
        }
        await preflight.body?.cancel()
        const target = liveHubConnectTarget(request, liveHub)
        const upgraded = bunServer.upgrade(incoming, {
          data: {
            _tag: 'sync',
            bearer: target.bearer,
            clientClosed: false,
            pending: [],
            targetUrl: target.targetUrl,
            upstream: undefined,
          },
        })
        if (upgraded) {
          // Bun sends the 101 itself.
          return undefined
        }
        return Response.json(
          {
            code: 'internal',
            messageSafe:
              'Khala Sync live-tail upgrade failed unexpectedly; reconnect.',
            retryable: true,
          },
          { status: 500, headers: { 'cache-control': 'no-store' } },
        )
      }

      if (url.pathname === '/internal/cron') {
        if (!cronAuthorized(request, cronToken)) {
          return Response.json({ error: 'unauthorized' }, { status: 401 })
        }
        try {
          await runScheduled('cloud-scheduler')
          return Response.json({ ok: true })
        } catch (error) {
          log('scheduled_tick_failed', {
            error: error instanceof Error ? error.message : String(error),
          })
          return Response.json({ error: 'scheduled_failed' }, { status: 500 })
        }
      }

      try {
        return await worker.fetch!(request, runtime.env, ctx)
      } catch (error) {
        if (isBindingUnavailableError(error)) {
          return responseForBindingUnavailable(error)
        }
        log('fetch_unhandled_error', {
          error:
            error instanceof Error
              ? (error.stack ?? error.message)
              : String(error),
          path: url.pathname,
        })
        return Response.json(
          { error: 'internal_error' },
          { status: 500, headers: { 'cache-control': 'no-store' } },
        )
      }
    },
    hostname: '0.0.0.0',
    idleTimeout: 960,
    port,
    websocket: {
      idleTimeout: 960,
      open: ws =>
        ws.data._tag === 'sync'
          ? syncWebSocketHandlers.open(ws as never)
          : sarahWebSocketHandlers.open(ws as never),
      message: (ws, message) =>
        ws.data._tag === 'sync'
          ? syncWebSocketHandlers.message(ws as never, message)
          : sarahWebSocketHandlers.message(ws as never, message),
      close: (ws, code, reason) =>
        ws.data._tag === 'sync'
          ? syncWebSocketHandlers.close(ws as never, code, reason)
          : sarahWebSocketHandlers.close(ws as never, code, reason),
    },
  })

  // Optional in-process cron fallback (Cloud Scheduler is the primary driver).
  const internalCronMs = Number(process.env['INTERNAL_CRON_INTERVAL_MS'] ?? 0)
  const cronTimer =
    internalCronMs > 0
      ? setInterval(() => {
          runScheduled('internal-interval').catch(error => {
            log('scheduled_tick_failed', {
              error: error instanceof Error ? error.message : String(error),
            })
          })
        }, internalCronMs)
      : undefined

  log('listening', {
    port: server.port,
    webDist: runtime.webDistDir,
  })

  let shuttingDown = false
  const shutdown = async (signal: string): Promise<void> => {
    if (shuttingDown) return
    shuttingDown = true
    log('shutdown_start', { pendingBackgroundTasks: tasks.size(), signal })
    if (cronTimer !== undefined) clearInterval(cronTimer)
    await server.stop(true)
    await tasks.drain()
    await runtime.close()
    log('shutdown_done', {})
    process.exit(0)
  }

  process.on('SIGTERM', () => void shutdown('SIGTERM'))
  process.on('SIGINT', () => void shutdown('SIGINT'))
}

if (Runtime.isMain(import.meta.url)) {
  await main()
}
