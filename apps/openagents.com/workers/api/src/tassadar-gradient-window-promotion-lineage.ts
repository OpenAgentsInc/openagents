import { Option, Schema as S } from 'effect'

import {
  TassadarGradientWindowPromotionReceipt,
  tassadarGradientWindowPromotionReceiptRef,
} from './tassadar-gradient-window-promotion-receipt'
import {
  TassadarGradientWindowQuarantineRecord,
  tassadarGradientWindowQuarantineRecordRef,
} from './tassadar-gradient-window-quarantine-record'

/**
 * Promotion lineage-continuity guard for training.public_gradient_windows.v1.
 *
 * Two artifacts already bound the public-gradient-window runtime: the quarantine
 * record (tassadar-gradient-window-quarantine-record.ts), the durable row for an
 * admitted window living in quarantine, and the promoted-window receipt
 * (tassadar-gradient-window-promotion-receipt.ts), emitted once a window clears
 * every gate. Nothing, however, verified that a given promotion receipt is for
 * the SAME window that actually entered quarantine through the front door, with
 * the SAME evidence it was admitted on. Without that continuity check a runtime
 * could emit a promotion receipt for a window that bypassed intake, or whose
 * curated-data / construction / verification / psionic-H1 evidence was swapped
 * between admission and promotion.
 *
 * `verifyTassadarGradientWindowPromotionLineage` closes that loop. It is a pure,
 * TOTAL function over two untrusted inputs: it decodes both, and if either fails
 * to decode it returns a discontinuous decision rather than throwing, so it is
 * safe at the edge of a real runtime. It confirms the window refs match, that
 * both refs derive canonically from that window ref, and that every evidence ref
 * the quarantine record was admitted on is still carried by the promotion
 * receipt. Stage and compiled-core invariants are already structurally
 * guaranteed by the two schemas (literal `quarantined`/`promoted` stages and
 * literal `compiledCoreUnchanged: true`), so a record/receipt that violates them
 * fails to decode and is reported as unparsed.
 *
 * This advances blocker.product_promises.public_gradient_live_window_runtime_missing
 * by building the runtime's admission-to-promotion continuity edge. It does NOT
 * clear that blocker: no live runtime yet drives a real window from quarantine to
 * promotion, and no public window has been accepted, promoted, paid, or settled.
 */

export const TassadarGradientWindowPromotionLineageSchemaVersion =
  'openagents.training.public_gradient_window.promotion_lineage.v1'
export type TassadarGradientWindowPromotionLineageSchemaVersion =
  typeof TassadarGradientWindowPromotionLineageSchemaVersion

const lineageBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.promotion_lineage.${suffix}`

export type TassadarGradientWindowPromotionLineageDecision = Readonly<{
  breakReasonRefs: ReadonlyArray<string>
  continuous: boolean
  publicSafe: true
  receiptRef: string | null
  recordRef: string | null
  schemaVersion: TassadarGradientWindowPromotionLineageSchemaVersion
  windowRef: string | null
}>

const decodeRecord = S.decodeUnknownOption(
  TassadarGradientWindowQuarantineRecord,
)
const decodeReceipt = S.decodeUnknownOption(
  TassadarGradientWindowPromotionReceipt,
)

const discontinuous = (
  breakReasonRefs: ReadonlyArray<string>,
  refs: {
    receiptRef?: string | null
    recordRef?: string | null
    windowRef?: string | null
  } = {},
): TassadarGradientWindowPromotionLineageDecision => ({
  breakReasonRefs: [...new Set(breakReasonRefs)].sort(),
  continuous: false,
  publicSafe: true,
  receiptRef: refs.receiptRef ?? null,
  recordRef: refs.recordRef ?? null,
  schemaVersion: TassadarGradientWindowPromotionLineageSchemaVersion,
  windowRef: refs.windowRef ?? null,
})

const carriesAll = (
  admitted: ReadonlyArray<string>,
  promoted: ReadonlyArray<string>,
): boolean => {
  const carried = new Set(promoted)

  return admitted.every(ref => carried.has(ref))
}

/**
 * Verify that a promoted-window receipt continues the lineage of the quarantine
 * record it claims to descend from.
 *
 * Pure and total: never throws. An unparseable record or receipt, a window-ref
 * mismatch, a non-canonical ref derivation, or dropped admission evidence all
 * yield a discontinuous decision carrying the break reasons.
 */
export const verifyTassadarGradientWindowPromotionLineage = (
  record: unknown,
  receipt: unknown,
): TassadarGradientWindowPromotionLineageDecision => {
  const decodedRecord = decodeRecord(record)
  const decodedReceipt = decodeReceipt(receipt)

  if (Option.isNone(decodedRecord) || Option.isNone(decodedReceipt)) {
    return discontinuous([
      ...(Option.isNone(decodedRecord)
        ? [lineageBlocker('quarantine_record_unparsed')]
        : []),
      ...(Option.isNone(decodedReceipt)
        ? [lineageBlocker('promotion_receipt_unparsed')]
        : []),
    ])
  }

  const admittedRecord = decodedRecord.value
  const promotedReceipt = decodedReceipt.value
  const refs = {
    receiptRef: promotedReceipt.receiptRef,
    recordRef: admittedRecord.recordRef,
    windowRef: admittedRecord.windowRef,
  }
  const reasons: Array<string> = []

  if (admittedRecord.windowRef !== promotedReceipt.windowRef) {
    reasons.push(lineageBlocker('window_ref_mismatch'))
  }
  if (
    admittedRecord.recordRef !==
    tassadarGradientWindowQuarantineRecordRef(admittedRecord.windowRef)
  ) {
    reasons.push(lineageBlocker('quarantine_record_ref_mismatch'))
  }
  if (
    promotedReceipt.receiptRef !==
    tassadarGradientWindowPromotionReceiptRef(promotedReceipt.windowRef)
  ) {
    reasons.push(lineageBlocker('promotion_receipt_ref_mismatch'))
  }
  if (
    !carriesAll(
      admittedRecord.evidenceRefs.curatedDataRefs,
      promotedReceipt.curatedDataRefs,
    )
  ) {
    reasons.push(lineageBlocker('curated_data_refs_not_carried'))
  }
  if (
    !carriesAll(
      admittedRecord.evidenceRefs.constructionReceiptRefs,
      promotedReceipt.constructionReceiptRefs,
    )
  ) {
    reasons.push(lineageBlocker('construction_receipt_refs_not_carried'))
  }
  if (
    !carriesAll(
      admittedRecord.evidenceRefs.verificationReceiptRefs,
      promotedReceipt.verificationReceiptRefs,
    )
  ) {
    reasons.push(lineageBlocker('verification_receipt_refs_not_carried'))
  }
  if (
    !carriesAll(
      admittedRecord.evidenceRefs.psionicH1EvidenceRefs,
      promotedReceipt.sourceRefs,
    )
  ) {
    reasons.push(lineageBlocker('psionic_h1_evidence_not_carried'))
  }

  if (reasons.length > 0) {
    return discontinuous(reasons, refs)
  }

  return {
    breakReasonRefs: [],
    continuous: true,
    publicSafe: true,
    receiptRef: promotedReceipt.receiptRef,
    recordRef: admittedRecord.recordRef,
    schemaVersion: TassadarGradientWindowPromotionLineageSchemaVersion,
    windowRef: admittedRecord.windowRef,
  }
}
