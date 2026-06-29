// The durable PULL queue store for out-of-Worker acceptance jobs (EPIC #6017).
//
// The dispatch (`acceptance-dispatch.ts`) enqueues a job for each khala-code completion
// with an executable artifact. The out-of-Worker runner cannot be a Cloudflare Queue
// consumer (a consumer is a Worker; chromium never runs in a Worker), so it PULLS work
// over an authenticated lease endpoint. This store is the pull queue those lease/ack
// routes operate on. It is Worker-safe (D1 only; no Playwright, no chromium).
//
// LEASE SEMANTICS (at-least-once, idempotent downstream):
//   - `enqueue(message)` inserts a `pending` row keyed by request id (ON CONFLICT keeps
//     the existing row so a duplicate dispatch never resets an in-flight lease).
//   - `lease(now, ttlMs)` atomically claims the oldest claimable job — `pending` OR a
//     `leased` row whose lease has EXPIRED (a crashed runner's job becomes re-leasable)
//     — stamping a fresh `leaseId` + expiry and bumping `attempts`. Returns null when
//     nothing is claimable (the idle case).
//   - `ack(leaseId, delivered)` removes the job when the runner delivered the verdict
//     (the verdict callback backfilled the receipt), or returns it to `pending` when the
//     delivery failed (re-leasable). An ack for a stale/unknown lease is a no-op.
// Downstream idempotency: the verdict callback's backfill is already idempotent, so an
// at-least-once re-lease (e.g. an ack lost after delivery) never double-writes a receipt.

import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from '../json-boundary'
import { isoTimestampAfterIso } from '../runtime-primitives'
import { AcceptanceJobMessage } from './acceptance-dispatch'

export type AcceptanceQueueStatus = 'pending' | 'leased'

export type LeasedAcceptanceJob = Readonly<{
  leaseId: string
  message: AcceptanceJobMessage
}>

export type AcceptanceJobQueueStore = Readonly<{
  // Insert a pending job. Idempotent per request id (a duplicate dispatch is ignored).
  enqueue: (message: AcceptanceJobMessage) => Effect.Effect<void>
  // Claim the next available job (pending or lease-expired). Null when none claimable.
  lease: (
    input: Readonly<{ nowIso: string; leaseTtlMs: number; newLeaseId: string }>,
  ) => Effect.Effect<LeasedAcceptanceJob | null>
  // Terminal ack: delivered => remove; retryable => return to pending. No-op for an
  // unknown/stale lease id.
  ack: (
    input: Readonly<{ leaseId: string; delivered: boolean; nowIso: string }>,
  ) => Effect.Effect<void>
}>

const decodeJobPayload = (text: string): AcceptanceJobMessage =>
  parseJsonWithSchema(AcceptanceJobMessage, text)

const encodeJobPayload = (message: AcceptanceJobMessage): string =>
  JSON.stringify(S.encodeSync(AcceptanceJobMessage)(message))

const makeDeterministicInMemoryClock = (): (() => string) => {
  let sequenceMs = 0

  return () => isoTimestampAfterIso('1970-01-01T00:00:00.000Z', sequenceMs++)
}

