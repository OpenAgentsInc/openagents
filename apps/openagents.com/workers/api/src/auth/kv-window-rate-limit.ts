// Durable fixed-window rate-limit buckets over the owned `AuthKvStore`.
//
// Extracted from `auth/email-otp-hardening.ts` so a second abuse-bounded auth
// path (Omega Nostr self-provisioning) does not become the repository's THIRD
// hand-rolled counter. The semantics are exactly the ones that module already
// proved:
//
//   - Window rotation lives IN THE KEY (the key embeds the window start), so a
//     new window always reads a fresh bucket regardless of the stored row's
//     TTL. `expirationTtl` is garbage collection only; correctness never
//     depends on the expiry firing.
//   - The read-modify-write is NOT atomic (`AuthKvStore` has no CAS or atomic
//     increment). Concurrent requests inside one window can undercount. That is
//     an accepted, documented weakness: these buckets bound sustained abuse,
//     not a single instantaneous burst. Any caller that needs an exact bound
//     must not rely on this module alone.

import { safeJsonRecord } from '../json-boundary'
import { epochMillisToIsoTimestamp } from '../runtime-primitives'
import type { AuthKvStore } from './auth-kv'

const RATE_LIMIT_EXPIRY_GRACE_MS = 60_000

export type KvWindowRateLimitBucket<Scope extends string> = Readonly<{
  limit: number
  scope: Scope
  /** Already-hashed or otherwise non-identifying bucket subject. */
  subject: string
  windowSeconds: number
}>

export type KvWindowRateLimitAllowed<Scope extends string> = Readonly<{
  _tag: 'Allowed'
  remaining: ReadonlyArray<
    Readonly<{
      limit: number
      remaining: number
      resetAt: string
      scope: Scope
    }>
  >
}>

export type KvWindowRateLimitRejected<Scope extends string> = Readonly<{
  _tag: 'RateLimited'
  limit: number
  resetAt: string
  retryAfterSeconds: number
  scope: Scope
  windowSeconds: number
}>

export type KvWindowRateLimitResult<Scope extends string> =
  | KvWindowRateLimitAllowed<Scope>
  | KvWindowRateLimitRejected<Scope>

type StoredBucketState = Readonly<{
  count: number
  key: string
  resetAtMs: number
}>

/**
 * First hop of `x-forwarded-for`, else `x-real-ip`, else `unknown`.
 *
 * NOTE for anyone reading this as an abuse bound: on Cloud Run behind the
 * Google front end the first hop is client-controlled only in the sense that a
 * client may PREPEND entries — the GFE appends the real peer address, so the
 * first entry can be spoofed. Treat a per-IP bucket here as a friction control
 * against casual abuse, never as a hard identity bound. An `unknown` result
 * collapses every header-less caller into ONE shared bucket, which is
 * deliberately conservative (they throttle each other).
 */
export const clientIpFromRequest = (request: Request): string => {
  const forwardedFor = request.headers.get('x-forwarded-for')?.trim()
  const firstForwarded = forwardedFor?.split(',')[0]?.trim()

  if (firstForwarded !== undefined && firstForwarded !== '') {
    return firstForwarded
  }

  const realIp = request.headers.get('x-real-ip')?.trim()

  return realIp === undefined || realIp === '' ? 'unknown' : realIp
}

export const stableRateLimitSubject = async (
  value: string,
): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return Array.from(new Uint8Array(digest), byte =>
    byte.toString(16).padStart(2, '0'),
  ).join('')
}

const bucketWindowStartMs = <Scope extends string>(
  bucket: KvWindowRateLimitBucket<Scope>,
  nowMs: number,
): number => {
  const windowMs = bucket.windowSeconds * 1000

  return Math.floor(nowMs / windowMs) * windowMs
}

const bucketResetAtMs = <Scope extends string>(
  bucket: KvWindowRateLimitBucket<Scope>,
  nowMs: number,
): number => bucketWindowStartMs(bucket, nowMs) + bucket.windowSeconds * 1000

const bucketStorageKey = <Scope extends string>(
  keyPrefix: string,
  bucket: KvWindowRateLimitBucket<Scope>,
  nowMs: number,
): string =>
  `${keyPrefix}:${bucket.scope}:${bucket.subject}:${bucketWindowStartMs(bucket, nowMs)}`

const readBucketState = async <Scope extends string>(
  kv: AuthKvStore,
  keyPrefix: string,
  bucket: KvWindowRateLimitBucket<Scope>,
  nowMs: number,
): Promise<StoredBucketState> => {
  const key = bucketStorageKey(keyPrefix, bucket, nowMs)
  const raw = await kv.get(key, 'text')
  const parsed = raw === null ? undefined : safeJsonRecord(raw)
  const count = parsed?.count

  return {
    count:
      typeof count === 'number' && Number.isFinite(count)
        ? Math.max(0, Math.floor(count))
        : 0,
    key,
    resetAtMs: bucketResetAtMs(bucket, nowMs),
  }
}

const writeBucketState = async <Scope extends string>(
  kv: AuthKvStore,
  bucket: KvWindowRateLimitBucket<Scope>,
  state: StoredBucketState,
  count: number,
  nowMs: number,
): Promise<void> => {
  await kv.put(
    state.key,
    JSON.stringify({
      count,
      resetAt: epochMillisToIsoTimestamp(state.resetAtMs),
      scope: bucket.scope,
    }),
    {
      expirationTtl: Math.max(
        1,
        Math.ceil((state.resetAtMs + RATE_LIMIT_EXPIRY_GRACE_MS - nowMs) / 1000),
      ),
    },
  )
}

/**
 * Read every bucket, refuse if ANY is already at its limit, otherwise increment
 * all of them. The first exhausted bucket (in the order given) names the scope
 * in the rejection, so callers can report which bound fired without leaking the
 * subject.
 */
export const reserveKvWindowRateLimit = async <Scope extends string>(
  kv: AuthKvStore,
  keyPrefix: string,
  buckets: ReadonlyArray<KvWindowRateLimitBucket<Scope>>,
  nowMs: number,
): Promise<KvWindowRateLimitResult<Scope>> => {
  const states = await Promise.all(
    buckets.map(async bucket => ({
      bucket,
      state: await readBucketState(kv, keyPrefix, bucket, nowMs),
    })),
  )
  const limited = states.find(
    ({ bucket, state }) => state.count >= bucket.limit,
  )

  if (limited !== undefined) {
    return {
      _tag: 'RateLimited',
      limit: limited.bucket.limit,
      resetAt: epochMillisToIsoTimestamp(limited.state.resetAtMs),
      retryAfterSeconds: Math.max(
        1,
        Math.ceil((limited.state.resetAtMs - nowMs) / 1000),
      ),
      scope: limited.bucket.scope,
      windowSeconds: limited.bucket.windowSeconds,
    }
  }

  await Promise.all(
    states.map(({ bucket, state }) =>
      writeBucketState(kv, bucket, state, state.count + 1, nowMs),
    ),
  )

  return {
    _tag: 'Allowed',
    remaining: states.map(({ bucket, state }) => ({
      limit: bucket.limit,
      remaining: Math.max(0, bucket.limit - state.count - 1),
      resetAt: epochMillisToIsoTimestamp(state.resetAtMs),
      scope: bucket.scope,
    })),
  }
}
