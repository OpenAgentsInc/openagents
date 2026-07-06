/**
 * Postgres Mutex backend — advisory locks, no table required.
 *
 * Acquire reserves a dedicated connection from the pool, opens a
 * transaction, and takes `pg_advisory_xact_lock(hashtextextended(name, 0))`
 * (blocks until granted). Release commits the transaction — Postgres drops
 * xact-scoped advisory locks automatically at transaction end, so even a
 * crashed/killed session can never wedge the name. The reserved connection
 * is returned to the pool in all cases.
 */
import { Effect, Layer } from "effect"
import { Mutex, MutexBackendError, type MutexShape } from "./mutex.ts"
import { OaInfraSql } from "./sql.ts"
import type { SQL } from "bun"

const BACKEND = "postgres"

type Reserved = Awaited<ReturnType<SQL["reserve"]>>

export const makePostgresMutex = (sql: SQL): MutexShape => {
  const acquire = (name: string) =>
    Effect.tryPromise({
      try: async (): Promise<Reserved> => {
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

  const release = (reserved: Reserved) =>
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

/** Postgres Mutex Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<Mutex, never, OaInfraSql> = Layer.effect(
  Mutex,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    return makePostgresMutex(sql)
  }),
)
