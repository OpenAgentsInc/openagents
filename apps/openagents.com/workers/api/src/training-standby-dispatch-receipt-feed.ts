import { Schema as S } from 'effect'

import { StandbyDispatchBlocker } from './training-standby-dispatch'
import {
  StandbyDispatchReceipt,
  type StandbyDispatchReceipt as StandbyDispatchReceiptValue,
} from './training-standby-dispatch-receipt'
import { verifyStandbyDispatchReceipt } from './training-standby-dispatch-receipt-verifier'

/**
 * Standby-promotion receipt FEED builder for training.marathon_operations.v1.
 *
 * The receipt emitter (training-standby-dispatch-receipt.ts) BUILDS one receipt,
 * and the read-side verifier (training-standby-dispatch-receipt-verifier.ts)
 * validates ONE untrusted read-back receipt. But a public receipt route does not
 * serve a single receipt — it serves a COLLECTION. Nothing yet turned an untrusted
 * list of published promotion receipts into the one public-safe, verified,
 * de-duplicated, ordered feed such a route would publish. The curtailment-drill
 * lane already has this aggregation layer
 * (training-curtailment-drill-receipt-feed.ts); this module supplies the matching
 * layer for the standby-dispatch lane, mirroring that feed and the gradient-window
 * promotion-receipt feed (tassadar-gradient-window-promotion-receipt-feed.ts).
 *
 * `buildStandbyDispatchReceiptFeed` is a pure, TOTAL function over an array of
 * untrusted receipts: it decodes and runs each through the read-side verifier,
 * admits only receipts that pass every authenticity invariant, drops duplicates
 * (same canonical receipt ref) keeping the first, counts and explains every
 * rejection, and returns the accepted entries deterministically ordered by receipt
 * ref. It never throws, so it is safe at the edge of a real public feed.
 *
 * This advances blocker.product_promises.standby_dispatch_missing by building the
 * aggregation layer a public promotion-receipt feed needs. It does NOT clear that
 * blocker: no live standby has been promoted into a real run, no runtime emits a
 * real receipt, and no route serves this feed — so a real feed is empty. It grants
 * no dispatch, settlement, promise-state, or green-claim authority.
 */

export const StandbyDispatchReceiptFeedSchemaVersion =
  'openagents.training.marathon_operations.standby_dispatch_receipt_feed.v1'
export type StandbyDispatchReceiptFeedSchemaVersion =
  typeof StandbyDispatchReceiptFeedSchemaVersion

export type StandbyDispatchReceiptFeedRejectionReason =
  | 'receipt_malformed'
  | 'receipt_not_verified'
  | 'duplicate_receipt_ref'

export type StandbyDispatchReceiptFeedEntry = Readonly<{
  promotedIntoWindowRef: string
  receiptRef: string
  runRef: string
  standbyContributorRef: string
}>

export type StandbyDispatchReceiptFeed = Readonly<{
  acceptedEntries: ReadonlyArray<StandbyDispatchReceiptFeedEntry>
  acceptedReceiptCount: number
  authorityBoundary: string
  blockerRef: typeof StandbyDispatchBlocker
  publicSafe: true
  rejectedReceiptCount: number
  rejectionReasonRefs: ReadonlyArray<StandbyDispatchReceiptFeedRejectionReason>
  schemaVersion: StandbyDispatchReceiptFeedSchemaVersion
}>

const feedAuthorityBoundary =
  'A standby-promotion receipt feed is a read-only, public-safe aggregation of published promotion receipts that each pass the read-side verifier (canonical content-addressed ref bound to the run + standby refs, public-safe run/standby/promoted-window refs). It grants no dispatch, settlement, promise-state, or green-claim authority, and does not assert any real standby was promoted into a live run — only that each admitted receipt is what it claims to be.'

/**
 * Aggregate an untrusted list of published promotion receipts into a public-safe
 * feed.
 *
 * Pure and total: never throws. Each receipt is decoded and verified by the
 * read-side verifier; malformed, unverified, and duplicate-ref receipts are dropped
 * and counted with their reasons, and the accepted entries are returned ordered by
 * receipt ref.
 */
export const buildStandbyDispatchReceiptFeed = (
  receipts: ReadonlyArray<unknown>,
): StandbyDispatchReceiptFeed => {
  const acceptedByRef = new Map<string, StandbyDispatchReceiptFeedEntry>()
  const rejectionReasons: Array<StandbyDispatchReceiptFeedRejectionReason> = []
  let rejectedReceiptCount = 0

  for (const receipt of receipts) {
    let decoded: StandbyDispatchReceiptValue
    try {
      decoded = S.decodeUnknownSync(StandbyDispatchReceipt)(receipt)
    } catch {
      rejectedReceiptCount += 1
      rejectionReasons.push('receipt_malformed')
      continue
    }

    const verdict = verifyStandbyDispatchReceipt(decoded)
    if (!verdict.verified) {
      rejectedReceiptCount += 1
      rejectionReasons.push('receipt_not_verified')
      continue
    }

    if (acceptedByRef.has(decoded.receiptRef)) {
      rejectedReceiptCount += 1
      rejectionReasons.push('duplicate_receipt_ref')
      continue
    }

    acceptedByRef.set(decoded.receiptRef, {
      promotedIntoWindowRef: decoded.promotedIntoWindowRef,
      receiptRef: decoded.receiptRef,
      runRef: decoded.runRef,
      standbyContributorRef: decoded.standbyContributorRef,
    })
  }

  const acceptedEntries = [...acceptedByRef.values()].sort((a, b) =>
    a.receiptRef < b.receiptRef ? -1 : a.receiptRef > b.receiptRef ? 1 : 0,
  )

  return {
    acceptedEntries,
    acceptedReceiptCount: acceptedEntries.length,
    authorityBoundary: feedAuthorityBoundary,
    blockerRef: StandbyDispatchBlocker,
    publicSafe: true,
    rejectedReceiptCount,
    rejectionReasonRefs: [...new Set(rejectionReasons)].sort(),
    schemaVersion: StandbyDispatchReceiptFeedSchemaVersion,
  }
}
