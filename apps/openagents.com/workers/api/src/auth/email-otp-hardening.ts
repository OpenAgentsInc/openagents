import { safeJsonRecord } from '../json-boundary'
import { epochMillisToIsoTimestamp } from '../runtime-primitives'

export const AUTH_EMAIL_OTP_CODE_TTL_SECONDS = 10 * 60
export const AUTH_EMAIL_OTP_ISSUED_AT_CLAIM = 'oa_otp_issued_at'
export const AUTH_EMAIL_OTP_EXPIRES_AT_CLAIM = 'oa_otp_expires_at'

export type AuthEmailOtpRateLimitScope = 'ip' | 'email' | 'global'

export type AuthEmailOtpRateLimitBucket = Readonly<{
  limit: number
  scope: AuthEmailOtpRateLimitScope
  subject: string
  windowSeconds: number
}>

export type AuthEmailOtpRateLimitPolicy = Readonly<{
  email: Readonly<{ limit: number; windowSeconds: number }>
  global: Readonly<{ limit: number; windowSeconds: number }>
  ip: Readonly<{ limit: number; windowSeconds: number }>
}>

export const defaultAuthEmailOtpRateLimitPolicy: AuthEmailOtpRateLimitPolicy = {
  email: { limit: 4, windowSeconds: 10 * 60 },
  global: { limit: 180, windowSeconds: 60 * 60 },
  ip: { limit: 8, windowSeconds: 10 * 60 },
}

export type AuthEmailOtpRateLimitRuntime = Readonly<{
  nowIso: () => string
  nowMs: () => number
}>

export type AuthEmailOtpRateLimitInput = Readonly<{
  email: string
  ipAddress: string
}>

export type AuthEmailOtpRateLimitAllowed = Readonly<{
  _tag: 'Allowed'
  remaining: ReadonlyArray<
    Readonly<{
      limit: number
      remaining: number
      resetAt: string
      scope: AuthEmailOtpRateLimitScope
    }>
  >
}>

export type AuthEmailOtpRateLimitRejected = Readonly<{
  _tag: 'RateLimited'
  limit: number
  resetAt: string
  retryAfterSeconds: number
  scope: AuthEmailOtpRateLimitScope
  windowSeconds: number
}>

export type AuthEmailOtpRateLimitResult =
  | AuthEmailOtpRateLimitAllowed
  | AuthEmailOtpRateLimitRejected

type StoredBucketState = Readonly<{
  count: number
  key: string
  resetAtMs: number
}>

const textEncoder = new TextEncoder()
const RATE_LIMIT_KEY_PREFIX = 'auth:email_otp_rate'
const RATE_LIMIT_EXPIRY_GRACE_MS = 60_000

export const normalizeAuthEmailOtpEmail = (email: string): string =>
  email.trim().toLowerCase()

export const authEmailOtpClientIp = (request: Request): string => {
  const cloudflareIp = request.headers.get('cf-connecting-ip')?.trim()

  if (cloudflareIp !== undefined && cloudflareIp !== '') {
    return cloudflareIp
  }

  const forwardedFor = request.headers.get('x-forwarded-for')?.trim()
  const firstForwarded = forwardedFor?.split(',')[0]?.trim()

  return firstForwarded === undefined || firstForwarded === ''
    ? 'unknown'
    : firstForwarded
}

export const authEmailOtpSendForm = (
  formData: FormData,
): Readonly<{ action: 'request' | 'resend'; email: string }> | undefined => {
  const action = formData.get('action')?.toString()

  if (action !== 'request' && action !== 'resend') {
    return undefined
  }

  const email = formData.get('email')?.toString()

  return email === undefined || email.trim() === ''
    ? undefined
    : { action, email: normalizeAuthEmailOtpEmail(email) }
}

export const stampAuthEmailOtpClaims = (
  claims: Record<string, string>,
  runtime: AuthEmailOtpRateLimitRuntime,
): void => {
  const issuedAtMs = runtime.nowMs()

  claims[AUTH_EMAIL_OTP_ISSUED_AT_CLAIM] = epochMillisToIsoTimestamp(issuedAtMs)
  claims[AUTH_EMAIL_OTP_EXPIRES_AT_CLAIM] = epochMillisToIsoTimestamp(
    issuedAtMs + AUTH_EMAIL_OTP_CODE_TTL_SECONDS * 1000,
  )
}

