import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * CFG-9 (#8524): the openagents.com monolith on Google Cloud Run.
 *
 * A Bun HTTP entrypoint wrapping the EXISTING Worker `export default`
 * handler (`src/index.ts`) with a process.env-backed Env (env.ts):
 *
 * - every HTTP route the Worker serves, same hostname-based issuer routing
 *   (openagents.com vs auth.openagents.com), same SPA asset fallback
 * - `POST /internal/cron` (bearer-protected) invokes the Worker's
 *   `scheduled()` task table — driven by Cloud Scheduler every minute
 * - `GET /internal/healthz` liveness probe
 * - queue delivery arrives over HTTP from the oa-queue-worker pump (CFG-7)
 * - `ctx.waitUntil` work is tracked and drained on SIGTERM
 *
 * Run with the cloudflare:workers stub preloaded:
 *   bun --preload ./src/cloudrun/preload.ts ./src/cloudrun/server.ts
 */

import worker from '../index'
import { assertAssetsDirExists } from './assets'
import { holdingPageInterception } from './holding-page'
import {
  isBindingUnavailableError,
  responseForBindingUnavailable,
} from './binding-unavailable'
import { buildCloudRunRuntime } from './env'
import {
  makeBackgroundTasks,
  makeExecutionContext,
} from './execution-context'
import {
  cronAuthorized,
  withForwardedHost,
  withForwardedProto,
} from './http-utils'
import {
  type SyncBridgeData,
  isSyncConnectUpgrade,
  liveHubConnectTarget,
  makeSyncBridgeWebSocketHandlers,
  withoutUpgradeHeaders,
} from './sync-connect-bridge'
// #8652 PORTAL-1: client portal mounts at openagents.com/portal (EN surface).
import { handlePortalUiRequest } from './portal-ui'
// #8634/#8635 scope 5: retained /forum* serves the Effect Native conversion.
import { handleForumUiRequest } from './forum-ui'

const log = (event: string, detail: Record<string, unknown> = {}): void => {
  console.log(
    JSON.stringify({ event: `cloudrun.${event}`, ...detail, at: new Date().toISOString() }),
  )
}


const main = async (): Promise<void> => {
  const runtime = buildCloudRunRuntime(process.env)
  assertAssetsDirExists(runtime.webDistDir)

  const tasks = makeBackgroundTasks((event, error) => {
    log(event, { error: error instanceof Error ? error.message : String(error) })
  })
  const ctx = makeExecutionContext(tasks)
  const cronToken = process.env['CLOUD_RUN_CRON_TOKEN']
  const port = Number(process.env['PORT'] ?? 8080)
  const trustForwardedHost =
    process.env['OPENAGENTS_TRUST_FORWARDED_HOST'] === '1'

  // Queue consumption is the separate apps/oa-queue-worker Cloud Run pump
  // (CFG-7): it leases oa_infra_jobs and POSTs to this app's
  // /api/internal/queue/deliver route — no in-process consumer here.

  const runScheduled = async (source: string): Promise<void> => {
    const scheduledTime = Date.now()
    const controller = {
      cron: '* * * * *',
      noRetry: () => undefined,
      scheduledTime,
    } as ScheduledController
    log('scheduled_tick_start', { source })
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

  const server = Runtime.serve<SyncBridgeData, never>({
    fetch: async (incoming, bunServer): Promise<Response | undefined> => {
      const request = withForwardedHost(
        withForwardedProto(incoming),
        trustForwardedHost,
      )
      const url = new URL(request.url)

      if (url.pathname === '/internal/healthz') {
        return Response.json({ ok: true, service: 'openagents-monolith' })
      }

      // Sarah removed at owner direction 2026-07-10 (epic #8610; supersedes
      // the #8594 SM-5 path mount): the web surface AND every /sarah/api/*
      // route are gone (apps/sarah deleted). Explicit 404 tombstone so the
      // Worker's SPA unknown-document 302-to-home never resurrects a /sarah
      // page and stale clients get a typed not_found instead of HTML.
      if (url.pathname === '/sarah' || url.pathname.startsWith('/sarah/')) {
        return Response.json(
          { error: 'not_found', path: url.pathname },
          { status: 404 },
        )
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

      // Public-site homepage placeholder (owner request): openagents.com/www
      // show a "be right back" page at `/` ONLY. Every other route passes
      // straight through. Remove `holding-page.ts` + this block to restore the
      // real homepage too.
      const holding = holdingPageInterception(url)
      if (holding !== undefined) {
        return holding
      }

      // CFG-5 LiveHub WS bridge: Bun fetch cannot carry a WebSocket upgrade,
      // so run the worker route's full pre-upgrade pipeline (upgrade headers
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
          error: error instanceof Error ? (error.stack ?? error.message) : String(error),
          path: url.pathname,
        })
        return Response.json(
          { error: 'internal_error' },
          { status: 500, headers: { 'cache-control': 'no-store' } },
        )
      }
    },
    hostname: '0.0.0.0',
    idleTimeout: 240,
    port,
    websocket: makeSyncBridgeWebSocketHandlers(),
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
    server.stop()
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
