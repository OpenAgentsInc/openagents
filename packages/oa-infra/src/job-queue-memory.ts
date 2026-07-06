/**
 * In-memory JobQueue backend — reference implementation of the JobQueue
 * contract (same lifecycle as the Postgres backend, single-process only).
 */
import { Effect, Layer } from "effect"
import {
  DEFAULT_MAX_ATTEMPTS,
  DEFAULT_VISIBILITY_MS,
  JobNotFoundError,
  JobQueue,
  type DeadLetterJob,
  type JobQueueShape,
  type LeasedJob,
} from "./job-queue.ts"

type JobStatus = "pending" | "leased" | "dead"

interface MemoryJob {
  readonly id: string
  readonly topic: string
  readonly payload: string
  status: JobStatus
  attempts: number
  readonly maxAttempts: number
  runAtMs: number
  leaseExpiresAtMs: number | null
  lastError: string | null
  readonly createdAtSeq: number
}

export const makeMemoryJobQueue = (): JobQueueShape => {
  const jobs = new Map<string, MemoryJob>()
  let seq = 0

  const enqueue = (
    topic: string,
    payload: string,
    options?: { readonly delayMs?: number; readonly maxAttempts?: number },
  ) =>
    Effect.sync(() => {
      const id = crypto.randomUUID()
      jobs.set(id, {
        id,
        topic,
        payload,
        status: "pending",
        attempts: 0,
        maxAttempts: options?.maxAttempts ?? DEFAULT_MAX_ATTEMPTS,
        runAtMs: Date.now() + (options?.delayMs ?? 0),
        leaseExpiresAtMs: null,
        lastError: null,
        createdAtSeq: seq++,
      })
      return id
    })

  const lease = (
    topic: string,
    options?: { readonly batch?: number; readonly visibilityMs?: number },
  ) =>
    Effect.sync(() => {
      const now = Date.now()
      const batch = options?.batch ?? 1
      const visibilityMs = options?.visibilityMs ?? DEFAULT_VISIBILITY_MS

      // Lapsed leases: dead-letter exhausted jobs, others become claimable.
      for (const job of jobs.values()) {
        if (
          job.topic === topic &&
          job.status === "leased" &&
          job.leaseExpiresAtMs !== null &&
          job.leaseExpiresAtMs <= now &&
          job.attempts >= job.maxAttempts
        ) {
          job.status = "dead"
          job.leaseExpiresAtMs = null
          job.lastError = job.lastError ?? "lease expired with no attempts left"
        }
      }

      const due = [...jobs.values()]
        .filter(
          (job) =>
            job.topic === topic &&
            ((job.status === "pending" && job.runAtMs <= now) ||
              (job.status === "leased" &&
                job.leaseExpiresAtMs !== null &&
                job.leaseExpiresAtMs <= now)),
        )
        .sort((a, b) => a.runAtMs - b.runAtMs || a.createdAtSeq - b.createdAtSeq)
        .slice(0, batch)

      const leased: Array<LeasedJob> = []
      for (const job of due) {
        job.status = "leased"
        job.attempts += 1
        job.leaseExpiresAtMs = now + visibilityMs
        leased.push({
          id: job.id,
          topic: job.topic,
          payload: job.payload,
          attempts: job.attempts,
        })
      }
      return leased as ReadonlyArray<LeasedJob>
    })

  const ack = (jobId: string) =>
    Effect.suspend(() => {
      const job = jobs.get(jobId)
      if (job === undefined || job.status !== "leased") {
        return Effect.fail(new JobNotFoundError({ jobId, operation: "ack" }))
      }
      jobs.delete(jobId)
      return Effect.void
    })

  const nack = (
    jobId: string,
    options?: { readonly retryDelayMs?: number; readonly error?: string },
  ) =>
    Effect.suspend(() => {
      const job = jobs.get(jobId)
      if (job === undefined || job.status !== "leased") {
        return Effect.fail(new JobNotFoundError({ jobId, operation: "nack" }))
      }
      job.lastError = options?.error ?? job.lastError
      job.leaseExpiresAtMs = null
      if (job.attempts >= job.maxAttempts) {
        job.status = "dead"
      } else {
        job.status = "pending"
        job.runAtMs = Date.now() + (options?.retryDelayMs ?? 0)
      }
      return Effect.void
    })

  const deadLetters = (topic: string, options?: { readonly limit?: number }) =>
    Effect.sync(() =>
      [...jobs.values()]
        .filter((job) => job.topic === topic && job.status === "dead")
        .sort((a, b) => a.createdAtSeq - b.createdAtSeq)
        .slice(0, options?.limit ?? 100)
        .map(
          (job): DeadLetterJob => ({
            id: job.id,
            topic: job.topic,
            payload: job.payload,
            attempts: job.attempts,
            lastError: job.lastError,
          }),
        ),
    )

  return { enqueue, lease, ack, nack, deadLetters }
}

export const layerMemory = (): Layer.Layer<JobQueue> =>
  Layer.sync(JobQueue, makeMemoryJobQueue)
