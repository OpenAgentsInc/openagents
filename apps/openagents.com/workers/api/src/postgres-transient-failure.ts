/**
 * Naming — and surviving — a transient Postgres connection failure.
 *
 * WHY THIS EXISTS (incident 2026-07-31, P0). `POST /api/omega/auth/session`
 * flapped in production: 200 -> 503 -> 200 within seconds for the same
 * identity, each failure taking ~30s to surface and carrying the single typed
 * code `omega_nostr_auth_storage_unavailable`.
 *
 * The measured cause was NOT "the database is down". Every 503 landed within
 * ~10-70 SECONDS OF A NEW CLOUD RUN REVISION STARTING. On a fresh instance the
 * Cloud SQL Auth Proxy sidecar has not finished its first instance refresh, and
 * it logs exactly:
 *
 *   Cloud SQL connection failed ... refresh failed: context deadline exceeded
 *   [openagentsgemini:us-central1:khala-sync-pg] failed to connect to instance:
 *     SFEClient is nil
 *   dial tcp 34.70.178.7:3307: i/o timeout
 *
 * Cloud Run routes traffic to the instance as soon as the HTTP server listens,
 * so requests arrive BEFORE the unix socket at
 * `/cloudsql/<instance>/.s.PGSQL.5432` can accept a connection. postgres.js
 * then fails the connect and the route turned every one of those into the same
 * undiagnosable string.
 *
 * Two things follow, and this module owns both:
 *
 * 1. A failure whose connection was NEVER ESTABLISHED is materially different
 *    from a statement that reached Postgres and failed. The first is transient,
 *    self-healing, and safe to retry for ANY operation (nothing was sent). The
 *    second is not.
 * 2. A diagnostic that collapses several causes into one string is itself part
 *    of the defect. `postgresFailureClass` names the cause so the 503 code, the
 *    log line, and the client's retry decision can all be made on evidence.
 *
 * Classification is driven by postgres.js's own error `code` (see its
 * `src/errors.js`: `Errors.connection(code, options, socket)` sets
 * `code`/`errno`) plus the Node socket errnos that reach us through it.
 */

/**
 * What actually went wrong, in the only three categories that change a
 * decision.
 *
 * - `connect_unavailable` — the connection was never established: the connect
 *   timed out, the socket was refused, or the Cloud SQL socket path does not
 *   exist yet. NO statement reached Postgres, so ANY operation may be retried
 *   without changing its meaning. This is the cold-start proxy window.
 * - `connection_lost` — an established connection went away. A statement MAY
 *   have executed, so only reads may be retried; retrying a write could double
 *   an effect or mis-report `putIfAbsent`.
 * - `server_error` — Postgres answered and refused (a `PostgresError`:
 *   constraint violation, missing relation, permission). Retrying is pointless
 *   and hides the real defect.
 * - `unknown` — anything else. Treated as non-retryable on purpose: a class we
 *   cannot name is a class we must not silently paper over.
 */
export type PostgresFailureClass =
  | 'connect_unavailable'
  | 'connection_lost'
  | 'server_error'
  | 'unknown'

/**
 * postgres.js connection-phase codes. `CONNECT_TIMEOUT` is the one the
 * 2026-07-31 incident produced (`write CONNECT_TIMEOUT
 * /cloudsql/.../.s.PGSQL.5432`).
 */
const CONNECT_UNAVAILABLE_CODES: ReadonlySet<string> = new Set([
  'CONNECT_TIMEOUT',
  // Node socket errnos surfaced by postgres.js when the Cloud SQL unix socket
  // is not accepting yet, or the proxy has not created the socket file at all.
  'ECONNREFUSED',
  'ENOENT',
  'EAGAIN',
  'EAI_AGAIN',
  'EHOSTUNREACH',
  'ENETUNREACH',
  'ETIMEDOUT',
])

/** postgres.js codes for a connection that existed and then went away. */
const CONNECTION_LOST_CODES: ReadonlySet<string> = new Set([
  'CONNECTION_CLOSED',
  'CONNECTION_DESTROYED',
  'CONNECTION_ENDED',
  'ECONNRESET',
  'EPIPE',
])

const errorCode = (error: unknown): string | undefined => {
  if (typeof error !== 'object' || error === null) return undefined
  const code = (error as { code?: unknown }).code
  return typeof code === 'string' ? code : undefined
}

const errorName = (error: unknown): string | undefined => {
  if (typeof error !== 'object' || error === null) return undefined
  const name = (error as { name?: unknown }).name
  return typeof name === 'string' ? name : undefined
}

/**
 * The underlying failure, unwrapped.
 *
 * Effect's `Effect.tryPromise` bare form and several wrappers in this codebase
 * re-throw with the original rejection on `.cause`. Classifying the wrapper
 * would report `unknown` for a failure we can name exactly, so walk the chain
 * (bounded — a cyclic `cause` must not hang an auth request).
 */
