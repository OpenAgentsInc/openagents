/**
 * JobQueue — owned queue primitive (CFG-2, issue #8517, audit §5).
 *
 * Semantics WE define (any backend must pass src/conformance/job-queue.ts):
 * - `enqueue(topic, payload)` makes the job leasable once its optional
 *   `delayMs` has elapsed.
 * - `lease(topic, { batch, visibilityMs })` atomically claims up to `batch`
 *   due jobs; a claimed job is INVISIBLE to other lessees until it is
 *   acked, nacked, or its visibility window lapses. `attempts` counts
 *   deliveries and is already incremented on the returned jobs.
 * - `ack(jobId)` completes (removes) a leased job. `nack(jobId)` returns it
 *   after `retryDelayMs`, or moves it to the dead-letter state when
 *   `attempts >= maxAttempts`. Both fail `JobNotFoundError` when the job is
 *   not currently leased (double-ack, expired lease that was re-leased, ...).
 * - A lapsed visibility window makes the job leasable again (the redelivery
 *   consumes an attempt); jobs that lapse with no attempts left go to the
 *   dead-letter state.
 * - `deadLetters(topic)` lists dead jobs for inspection/replay tooling.
 *
 * Backends: in-memory (job-queue-memory.ts) and Postgres FOR UPDATE SKIP
 * LOCKED (job-queue-postgres.ts, migrations/0002_oa_infra_job_queue.sql).
 * Swap targets per the audit: Pub/Sub, SQS, NATS.
 */
import { Context, Schema } from "effect"
import type { Effect } from "effect"

/** Unrecoverable backend failure (connection loss, vendor error, ...). */
export class JobQueueBackendError extends Schema.TaggedErrorClass<JobQueueBackendError>()(
  "JobQueueBackendError",
  {
    backend: Schema.String,
    operation: Schema.String,
    cause: Schema.Defect,
  },
) {}

/** ack/nack on a job that is not currently leased. */
export class JobNotFoundError extends Schema.TaggedErrorClass<JobNotFoundError>()(
  "JobNotFoundError",
  {
    jobId: Schema.String,
    operation: Schema.String,
  },
) {}

export interface EnqueueOptions {
  /** Delay before the job becomes leasable. Default 0. */
  readonly delayMs?: number
  /** Delivery attempts before dead-lettering. Default 5. */
  readonly maxAttempts?: number
}

export interface LeaseOptions {
  /** Max jobs claimed by this call. Default 1. */
  readonly batch?: number
  /** Invisibility window for claimed jobs, ms. Default 30_000. */
  readonly visibilityMs?: number
}

export interface NackOptions {
  /** Delay before the job becomes leasable again. Default 0. */
  readonly retryDelayMs?: number
  /** Recorded on the job for dead-letter forensics. */
  readonly error?: string
}

export interface LeasedJob {
  readonly id: string
  readonly topic: string
  readonly payload: string
  /** Delivery count INCLUDING this delivery (first lease = 1). */
  readonly attempts: number
}

export interface DeadLetterJob {
  readonly id: string
  readonly topic: string
  readonly payload: string
  readonly attempts: number
  readonly lastError: string | null
}

export interface JobQueueShape {
  /** Returns the new job id. */
  readonly enqueue: (
    topic: string,
    payload: string,
    options?: EnqueueOptions,
  ) => Effect.Effect<string, JobQueueBackendError>
  readonly lease: (
    topic: string,
    options?: LeaseOptions,
  ) => Effect.Effect<ReadonlyArray<LeasedJob>, JobQueueBackendError>
  readonly ack: (jobId: string) => Effect.Effect<void, JobQueueBackendError | JobNotFoundError>
  readonly nack: (
    jobId: string,
    options?: NackOptions,
  ) => Effect.Effect<void, JobQueueBackendError | JobNotFoundError>
  readonly deadLetters: (
    topic: string,
    options?: { readonly limit?: number },
  ) => Effect.Effect<ReadonlyArray<DeadLetterJob>, JobQueueBackendError>
}

export class JobQueue extends Context.Service<JobQueue, JobQueueShape>()(
  "@openagentsinc/oa-infra/JobQueue",
) {}

export const DEFAULT_MAX_ATTEMPTS = 5
export const DEFAULT_VISIBILITY_MS = 30_000
