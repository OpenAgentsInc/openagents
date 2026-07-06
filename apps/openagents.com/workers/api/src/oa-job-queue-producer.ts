/**
 * CFG-7 (#8522): Postgres JobQueue producer seam for the Worker runtime.
 *
 * The four Cloudflare Queues are evacuated to the oa-infra Postgres JobQueue
 * (packages/oa-infra, `oa_infra_jobs`, FOR UPDATE SKIP LOCKED). oa-infra's
 * full backend uses Bun SQL and cannot run in workerd, so this seam is the
 * workerd-compatible PRODUCER half only: `enqueue` is a single INSERT that
 * matches oa-infra's `job-queue-postgres.ts` enqueue statement exactly, run
 * over the same transaction-mode-safe postgres.js client path the Worker
 * already uses for Khala Sync (`defaultMakeKhalaSyncSqlClient` on the
 * `KHALA_SYNC_DB` connection string).
 *
 * Consumers live in `apps/oa-queue-worker` (Cloud Run, Bun): it leases jobs
 * with the oa-infra Postgres JobQueue and delivers each one to the Worker's
 * internal delivery route (`/internal/queue/deliver`), where the original
 * queue-handler code still runs with its D1/Durable Object bindings.
 */
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

/**
 * Topics mirror the retired Cloudflare queue names 1:1 so operators can
 * correlate old dashboards, wrangler history, and the new `oa_infra_jobs`
 * rows without a mapping table.
 */
export const OA_JOB_TOPIC_ADJUTANT_ENRICHMENT =
  'openagents-adjutant-enrichment-jobs'
export const OA_JOB_TOPIC_EVENT_LEDGER_INGEST =
  'openagents-event-ledger-ingest'
export const OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA =
  'openagents-pylon-codex-raw-event-metadata'

export const OA_JOB_TOPICS = [
  OA_JOB_TOPIC_ADJUTANT_ENRICHMENT,
  OA_JOB_TOPIC_EVENT_LEDGER_INGEST,
  OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA,
] as const

export type OaJobTopic = (typeof OA_JOB_TOPICS)[number]

/**
 * Mirrors the retired wrangler consumers' `max_retries: 3`: first delivery
 * plus three redeliveries before the job dead-letters (oa-infra `attempts`
 * counts deliveries, so `max_attempts = 4`).
 */
export const OA_JOB_MAX_ATTEMPTS = 4

export type OaJobEnqueue = (
  topic: OaJobTopic,
  payload: string,
) => Promise<string>

export type OaJobQueueProducerEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

export type MakeOaJobEnqueueOptions = Readonly<{
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

/**
 * Single-INSERT enqueue into `oa_infra_jobs` (semantics identical to
 * oa-infra's Postgres backend: status defaults to `pending`, `run_at`
 * defaults to `now()`). Returns the new job id.
 *
 * `undefined` when the `KHALA_SYNC_DB` connection is not configured (local
 * dev / tests without Postgres) so call sites keep their existing
 * no-queue fallback behavior.
 */
export const makeOaJobEnqueueForEnv = (
  env: OaJobQueueProducerEnv,
  options: MakeOaJobEnqueueOptions = {},
): OaJobEnqueue | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString

  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient

  return async (topic, payload) => {
    const client = await makeSqlClient(connectionString)
    try {
      const rows = (await client.sql`
        INSERT INTO oa_infra_jobs (topic, payload, max_attempts)
        VALUES (${topic}, ${payload}, ${OA_JOB_MAX_ATTEMPTS})
        RETURNING id
      `) as unknown as ReadonlyArray<{ id: string }>
      const row = rows[0]
      if (row === undefined) {
        throw { error: 'oa_infra_jobs_insert_returned_no_row' }
      }
      return row.id
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the sync push route.
      }
    }
  }
}
