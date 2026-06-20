import { Option, Schema as S } from 'effect'

import {
  TassadarGradientWindowPromotionReceipt,
  tassadarGradientWindowPromotionReceiptRef,
} from './tassadar-gradient-window-promotion-receipt'

/**
 * Standalone promoted-window receipt verifier for
 * training.public_gradient_windows.v1.
 *
 * The receipt emitter (tassadar-gradient-window-promotion-receipt.ts) can BUILD
 * a public-safe receipt from a fully-passed regime projection, and the lineage
 * guard (tassadar-gradient-window-promotion-lineage.ts) checks that a receipt
 * continues the lineage of the quarantine record it descends from. But a public
 * consumer who dereferences a published receipt from a feed has neither the
 * source projection nor the quarantine record in hand — only the receipt bytes.
 * Nothing let such a reader confirm, without trusting the emitter, that an
 * untrusted read-back receipt is actually a legitimate promoted-window receipt.
 *
 * `verifyTassadarGradientWindowPromotionReceipt` closes that gap. It is a pure,
 * TOTAL function over a single untrusted input: it decodes the receipt, and if
 * it fails to decode it returns an invalid decision rather than throwing, so it
 * is safe at the edge of a real public feed. It re-checks, on the read-back
 * receipt, the same invariants the emitter enforced at build time — the receipt
 * ref derives canonically from the window ref, and the recompute, replication,
 * canary, promotion-decision, and rollback lineage arrays are all non-empty —
 * plus a public-safety scan. The `promoted` stage, `compiledCoreUnchanged: true`
 * literal, and `publicSafe: true` literal are structurally guaranteed by the
 * schema, so a receipt that violates them fails to decode and is reported as
 * unparsed.
 *
 * This advances blocker.product_promises.public_gradient_promoted_window_receipts_missing
 * by building the read-side validator a public receipt feed needs. It does NOT
 * clear that blocker: no live runtime emits a real receipt, no route serves one,
 * and no public window has been accepted, promoted, paid, or settled.
 */

export const TassadarGradientWindowPromotionReceiptVerificationSchemaVersion =
  'openagents.training.public_gradient_window.promotion_receipt_verification.v1'
export type TassadarGradientWindowPromotionReceiptVerificationSchemaVersion =
  typeof TassadarGradientWindowPromotionReceiptVerificationSchemaVersion

const verificationBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.promotion_receipt_verification.${suffix}`

export type TassadarGradientWindowPromotionReceiptVerification = Readonly<{
  invalidReasonRefs: ReadonlyArray<string>
  publicSafe: true
  receiptRef: string | null
  schemaVersion: TassadarGradientWindowPromotionReceiptVerificationSchemaVersion
  settlementEligible: boolean
  valid: boolean
  windowRef: string | null
}>

const decodeReceipt = S.decodeUnknownOption(
  TassadarGradientWindowPromotionReceipt,
)

const unsafeReceiptPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer|cookie|email[_-]?(address|body|raw)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|mnemonic|path|private|seed))/i

const invalid = (
  invalidReasonRefs: ReadonlyArray<string>,
  refs: {
    receiptRef?: string | null
    settlementEligible?: boolean
    windowRef?: string | null
  } = {},
): TassadarGradientWindowPromotionReceiptVerification => ({
  invalidReasonRefs: [...new Set(invalidReasonRefs)].sort(),
  publicSafe: true,
  receiptRef: refs.receiptRef ?? null,
  schemaVersion: TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
  settlementEligible: refs.settlementEligible ?? false,
  valid: false,
  windowRef: refs.windowRef ?? null,
})

/**
 * Verify that an untrusted, read-back receipt is a legitimate promoted-window
 * receipt.
 *
 * Pure and total: never throws. An unparseable receipt, a non-canonical receipt
 * ref, an empty window ref, missing recompute/replication/canary/
 * promotion-decision/rollback lineage, or unsafe material all yield an invalid
 * decision carrying the reasons.
 */
export const verifyTassadarGradientWindowPromotionReceipt = (
  receipt: unknown,
): TassadarGradientWindowPromotionReceiptVerification => {
  const decoded = decodeReceipt(receipt)
  if (Option.isNone(decoded)) {
    return invalid([verificationBlocker('promotion_receipt_unparsed')])
  }

  const value = decoded.value
  const refs = {
    receiptRef: value.receiptRef,
    settlementEligible: value.settlementEligible,
    windowRef: value.windowRef,
  }
  const reasons: Array<string> = []

  if (value.windowRef.trim().length === 0) {
    reasons.push(verificationBlocker('window_ref_missing'))
  }
  if (
    value.receiptRef !==
    tassadarGradientWindowPromotionReceiptRef(value.windowRef)
  ) {
    reasons.push(verificationBlocker('receipt_ref_mismatch'))
  }

  const lineage: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = [
    ['recompute_receipt_refs_missing', value.recomputeReceiptRefs],
    ['replication_receipt_refs_missing', value.replicationReceiptRefs],
    ['canary_receipt_refs_missing', value.canaryReceiptRefs],
    ['promotion_decision_refs_missing', value.promotionDecisionRefs],
    ['rollback_refs_missing', value.rollbackRefs],
  ]
  for (const [suffix, lineageRefs] of lineage) {
    if (lineageRefs.length === 0) {
      reasons.push(verificationBlocker(suffix))
    }
  }

  if (unsafeReceiptPattern.test(JSON.stringify(value))) {
    reasons.push(verificationBlocker('unsafe_material'))
  }

  if (reasons.length > 0) {
    return invalid(reasons, refs)
  }

  return {
    invalidReasonRefs: [],
    publicSafe: true,
    receiptRef: value.receiptRef,
    schemaVersion:
      TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
    settlementEligible: value.settlementEligible,
    valid: true,
    windowRef: value.windowRef,
  }
}
