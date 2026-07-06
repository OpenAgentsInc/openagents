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
 * - a Postgres queue-consumer loop drives the Worker's `queue()` handler
 * - `ctx.waitUntil` work is tracked and drained on SIGTERM
 *
 * Run with the cloudflare:workers stub preloaded:
 *   bun --preload ./src/cloudrun/preload.ts ./src/cloudrun/server.ts
 */

import worker from '../index'
import { assertAssetsDirExists } from './assets'
import {
  isBindingUnavailableError,
  responseForBindingUnavailable,
} from './binding-unavailable'
import { buildCloudRunRuntime } from './env'
import {
  makeBackgroundTasks,
  makeExecutionContext,
} from './execution-context'
import { cronAuthorized, withForwardedProto } from './http-utils'
import { runQueueConsumerLoop } from './queue-postgres'
import {
  type SyncBridgeData,
  isSyncConnectUpgrade,
  liveHubConnectTarget,
  makeSyncBridgeWebSocketHandlers,
  withoutUpgradeHeaders,
} from './sync-connect-bridge'

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

  // The Worker queue() consumer, driven from the Postgres JobQueue.
  const consumer =
    runtime.jobQueue === undefined
      ? undefined
      : runQueueConsumerLoop({
          ctx,
          env: runtime.env,
          handler: (batch, env, executionCtx) =>
            worker.queue!(batch, env as never, executionCtx),
          jobQueue: runtime.jobQueue,
          log: (event, detail) => log(event, detail),
        })

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

  const server = Bun.serve<SyncBridgeData, never>({
    fetch: async (incoming, bunServer): Promise<Response | undefined> => {
      const request = withForwardedProto(incoming)
      const url = new URL(request.url)

      if (url.pathname === '/internal/healthz') {
        return Response.json({ ok: true, service: 'openagents-monolith' })
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
    infraSql: runtime.infraSql !== undefined,
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
    await consumer?.stop()
    await tasks.drain()
    await runtime.close()
    log('shutdown_done', {})
    process.exit(0)
  }

  process.on('SIGTERM', () => void shutdown('SIGTERM'))
  process.on('SIGINT', () => void shutdown('SIGINT'))
}

if (import.meta.main) {
  await main()
}
