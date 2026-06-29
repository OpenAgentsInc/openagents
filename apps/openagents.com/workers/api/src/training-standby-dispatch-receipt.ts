import { Schema as S } from 'effect'

import {
  StandbyDispatchBlocker,
  StandbyDispatchSchemaVersion,
  TrainingStandbyDispatch,
  evaluateStandbyDispatch,
} from './training-standby-dispatch'

type TrainingStandbyDispatchValue = typeof TrainingStandbyDispatch.Type

/**
 * Standby-promotion receipt emitter for training.marathon_operations.v1.
 *
 * The standby-dispatch PREDICATE (training-standby-dispatch.ts) decides whether a
 * specific pre-warmed standby Pylon may be PROMOTED into a live collective right
 * now: it is qualified, not banned for the round, bootstrapped-and-verified from
 * the live sealed window behind the join barrier, filling a real vacancy, and
 * heartbeating recently. It does not, however, emit the public-safe RECEIPT the
 * live runtime must publish once a standby has actually been promoted — the
 * artifact a reviewer dereferences to confirm "this standby was admitted into this
 * run, against this sealed window, into a real vacancy". That receipt shape and
 * its derivation are what this module adds.
 *
 * Like the curtailment-drill and gradient-window promotion-receipt emitters, this
 * REFUSES to fabricate a receipt: it re-runs the dispatch predicate and throws
 * unless the standby is promotable, so a receipt can never be minted for an
 * unqualified, banned, unbootstrapped, window-mismatched, no-vacancy, or stale
 * standby. The receipt ref is derived deterministically from the run ref and the
 * standby contributor ref, so the same promotion always maps to the same id.
 *
 * It is contract-level only. Emitting a receipt here records that a recorded
 * promotion satisfied the admissibility conditions; it grants no dispatch,
 * settlement, promise-state, or green-claim authority. No live standby has been
 * promoted into a real run, so the public projection's
 * `livePromotionReceiptAvailable` / `receiptBackedPromotionAvailable` flags stay
 * false — this is the format the runtime will emit once a real promotion happens.
 */

export const StandbyDispatchReceiptSchemaVersion =
  'openagents.training.marathon_operations.standby_dispatch_receipt.v1'
export type StandbyDispatchReceiptSchemaVersion =
  typeof StandbyDispatchReceiptSchemaVersion

export const StandbyDispatchReceipt = S.Struct({
  authorityBoundary: S.String,
  blockerRef: S.Literal(StandbyDispatchBlocker),
  outcome: S.Literal('promote_standby'),
  predicateSchemaVersion: S.Literal(StandbyDispatchSchemaVersion),
  promotedIntoWindowRef: S.String,
  publicSafe: S.Literal(true),
  receiptRef: S.String,
  runRef: S.String,
  schemaVersion: S.Literal(StandbyDispatchReceiptSchemaVersion),
  sourceRefs: S.Array(S.String),
  standbyContributorRef: S.String,
})
export type StandbyDispatchReceipt = typeof StandbyDispatchReceipt.Type

export class StandbyDispatchReceiptUnsafe extends S.TaggedErrorClass<StandbyDispatchReceiptUnsafe>()(
  'StandbyDispatchReceiptUnsafe',
  {
    blockerRef: S.Literal(StandbyDispatchBlocker),
    reason: S.String,
  },
) {}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

/**
 * Derive the canonical, public-safe standby-promotion receipt ref from the run
 * ref and standby contributor ref so the same promotion always maps to the same
 * receipt id.
 */
export const standbyDispatchReceiptRef = (
  runRef: string,
  standbyContributorRef: string,
): string =>
  `receipt.public.training.marathon_operations.standby_dispatch.${safeSuffix(
    runRef,
  )}.${safeSuffix(standbyContributorRef)}`

const receiptAuthorityBoundary =
  'A standby-promotion receipt records that one recorded standby was admissible for promotion into a live run: qualified, not banned for the round, bootstrapped-and-verified from the live sealed window behind the join barrier, filling a real vacancy, and heartbeating recently. It grants no dispatch, settlement, promise-state, or green-claim authority, and is emitted only for a standby the predicate scored as promotable.'

const receiptSourceRefs: ReadonlyArray<string> = [
  'apps/openagents.com/workers/api/src/training-standby-dispatch.ts',
  'apps/openagents.com/workers/api/src/training-standby-dispatch-receipt.ts',
  'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
]

/**
 * Build the public-safe standby-promotion receipt from a dispatch descriptor.
 *
 * Re-runs the dispatch predicate and throws StandbyDispatchReceiptUnsafe unless
 * the standby is PROMOTABLE — a receipt is never emitted for an unqualified,
 * banned, unbootstrapped, window-mismatched, no-vacancy, or stale standby, so this
 * cannot manufacture a promotion claim.
 */
export const buildStandbyDispatchReceipt = (
  dispatch: TrainingStandbyDispatchValue,
): StandbyDispatchReceipt => {
  const gate = evaluateStandbyDispatch(dispatch)
  if (!gate.promotable) {
    throw new StandbyDispatchReceiptUnsafe({
      blockerRef: StandbyDispatchBlocker,
      reason: `A standby-promotion receipt may only be emitted for a promotable standby; this dispatch is ${gate.decision} (${gate.reasons.join(', ')}).`,
    })
  }

  return StandbyDispatchReceipt.make({
    authorityBoundary: receiptAuthorityBoundary,
    blockerRef: StandbyDispatchBlocker,
    outcome: 'promote_standby',
    predicateSchemaVersion: StandbyDispatchSchemaVersion,
    promotedIntoWindowRef: dispatch.liveSealedWindowRef,
    publicSafe: true,
    receiptRef: standbyDispatchReceiptRef(
      dispatch.runRef,
      dispatch.standbyContributorRef,
    ),
    runRef: dispatch.runRef,
    schemaVersion: StandbyDispatchReceiptSchemaVersion,
    sourceRefs: receiptSourceRefs,
    standbyContributorRef: dispatch.standbyContributorRef,
  })
}

/**
 * Decode an untrusted dispatch descriptor and build its receipt. A descriptor
 * that fails to decode, or a standby that is not promotable, throws
 * StandbyDispatchReceiptUnsafe — failing toward no-receipt rather than minting one
 * for an unverifiable promotion.
 */
export const buildUntrustedStandbyDispatchReceipt = (
  input: unknown,
): StandbyDispatchReceipt => {
  let decoded: TrainingStandbyDispatchValue
  try {
    decoded = S.decodeUnknownSync(TrainingStandbyDispatch)(input)
  } catch {
    throw new StandbyDispatchReceiptUnsafe({
      blockerRef: StandbyDispatchBlocker,
      reason:
        'A standby-promotion receipt cannot be built from a malformed dispatch descriptor.',
    })
  }
  return buildStandbyDispatchReceipt(decoded)
}
