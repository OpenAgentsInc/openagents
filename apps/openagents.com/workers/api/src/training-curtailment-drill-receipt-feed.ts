import { Schema as S } from 'effect'

import {
  CurtailmentDrillBlocker,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
} from './training-curtailment-drill'
import {
  CurtailmentDrillReceipt,
  type CurtailmentDrillReceipt as CurtailmentDrillReceiptValue,
} from './training-curtailment-drill-receipt'
import { verifyCurtailmentDrillReceipt } from './training-curtailment-drill-receipt-verifier'

/**
 * Curtailment-drill receipt FEED builder for training.marathon_operations.v1.
 *
 * The receipt emitter (training-curtailment-drill-receipt.ts) BUILDS one receipt,
 * and the read-side verifier (training-curtailment-drill-receipt-verifier.ts)
 * validates ONE untrusted read-back receipt. But a public receipt route does not
 * serve a single receipt — it serves a COLLECTION. Nothing yet turned an untrusted
 * list of published drill receipts into the one public-safe, verified,
 * de-duplicated, ordered feed such a route would publish. The
 * durable-checkpoint-seal and standby-dispatch lanes have the same gap; this module
 * supplies the aggregation layer for the curtailment lane, mirroring the
 * gradient-window promotion-receipt feed (tassadar-gradient-window-promotion-receipt-feed.ts).
 *
 * `buildCurtailmentDrillReceiptFeed` is a pure, TOTAL function over an array of
 * untrusted receipts: it decodes and runs each through the read-side verifier,
 * admits only receipts that pass every authenticity invariant, drops duplicates
 * (same canonical receipt ref) keeping the first, counts and explains every
 * rejection, and returns the accepted entries deterministically ordered by receipt
 * ref. It never throws, so it is safe at the edge of a real public feed.
 *
 * This advances blocker.product_promises.curtailment_drill_missing by building the
 * aggregation layer a public drill-receipt feed needs. It does NOT clear that
 * blocker: no scheduled live drill has run, no runtime emits a real receipt, and no
 * route serves this feed — so a real feed is empty. It grants no dispatch,
 * settlement, flexible-load-market, promise-state, or green-claim authority.
 */

export const CurtailmentDrillReceiptFeedSchemaVersion =
  'openagents.training.marathon_operations.curtailment_drill_receipt_feed.v1'
export type CurtailmentDrillReceiptFeedSchemaVersion =
  typeof CurtailmentDrillReceiptFeedSchemaVersion

export type CurtailmentDrillReceiptFeedRejectionReason =
  | 'receipt_malformed'
  | 'receipt_not_verified'
  | 'duplicate_receipt_ref'

export type CurtailmentDrillReceiptFeedEntry = Readonly<{
  ackLatencyMs: number
  drillRef: string
  haltLatencyMs: number
  receiptRef: string
  runRef: string
}>

export type CurtailmentDrillReceiptFeed = Readonly<{
  acceptedEntries: ReadonlyArray<CurtailmentDrillReceiptFeedEntry>
  acceptedReceiptCount: number
  ackSlaMs: typeof MaxCurtailmentAckLatencyMs
  authorityBoundary: string
  blockerRef: typeof CurtailmentDrillBlocker
  haltSlaMs: typeof MaxCurtailmentHaltLatencyMs
  publicSafe: true
  rejectedReceiptCount: number
  rejectionReasonRefs: ReadonlyArray<CurtailmentDrillReceiptFeedRejectionReason>
  schemaVersion: CurtailmentDrillReceiptFeedSchemaVersion
}>

const feedAuthorityBoundary =
  'A curtailment-drill receipt feed is a read-only, public-safe aggregation of published drill receipts that each pass the read-side verifier (canonical content-addressed ref bound to the drill ref, public-safe drill/run refs, ack/halt latencies within their SLAs). It grants no dispatch, settlement, flexible-load-market, promise-state, or green-claim authority, and does not assert any real scheduled curtailment drill was run — only that each admitted receipt is what it claims to be.'

/**
 * Aggregate an untrusted list of published drill receipts into a public-safe feed.
 *
 * Pure and total: never throws. Each receipt is decoded and verified by the
 * read-side verifier; malformed, unverified, and duplicate-ref receipts are dropped
 * and counted with their reasons, and the accepted entries are returned ordered by
 * receipt ref.
 */
export const buildCurtailmentDrillReceiptFeed = (
  receipts: ReadonlyArray<unknown>,
): CurtailmentDrillReceiptFeed => {
  const acceptedByRef = new Map<string, CurtailmentDrillReceiptFeedEntry>()
  const rejectionReasons: Array<CurtailmentDrillReceiptFeedRejectionReason> = []
  let rejectedReceiptCount = 0

  for (const receipt of receipts) {
    let decoded: CurtailmentDrillReceiptValue
    try {
      decoded = S.decodeUnknownSync(CurtailmentDrillReceipt)(receipt)
    } catch {
      rejectedReceiptCount += 1
      rejectionReasons.push('receipt_malformed')
      continue
    }

    const verdict = verifyCurtailmentDrillReceipt(decoded)
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
      ackLatencyMs: decoded.ackLatencyMs,
      drillRef: decoded.drillRef,
      haltLatencyMs: decoded.haltLatencyMs,
      receiptRef: decoded.receiptRef,
      runRef: decoded.runRef,
    })
  }

  const acceptedEntries = [...acceptedByRef.values()].sort((a, b) =>
    a.receiptRef < b.receiptRef ? -1 : a.receiptRef > b.receiptRef ? 1 : 0,
  )

  return {
    acceptedEntries,
    acceptedReceiptCount: acceptedEntries.length,
    ackSlaMs: MaxCurtailmentAckLatencyMs,
    authorityBoundary: feedAuthorityBoundary,
    blockerRef: CurtailmentDrillBlocker,
    haltSlaMs: MaxCurtailmentHaltLatencyMs,
    publicSafe: true,
    rejectedReceiptCount,
    rejectionReasonRefs: [...new Set(rejectionReasons)].sort(),
    schemaVersion: CurtailmentDrillReceiptFeedSchemaVersion,
  }
}
