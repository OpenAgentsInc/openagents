import { Schema as S } from 'effect'

import type { TassadarGradientWindowPromotionProjection } from './tassadar-gradient-window-regime'

/**
 * Promoted-window receipt emitter for training.public_gradient_windows.v1.
 *
 * The gradient-window regime (tassadar-gradient-window-regime.ts) decides
 * whether a candidate update MAY promote. It does not, however, emit the
 * public-safe RECEIPT that the live runtime must publish once a window has
 * actually promoted — the artifact a reviewer or contributor would
 * dereference to confirm "this public window was accepted, recomputed,
 * replicated, canaried, and promoted". That receipt shape and its derivation
 * are the substrate this module supplies.
 *
 * This emitter does NOT assert that any real public window has promoted: it
 * only converts a fully-passed projection into a canonical receipt and refuses
 * to fabricate one from a projection that has not cleared every gate. No real
 * contributor window has been accepted, promoted, paid, or settled today, so
 * the planned promise stays planned; this is the receipt format the runtime
 * will emit once one does.
 */

export const TassadarGradientWindowPromotionReceiptSchemaVersion =
  'openagents.training.public_gradient_window.promotion_receipt.v1'
export type TassadarGradientWindowPromotionReceiptSchemaVersion =
  typeof TassadarGradientWindowPromotionReceiptSchemaVersion

export const TassadarGradientWindowPromotionReceipt = S.Struct({
  authority: S.Struct({
    canonicalCheckpointMutationAllowed: S.Literal(true),
    compiledCoreGradientMutationAllowed: S.Literal(false),
    directSubmissionMutationAllowed: S.Literal(false),
    quarantineCheckpointMutationAllowed: S.Boolean,
    settlementMutationAllowed: S.Literal(false),
  }),
  authorityBoundary: S.String,
  canaryReceiptRefs: S.Array(S.String),
  compiledCoreUnchanged: S.Literal(true),
  constructionReceiptRefs: S.Array(S.String),
  curatedDataRefs: S.Array(S.String),
  gateRef: S.String,
  promotionDecisionRefs: S.Array(S.String),
  publicSafe: S.Literal(true),
  receiptRef: S.String,
  recomputeReceiptRefs: S.Array(S.String),
  replicationReceiptRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  schemaVersion: S.Literal(
    TassadarGradientWindowPromotionReceiptSchemaVersion,
  ),
  settlementEligible: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  stage: S.Literal('promoted'),
  verificationReceiptRefs: S.Array(S.String),
  windowRef: S.String,
})
export type TassadarGradientWindowPromotionReceipt =
  typeof TassadarGradientWindowPromotionReceipt.Type

export class TassadarGradientWindowPromotionReceiptUnsafe extends S.TaggedErrorClass<TassadarGradientWindowPromotionReceiptUnsafe>()(
  'TassadarGradientWindowPromotionReceiptUnsafe',
  {
    blockerRefs: S.Array(S.String),
    reason: S.String,
  },
) {}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

/**
 * Derive the canonical, public-safe promoted-window receipt ref from a window
 * ref so the same promoted window always maps to the same receipt id.
 */
export const tassadarGradientWindowPromotionReceiptRef = (
  windowRef: string,
): string =>
  `receipt.public.tassadar_gradient_window.promoted.${safeSuffix(windowRef)}`

/**
 * Build the public-safe promoted-window receipt from a regime projection.
 *
 * Refuses (throws TassadarGradientWindowPromotionReceiptUnsafe) unless the
 * projection has cleared every gate: it must be at the `promoted` stage with
 * promotion allowed, the compiled core unchanged, no outstanding blockers, and
 * non-empty recompute, replication, canary, promotion-decision, and rollback
 * receipt refs. A receipt is never emitted for a window that has not actually
 * promoted, so this cannot manufacture a promoted-window claim.
 */
export const buildTassadarGradientWindowPromotionReceipt = (
  projection: TassadarGradientWindowPromotionProjection,
): TassadarGradientWindowPromotionReceipt => {
  if (projection.stage !== 'promoted' || !projection.promotionAllowed) {
    throw new TassadarGradientWindowPromotionReceiptUnsafe({
      blockerRefs: projection.blockerRefs,
      reason:
        'A promoted-window receipt may only be emitted for a projection at the promoted stage with promotion allowed.',
    })
  }

  if (!projection.compiledCoreUnchanged) {
    throw new TassadarGradientWindowPromotionReceiptUnsafe({
      blockerRefs: projection.blockerRefs,
      reason:
        'A promoted-window receipt requires the compiled exact core to be unchanged across the window.',
    })
  }

  if (projection.blockerRefs.length > 0) {
    throw new TassadarGradientWindowPromotionReceiptUnsafe({
      blockerRefs: projection.blockerRefs,
      reason:
        'A promoted-window receipt requires zero outstanding gradient-window blockers.',
    })
  }

  const lineage: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = [
    ['recomputeReceiptRefs', projection.recomputeReceiptRefs],
    ['replicationReceiptRefs', projection.replicationReceiptRefs],
    ['canaryReceiptRefs', projection.canaryReceiptRefs],
    ['promotionDecisionRefs', projection.promotionDecisionRefs],
    ['rollbackRefs', projection.rollbackRefs],
  ]
  const missing = lineage.find(([, refs]) => refs.length === 0)
  if (missing !== undefined) {
    throw new TassadarGradientWindowPromotionReceiptUnsafe({
      blockerRefs: projection.blockerRefs,
      reason:
        `A promoted-window receipt requires non-empty ${missing[0]}; a promoted window must carry full recompute, replication, canary, promotion-decision, and rollback lineage.`,
    })
  }

  return TassadarGradientWindowPromotionReceipt.make({
    authority: {
      canonicalCheckpointMutationAllowed: true,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      quarantineCheckpointMutationAllowed:
        projection.authority.quarantineCheckpointMutationAllowed,
      settlementMutationAllowed: false,
    },
    authorityBoundary:
      'A promoted-window receipt records that one bounded public training window cleared quarantine, recompute, replication, canary, and the explicit promotion gate with the compiled exact core unchanged. It grants no settlement, aggregation, compiled-core-gradient, or direct-submission authority, and settlement remains a separate gated step.',
    canaryReceiptRefs: projection.canaryReceiptRefs,
    compiledCoreUnchanged: true,
    constructionReceiptRefs: projection.constructionReceiptRefs,
    curatedDataRefs: projection.curatedDataRefs,
    gateRef: projection.gateRef,
    promotionDecisionRefs: projection.promotionDecisionRefs,
    publicSafe: true,
    receiptRef: tassadarGradientWindowPromotionReceiptRef(
      projection.windowRef,
    ),
    recomputeReceiptRefs: projection.recomputeReceiptRefs,
    replicationReceiptRefs: projection.replicationReceiptRefs,
    rollbackRefs: projection.rollbackRefs,
    schemaVersion: TassadarGradientWindowPromotionReceiptSchemaVersion,
    settlementEligible: projection.settlementEligible,
    settlementReceiptRefs: projection.settlementReceiptRefs,
    sourceRefs: projection.sourceRefs,
    stage: 'promoted',
    verificationReceiptRefs: projection.verificationReceiptRefs,
    windowRef: projection.windowRef,
  })
}
