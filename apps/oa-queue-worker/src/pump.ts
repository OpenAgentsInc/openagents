/**
 * CFG-7 (#8522): the pump core — lease/deliver/ack/nack per topic against
 * the oa-infra JobQueue service. Pure over the JobQueue interface so tests
 * run it against the in-memory backend; production provides the Postgres
 * FOR UPDATE SKIP LOCKED backend.
 *
 * Delivery contract (mirrors workers/api src/oa-queue-delivery-routes.ts):
 *   POST {deliveryUrl}/api/internal/queue/deliver
 *   Authorization: Bearer {token}
 *   { topic, jobId, attempts, payload }
 *   2xx -> ack (job removed); anything else -> nack (retry after
 *   retryDelayMs; the producer's max_attempts dead-letters exhausted jobs).
 */
import { Effect } from 'effect'
import {
  JobQueue,
  type JobQueueBackendError,
  type LeasedJob,
} from '@openagentsinc/oa-infra/job-queue'

import type { TopicConfig } from './topics.ts'

export const OA_QUEUE_DELIVER_PATH = '/api/internal/queue/deliver'

export type DeliveryResult = Readonly<{
  ok: boolean
  status: number
  reason?: string
}>

export type DeliverFn = (job: LeasedJob) => Promise<DeliveryResult>

export type FetchLike = (
  url: string,
  init: Readonly<{
    body: string
    headers: Record<string, string>
    method: string
  }>,
) => Promise<Readonly<{ ok: boolean; status: number; text: () => Promise<string> }>>

const boundedReason = (raw: string): string =>
  raw.replaceAll(/\s+/g, ' ').slice(0, 300)

export const makeHttpDeliver = (options: {
  readonly baseUrl: string
  readonly token: string
  readonly fetchImpl?: FetchLike
}): DeliverFn => {
  const fetchImpl = options.fetchImpl ?? (fetch as unknown as FetchLike)
  const url = `${options.baseUrl.replace(/\/$/, '')}${OA_QUEUE_DELIVER_PATH}`

  return async job => {
    const response = await fetchImpl(url, {
      body: JSON.stringify({
        attempts: job.attempts,
        jobId: job.id,
        payload: job.payload,
        topic: job.topic,
      }),
      headers: {
        authorization: `Bearer ${options.token}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    })

    if (response.ok) {
      return { ok: true, status: response.status }
    }

    const body = await response.text().catch(() => '')
    return {
      ok: false,
      reason: boundedReason(`http_${response.status}: ${body}`),
      status: response.status,
    }
  }
}

export type PumpLog = (
  event: string,
  fields: Record<string, string | number>,
) => void

export type DrainOutcome = Readonly<{
  acked: number
  leased: number
  nacked: number
}>

/**
 * One drain cycle for one topic: lease up to `batch` due jobs, deliver each,
 * ack on success, nack on failure. Delivery failures NEVER fail the cycle —
 * a broken app route must not crash the pump (jobs retry and eventually
 * dead-letter via max_attempts). ack/nack `JobNotFoundError` (lease lapsed
 * and got re-leased elsewhere) is logged and swallowed.
 */
export const drainTopicOnce = (
  config: TopicConfig,
  deliver: DeliverFn,
  log: PumpLog = () => {},
): Effect.Effect<DrainOutcome, JobQueueBackendError, JobQueue> =>
  Effect.gen(function* () {
    const queue = yield* JobQueue
    const jobs = yield* queue.lease(config.topic, {
      batch: config.batch,
      visibilityMs: config.visibilityMs,
    })

    let acked = 0
    let nacked = 0

    for (const job of jobs) {
      if (config.delivery === 'ack-local') {
        yield* queue.ack(job.id).pipe(
          Effect.catchTag('JobNotFoundError', () => Effect.void),
        )
        acked += 1
        log('oa_queue_smoke_acked', { attempts: job.attempts, jobId: job.id })
        continue
      }

      const result = yield* Effect.promise(() =>
        deliver(job).catch(
          (error: unknown): DeliveryResult => ({
            ok: false,
            reason: boundedReason(
              error instanceof Error ? error.message : String(error),
            ),
            status: 0,
          }),
        ),
      )

      if (result.ok) {
        yield* queue.ack(job.id).pipe(
          Effect.catchTag('JobNotFoundError', () =>
            Effect.sync(() => {
              log('oa_queue_ack_lost_lease', { jobId: job.id })
            }),
          ),
        )
        acked += 1
        continue
      }

      log('oa_queue_delivery_failed', {
        attempts: job.attempts,
        jobId: job.id,
        reason: result.reason ?? '',
        status: result.status,
        topic: job.topic,
      })
      yield* queue
        .nack(job.id, {
          ...(result.reason === undefined ? {} : { error: result.reason }),
          retryDelayMs: config.retryDelayMs,
        })
        .pipe(
          Effect.catchTag('JobNotFoundError', () =>
            Effect.sync(() => {
              log('oa_queue_nack_lost_lease', { jobId: job.id })
            }),
          ),
        )
      nacked += 1
    }

    return { acked, leased: jobs.length, nacked }
  })

/**
 * One full pump cycle across every configured topic. A backend error on one
 * topic is logged and does not abort the other topics (the Promise.allSettled
 * lesson from the Worker cron, #8409).
 */
export const drainAllTopicsOnce = (
  topics: ReadonlyArray<TopicConfig>,
  deliver: DeliverFn,
  log: PumpLog = () => {},
): Effect.Effect<number, never, JobQueue> =>
  Effect.gen(function* () {
    let processed = 0
    for (const config of topics) {
      const outcome = yield* drainTopicOnce(config, deliver, log).pipe(
        Effect.catchTag('JobQueueBackendError', error =>
          Effect.sync((): DrainOutcome => {
            log('oa_queue_backend_error', {
              operation: error.operation,
              topic: config.topic,
            })
            return { acked: 0, leased: 0, nacked: 0 }
          }),
        ),
      )
      processed += outcome.leased
    }
    return processed
  })