export const authEmailOtpClaimsAreFresh = (
  claims: Readonly<Record<string, string>>,
  runtime: AuthEmailOtpRateLimitRuntime,
): boolean => {
  const expiresAt = claims[AUTH_EMAIL_OTP_EXPIRES_AT_CLAIM]

  if (expiresAt === undefined) {
    return false
  }

  const expiresAtMs = Date.parse(expiresAt)

  return Number.isFinite(expiresAtMs) && expiresAtMs > runtime.nowMs()
}

export const reserveAuthEmailOtpSend = async (
  db: D1Database,
  input: AuthEmailOtpRateLimitInput,
  runtime: AuthEmailOtpRateLimitRuntime,
  policy: AuthEmailOtpRateLimitPolicy = defaultAuthEmailOtpRateLimitPolicy,
): Promise<AuthEmailOtpRateLimitResult> => {
  const nowMs = runtime.nowMs()
  const buckets = await authEmailOtpRateLimitBuckets(input, policy)
  const states = await Promise.all(
    buckets.map(async bucket => ({
      bucket,
      state: await readBucketState(db, bucket, nowMs),
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
      writeBucketState(db, bucket, state, state.count + 1, runtime.nowIso()),
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

const authEmailOtpRateLimitBuckets = async (
  input: AuthEmailOtpRateLimitInput,
  policy: AuthEmailOtpRateLimitPolicy,
): Promise<ReadonlyArray<AuthEmailOtpRateLimitBucket>> => [
  {
    ...policy.ip,
    scope: 'ip',
    subject: await stableHash(input.ipAddress),
  },
  {
    ...policy.email,
    scope: 'email',
    subject: await stableHash(normalizeAuthEmailOtpEmail(input.email)),
  },
  {
    ...policy.global,
    scope: 'global',
    subject: 'all',
  },
]

const readBucketState = async (
  db: D1Database,
  bucket: AuthEmailOtpRateLimitBucket,
  nowMs: number,
): Promise<StoredBucketState> => {
  const key = bucketStorageKey(bucket, nowMs)
  const row = await db
    .prepare(
      `SELECT value_json
       FROM openauth_storage
       WHERE key = ?
         AND (expires_at IS NULL OR expires_at > ?)`,
    )
    .bind(key, nowMs)
    .first<Readonly<{ value_json: string }>>()
  const parsed =
    row === null || row === undefined
      ? undefined
      : safeJsonRecord(row.value_json)
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

const writeBucketState = async (
  db: D1Database,
  bucket: AuthEmailOtpRateLimitBucket,
  state: StoredBucketState,
  count: number,
  nowIso: string,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO openauth_storage
        (key, value_json, expires_at, updated_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(key) DO UPDATE SET
        value_json = excluded.value_json,
        expires_at = excluded.expires_at,
        updated_at = excluded.updated_at`,
    )
    .bind(
      state.key,
      JSON.stringify({
        count,
        resetAt: epochMillisToIsoTimestamp(state.resetAtMs),
        scope: bucket.scope,
      }),
      state.resetAtMs + RATE_LIMIT_EXPIRY_GRACE_MS,
      nowIso,
    )
    .run()
}

const bucketStorageKey = (
  bucket: AuthEmailOtpRateLimitBucket,
  nowMs: number,
): string =>
  `${RATE_LIMIT_KEY_PREFIX}:${bucket.scope}:${bucket.subject}:${bucketWindowStartMs(bucket, nowMs)}`

const bucketWindowStartMs = (
  bucket: AuthEmailOtpRateLimitBucket,
  nowMs: number,
): number => {
  const windowMs = bucket.windowSeconds * 1000

  return Math.floor(nowMs / windowMs) * windowMs
}

const bucketResetAtMs = (
  bucket: AuthEmailOtpRateLimitBucket,
  nowMs: number,
): number => bucketWindowStartMs(bucket, nowMs) + bucket.windowSeconds * 1000

const stableHash = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    textEncoder.encode(value),
  )

  return Array.from(new Uint8Array(digest), byte =>
    byte.toString(16).padStart(2, '0'),
  ).join('')
}