const causeChain = function* (error: unknown): Generator<unknown> {
  let current = error
  for (let depth = 0; depth < 8; depth += 1) {
    yield current
    if (typeof current !== 'object' || current === null) return
    const cause = (current as { cause?: unknown }).cause
    if (cause === undefined || cause === null || cause === current) return
    current = cause
  }
}

/** Name the cause of a Postgres-path failure. Never throws. */
export const postgresFailureClass = (error: unknown): PostgresFailureClass => {
  for (const link of causeChain(error)) {
    const code = errorCode(link)
    if (code !== undefined) {
      if (CONNECT_UNAVAILABLE_CODES.has(code)) return 'connect_unavailable'
      if (CONNECTION_LOST_CODES.has(code)) return 'connection_lost'
    }
    // postgres.js wraps a server refusal in `PostgresError` (its own class
    // name), whose `code` is the five-character SQLSTATE.
    if (errorName(link) === 'PostgresError') return 'server_error'
  }
  return 'unknown'
}

/**
 * The public-safe code for this failure, for logs and typed error bodies.
 * Deliberately NOT the raw message: a Postgres message can carry row values.
 */
export const postgresFailureCode = (error: unknown): string => {
  for (const link of causeChain(error)) {
    const code = errorCode(link)
    if (code !== undefined) return code
  }
  return 'unclassified'
}

/** Which failures a caller is willing to retry. */
export type PostgresRetryScope =
  /** Only failures where no statement can have executed. Safe for writes. */
  | 'connect-only'
  /** Also a lost established connection. Reads only. */
  | 'connection'

export const isRetryablePostgresFailure = (
  error: unknown,
  scope: PostgresRetryScope,
): boolean => {
  const failureClass = postgresFailureClass(error)
  return failureClass === 'connect_unavailable'
    ? true
    : failureClass === 'connection_lost' && scope === 'connection'
}

export type PostgresRetryOptions = Readonly<{
  /** Total attempts including the first. */
  attempts?: number
  /** Base backoff in ms; attempt N waits `baseDelayMs * 2^(N-1)`. */
  baseDelayMs?: number
  /** Hard ceiling on a single backoff wait. */
  maxDelayMs?: number
  scope: PostgresRetryScope
  /** Observability seam: called once per retried failure. */
  onRetry?: (
    info: Readonly<{
      attempt: number
      delayMs: number
      failureClass: PostgresFailureClass
      failureCode: string
    }>,
  ) => void
  /** Test seam. Default: a real timer. */
  sleep?: (ms: number) => Promise<void>
}>

/**
 * Attempt defaults.
 *
 * Sized against the measured incident, not a guess. A single failing connect
 * costs up to the pool's `connect_timeout` (10s), so an unbounded retry budget
 * would turn a 30s failure into a multi-minute one. Three attempts with 250ms
 * and 750ms backoff adds at most ~1s of waiting to a request that is already
 * failing, and clears the common case where the Cloud SQL proxy became ready
 * between two attempts. Riding out the FULL cold-start window is the startup
 * readiness gate's job (see `cloudrun/postgres-readiness.ts`), not a single
 * request's.
 */
const DEFAULT_ATTEMPTS = 3
const DEFAULT_BASE_DELAY_MS = 250
const DEFAULT_MAX_DELAY_MS = 2_000

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => {
    setTimeout(resolve, ms)
  })

/**
 * Run `operation`, retrying ONLY the failures `scope` admits.
 *
 * The last failure is rethrown unchanged so the caller still sees — and can
 * still name — the real cause. This never converts a persistent outage into a
 * success or into a quieter error.
 */
export const retryTransientPostgres = async <A>(
  operation: () => Promise<A>,
  options: PostgresRetryOptions,
): Promise<A> => {
  const attempts = Math.max(1, options.attempts ?? DEFAULT_ATTEMPTS)
  const baseDelayMs = options.baseDelayMs ?? DEFAULT_BASE_DELAY_MS
  const maxDelayMs = options.maxDelayMs ?? DEFAULT_MAX_DELAY_MS
  const sleep = options.sleep ?? defaultSleep

  let lastError: unknown
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await operation()
    } catch (error) {
      lastError = error
      const retryable = isRetryablePostgresFailure(error, options.scope)
      if (!retryable || attempt === attempts) break
      const delayMs = Math.min(
        maxDelayMs,
        baseDelayMs * Math.pow(2, attempt - 1),
      )
      options.onRetry?.({
        attempt,
        delayMs,
        failureClass: postgresFailureClass(error),
        failureCode: postgresFailureCode(error),
      })
      await sleep(delayMs)
    }
  }
  throw lastError
}
