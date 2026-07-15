/**
 * CFG-9 (#8524): the `Env` factory for the Cloud Run monolith.
 *
 * Builds an object satisfying the Worker's `OpenAgentsWorkerEnv`
 * (`WorkerBindings & OpenAgentsWorkerConfigEnv`) from `process.env` plus
 * owned backends:
 *
 * - vars + secrets: pass-through from process.env (Secret Manager mounts)
 * - OPENAGENTS_DB:   the Cloud SQL Postgres compatibility adapter. Cloud SQL is
 *                    required; there is no remote-database fallback.
 * - auth KV + OpenAuth issuer storage: CFG-3's KvStore over KHALA_SYNC_DB
 *   (auth/auth-kv.ts, auth/openauth-storage.ts) — config-driven, no shim
 * - KHALA_SYNC_DB:   DIRECT Cloud SQL connection string (kills Hyperdrive)
 * - queues: CFG-7's producer seam + oa-queue-worker pump (config-driven)
 * - durable inference streams + Khala Sync hub: config-driven Postgres /
 *   LiveHub seams landed by CFG-6/CFG-5 (KHALA_SYNC_DB,
 *   KHALA_SYNC_LIVE_HUB_URL/_TOKEN) — nothing to shim here
 * - browser automation: absent; email uses Resend; artifacts use GCS
 */
import path from 'node:path'

import type { OpenAgentsWorkerEnv } from '../bindings'
import { makeKhalaSyncWritesDatabase } from '../khala-sync-domain-writes-database'
import { makeAssetsFetcher } from './assets'

export type ProcessEnv = Readonly<Record<string, string | undefined>>

export type CloudRunRuntime = Readonly<{
  env: OpenAgentsWorkerEnv
  webDistDir: string
  close: () => Promise<void>
}>

const passthroughFetcher = (): Fetcher =>
  ({
    connect: () => {
      throw new Error(
        'Fetcher.connect is not supported on the Cloud Run monolith',
      )
    },
    fetch: (input: RequestInfo | URL, init?: RequestInit) =>
      fetch(input as Request, init),
  }) as unknown as Fetcher

export const buildCloudRunRuntime = (
  processEnv: ProcessEnv = process.env,
): CloudRunRuntime => {
  const webDistDir = path.resolve(
    processEnv['OPENAGENTS_WEB_DIST'] ??
      path.resolve(import.meta.dirname, '..', '..', '..', '..', 'apps/start/dist/client'),
  )

  const khalaSyncUrl = processEnv['KHALA_SYNC_DATABASE_URL']
  if (khalaSyncUrl === undefined || khalaSyncUrl.length === 0) {
    throw new Error('KHALA_SYNC_DATABASE_URL is required')
  }
  const khalaSyncBinding = { connectionString: khalaSyncUrl }
  const openAgentsDb = makeKhalaSyncWritesDatabase({
    KHALA_SYNC_DB: khalaSyncBinding,
  })
  if (openAgentsDb === undefined) {
    throw new Error('Cloud SQL database adapter initialization failed')
  }

  const bindings = {
    // ---- storage/data backends -------------------------------------------
    OPENAGENTS_DB: openAgentsDb,
    KHALA_SYNC_DB: khalaSyncBinding,

    // Queues: CFG-7 deleted the Queue bindings — producers enqueue via
    // makeOaJobEnqueueForEnv over KHALA_SYNC_DB, and the separate
    // apps/oa-queue-worker Cloud Run pump delivers leased jobs back to
    // /api/internal/queue/deliver on this app. Nothing to shim here.

    // ---- fetchers -----------------------------------------------------------
    ASSETS: makeAssetsFetcher(webDistDir) as unknown as Fetcher,
    MARKET_RELAY_SERVICE: passthroughFetcher(),
    // Browser automation is intentionally absent. Email uses Resend and
    // artifact storage is resolved from the GCS configuration above.
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
