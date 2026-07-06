/**
 * KvStore — owned key/value primitive (CFG-2, issue #8517, audit §5).
 *
 * Semantics WE define (any backend must pass src/conformance/kv-store.ts):
 * - `get` of a missing OR expired key is `null` (expiry is observable
 *   immediately once the TTL elapses; backends may reap lazily).
 * - `put` overwrites unconditionally. `ttlMs` (when given) replaces any
 *   previous TTL; omitting it makes the key non-expiring again.
 * - `delete` is idempotent — deleting a missing key succeeds.
 * - `listPrefix` returns every non-expired entry whose key starts with the
 *   LITERAL prefix string (no pattern semantics — `%`/`_` in the prefix are
 *   ordinary characters), ordered by key ascending. Added for CFG-3
 *   (issue #8518): the OpenAuth `StorageAdapter.scan` contract needs a
 *   bounded prefix scan over session/refresh keys.
 *
 * Backends: in-memory (kv-store-memory.ts) and Postgres
 * (kv-store-postgres.ts, migrations/0001_oa_infra_kv.sql). Swap targets per
 * the audit: Memorystore/Redis — behind this same interface.
 */
import { Context, Schema } from "effect"
import type { Effect } from "effect"

/** Unrecoverable backend failure (connection loss, vendor error, ...). */
export class KvStoreBackendError extends Schema.TaggedErrorClass<KvStoreBackendError>()(
  "KvStoreBackendError",
  {
    backend: Schema.String,
    operation: Schema.String,
    cause: Schema.Defect,
  },
) {}

export interface KvPutOptions {
  /** Time-to-live in milliseconds. Omit for a non-expiring key. */
  readonly ttlMs?: number
}

export interface KvListEntry {
  readonly key: string
  readonly value: string
}

export interface KvStoreShape {
  /** `null` when the key is missing or its TTL has elapsed. */
  readonly get: (key: string) => Effect.Effect<string | null, KvStoreBackendError>
  readonly put: (
    key: string,
    value: string,
    options?: KvPutOptions,
  ) => Effect.Effect<void, KvStoreBackendError>
  /** Idempotent: deleting a missing key succeeds. */
  readonly delete: (key: string) => Effect.Effect<void, KvStoreBackendError>
  /**
   * Every non-expired entry whose key starts with the LITERAL `prefix`
   * (never pattern-interpreted), ordered by key ascending.
   */
  readonly listPrefix: (
    prefix: string,
  ) => Effect.Effect<ReadonlyArray<KvListEntry>, KvStoreBackendError>
}

export class KvStore extends Context.Service<KvStore, KvStoreShape>()(
  "@openagentsinc/oa-infra/KvStore",
) {}