// An in-memory queue store: the reference implementation tests run against and the D1
// store mirrors. Pure + synchronous under the Effect wrapper. The `lease` claim is
// single-threaded here (Effect.sync), matching D1's single-statement atomic UPDATE.
export const makeInMemoryAcceptanceJobQueueStore =
  (
    nowIso: () => string = makeDeterministicInMemoryClock(),
  ): AcceptanceJobQueueStore => {
    type Row = {
      requestId: string
      status: AcceptanceQueueStatus
      payload: string
      leaseId: string | null
      leaseExpiresAt: string | null
      attempts: number
      createdAt: string
    }
    const rows = new Map<string, Row>()

    return {
      ack: ({ leaseId, delivered, nowIso }) =>
        Effect.sync(() => {
          for (const row of rows.values()) {
            if (row.leaseId === leaseId && row.status === 'leased') {
              if (delivered) {
                rows.delete(row.requestId)
              } else {
                row.status = 'pending'
                row.leaseId = null
                row.leaseExpiresAt = null
              }
              void nowIso
              return
            }
          }
        }),
      enqueue: message =>
        Effect.sync(() => {
          if (rows.has(message.requestId)) return
          rows.set(message.requestId, {
            attempts: 0,
            createdAt: nowIso(),
            leaseExpiresAt: null,
            leaseId: null,
            payload: encodeJobPayload(message),
            requestId: message.requestId,
            status: 'pending',
          })
        }),
      lease: ({ nowIso, leaseTtlMs, newLeaseId }) =>
        Effect.sync(() => {
          const now = Date.parse(nowIso)
          const claimable = [...rows.values()]
            .filter(
              row =>
                row.status === 'pending' ||
                (row.status === 'leased' &&
                  row.leaseExpiresAt !== null &&
                  Date.parse(row.leaseExpiresAt) <= now),
            )
            .sort((a, b) => a.createdAt.localeCompare(b.createdAt))
          const row = claimable[0]
          if (row === undefined) return null
          row.status = 'leased'
          row.leaseId = newLeaseId
          row.leaseExpiresAt = isoTimestampAfterIso(nowIso, leaseTtlMs)
          row.attempts += 1
          return { leaseId: newLeaseId, message: decodeJobPayload(row.payload) }
        }),
    }
  }

// A D1-backed queue store (prod). Mirrors the in-memory reference against the
// `khala_acceptance_jobs` table (migration 0222). The lease claim is a single atomic
// UPDATE ... WHERE request_id = (subselect oldest claimable) so two concurrent runner
// pulls cannot lease the same job.
export const makeD1AcceptanceJobQueueStore = (
  db: D1Database,
  nowIso: () => string,
): AcceptanceJobQueueStore => ({
  ack: ({ leaseId, delivered, nowIso: at }) =>
    Effect.tryPromise(() =>
      delivered
        ? db
            .prepare(
              `DELETE FROM khala_acceptance_jobs
                 WHERE lease_id = ? AND status = 'leased'`,
            )
            .bind(leaseId)
            .run()
        : db
            .prepare(
              `UPDATE khala_acceptance_jobs
                  SET status = 'pending', lease_id = NULL, lease_expires_at = NULL,
                      updated_at = ?
                WHERE lease_id = ? AND status = 'leased'`,
            )
            .bind(at, leaseId)
            .run(),
    ).pipe(Effect.asVoid, Effect.orDie),

  enqueue: message =>
    Effect.tryPromise(() =>
      db
        .prepare(
          `INSERT INTO khala_acceptance_jobs (
             request_id, status, job_payload, lease_id, lease_expires_at,
             attempts, created_at, updated_at
           ) VALUES (?, 'pending', ?, NULL, NULL, 0, ?, ?)
           ON CONFLICT(request_id) DO NOTHING`,
        )
        .bind(message.requestId, encodeJobPayload(message), nowIso(), nowIso())
        .run(),
    ).pipe(Effect.asVoid, Effect.orDie),

  lease: ({ nowIso: at, leaseTtlMs, newLeaseId }) =>
    Effect.tryPromise(async () => {
      const expiresAt = isoTimestampAfterIso(at, leaseTtlMs)
      // Atomic claim: stamp the oldest claimable row (pending OR lease-expired).
      await db
        .prepare(
          `UPDATE khala_acceptance_jobs
              SET status = 'leased', lease_id = ?, lease_expires_at = ?,
                  attempts = attempts + 1, updated_at = ?
            WHERE request_id = (
              SELECT request_id FROM khala_acceptance_jobs
               WHERE status = 'pending'
                  OR (status = 'leased' AND lease_expires_at IS NOT NULL
                      AND lease_expires_at <= ?)
               ORDER BY created_at ASC
               LIMIT 1
            )`,
        )
        .bind(newLeaseId, expiresAt, at, at)
        .run()
      // Read back the row we just claimed by our unique lease id.
      const row = await db
        .prepare(
          `SELECT job_payload FROM khala_acceptance_jobs
             WHERE lease_id = ? AND status = 'leased' LIMIT 1`,
        )
        .bind(newLeaseId)
        .first<{ job_payload: string }>()
      return row
    }).pipe(
      Effect.map(row =>
        row === null
          ? null
          : {
              leaseId: newLeaseId,
              message: decodeJobPayload(String(row.job_payload)),
            },
      ),
      Effect.orDie,
    ),
})
