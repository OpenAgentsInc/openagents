// CFG-3 (#8518): the AUTH_STORAGE Cloudflare KV namespace is evacuated to
// owned Postgres behind @openagentsinc/oa-infra's KvStore interface. Cloudflare
// killed two account-level features in one week (Analytics Engine #8501, R2
// #8516); KV sat on the ENTIRE login path (github identity/write tokens,
// mobile access-token revocation, device-login attempts, push-token prune
// keys), so it moves off Cloudflare storage entirely — HARD CUT, no KV
// fallback reads (owner-approved: existing KV-backed state invalidates once).
//
// Storage home: the `oa_infra_kv` table in the khala-sync Cloud SQL database
// (khala-sync-server migration 0041; identical DDL to oa-infra migrations
// 0001+0004). The Worker reaches it exactly the way every other khala-sync
// seam does — a transaction-mode-safe postgres.js client acquired per
// operation via `defaultMakeKhalaSyncSqlClient` over the `KHALA_SYNC_DB`
// Hyperdrive binding (SPEC §4 discipline: one connection, unnamed statements,
// no session state). NOTE: until CFG-9 moves this Worker's runtime off
// Cloudflare, auth reads traverse Hyperdrive (the #8409 pool-incident path);
// every operation here FAILS CLOSED — callers already treat storage errors as
// deny/absent — and CFG-9's direct-Postgres runtime removes Hyperdrive from
// this path entirely.
//
// The SQL semantics live in ONE place: oa-infra's `makePostgresKvStore`
// (packages/oa-infra/src/kv-store-postgres.ts), which passes the KvStore
// conformance suite (lazy TTL expiry, literal-prefix listPrefix) against a
// real Postgres in packages/oa-infra/src/postgres-backends.test.ts. This
// module only adapts it to the Worker: per-operation client lifecycle, a
// Promise facade shaped like the KVNamespace subset the route code already
// used (`get`/`get(_, 'json')`/`put({ expirationTtl })`/`delete`), and env
// plumbing with an injectable store for tests.

import { Effect } from 'effect'
import type { KvStoreShape } from '@openagentsinc/oa-infra/kv-store'
import {
  makePostgresKvStore,
  type KvSql,
} from '@openagentsinc/oa-infra/kv-store-postgres'
import { makeMemoryKvStore } from '@openagentsinc/oa-infra/kv-store-memory'

import { parseJsonUnknown } from '../json-boundary'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type MakeKhalaSyncPushSqlClient,
} from '../khala-sync-push-routes'

// The ONE named Effect->Promise bridge in this module (zero-debt budget 1):
// oa-infra's KvStore surface is Effect-typed; every route consumer of the
// auth KV store is Promise-shaped. Ratchet away when the consumers become
// Effect programs end-to-end (or when CFG-9's runtime adopts the KvStore
// Layer directly).
const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

/**
 * The read surface consumers use — the same call shapes the KVNamespace
 * binding offered (`'json'` parses, `'text'`/bare returns the raw string,
 * missing/expired keys are `null`).
 */
export interface AuthKvGet {
  (key: string): Promise<string | null>
  (key: string, type: 'text'): Promise<string | null>
  (key: string, type: 'json'): Promise<unknown>
}

export type AuthKvStore = Readonly<{
  get: AuthKvGet
  /** `expirationTtl` is SECONDS (KV semantics). Omit for a non-expiring key. */
  put: (
    key: string,
    value: string,
    options?: Readonly<{ expirationTtl?: number }>,
  ) => Promise<void>
  /** Idempotent. */
  delete: (key: string) => Promise<void>
  /**
   * Every non-expired entry whose key starts with the LITERAL prefix,
   * ordered by key ascending (the OpenAuth `StorageAdapter.scan` seam).
   */
  listPrefix: (
    prefix: string,
  ) => Promise<ReadonlyArray<Readonly<{ key: string; value: string }>>>
}>

/**
 * Typed invariant error for a missing storage binding (never a generic
 * `throw new Error`). Auth storage is load-bearing for every authed request;
 * with no KHALA_SYNC_DB binding and no injected store there is nothing safe
 * to serve, so every operation fails closed with this error.
 */
export class AuthKvUnavailableError extends Error {
  override readonly name = 'AuthKvUnavailableError'

  constructor() {
    super(
      'auth KV storage is unavailable: no KHALA_SYNC_DB binding and no injected AUTH_KV store',
    )
  }
}

