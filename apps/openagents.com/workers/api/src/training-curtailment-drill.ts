import { Schema as S } from 'effect'

/**
 * Scheduled curtailment-drill outcome contract for
 * training.marathon_operations.v1.
 *
 * Marathon discipline requires proving that a long run can respond to a grid
 * curtailment / demand-response signal: acknowledge the signal promptly, ramp
 * down within the response SLA, seal the in-flight window on a durable
 * content-addressed checkpoint BEFORE halting (so no progress is lost), and then
 * resume from that sealed checkpoint with verified state. The durable seal
 * (#4849 + the durability predicate) and the bootstrap-from-seal resume path
 * (#4850/#4851) already exist; what is still missing is the predicate that
 * decides whether a SCHEDULED curtailment drill actually PASSED.
 *
 * This module supplies that predicate: a typed drill-outcome descriptor plus a
 * pure evaluator that returns `drill_passed` vs `drill_incomplete` with
 * enumerated reasons. It FAILS TOWARD INCOMPLETE — a drill is never scored as
 * passed on missing or out-of-SLA evidence, mirroring the seal-in-flight join
 * barrier that fails toward queueing. A drill that is not a scheduled drill, that
 * blew an SLA, that halted without a durable seal, or that could not verify its
 * resume is INCOMPLETE, never a pass.
 *
 * It is contract-level only: a `drill_passed` verdict means the recorded drill
 * outcome satisfies the curtailment-readiness conditions; it grants no dispatch,
 * settlement, promise-state, or green-claim authority, and proving it against a
 * live grid curtailment event on a real run remains future work.
 */

export const CurtailmentDrillBlocker =
  'blocker.product_promises.curtailment_drill_missing'

export const CurtailmentDrillSchemaVersion =
  'openagents.training.marathon_operations.curtailment_drill.v1'

/**
 * Maximum latency from curtailment signal to acknowledgement. A drill that does
 * not acknowledge promptly cannot be trusted to honor a real demand-response
 * obligation, so a slow ack fails the drill.
 */
export const MaxCurtailmentAckLatencyMs = 30_000

/**
 * Maximum latency from curtailment signal to a completed, sealed halt. This is
 * the load-shed response SLA the drill exists to prove. Exceeding it means the
 * run would still be drawing power past the window a grid operator expects.
 */
export const MaxCurtailmentHaltLatencyMs = 300_000

export const CurtailmentDrillPublicSafeRefPattern =
  /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/
const PublicSafeRefPattern = CurtailmentDrillPublicSafeRefPattern
const PublicSafeRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(PublicSafeRefPattern),
)

const NonNegativeMs = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 86_400_000 }),
)

export const TrainingCurtailmentDrill = S.Struct({
  /** The recorded drill exercise. */
  drillRef: PublicSafeRef,
  /** The live run the drill was exercised against. */
  runRef: PublicSafeRef,
  /**
   * True only when this was a SCHEDULED curtailment drill. The promise is about
   * a planned, rehearsed curtailment exercise; an unscheduled/ad-hoc trip does
   * not satisfy the drill blocker even if it happened to behave well.
   */
  scheduled: S.Boolean,
  /** True once the run acknowledged the curtailment signal. */
  signalAcknowledged: S.Boolean,
  /** Latency in ms from curtailment signal to acknowledgement. */
  ackLatencyMs: NonNegativeMs,
  /** True once the run completed a clean halt (load shed) in response. */
  haltCompleted: S.Boolean,
  /** Latency in ms from curtailment signal to completed, sealed halt. */
  haltLatencyMs: NonNegativeMs,
  /**
   * True only when the in-flight window sealed on a durable content-addressed
   * checkpoint BEFORE the halt completed (per the durable-seal predicate).
   * Halting without a durable seal discards progress.
   */
  durableCheckpointSealed: S.Boolean,
  /**
   * True only when the run resumed from the sealed checkpoint after the
   * curtailment window and verified the restored state. A drill that cannot
   * prove resume has proven a halt, not a recoverable curtailment.
   */
  resumeVerified: S.Boolean,
})
export type TrainingCurtailmentDrill = typeof TrainingCurtailmentDrill.Type

