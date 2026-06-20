import { Schema as S } from 'effect'

/**
 * Standby-Pylon dispatch contract for training.marathon_operations.v1.
 *
 * Marathon discipline requires that when a live contributor (Pylon) drops out of
 * a long run, a pre-warmed standby can be PROMOTED into the collective without
 * losing the window. The collective-failure semantics (psionic#1126) already
 * define ban-for-round, partial-result preservation, and a standby-gated abort;
 * the bootstrap-from-durable-seal grant (#4850/#4851) already lets a fresh
 * contributor rejoin behind a seal-in-flight join barrier. What is still missing
 * is the admissibility predicate that decides whether a specific standby may
 * actually be promoted RIGHT NOW, or must be held back.
 *
 * This module supplies that predicate: a typed dispatch descriptor plus a pure
 * evaluator that returns `promote_standby` vs `hold_standby` with enumerated
 * reasons. It FAILS TOWARD HOLD — a standby is never promoted on incomplete or
 * stale evidence, mirroring the join barrier that fails toward queueing. A HOLD
 * is the safe default: the run keeps its existing contributors (or escalates to
 * the standby-gated abort path), never silently admits an unqualified node.
 *
 * It is contract-level only: a promote verdict means the standby is ELIGIBLE for
 * promotion; it grants no dispatch, settlement, promise-state, or green-claim
 * authority, and proving a live promotion against a real run remains future work.
 */

export const StandbyDispatchBlocker =
  'blocker.product_promises.standby_dispatch_missing'

export const StandbyDispatchSchemaVersion =
  'openagents.training.marathon_operations.standby_dispatch.v1'

/**
 * Maximum age of a standby's last heartbeat before it is treated as stale. A
 * standby that has not reported liveness recently cannot be trusted to step in;
 * promoting a dead "standby" would stall the collective instead of healing it.
 */
export const MaxStandbyHeartbeatStalenessMs = 120_000

export const StandbyDispatchPublicSafeRefPattern =
  /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/
const PublicSafeRefPattern = StandbyDispatchPublicSafeRefPattern
const PublicSafeRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(PublicSafeRefPattern),
)

const NonNegativeCount = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 1_000_000 }),
)

export const TrainingStandbyDispatch = S.Struct({
  /** The standby contributor being considered for promotion. */
  standbyContributorRef: PublicSafeRef,
  /** The live run the standby would join. */
  runRef: PublicSafeRef,
  /** True only once the standby passed hardware/preflight qualification. */
  qualified: S.Boolean,
  /**
   * True when the standby is banned for the current round under collective
   * failure semantics (psionic#1126). A banned node is never promoted back into
   * the same round it was ejected from.
   */
  bannedForRound: S.Boolean,
  /**
   * True only once the standby finished bootstrapping from a durable seal behind
   * the seal-in-flight join barrier (#4850/#4851) and verified the restored
   * state. Promoting before bootstrap completes admits a node with no model.
   */
  bootstrapSealVerified: S.Boolean,
  /** The window the standby actually bootstrapped from. */
  bootstrapSealWindowRef: PublicSafeRef,
  /** The window the live run is currently sealed at. */
  liveSealedWindowRef: PublicSafeRef,
  /**
   * How many live contributor slots are currently vacant. Promotion is only
   * admissible into an actual vacancy; promoting into a full collective is churn.
   */
  liveVacancyCount: NonNegativeCount,
  /** Age in milliseconds of the standby's most recent heartbeat. */
  lastHeartbeatAgeMs: NonNegativeCount,
})
export type TrainingStandbyDispatch = typeof TrainingStandbyDispatch.Type

export type StandbyDispatchDecision = 'promote_standby' | 'hold_standby'

export type StandbyDispatchReason =
  | 'standby_not_qualified'
  | 'standby_banned_for_round'
  | 'bootstrap_seal_not_verified'
  | 'bootstrap_seal_window_mismatch'
  | 'no_live_vacancy'
  | 'standby_heartbeat_stale'
  | 'dispatch_descriptor_malformed'

export type StandbyDispatchGate = Readonly<{
  authorityBoundary: string
  blockerRef: typeof StandbyDispatchBlocker
  decision: StandbyDispatchDecision
  promotable: boolean
  reasons: ReadonlyArray<StandbyDispatchReason>
  schemaVersion: typeof StandbyDispatchSchemaVersion
}>

const dispatchAuthorityBoundary =
  'Standby dispatch evaluation is a promotion-admissibility predicate only. A promote verdict means a standby is eligible to join a live run; it grants no dispatch, settlement, promise-state, or green-claim authority, and a hold verdict is the safe default — the run keeps its existing contributors or escalates to the standby-gated abort path, never silently admitting an unqualified node.'

export const malformedStandbyDispatchGate = (): StandbyDispatchGate => ({
  authorityBoundary: dispatchAuthorityBoundary,
  blockerRef: StandbyDispatchBlocker,
  decision: 'hold_standby',
  promotable: false,
  reasons: ['dispatch_descriptor_malformed'],
  schemaVersion: StandbyDispatchSchemaVersion,
})

/**
 * Pure promotion-admissibility predicate for an already-decoded dispatch
 * descriptor. A standby may be promoted only when every condition holds; any
 * failing condition routes to HOLD with the failing reasons enumerated.
 */
export const evaluateStandbyDispatch = (
  dispatch: TrainingStandbyDispatch,
): StandbyDispatchGate => {
  const reasons: Array<StandbyDispatchReason> = []

  if (!dispatch.qualified) {
    reasons.push('standby_not_qualified')
  }
  if (dispatch.bannedForRound) {
    reasons.push('standby_banned_for_round')
  }
  if (!dispatch.bootstrapSealVerified) {
    reasons.push('bootstrap_seal_not_verified')
  }
  if (dispatch.bootstrapSealWindowRef !== dispatch.liveSealedWindowRef) {
    reasons.push('bootstrap_seal_window_mismatch')
  }
  if (dispatch.liveVacancyCount < 1) {
    reasons.push('no_live_vacancy')
  }
  if (dispatch.lastHeartbeatAgeMs > MaxStandbyHeartbeatStalenessMs) {
    reasons.push('standby_heartbeat_stale')
  }

  const promotable = reasons.length === 0

  return {
    authorityBoundary: dispatchAuthorityBoundary,
    blockerRef: StandbyDispatchBlocker,
    decision: promotable ? 'promote_standby' : 'hold_standby',
    promotable,
    reasons,
    schemaVersion: StandbyDispatchSchemaVersion,
  }
}

/**
 * Decode an untrusted dispatch descriptor and evaluate it. A descriptor that
 * fails to decode (missing fields, malformed refs, negative counts) yields a
 * HOLD verdict — failing toward keeping the run intact rather than promoting an
 * unverifiable standby.
 */
export const evaluateUntrustedStandbyDispatch = (
  input: unknown,
): StandbyDispatchGate => {
  let decoded: TrainingStandbyDispatch
  try {
    decoded = S.decodeUnknownSync(TrainingStandbyDispatch)(input)
  } catch {
    return malformedStandbyDispatchGate()
  }
  return evaluateStandbyDispatch(decoded)
}
