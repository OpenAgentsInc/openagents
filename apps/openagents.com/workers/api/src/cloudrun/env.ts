/**
 * CFG-9 (#8524): the `Env` factory for the Cloud Run monolith.
 *
 * Builds an object satisfying the Worker's `OpenAgentsWorkerEnv`
 * (`WorkerBindings & OpenAgentsWorkerConfigEnv`) from `process.env` plus
 * owned backends:
 *
 * - vars + secrets: pass-through from process.env (Secret Manager mounts)
 * - OPENAGENTS_DB:   the Cloud SQL Postgres D1-adapter (CFG D1 evacuation,
 *                    #8515). When `KHALA_SYNC_DATABASE_URL` is configured this
 *                    is `makeKhalaSyncWritesDatabase` — the same Postgres
 *                    `makePostgresD1Database` adapter the per-domain WRITES
 *                    cutovers already run on — so every remaining
 *                    `openAgentsDatabase(env)` code path serves off Cloud SQL
 *                    Postgres instead of the 401-dead Cloudflare D1 bridge.
 *                    Already-cut domains short-circuit their own D1 leg in
 *                    `postgres` mode, so this repoint does NOT double-write
 *                    them; the credits money path (pay_ins/pay_in_legs/
 *                    agent_balances) and the token_usage_events ledger run on
 *                    their own separate Postgres handles and are untouched.
 *                    Falls back to the d1-http bridge (typed-503 when
 *                    unconfigured) only when no `KHALA_SYNC_DATABASE_URL`.
 * - auth KV + OpenAuth issuer storage: CFG-3's KvStore over KHALA_SYNC_DB
 *   (auth/auth-kv.ts, auth/openauth-storage.ts) — config-driven, no shim
 * - KHALA_SYNC_DB:   DIRECT Cloud SQL connection string (kills Hyperdrive)
 * - queues: CFG-7's producer seam + oa-queue-worker pump (config-driven)
 * - durable inference streams + Khala Sync hub: config-driven Postgres /
 *   LiveHub seams landed by CFG-6/CFG-5 (KHALA_SYNC_DB,
 *   KHALA_SYNC_LIVE_HUB_URL/_TOKEN) — nothing to shim here
 * - DO/containers with no replacement: typed-unavailable namespaces
 * - EMAIL/BROWSER/ARTIFACTS: absent — each has an existing absence degrade
 */

import path from 'node:path'

import type { OpenAgentsWorkerEnv } from '../bindings'
import { makeKhalaSyncWritesDatabase } from '../khala-sync-domain-writes-database'
import { makeAssetsFetcher } from './assets'
import { d1FromProcessEnv } from './d1-http'
import { makeUnavailableDurableObjectNamespace } from './do-shims'

export type ProcessEnv = Readonly<Record<string, string | undefined>>

export type CloudRunRuntime = Readonly<{
  env: OpenAgentsWorkerEnv
  webDistDir: string
  close: () => Promise<void>
}>

const passthroughFetcher = (): Fetcher =>
  ({
    connect: () => {
      throw new Error('Fetcher.connect is not supported on the Cloud Run monolith')
    },
    fetch: (input: RequestInfo | URL, init?: RequestInit) =>
      fetch(input as Request, init),
  }) as unknown as Fetcher

export const buildCloudRunRuntime = (
  processEnv: ProcessEnv = process.env,
): CloudRunRuntime => {
  const webDistDir = path.resolve(
    processEnv['OPENAGENTS_WEB_DIST'] ??
      path.resolve(import.meta.dir, '..', '..', '..', '..', 'apps/web/dist'),
  )

  const khalaSyncUrl = processEnv['KHALA_SYNC_DATABASE_URL']
  const khalaSyncBinding =
    khalaSyncUrl === undefined || khalaSyncUrl.length === 0
      ? undefined
      : { connectionString: khalaSyncUrl }

  // CFG D1 evacuation (#8515): OPENAGENTS_DB now serves off Cloud SQL Postgres
  // through the same D1-shaped adapter the per-domain WRITES cutovers use, so
  // every remaining `openAgentsDatabase(env)` path stops hitting the 401-dead
  // Cloudflare D1 bridge. When `KHALA_SYNC_DATABASE_URL` is absent (tests /
  // unconfigured) this falls back to the d1-http bridge, which itself degrades
  // to the typed `BindingUnavailableError` 503 proxy.
  const openAgentsDb =
    makeKhalaSyncWritesDatabase({ KHALA_SYNC_DB: khalaSyncBinding }) ??
    d1FromProcessEnv(processEnv)

  const bindings = {
    // ---- storage/data backends -------------------------------------------
    OPENAGENTS_DB: openAgentsDb,
    ...(khalaSyncBinding === undefined
      ? {}
      : { KHALA_SYNC_DB: khalaSyncBinding }),

    // Queues: CFG-7 deleted the Queue bindings — producers enqueue via
    // makeOaJobEnqueueForEnv over KHALA_SYNC_DB, and the separate
    // apps/oa-queue-worker Cloud Run pump delivers leased jobs back to
    // /api/internal/queue/deliver on this app. Nothing to shim here.

    // ---- durable objects ---------------------------------------------------
    // Durable inference streams are config-driven Postgres now (CFG-6,
    // durableInferenceStreamNamespaceForEnv over KHALA_SYNC_DB) — no DO
    // namespace binding to shim. Khala Sync hub traffic is config-driven
    // LiveHub (CFG-5, resolveKhalaSyncHubNamespace over
    // KHALA_SYNC_LIVE_HUB_URL/_TOKEN); the /connect WS leg is bridged by
    // server.ts because Bun fetch cannot carry a WebSocket upgrade.
    SYNC_ROOM: makeUnavailableDurableObjectNamespace('SYNC_ROOM'),
    MDK_SIDECAR: makeUnavailableDurableObjectNamespace('MDK_SIDECAR'),
    AGENT_DEFINITION_SCHEDULER: makeUnavailableDurableObjectNamespace(
      'AGENT_DEFINITION_SCHEDULER',
    ),
    EVENT_LEDGER_OWNER:
      makeUnavailableDurableObjectNamespace('EVENT_LEDGER_OWNER'),
    // KHALA_SYNC_HUB intentionally ABSENT until the CFG-5 LiveHub service
    // adapter lands — the connect/log routes have a typed absent-binding path.
    // MDK_TREASURY / MDK_TIPS_BUFFER intentionally ABSENT — their fetch
    // helpers treat absence as "not configured" (money-path migration is a
    // separate later lane, catalogued on #8524).

    // ---- fetchers -----------------------------------------------------------
    ASSETS: makeAssetsFetcher(webDistDir) as unknown as Fetcher,
    MARKET_RELAY_SERVICE: passthroughFetcher(),
    // EMAIL / BROWSER intentionally ABSENT (existing absence degrades).
  }

  const env = {
    ...processEnv,
    ...bindings,
  } as unknown as OpenAgentsWorkerEnv

  return {
    close: async () => undefined,
    env,
    webDistDir,
  }
}
