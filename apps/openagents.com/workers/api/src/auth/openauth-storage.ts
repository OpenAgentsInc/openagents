// CFG-3 (#8518): the OpenAuth issuer `StorageAdapter` (sessions, refresh
// tokens, PKCE codes, signing keys, email-OTP state) serves from owned
// Postgres via @openagentsinc/oa-infra's KvStore — the D1 `openauth_storage`
// table and its KS-8.18 read-back mirror are legacy as of this lane (CFG-3
// claims the openauth_storage cutover; CFG-4's identity domain does not need
// to port it). HARD CUT (owner-approved): fresh storage means fresh issuer
// signing keys, so existing sessions invalidate and users re-login once.
//
// Semantics preserved from the previous D1 adapter at the contract level:
// keys are `joinKey`-joined (0x1f separator), values are JSON records, `get`
// of a missing/expired key is `undefined`, `set` with an `expiry` Date
// becomes a TTL, `scan` yields non-expired entries under
// `joinKey([...prefix, ''])` ordered by key. Expiry enforcement now lives in
// the KvStore backend (lazy reap on read — conformance-tested in
// packages/oa-infra), so this adapter no longer needs its own clock on the
// read path.

import {
  type StorageAdapter,
  joinKey,
  splitKey,
} from '@openauthjs/openauth/storage/storage'

import { safeJsonRecord } from '../json-boundary'
import { currentEpochMillis, currentIsoTimestamp } from '../runtime-primitives'
import {
  type AuthKvEnv,
  type AuthKvStore,
  type MakeAuthKvOptions,
  authKvStoreForEnv,
} from './auth-kv'

export type OpenAuthStorageRuntime = Readonly<{
  nowIso: () => string
  nowMs: () => number
}>

export const systemOpenAuthStorageRuntime: OpenAuthStorageRuntime = {
  nowIso: currentIsoTimestamp,
  nowMs: currentEpochMillis,
}

/**
 * OpenAuth `StorageAdapter` over the owned key/value store. `runtime` is the
 * clock used to convert `set`'s absolute `expiry` Date into a TTL (the store
 * itself owns expiry enforcement).
 */
export const makeKvOpenAuthStorage = (
  kv: AuthKvStore,
  runtime: OpenAuthStorageRuntime = systemOpenAuthStorageRuntime,
): StorageAdapter => ({
  get: async key => {
    const raw = await kv.get(joinKey(key), 'text')

    return raw === null ? undefined : safeJsonRecord(raw)
  },

  set: async (key, value: unknown, expiry) => {
    const ttlMs =
      expiry === undefined ? undefined : expiry.getTime() - runtime.nowMs()

    await kv.put(
      joinKey(key),
      JSON.stringify(value),
      ttlMs === undefined
        ? undefined
        : // KV-facade TTLs are seconds; round up so a sub-second remainder
          // never truncates a live expiry to "already expired".
          { expirationTtl: Math.max(1, Math.ceil(ttlMs / 1000)) },
    )
  },

  remove: async key => {
    await kv.delete(joinKey(key))
  },

  scan: async function* (prefix) {
    const entries = await kv.listPrefix(joinKey([...prefix, '']))

    for (const entry of entries) {
      const parsed = safeJsonRecord(entry.value)

      if (parsed !== undefined) {
        yield [splitKey(entry.key), parsed]
      }
    }
  },
})

/**
 * The production OpenAuth storage for this env — Postgres KvStore, never D1,
 * never Cloudflare KV (hard cut, #8518). Replaces the KS-8.18
 * `makeD1Storage` + read-back-mirror drop-in that previously lived in
 * identity-auth-domain-store.ts.
 */
export const makeOpenAuthStorageForEnv = (
  env: AuthKvEnv,
  runtime: OpenAuthStorageRuntime = systemOpenAuthStorageRuntime,
  options: MakeAuthKvOptions = {},
): StorageAdapter =>
  makeKvOpenAuthStorage(authKvStoreForEnv(env, options), runtime)