/** Promise facade over an oa-infra KvStore (Effect) implementation. */
export const makeAuthKvStore = (kv: KvStoreShape): AuthKvStore => {
  const get = (async (key: string, type?: 'text' | 'json') => {
    const raw = await run(kv.get(key))
    if (raw === null) {
      return null
    }
    return type === 'json' ? parseJsonUnknown(raw) : raw
  }) as AuthKvGet

  return {
    get,
    put: async (key, value, options) => {
      const expirationTtl = options?.expirationTtl
      await run(
        kv.put(
          key,
          value,
          expirationTtl === undefined
            ? undefined
            : { ttlMs: expirationTtl * 1000 },
        ),
      )
    },
    delete: async key => {
      await run(kv.delete(key))
    },
    listPrefix: prefix => run(kv.listPrefix(prefix)),
  }
}

/**
 * In-memory AuthKvStore for tests — the SAME facade over oa-infra's
 * conformance-passing memory backend, so test fakes share exact semantics
 * with production instead of re-implementing them.
 */
export const makeMemoryAuthKvStore = (): AuthKvStore =>
  makeAuthKvStore(makeMemoryKvStore())

/**
 * oa-infra KvStore over the khala-sync postgres.js client path, acquiring a
 * fresh transaction-mode-safe client PER OPERATION and always ending it —
 * the same client lifecycle discipline as every other KS-8 Worker store
 * (workerd cannot hold connections across requests; CFG-9's Bun runtime can
 * swap this for oa-infra's pooled `layerPostgres` with zero schema change).
 */
export const makePerOperationPostgresKvStore = (
  acquireSql: () => Promise<
    Readonly<{ sql: unknown; end: () => Promise<void> }>
  >,
): KvStoreShape => {
  const perOp =
    <Args extends ReadonlyArray<unknown>, A, E>(
      op: (kv: KvStoreShape) => (...args: Args) => Effect.Effect<A, E>,
    ) =>
    (...args: Args): Effect.Effect<A, E> =>
      Effect.acquireUseRelease(
        Effect.promise(acquireSql),
        client =>
          // postgres.js and Bun SQL share the tagged-template call shape
          // `makePostgresKvStore` needs; the cast is the single deliberate
          // driver seam (same discipline as defaultMakeKhalaSyncSqlClient).
          op(makePostgresKvStore(client.sql as KvSql))(...args),
        client =>
          // Best-effort teardown, same discipline as the push route.
          Effect.promise(() => client.end().catch(() => undefined)),
      )

  return {
    get: perOp(kv => kv.get),
    put: perOp(kv => kv.put),
    delete: perOp(kv => kv.delete),
    listPrefix: perOp(kv => kv.listPrefix),
  }
}

export type AuthKvEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  /** Test/in-process override; production Workers never set this. */
  AUTH_KV?: AuthKvStore | undefined
}>

export type MakeAuthKvOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

const unavailableAuthKvStore: AuthKvStore = {
  get: (async () => {
    throw new AuthKvUnavailableError()
  }) as AuthKvGet,
  put: async () => {
    throw new AuthKvUnavailableError()
  },
  delete: async () => {
    throw new AuthKvUnavailableError()
  },
  listPrefix: async () => {
    throw new AuthKvUnavailableError()
  },
}

/** The KvStore (Effect surface) for this env — the OpenAuth adapter seam. */
export const authKvKvStoreForEnv = (
  env: AuthKvEnv,
  options: MakeAuthKvOptions = {},
): KvStoreShape | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePerOperationPostgresKvStore(() => makeSqlClient(connectionString))
}

/**
 * The auth KV store for this env: the injected `AUTH_KV` when present
 * (tests), otherwise Postgres over `KHALA_SYNC_DB`, otherwise a fail-closed
 * store whose every operation rejects with `AuthKvUnavailableError` —
 * NEVER a Cloudflare KV namespace (hard cut, #8518).
 */
export const authKvStoreForEnv = (
  env: AuthKvEnv,
  options: MakeAuthKvOptions = {},
): AuthKvStore => {
  if (env.AUTH_KV !== undefined) {
    return env.AUTH_KV
  }
  const kv = authKvKvStoreForEnv(env, options)
  return kv === undefined ? unavailableAuthKvStore : makeAuthKvStore(kv)
}
