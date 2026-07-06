/**
 * Mutex — owned named-serialization primitive (CFG-2, issue #8517, audit §5).
 *
 * Semantics WE define (any backend must pass src/conformance/mutex.ts):
 * - `withLock(name, effect)` runs `effect` while holding the exclusive lock
 *   for `name`; two `withLock` sections with the same name NEVER overlap.
 * - Different names do not contend.
 * - The lock is released on success, failure, AND interruption — a failing
 *   critical section never wedges the name.
 * - Locks are NOT reentrant. Nesting `withLock` on the same name deadlocks
 *   (same as Postgres advisory locks across sessions); don't do it.
 *
 * Backends: in-memory (mutex-memory.ts) and Postgres advisory locks
 * (mutex-postgres.ts — `pg_advisory_xact_lock(hashtextextended(name, 0))`
 * held on a dedicated transaction for the section's duration). Swap
 * targets per the audit: Redis locks.
 */
import { Context, Schema } from "effect"
import type { Effect } from "effect"

/** Unrecoverable backend failure while acquiring/releasing the lock. */
export class MutexBackendError extends Schema.TaggedErrorClass<MutexBackendError>()(
  "MutexBackendError",
  {
    backend: Schema.String,
    operation: Schema.String,
    cause: Schema.Defect,
  },
) {}

export interface MutexShape {
  readonly withLock: <A, E, R>(
    name: string,
    effect: Effect.Effect<A, E, R>,
  ) => Effect.Effect<A, E | MutexBackendError, R>
}

export class Mutex extends Context.Service<Mutex, MutexShape>()(
  "@openagentsinc/oa-infra/Mutex",
) {}
