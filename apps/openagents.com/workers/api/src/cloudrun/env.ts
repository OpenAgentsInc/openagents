/**
 * CFG-9 (#8524): the `Env` factory for the Cloud Run monolith.
 *
 * Builds an object satisfying the Worker's `OpenAgentsWorkerEnv`
 * (`WorkerBindings & OpenAgentsWorkerConfigEnv`) from `process.env` plus
 * owned backends:
 *
 * - vars + secrets: pass-through from process.env (Secret Manager mounts)
 * - OPENAGENTS_DB:   D1-over-HTTP bridge (d1-http.ts) until CFG-4 finishes
 *                    the Postgres hard cutover; typed-503 when unconfigured
 * - AUTH_STORAGE:    Postgres `oa_infra_kv` KVNamespace adapter (CFG-3 table)
 * - KHALA_SYNC_DB:   DIRECT Cloud SQL connection string (kills Hyperdrive)
 * - queues:          Postgres JobQueue producers (CFG-7 seam)
 * - INFERENCE_DURABLE_STREAM: in-process durable-stream (CFG-6 replaces)
 * - KHALA_SYNC_HUB:  absent (typed absent-binding path) until CFG-5 LiveHub
 * - DO/containers with no replacement: typed-unavailable namespaces
 * - EMAIL/BROWSER/ARTIFACTS: absent — each has an existing absence degrade
 */

import { SQL } from 'bun'
import path from 'node:path'

import type { OpenAgentsWorkerEnv } from '../bindings'
import { makeAssetsFetcher } from './assets'
import { unavailableBinding } from './binding-unavailable'
import { d1FromProcessEnv } from './d1-http'
import {
  makeInMemoryDurableStreamNamespace,
  makeUnavailableDurableObjectNamespace,
} from './do-shims'
import { makePostgresKvNamespace } from './kv-postgres'
import { QUEUE_TOPICS, makePostgresQueue } from './queue-postgres'
import type { JobQueueShape } from '@openagentsinc/oa-infra/job-queue'
import { makePostgresJobQueue } from '@openagentsinc/oa-infra/job-queue-postgres'

export type ProcessEnv = Readonly<Record<string, string | undefined>>

export type CloudRunRuntime = Readonly<{
  env: OpenAgentsWorkerEnv
  /** Shared Bun SQL pool over OA_INFRA_DATABASE_URL (undefined when unset). */
  infraSql: SQL | undefined
  /** Postgres JobQueue over infraSql (undefined when unset). */
  jobQueue: JobQueueShape | undefined
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

  const infraDatabaseUrl = processEnv['OA_INFRA_DATABASE_URL']
  const infraSql =
    infraDatabaseUrl === undefined || infraDatabaseUrl.length === 0
      ? undefined
      : new SQL({
          max: Number(processEnv['OA_INFRA_DATABASE_POOL_MAX'] ?? 10),
          url: infraDatabaseUrl,
        })

  const jobQueue = infraSql === undefined ? undefined : makePostgresJobQueue(infraSql)

  const queueBinding = (bindingName: string, topic: string): Queue =>
    jobQueue === undefined
      ? unavailableBinding<Queue>(bindingName)
      : makePostgresQueue(jobQueue, topic)

  const khalaSyncUrl = processEnv['KHALA_SYNC_DATABASE_URL']

  const bindings = {
    // ---- storage/data backends -------------------------------------------
    OPENAGENTS_DB: d1FromProcessEnv(processEnv),
    AUTH_STORAGE:
      infraSql === undefined
        ? unavailableBinding<KVNamespace>('AUTH_STORAGE')
        : makePostgresKvNamespace(infraSql),
    ...(khalaSyncUrl === undefined || khalaSyncUrl.length === 0
      ? {}
      : { KHALA_SYNC_DB: { connectionString: khalaSyncUrl } }),

    // ---- queues (CFG-7 Postgres JobQueue) --------------------------------
    RUNNER_EVENTS: queueBinding('RUNNER_EVENTS', QUEUE_TOPICS.RUNNER_EVENTS),
    ADJUTANT_ENRICHMENT_QUEUE: queueBinding(
      'ADJUTANT_ENRICHMENT_QUEUE',
      QUEUE_TOPICS.ADJUTANT_ENRICHMENT_QUEUE,
    ),
    EVENT_LEDGER_INGEST_QUEUE: queueBinding(
      'EVENT_LEDGER_INGEST_QUEUE',
      QUEUE_TOPICS.EVENT_LEDGER_INGEST_QUEUE,
    ),
    PYLON_CODEX_RAW_EVENT_METADATA_QUEUE: queueBinding(
      'PYLON_CODEX_RAW_EVENT_METADATA_QUEUE',
      QUEUE_TOPICS.PYLON_CODEX_RAW_EVENT_METADATA_QUEUE,
    ),

    // ---- durable objects ---------------------------------------------------
    INFERENCE_DURABLE_STREAM:
      makeInMemoryDurableStreamNamespace() as unknown as DurableObjectNamespace,
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
    close: async () => {
      await infraSql?.end()
    },
    env,
    infraSql,
    jobQueue,
    webDistDir,
  }
}
