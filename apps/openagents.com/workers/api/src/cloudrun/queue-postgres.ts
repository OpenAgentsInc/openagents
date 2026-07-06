/**
 * CFG-9 (#8524): Cloudflare Queues replacement for the Cloud Run monolith,
 * bridging the Worker's `Queue` producer bindings and `queue()` consumer to
 * the owned Postgres JobQueue (CFG-2/CFG-7, oa-infra
 * migrations/0002_oa_infra_job_queue.sql, FOR UPDATE SKIP LOCKED).
 *
 * - Producers: each wrangler queue binding becomes `makePostgresQueue(sql,
 *   topic)` — `send`/`sendBatch` enqueue JSON payloads on that topic.
 * - Consumer: `runQueueConsumerLoop` leases due jobs per topic and drives
 *   the EXISTING worker `queue()` handler with a Workers-shaped
 *   `MessageBatch`, preserving ack/retry semantics (max_retries 3 in
 *   wrangler.jsonc → maxAttempts 4 deliveries).
 */

import { Effect } from 'effect'
import {
  DEFAULT_VISIBILITY_MS,
  type JobQueueShape,
  type LeasedJob,
} from '@openagentsinc/oa-infra/job-queue'

// NOTE: this module deliberately never imports the Postgres backend (which
// value-imports `bun`); callers construct the `JobQueueShape` (env.ts) so
// the delivery semantics here stay unit-testable under vitest/node.

/** wrangler.jsonc consumers use max_retries 3 → 4 total deliveries. */
export const QUEUE_MAX_ATTEMPTS = 4

export type QueueTopics = Readonly<{
  RUNNER_EVENTS: string
  ADJUTANT_ENRICHMENT_QUEUE: string
  EVENT_LEDGER_INGEST_QUEUE: string
  PYLON_CODEX_RAW_EVENT_METADATA_QUEUE: string
}>

export const QUEUE_TOPICS: QueueTopics = {
  ADJUTANT_ENRICHMENT_QUEUE: 'openagents-adjutant-enrichment-jobs',
  EVENT_LEDGER_INGEST_QUEUE: 'openagents-event-ledger-ingest',
  PYLON_CODEX_RAW_EVENT_METADATA_QUEUE:
    'openagents-pylon-codex-raw-event-metadata',
  RUNNER_EVENTS: 'openagents-autopilot-runner-events',
}

export const makePostgresQueue = (
  jobQueue: JobQueueShape,
  topic: string,
): Queue => {
  const enqueue = async (
    body: unknown,
    options?: { delaySeconds?: number },
  ): Promise<void> => {
    await Effect.runPromise(
      jobQueue.enqueue(topic, JSON.stringify(body ?? null), {
        delayMs: (options?.delaySeconds ?? 0) * 1000,
        maxAttempts: QUEUE_MAX_ATTEMPTS,
      }),
    )
  }

  const queue = {
    send: (body: unknown, options?: { delaySeconds?: number }) =>
      enqueue(body, options),
    sendBatch: async (
      messages: Iterable<{ body: unknown; delaySeconds?: number }>,
      options?: { delaySeconds?: number },
    ): Promise<void> => {
      for (const message of messages) {
        await enqueue(message.body, {
          delaySeconds: message.delaySeconds ?? options?.delaySeconds ?? 0,
        })
      }
    },
  }

  return queue as unknown as Queue
}

type QueueHandler = (
  batch: MessageBatch,
  env: unknown,
  ctx: ExecutionContext,
) => Promise<void>

type BatchOutcome = Readonly<{
  processed: number
  retried: number
}>

/**
 * Drive the worker `queue()` handler once for a leased batch. Messages the
 * handler acked are acked in Postgres; explicit retries and thrown batches
 * are nacked (which dead-letters once attempts are exhausted).
 */
