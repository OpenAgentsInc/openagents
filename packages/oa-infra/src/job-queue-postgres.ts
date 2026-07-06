/**
 * Postgres JobQueue backend (migrations/0002_oa_infra_job_queue.sql).
 *
 * Leasing is one atomic statement: candidate rows are picked with
 * `FOR UPDATE SKIP LOCKED` (concurrent lessees never block or double-claim)
 * and flipped to `leased` in the same CTE. Exhausted jobs whose lease lapsed
 * are dead-lettered lazily at lease time — no background reaper required.
 */
import { Effect, Layer } from "effect"
import {
  DEFAULT_MAX_ATTEMPTS,
  DEFAULT_VISIBILITY_MS,
  JobNotFoundError,
  JobQueue,
  JobQueueBackendError,
  type DeadLetterJob,
  type JobQueueShape,
  type LeasedJob,
} from "./job-queue.ts"
import { OaInfraSql } from "./sql.ts"
import type { SQL } from "bun"

const BACKEND = "postgres"

const tryPg = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new JobQueueBackendError({ backend: BACKEND, operation, cause }),
  })

export const makePostgresJobQueue = (sql: SQL): JobQueueShape => {
  const enqueue = (
    topic: string,
    payload: string,
    options?: { readonly delayMs?: number; readonly maxAttempts?: number },
  ) =>
    tryPg("enqueue", async () => {
      const delayMs = options?.delayMs ?? 0
      const maxAttempts = options?.maxAttempts ?? DEFAULT_MAX_ATTEMPTS
      const rows: Array<{ id: string }> = await sql`
        INSERT INTO oa_infra_jobs (topic, payload, max_attempts, run_at)
        VALUES (
          ${topic},
          ${payload},
          ${maxAttempts},
          now() + make_interval(secs => ${delayMs}::bigint / 1000.0)
        )
        RETURNING id
      `
      const row = rows[0]
      if (row === undefined) throw new Error("INSERT ... RETURNING produced no row")
      return row.id
    })

  const lease = (
    topic: string,
    options?: { readonly batch?: number; readonly visibilityMs?: number },
  ) =>
    tryPg("lease", async () => {
      const batch = options?.batch ?? 1
      const visibilityMs = options?.visibilityMs ?? DEFAULT_VISIBILITY_MS
      // Lazily dead-letter jobs whose lease lapsed with no attempts left.
      await sql`
        UPDATE oa_infra_jobs
        SET status = 'dead',
            dead_at = now(),
            lease_expires_at = NULL,
            last_error = COALESCE(last_error, 'lease expired with no attempts left')
        WHERE topic = ${topic}
          AND status = 'leased'
          AND lease_expires_at <= now()
          AND attempts >= max_attempts
      `
      const rows: Array<{ id: string; topic: string; payload: string; attempts: number }> =
        await sql`
          WITH candidates AS (
            SELECT id
            FROM oa_infra_jobs
            WHERE topic = ${topic}
              AND (
                (status = 'pending' AND run_at <= now())
                OR (status = 'leased' AND lease_expires_at <= now())
              )
            ORDER BY run_at, created_at
            LIMIT ${batch}
            FOR UPDATE SKIP LOCKED
          )
          UPDATE oa_infra_jobs job
          SET status = 'leased',
              attempts = job.attempts + 1,
              lease_expires_at = now() + make_interval(secs => ${visibilityMs}::bigint / 1000.0)
          FROM candidates
          WHERE job.id = candidates.id
          RETURNING job.id, job.topic, job.payload, job.attempts
        `
      return rows.map(
        (row): LeasedJob => ({
          id: row.id,
          topic: row.topic,
          payload: row.payload,
          attempts: row.attempts,
        }),
      )
    })

  const ack = (jobId: string) =>
    tryPg("ack", async () => {
      const rows: Array<{ id: string }> = await sql`
        DELETE FROM oa_infra_jobs
        WHERE id = ${jobId} AND status = 'leased'
        RETURNING id
      `
      return rows.length > 0
    }).pipe(
      Effect.flatMap((acked) =>
        acked ? Effect.void : Effect.fail(new JobNotFoundError({ jobId, operation: "ack" })),
      ),
    )

  const nack = (
    jobId: string,
    options?: { readonly retryDelayMs?: number; readonly error?: string },
  ) =>
    tryPg("nack", async () => {
      const retryDelayMs = options?.retryDelayMs ?? 0
      const lastError = options?.error ?? null
      const rows: Array<{ id: string }> = await sql`
        UPDATE oa_infra_jobs
        SET status = CASE WHEN attempts >= max_attempts THEN 'dead' ELSE 'pending' END,
            dead_at = CASE WHEN attempts >= max_attempts THEN now() ELSE NULL END,
            run_at = now() + make_interval(secs => ${retryDelayMs}::bigint / 1000.0),
            lease_expires_at = NULL,
            last_error = COALESCE(${lastError}, last_error)
        WHERE id = ${jobId} AND status = 'leased'
        RETURNING id
      `
      return rows.length > 0
    }).pipe(
      Effect.flatMap((nacked) =>
        nacked ? Effect.void : Effect.fail(new JobNotFoundError({ jobId, operation: "nack" })),
      ),
    )

  const deadLetters = (topic: string, options?: { readonly limit?: number }) =>
    tryPg("deadLetters", async () => {
      const limit = options?.limit ?? 100
      const rows: Array<{
        id: string
        topic: string
        payload: string
        attempts: number
        last_error: string | null
      }> = await sql`
        SELECT id, topic, payload, attempts, last_error
        FROM oa_infra_jobs
        WHERE topic = ${topic} AND status = 'dead'
        ORDER BY dead_at, created_at
        LIMIT ${limit}
      `
      return rows.map(
        (row): DeadLetterJob => ({
          id: row.id,
          topic: row.topic,
          payload: row.payload,
          attempts: row.attempts,
          lastError: row.last_error,
        }),
      )
    })

  return { enqueue, lease, ack, nack, deadLetters }
}

/** Postgres JobQueue Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<JobQueue, never, OaInfraSql> = Layer.effect(
  JobQueue,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    return makePostgresJobQueue(sql)
  }),
)
