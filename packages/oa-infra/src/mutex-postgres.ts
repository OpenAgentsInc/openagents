/**
 * Postgres Mutex backend — driver-agnostic core (advisory locks, no table).
 *
 * Acquire reserves a dedicated connection from the pool, opens a
 * transaction, and takes `pg_advisory_xact_lock(hashtextextended(name, 0))`
 * (blocks until granted). Release commits the transaction — Postgres drops
 * xact-scoped advisory locks automatically at transaction end, so even a
 * crashed/killed session can never wedge the name. The reserved connection
 * is returned to the pool in all cases.
 *
 * WHY THIS FILE IMPORTS NO BUN TYPES (CFG-17, issue #8533): the Cloud Run
 * monolith / `openagents.com` Worker seam reaches Postgres through a
 * postgres.js client over the KHALA_SYNC_DB connection (the same
 * transaction-mode-safe discipline every other khala-sync store uses), not
 * Bun's built-in `SQL`. Bun's `SQL` and postgres.js (>=3.4) both expose the
 * same `reserve()` → reserved-connection surface, so the backend accepts
 * that structural slice (`MutexSqlClient`) and both drivers plug in. The Bun
 * `SQL` Layer stays in ./mutex-postgres-layer.ts; the postgres.js driver run
 * of the conformance suite lives in ./postgres-backends.test.ts (mirrors the
 * kv-store and durable-stream driver-seam split).
 */
import { Effect } from "effect"
import { MutexBackendError, type MutexShape } from "./mutex.ts"

const BACKEND = "postgres"

/**
 * A reserved single connection: a tagged-template query function plus
 * `release()` to return it to the pool. Satisfied by Bun's `ReservedSQL`
 * and by postgres.js's reserved connection (`await sql.reserve()`).
 */
export interface MutexReservedSql {
  (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>): PromiseLike<unknown>
  readonly release: () => void
}

/**
 * The structural client slice this backend needs — a `reserve()` that hands
 * back a dedicated connection. Satisfied by Bun's built-in `SQL` and by
 * postgres.js (`postgres(url, { prepare: false })`). Callers hand their
 * driver instance across this seam with a single deliberate cast (the same
 * convention as durable-stream's `DurableStreamSqlClient`); behavioral
 * equivalence is proven by running the conformance suite against both
 * drivers.
 */
export interface MutexSqlClient {
  readonly reserve: () => Promise<MutexReservedSql>
}

export const makePostgresMutex = (sql: MutexSqlClient): MutexShape => {
  const acquire = (name: string) =>
    Effect.tryPromise({
      try: async (): Promise<MutexReservedSql> => {
        const reserved = await sql.reserve()
        try {
          await reserved`BEGIN`
          await reserved`SELECT pg_advisory_xact_lock(hashtextextended(${name}, 0))`
          return reserved
        } catch (error) {
          try {
            await reserved`ROLLBACK`
          } catch {
            // connection-level failure; releasing below is all we can do
          }
          reserved.release()
          throw error
        }
      },
      catch: (cause) => new MutexBackendError({ backend: BACKEND, operation: "acquire", cause }),
    })

  const release = (reserved: MutexReservedSql) =>
    Effect.promise(async () => {
      try {
        await reserved`COMMIT`
      } catch {
        try {
          await reserved`ROLLBACK`
        } catch {
          // dropping the connection also drops the xact advisory lock
        }
      } finally {
        reserved.release()
      }
    })

  const withLock = <A, E, R>(name: string, effect: Effect.Effect<A, E, R>) =>
    Effect.acquireUseRelease(
      acquire(name),
      () => effect,
      (reserved) => release(reserved),
    )

  return { withLock }
}
