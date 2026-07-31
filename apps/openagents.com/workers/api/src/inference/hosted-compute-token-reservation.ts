// Pre-flight token reservations that bound the hosted-compute ceiling's
// read-versus-settle gap.
//
// THE GAP
// -------
// `hosted-compute-daily-ceiling.ts` compares a ceiling against
// `SUM(total_tokens)` over COMPLETED `token_usage_events` rows. Metering runs on
// `waitUntil` AFTER the upstream response, so every request issued before the
// first one settles reads the same pre-burst total and every one of them is
// admitted. The overshoot was therefore bounded by nothing except how many
// requests a caller could open at once.
//
// That is not a rounding error on this route. Measured over all 2,418
// `omega_provider_broker` ledger rows to date: the LARGEST single execution
// drew 301,155 tokens (p99 285,680, p95 226,327, mean 66,964) against a
// 1,000,000 daily ceiling. Four concurrent executions already exceed a fresh
// actor's entire day, and nothing capped the concurrency.
//
// THE MECHANISM
// -------------
// One durable marker per IN-FLIGHT execution, written BEFORE the ceiling is
// read and deleted after the execution settles. The ceiling then compares
//
//   exact settled tokens today  +  reservations held by OTHER in-flight
//                                  executions for this same actor
//
// against the ceiling.
//
// WHY RESERVE BEFORE READING. Because it is what makes concurrency visible.
// Each request writes its OWN distinct key, so there is no read-modify-write
// and no lost update to race over; and because the write precedes the read,
// any request that has already been admitted is guaranteed visible to every
// later reader. Two requests that interleave both see both markers, so they are
// judged against the true total. The only residual race makes the gate MORE
// strict (both may refuse), never more permissive — the correct direction for a
// spend gate.
//
// WHY "OTHER" AND NOT "ALL". A request excludes its own marker from the sum, so
// a serial caller — the overwhelmingly common case — sees `reservedByOthers = 0`
// and gets EXACTLY the allowance it had before this module existed. Counting
// one's own reservation would silently cut the advertised daily allowance by a
// full reservation's worth, which is a product change, not a safety fix.
//
// THE BOUND THIS ACHIEVES
// -----------------------
// Let `H` be the actor's remaining headroom and `R` the per-execution
// reservation. A request is admitted only while `served + (k-1)·R < ceiling`,
// so at most `⌊H/R⌋ + 1` executions are ever concurrently admitted. If no single
// execution draws more than `R`, the total drawn is at most
// `served + H + R = ceiling + R`:
//
//   **the ceiling can be exceeded by at most ONE reservation (320,000 tokens),
//   where before it could be exceeded without bound.**
//
// THE RESIDUAL, STATED EXACTLY
// ----------------------------
//  1. The bound holds while a single execution draws no more than `R`. `R` is
//     set to 320,000, above the largest draw ever observed on this route
//     (301,155 across 2,418 rows). An execution that exceeds `R` degrades the
//     bound to `(H/R + 1)·actual - H`. `R` is owner-tunable without a deploy.
//  2. A crashed or abandoned request leaves its marker until the TTL expires
//     (15 minutes), during which the actor's headroom is understated by up to
//     `R` per leaked marker. That direction is fail-closed: it can refuse a
//     legitimate request, never over-serve. `listPrefix` returns only
//     non-expired entries, so a leak self-heals without a sweeper.
//  3. This is a reservation against CONCURRENCY, not a settlement. It does not
//     make the ledger itself transactional, and it deliberately does not write
//     to `token_usage_events` — that table stays exact-only, because the public
//     tokens-served projection keys every increment to one exact row.
//
// WHY THIS STORE
// --------------
// `oa_infra_kv` is already deployed, already lives in the same Cloud SQL
// database the ceiling reads, and already offers the two operations this needs
// atomically: `putIfAbsent` (atomic create) and `listPrefix` (non-expired
// literal-prefix scan). No migration is required, and because store and ledger
// share one database they fail together — the fail-closed posture gains no new
// independent outage surface.
//
// It deliberately does NOT use `auth/kv-window-rate-limit.ts`, whose own header
// says its non-atomic read-modify-write "can undercount" under a burst and that
// "any caller that needs an exact bound must not rely on this module alone".
// Undercounting under a burst is the precise failure being fixed here.

import type { AuthKvStore } from '../auth/auth-kv'

/**
 * Per-execution provisional charge, in tokens.
 *
 * Sized ABOVE the largest single execution ever observed on this route
 * (301,155 tokens across 2,418 `omega_provider_broker` ledger rows), so the
 * overshoot bound holds for every draw within the historical envelope.
 * Owner-tunable without a code deploy.
 */