export type CurtailmentDrillDecision = 'drill_passed' | 'drill_incomplete'

export type CurtailmentDrillReason =
  | 'drill_not_scheduled'
  | 'curtailment_signal_not_acknowledged'
  | 'ack_latency_exceeded'
  | 'halt_not_completed'
  | 'halt_latency_exceeded'
  | 'durable_checkpoint_not_sealed'
  | 'resume_not_verified'
  | 'drill_descriptor_malformed'

export type CurtailmentDrillGate = Readonly<{
  authorityBoundary: string
  blockerRef: typeof CurtailmentDrillBlocker
  decision: CurtailmentDrillDecision
  passed: boolean
  reasons: ReadonlyArray<CurtailmentDrillReason>
  schemaVersion: typeof CurtailmentDrillSchemaVersion
}>

const drillAuthorityBoundary =
  'Curtailment-drill evaluation is a drill-outcome predicate only. A passed verdict means a recorded scheduled curtailment drill met the acknowledge/halt SLAs, sealed durably before halting, and verified resume; it grants no dispatch, settlement, promise-state, or green-claim authority, and an incomplete verdict is the safe default — a drill is never scored as passed on missing or out-of-SLA evidence.'

export const malformedCurtailmentDrillGate = (): CurtailmentDrillGate => ({
  authorityBoundary: drillAuthorityBoundary,
  blockerRef: CurtailmentDrillBlocker,
  decision: 'drill_incomplete',
  passed: false,
  reasons: ['drill_descriptor_malformed'],
  schemaVersion: CurtailmentDrillSchemaVersion,
})

/**
 * Pure outcome predicate for an already-decoded drill descriptor. A drill passes
 * only when every condition holds; any failing condition routes to INCOMPLETE
 * with the failing reasons enumerated.
 */
export const evaluateCurtailmentDrill = (
  drill: TrainingCurtailmentDrill,
): CurtailmentDrillGate => {
  const reasons: Array<CurtailmentDrillReason> = []

  if (!drill.scheduled) {
    reasons.push('drill_not_scheduled')
  }
  if (!drill.signalAcknowledged) {
    reasons.push('curtailment_signal_not_acknowledged')
  } else if (drill.ackLatencyMs > MaxCurtailmentAckLatencyMs) {
    reasons.push('ack_latency_exceeded')
  }
  if (!drill.haltCompleted) {
    reasons.push('halt_not_completed')
  } else if (drill.haltLatencyMs > MaxCurtailmentHaltLatencyMs) {
    reasons.push('halt_latency_exceeded')
  }
  if (!drill.durableCheckpointSealed) {
    reasons.push('durable_checkpoint_not_sealed')
  }
  if (!drill.resumeVerified) {
    reasons.push('resume_not_verified')
  }

  const passed = reasons.length === 0

  return {
    authorityBoundary: drillAuthorityBoundary,
    blockerRef: CurtailmentDrillBlocker,
    decision: passed ? 'drill_passed' : 'drill_incomplete',
    passed,
    reasons,
    schemaVersion: CurtailmentDrillSchemaVersion,
  }
}

/**
 * Decode an untrusted drill descriptor and evaluate it. A descriptor that fails
 * to decode (missing fields, malformed refs, negative or out-of-range latencies)
 * yields an INCOMPLETE verdict — failing toward not-yet-proven rather than
 * scoring an unverifiable drill as a pass.
 */
export const evaluateUntrustedCurtailmentDrill = (
  input: unknown,
): CurtailmentDrillGate => {
  let decoded: TrainingCurtailmentDrill
  try {
    decoded = S.decodeUnknownSync(TrainingCurtailmentDrill)(input)
  } catch {
    return malformedCurtailmentDrillGate()
  }
  return evaluateCurtailmentDrill(decoded)
}
