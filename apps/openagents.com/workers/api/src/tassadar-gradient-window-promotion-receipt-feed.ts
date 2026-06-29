import { verifyTassadarGradientWindowPromotionReceipt } from './tassadar-gradient-window-promotion-receipt-verify'

/**
 * Promoted-window receipt feed builder for training.public_gradient_windows.v1.
 *
 * The receipt emitter (tassadar-gradient-window-promotion-receipt.ts) BUILDS one
 * receipt, the read-side verifier (tassadar-gradient-window-promotion-receipt-verify.ts)
 * validates ONE untrusted read-back receipt, and the lineage guard
 * (tassadar-gradient-window-promotion-lineage.ts) checks one receipt against the
 * quarantine record it descends from. But a public receipt route does not serve a
 * single receipt — it serves a COLLECTION. Nothing turned an untrusted list of
 * read-back receipts into the one public-safe, verified, de-duplicated, ordered
 * feed such a route would publish.
 *
 * `buildTassadarGradientWindowPromotionReceiptFeed` closes that gap. It is a pure,
 * TOTAL function over an array of untrusted receipts: it runs each through the
 * read-side verifier, admits only receipts that pass every invariant, drops
 * duplicates (same canonical receipt ref) keeping the first, counts and explains
 * every rejection, and returns the accepted entries deterministically ordered by
 * receipt ref. It never throws, so it is safe at the edge of a real public feed.
 *
 * This advances blocker.product_promises.public_gradient_promoted_window_receipts_missing
 * by building the aggregation layer a public receipt feed needs. It does NOT clear
 * that blocker: no live runtime emits a real receipt, no route serves this feed,
 * and no public window has been accepted, promoted, paid, or settled — so a real
 * feed is empty.
 */

export const TassadarGradientWindowPromotionReceiptFeedSchemaVersion =
  'openagents.training.public_gradient_window.promotion_receipt_feed.v1'
export type TassadarGradientWindowPromotionReceiptFeedSchemaVersion =
  typeof TassadarGradientWindowPromotionReceiptFeedSchemaVersion

const feedBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.promotion_receipt_feed.${suffix}`

export type TassadarGradientWindowPromotionReceiptFeedEntry = Readonly<{
  receiptRef: string
  settlementEligible: boolean
  windowRef: string
}>

export type TassadarGradientWindowPromotionReceiptFeed = Readonly<{
  acceptedEntries: ReadonlyArray<TassadarGradientWindowPromotionReceiptFeedEntry>
  acceptedReceiptCount: number
  publicSafe: true
  rejectedReceiptCount: number
  rejectionReasonRefs: ReadonlyArray<string>
  schemaVersion: TassadarGradientWindowPromotionReceiptFeedSchemaVersion
  settlementEligibleReceiptCount: number
}>

/**
 * Aggregate an untrusted list of read-back receipts into a public-safe feed.
 *
 * Pure and total: never throws. Each receipt is verified by the read-side
 * verifier; invalid receipts and duplicate receipt refs are dropped and counted,
 * carrying their reasons, and the accepted entries are returned ordered by
 * receipt ref.
 */
export const buildTassadarGradientWindowPromotionReceiptFeed = (
  receipts: ReadonlyArray<unknown>,
): TassadarGradientWindowPromotionReceiptFeed => {
  const acceptedByRef = new Map<
    string,
    TassadarGradientWindowPromotionReceiptFeedEntry
  >()
  const rejectionReasons: Array<string> = []
  let rejectedReceiptCount = 0

  for (const receipt of receipts) {
    const verification = verifyTassadarGradientWindowPromotionReceipt(receipt)

    if (
      !verification.valid ||
      verification.receiptRef === null ||
      verification.windowRef === null
    ) {
      rejectedReceiptCount += 1
      rejectionReasons.push(...verification.invalidReasonRefs)
      continue
    }

    if (acceptedByRef.has(verification.receiptRef)) {
      rejectedReceiptCount += 1
      rejectionReasons.push(feedBlocker('duplicate_receipt_ref'))
      continue
    }

    acceptedByRef.set(verification.receiptRef, {
      receiptRef: verification.receiptRef,
      settlementEligible: verification.settlementEligible,
      windowRef: verification.windowRef,
    })
  }

  const acceptedEntries = [...acceptedByRef.values()].sort((a, b) =>
    a.receiptRef < b.receiptRef ? -1 : a.receiptRef > b.receiptRef ? 1 : 0,
  )

  return {
    acceptedEntries,
    acceptedReceiptCount: acceptedEntries.length,
    publicSafe: true,
    rejectedReceiptCount,
    rejectionReasonRefs: [...new Set(rejectionReasons)].sort(),
    schemaVersion: TassadarGradientWindowPromotionReceiptFeedSchemaVersion,
    settlementEligibleReceiptCount: acceptedEntries.filter(
      entry => entry.settlementEligible,
    ).length,
  }
}