export const DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS = 320_000

/**
 * How long a marker survives without an explicit release.
 *
 * Longer than any observed streaming generation, so a live request is never
 * un-reserved underneath itself; short enough that a crashed request's
 * phantom charge clears within one operator attention span.
 */
export const HOSTED_COMPUTE_RESERVATION_TTL_SECONDS = 900

const RESERVATION_KEY_NAMESPACE = 'hosted-compute-inflight:v1'

export const hostedComputeReservedTokens = (
  rawValue: string | undefined,
): number => {
  if (rawValue === undefined) {
    return DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS
  }
  const parsed = Number.parseInt(rawValue.trim(), 10)

  // Zero or negative would disable the bound silently, so both fall back.
  return Number.isFinite(parsed) && parsed > 0
    ? parsed
    : DEFAULT_HOSTED_COMPUTE_RESERVED_TOKENS
}

/**
 * Marker key prefix for one actor.
 *
 * The actor id is HASHED to a fixed-length digest for two reasons. It keeps
 * actor identifiers out of KV keys, and — load-bearing — it makes every actor's
 * prefix the same length, so one actor's prefix can never be a prefix of
 * another's. With raw ids, actor `agent:foo` would scan `agent:foo:bar`'s
 * markers and refuse on someone else's spend.
 */
export const hostedComputeReservationPrefix = (
  actorDigest: string,
): string => `${RESERVATION_KEY_NAMESPACE}:${actorDigest}:`

export const hostedComputeReservationKey = (
  actorDigest: string,
  attemptId: string,
): string => `${hostedComputeReservationPrefix(actorDigest)}${attemptId}`

/** Sum of reserved tokens held by entries OTHER than `ownKey`. */
export const reservedTokensByOthers = (
  entries: ReadonlyArray<Readonly<{ key: string; value: string }>>,
  ownKey: string,
  fallbackTokens: number,
): number =>
  entries
    .filter(entry => entry.key !== ownKey)
    .reduce((total, entry) => {
      const parsed = Number.parseInt(entry.value, 10)

      // An unreadable marker still represents a real in-flight execution, so it
      // counts at the current reservation size rather than being ignored.
      return total + (Number.isFinite(parsed) && parsed > 0 ? parsed : fallbackTokens)
    }, 0)

export type HostedComputeReservation = Readonly<{
  /** Tokens reserved by OTHER in-flight executions for this actor. */
  reservedByOthers: number
  /** Idempotent, never throws — releasing is best-effort by design. */
  release: () => Promise<void>
}>

export type HostedComputeReservationDeps = Readonly<{
  store: AuthKvStore
  /** Fixed-length digest of the actor id (see `hostedComputeReservationPrefix`). */
  actorDigest: string
  attemptId: string
  reservedTokens: number
  ttlSeconds?: number
  /** Fail-soft release logging; never rethrows into the response path. */
  onReleaseError?: (error: unknown) => void
}>

/**
 * Take a reservation and report what OTHER in-flight executions already hold.
 *
 * Throws if the store is unavailable. The caller's gate is fail-closed, so a
 * store outage refuses a non-admitted actor rather than serving unbounded — the
 * same posture the ledger read already has.
 */
export const reserveHostedComputeTokens = async (
  deps: HostedComputeReservationDeps,
): Promise<HostedComputeReservation> => {
  const ownKey = hostedComputeReservationKey(deps.actorDigest, deps.attemptId)

  // Write BEFORE reading: this ordering is what makes a concurrent burst
  // visible to every later reader.
  await deps.store.putIfAbsent(ownKey, String(deps.reservedTokens), {
    expirationTtl: deps.ttlSeconds ?? HOSTED_COMPUTE_RESERVATION_TTL_SECONDS,
  })

  let release = async (): Promise<void> => {
    try {
      await deps.store.delete(ownKey)
    } catch (error) {
      deps.onReleaseError?.(error)
    }
  }

  try {
    const entries = await deps.store.listPrefix(
      hostedComputeReservationPrefix(deps.actorDigest),
    )

    return {
      release,
      reservedByOthers: reservedTokensByOthers(
        entries,
        ownKey,
        deps.reservedTokens,
      ),
    }
  } catch (error) {
    // The marker is already written; drop it before the fail-closed refusal so
    // a store read blip does not leave a phantom charge for the full TTL.
    await release()
    release = async () => undefined
    throw error
  }
}