export const deliverLeasedBatch = async (options: {
  ctx: ExecutionContext
  env: unknown
  handler: QueueHandler
  jobQueue: JobQueueShape
  jobs: ReadonlyArray<LeasedJob>
  log: (event: string, detail: Record<string, unknown>) => void
  topic: string
}): Promise<BatchOutcome> => {
  const { ctx, env, handler, jobQueue, jobs, log, topic } = options
  if (jobs.length === 0) return { processed: 0, retried: 0 }

  const acked = new Set<string>()
  const retried = new Map<string, number | undefined>()

  const messages = jobs.map(job => ({
    ack: () => {
      acked.add(job.id)
    },
    attempts: job.attempts,
    body: JSON.parse(job.payload) as unknown,
    id: job.id,
    retry: (retryOptions?: { delaySeconds?: number }) => {
      retried.set(job.id, retryOptions?.delaySeconds)
    },
    timestamp: new Date(),
  }))

  const batch = {
    ackAll: () => {
      for (const job of jobs) acked.add(job.id)
    },
    messages,
    queue: topic,
    retryAll: (retryOptions?: { delaySeconds?: number }) => {
      for (const job of jobs) retried.set(job.id, retryOptions?.delaySeconds)
    },
  } as unknown as MessageBatch

  let batchError: unknown
  try {
    await handler(batch, env, ctx)
  } catch (error) {
    batchError = error
    log('queue_batch_handler_failed', {
      error: error instanceof Error ? error.message : String(error),
      topic,
    })
  }

  let processed = 0
  let retriedCount = 0
  for (const job of jobs) {
    if (acked.has(job.id) && batchError === undefined) {
      await Effect.runPromise(
        jobQueue.ack(job.id).pipe(Effect.catch(() => Effect.succeed(undefined))),
      )
      processed += 1
      continue
    }
    const delaySeconds = retried.get(job.id)
    await Effect.runPromise(
      jobQueue
        .nack(job.id, {
          error:
            batchError === undefined
              ? 'retry_requested'
              : batchError instanceof Error
                ? batchError.message.slice(0, 500)
                : String(batchError).slice(0, 500),
          retryDelayMs: (delaySeconds ?? 10) * 1000,
        })
        .pipe(Effect.catch(() => Effect.succeed(undefined))),
    )
    retriedCount += 1
  }

  return { processed, retried: retriedCount }
}

export type QueueConsumerLoop = Readonly<{
  stop: () => Promise<void>
}>

/**
 * Background consumer loop: polls every `pollMs` (default 2s), leasing up to
 * the per-topic batch size and running the worker queue handler.
 */
export const runQueueConsumerLoop = (options: {
  ctx: ExecutionContext
  env: unknown
  handler: QueueHandler
  jobQueue: JobQueueShape
  log?: (event: string, detail: Record<string, unknown>) => void
  pollMs?: number
  topics?: ReadonlyArray<{ batch: number; topic: string }>
}): QueueConsumerLoop => {
  const log =
    options.log ??
    ((event, detail) => console.error(`[cloudrun] ${event}`, detail))
  const jobQueue = options.jobQueue
  const topics = options.topics ?? [
    { batch: 1, topic: QUEUE_TOPICS.ADJUTANT_ENRICHMENT_QUEUE },
    { batch: 1, topic: QUEUE_TOPICS.EVENT_LEDGER_INGEST_QUEUE },
    { batch: 25, topic: QUEUE_TOPICS.PYLON_CODEX_RAW_EVENT_METADATA_QUEUE },
  ]
  const pollMs = options.pollMs ?? 2000

  let stopped = false
  let wake: (() => void) | undefined

  const loop = (async () => {
    while (!stopped) {
      let sawWork = false
      for (const { batch, topic } of topics) {
        if (stopped) break
        try {
          const jobs = await Effect.runPromise(
            jobQueue.lease(topic, {
              batch,
              visibilityMs: DEFAULT_VISIBILITY_MS * 2,
            }),
          )
          if (jobs.length > 0) {
            sawWork = true
            await deliverLeasedBatch({
              ctx: options.ctx,
              env: options.env,
              handler: options.handler,
              jobQueue,
              jobs,
              log,
              topic,
            })
          }
        } catch (error) {
          log('queue_consumer_poll_failed', {
            error: error instanceof Error ? error.message : String(error),
            topic,
          })
        }
      }
      if (!sawWork && !stopped) {
        await new Promise<void>(resolve => {
          wake = resolve
          setTimeout(resolve, pollMs)
        })
        wake = undefined
      }
    }
  })()

  return {
    stop: async () => {
      stopped = true
      wake?.()
      await loop
    },
  }
}
