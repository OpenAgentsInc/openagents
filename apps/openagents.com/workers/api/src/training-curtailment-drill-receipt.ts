import { Schema as S } from 'effect'

import {
  CurtailmentDrillBlocker,
  CurtailmentDrillSchemaVersion,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
  TrainingCurtailmentDrill,
  evaluateCurtailmentDrill,
} from './training-curtailment-drill'

type TrainingCurtailmentDrillValue = typeof TrainingCurtailmentDrill.Type

/**
 * Curtailment-drill receipt emitter for training.marathon_operations.v1.
 *
 * The curtailment-drill PREDICATE (training-curtailment-drill.ts) decides whether
 * a recorded scheduled drill PASSED: acked the signal within SLA, halted (shed
 * load) within SLA, sealed the in-flight window on a durable content-addressed
 * checkpoint before halting, and verified resume from that seal. It does not,
 * however, emit the public-safe RECEIPT the live runtime must publish once a
 * drill has actually passed — the artifact a reviewer dereferences to confirm
 * "this scheduled curtailment drill was run, met both SLAs, sealed durably, and
 * recovered". That receipt shape and its derivation are what this module adds.
 *
 * Like the gradient-window promotion-receipt emitter, this REFUSES to fabricate a
 * receipt: it re-runs the drill predicate and throws unless the drill passed, so
 * a receipt can never be minted for an unscheduled, out-of-SLA, unsealed, or
 * unverified-resume drill. The receipt ref is derived deterministically from the
 * drill ref, so the same drill always maps to the same receipt id.
 *
 * It is contract-level only. Emitting a receipt here records that a recorded
 * drill outcome satisfied the curtailment-readiness conditions; it grants no
 * dispatch, settlement, flexible-load-claim, promise-state, or green-claim
 * authority. No scheduled live drill has run, so the public projection's
 * `curtailmentDrillReceiptAvailable` flag stays false — this is the format the
 * runtime will emit once a real drill does.
 */

export const CurtailmentDrillReceiptSchemaVersion =
  'openagents.training.marathon_operations.curtailment_drill_receipt.v1'
export type CurtailmentDrillReceiptSchemaVersion =
  typeof CurtailmentDrillReceiptSchemaVersion

export const CurtailmentDrillReceipt = S.Struct({
  ackLatencyMs: S.Int,
  ackSlaMs: S.Literal(MaxCurtailmentAckLatencyMs),
  authorityBoundary: S.String,
  blockerRef: S.Literal(CurtailmentDrillBlocker),
  drillRef: S.String,
  haltLatencyMs: S.Int,
  haltSlaMs: S.Literal(MaxCurtailmentHaltLatencyMs),
  outcome: S.Literal('drill_passed'),
  predicateSchemaVersion: S.Literal(CurtailmentDrillSchemaVersion),
  publicSafe: S.Literal(true),
  receiptRef: S.String,
  runRef: S.String,
  schemaVersion: S.Literal(CurtailmentDrillReceiptSchemaVersion),
  sourceRefs: S.Array(S.String),
})
export type CurtailmentDrillReceipt = typeof CurtailmentDrillReceipt.Type

export class CurtailmentDrillReceiptUnsafe extends S.TaggedErrorClass<CurtailmentDrillReceiptUnsafe>()(
  'CurtailmentDrillReceiptUnsafe',
  {
    blockerRef: S.Literal(CurtailmentDrillBlocker),
    reason: S.String,
  },
) {}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

/**
 * Derive the canonical, public-safe curtailment-drill receipt ref from a drill
 * ref so the same drill always maps to the same receipt id.
 */
export const curtailmentDrillReceiptRef = (drillRef: string): string =>
  `receipt.public.training.marathon_operations.curtailment_drill.${safeSuffix(
    drillRef,
  )}`

const receiptAuthorityBoundary =
  'A curtailment-drill receipt records that one recorded scheduled curtailment drill acknowledged the signal within SLA, completed a sealed halt within the load-shed SLA, sealed the in-flight window on a durable content-addressed checkpoint before halting, and verified resume. It grants no dispatch, settlement, flexible-load-market, promise-state, or green-claim authority, and is emitted only for a drill the predicate scored as passed.'

const receiptSourceRefs: ReadonlyArray<string> = [
  'apps/openagents.com/workers/api/src/training-curtailment-drill.ts',
  'apps/openagents.com/workers/api/src/training-curtailment-drill-receipt.ts',
  'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
]

/**
 * Build the public-safe curtailment-drill receipt from a drill descriptor.
 *
 * Re-runs the drill predicate and throws CurtailmentDrillReceiptUnsafe unless the
 * drill PASSED — a receipt is never emitted for an unscheduled, out-of-SLA,
 * unsealed, or unverified-resume drill, so this cannot manufacture a curtailment
 * readiness claim.
 */
export const buildCurtailmentDrillReceipt = (
  drill: TrainingCurtailmentDrillValue,
): CurtailmentDrillReceipt => {
  const gate = evaluateCurtailmentDrill(drill)
  if (!gate.passed) {
    throw new CurtailmentDrillReceiptUnsafe({
      blockerRef: CurtailmentDrillBlocker,
      reason: `A curtailment-drill receipt may only be emitted for a passed drill; this drill is ${gate.decision} (${gate.reasons.join(', ')}).`,
    })
  }

  return CurtailmentDrillReceipt.make({
    ackLatencyMs: drill.ackLatencyMs,
    ackSlaMs: MaxCurtailmentAckLatencyMs,
    authorityBoundary: receiptAuthorityBoundary,
    blockerRef: CurtailmentDrillBlocker,
    drillRef: drill.drillRef,
    haltLatencyMs: drill.haltLatencyMs,
    haltSlaMs: MaxCurtailmentHaltLatencyMs,
    outcome: 'drill_passed',
    predicateSchemaVersion: CurtailmentDrillSchemaVersion,
    publicSafe: true,
    receiptRef: curtailmentDrillReceiptRef(drill.drillRef),
    runRef: drill.runRef,
    schemaVersion: CurtailmentDrillReceiptSchemaVersion,
    sourceRefs: receiptSourceRefs,
  })
}

/**
 * Decode an untrusted drill descriptor and build its receipt. A descriptor that
 * fails to decode, or a drill that did not pass, throws
 * CurtailmentDrillReceiptUnsafe — failing toward no-receipt rather than minting
 * one for an unverifiable drill.
 */
export const buildUntrustedCurtailmentDrillReceipt = (
  input: unknown,
): CurtailmentDrillReceipt => {
  let decoded: TrainingCurtailmentDrillValue
  try {
    decoded = S.decodeUnknownSync(TrainingCurtailmentDrill)(input)
  } catch {
    throw new CurtailmentDrillReceiptUnsafe({
      blockerRef: CurtailmentDrillBlocker,
      reason:
        'A curtailment-drill receipt cannot be built from a malformed drill descriptor.',
    })
  }
  return buildCurtailmentDrillReceipt(decoded)
}
