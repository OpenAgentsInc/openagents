// Per-execution identity for a ledger row that meters REAL upstream spend.
//
// THE DEFECT THIS REPLACES
// ------------------------
// The owner-funded hosted-Gemini proxy derived its `token_usage_events`
// idempotency key from a hash of the REQUEST BODY:
//
//   const requestIdempotencyKey =
//     request.headers.get('idempotency-key')?.trim() || `body:${bodyHash}`
//   const eventHash = await sha256Hex(
//     `omega:google_gemini:${actorId}:${model}:${requestIdempotencyKey}`)
//
// combined with `INSERT OR IGNORE`. Measured live on 2026-07-31: SEVEN
// identical proxy requests produced exactly ONE ledger row. Each of the seven
// called Google and drew real, owner-funded provider tokens; six left no trace.
// Every ceiling built on that ledger — the hosted-compute daily ceiling and the
// `nostr:` self-provision ceiling — therefore under-counted by a replay factor
// that an attacker, or a buggy retry loop, chooses.
//
// A client-supplied `Idempotency-Key` header had the SAME effect and was
// strictly worse, because it let a caller decide to be metered once no matter
// how many executions it bought. The route's own test asserted two upstream
// calls (`expect(upstream).toHaveBeenCalledTimes(index + 1)`) and one ledger
// row, as if that were correct.
//
// WHY IT WAS BODY-HASHED (the property worth keeping)
// ---------------------------------------------------
// The body hash was the only client-independent stable identity a stateless
// route had, and it bought a real property cheaply: a `token_usage_events` row
// must be exactly-once, because the public tokens-served projection keys every
// increment to exactly one row's idempotency key (INVARIANTS "Canonical Token
// Usage Ledger", and the public-projection reconciliation obligation). If the
// metering write is retried, it must not double-count.
//
// The mistake was the SCOPE of that identity. "Exactly once" is a property of
// an EXECUTION, not of a byte-identical request. Two requests carrying the same
// body are two executions that each burned upstream tokens.
//
// THE INVARIANT
// -------------
//   Meter every upstream execution. Suppress only what was not executed.
//
// This route ALWAYS executes upstream — there is no response cache, so no
// request is ever served without drawing provider tokens. Therefore every
// request must produce a ledger row, and the idempotency key must be minted PER
// EXECUTION ATTEMPT, before the upstream call, then threaded to the metering
// write.
//
// That keeps the genuine retry-safety property and narrows it to what is
// actually true:
//
//   * the METERING path may run more than once for one execution (background
//     work retried, handler re-entered, at-least-once delivery). It reuses the
//     one attempt id, so `INSERT OR IGNORE` collapses it to one row. Exactly
//     once per execution is preserved.
//   * the REQUEST may be retried by a client after a network failure. The
//     upstream call happens again and the owner is billed again, so the second
//     execution mints a new attempt id and meters again. That is not a double
//     charge — it is the second real charge, recorded.
//
// WHY NOT HONOR A CLIENT `Idempotency-Key` FOR SUPPRESSION
// --------------------------------------------------------
// Because suppressing the LEDGER without suppressing the CALL is exactly the
// defect, moved into a header the caller controls. Honoring it lets any client
// pin one key and draw unbounded owner-funded inference against a ceiling that
// never moves. Real HTTP idempotency semantics are "replay the stored RESPONSE,
// do not re-execute" — suppression belongs at the execution, and the ledger
// follows the execution.
//
// The header is still read, but only as correlation: it is recorded as a safe
// ref so an operator can group a client's retries, and it never decides whether
// a row is written. Implementing true response replay would mean persisting
// full SSE bodies, i.e. model COMPLETIONS, which the ledger invariants forbid
// storing; that is a separate design with its own privacy decision, not a
// bounded fix.

/**
 * A single upstream execution attempt.
 *
 * Minted once, immediately before the provider call, and carried through to the
 * metering write. Never derived from request content.
 */
export type MeteredExecutionAttempt = Readonly<{
  attemptId: string
}>

/** Shape guard, so an attempt id is always a safe ref inside a ledger key. */
const attemptIdPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

/**
 * Mint a fresh execution attempt.
 *
 * Randomness, not content, is the point: two byte-identical requests are two
 * executions and must become two ledger rows. `crypto.randomUUID` is available
 * on the Node 24 host and in the Workers-shaped runtime these routes are typed
 * against. It is injectable so tests can pin the identity.
 */
export const newMeteredExecutionAttempt = (
  randomUuid: () => string = () => crypto.randomUUID(),
): MeteredExecutionAttempt => {
  const attemptId = randomUuid()

  if (!attemptIdPattern.test(attemptId)) {
    throw new Error('metered_execution_attempt_id_malformed')
  }

  return { attemptId }
}

export type MeteredExecutionKeyInput = Readonly<{
  /** Ledger scope, e.g. `omega:google_gemini`. */
  scope: string
  actorUserId: string
  model: string
  attempt: MeteredExecutionAttempt
}>

/**
 * The pre-image hashed into a row's event id / idempotency key.
 *
 * The attempt id alone already makes the row unique; scope, actor, and model
 * stay in the composite so a key is self-describing under audit and can never
 * collide across scopes. Request CONTENT is deliberately absent — that absence
 * is the fix.
 */
export const meteredExecutionKeyInput = (
  input: MeteredExecutionKeyInput,
): string =>
  `${input.scope}:${input.actorUserId}:${input.model}:attempt:${input.attempt.attemptId}`
