import { Schema as S } from 'effect'

import { DurableCheckpointSealBlocker } from './training-durable-checkpoint-seal'
import {
  DurableCheckpointSealReceipt,
  type DurableCheckpointSealReceipt as DurableCheckpointSealReceiptValue,
} from './training-durable-checkpoint-seal-receipt'
import { verifyDurableCheckpointSealReceipt } from './training-durable-checkpoint-seal-receipt-verifier'

/**
 * Durable-checkpoint-seal receipt FEED builder for
 * training.marathon_operations.v1.
 *
 * The receipt emitter (training-durable-checkpoint-seal-receipt.ts) BUILDS one
 * receipt, and the read-side verifier
 * (training-durable-checkpoint-seal-receipt-verifier.ts) validates ONE untrusted
 * read-back receipt. But a public receipt route does not serve a single receipt —
 * it serves a COLLECTION. Nothing yet turned an untrusted list of published
 * durable-seal receipts into the one public-safe, verified, de-duplicated, ordered
 * feed such a route would publish. The standby-dispatch and curtailment-drill
 * lanes already have this aggregation layer
 * (training-standby-dispatch-receipt-feed.ts,
 * training-curtailment-drill-receipt-feed.ts); this module supplies the matching
 * layer for the durable-checkpoint-seal lane, mirroring those feeds.
 *
 * `buildDurableCheckpointSealReceiptFeed` is a pure, TOTAL function over an array
 * of untrusted receipts: it decodes and runs each through the read-side verifier,
 * admits only receipts that pass every authenticity invariant (canonical
 * content-addressed ref bound to window + digest, content-addressed digest,
 * durable-minimum replication), drops duplicates (same canonical receipt ref)
 * keeping the first, counts and explains every rejection, and returns the accepted
 * entries deterministically ordered by receipt ref. It never throws, so it is safe
 * at the edge of a real public feed.
 *
 * This advances blocker.product_promises.durable_checkpoint_seal_missing by
 * building the aggregation layer a public read-back-receipt feed needs. It does
 * NOT clear that blocker: no window has been sealed on a real remote
 * content-addressed checkpoint store, no runtime emits a real receipt, and no route
 * serves this feed — so a real feed is empty. It grants no dispatch, settlement,
 * storage-backend, promise-state, or green-claim authority.
 */

export const DurableCheckpointSealReceiptFeedSchemaVersion =
  'openagents.training.marathon_operations.durable_checkpoint_seal_receipt_feed.v1'
export type DurableCheckpointSealReceiptFeedSchemaVersion =
  typeof DurableCheckpointSealReceiptFeedSchemaVersion

export type DurableCheckpointSealReceiptFeedRejectionReason =
  | 'receipt_malformed'
  | 'receipt_not_verified'
  | 'duplicate_receipt_ref'

export type DurableCheckpointSealReceiptFeedEntry = Readonly<{
  checkpointDigestRef: string
  readbackRehashReceiptRef: string
  receiptRef: string
  remoteCheckpointObjectRef: string
  remoteCheckpointStoreRef: string
  replicationFactor: number
  storageClass: string
  windowRef: string
}>

export type DurableCheckpointSealReceiptFeed = Readonly<{
  acceptedEntries: ReadonlyArray<DurableCheckpointSealReceiptFeedEntry>
  acceptedReceiptCount: number
  authorityBoundary: string
  blockerRef: typeof DurableCheckpointSealBlocker
  publicSafe: true
  rejectedReceiptCount: number
  rejectionReasonRefs: ReadonlyArray<DurableCheckpointSealReceiptFeedRejectionReason>
  schemaVersion: DurableCheckpointSealReceiptFeedSchemaVersion
}>

const feedAuthorityBoundary =
  'A durable-checkpoint-seal receipt feed is a read-only, public-safe aggregation of published seal receipts that each pass the read-side verifier (canonical content-addressed ref bound to the window + checkpoint digest, content-addressed digest, durable-minimum replication). It grants no dispatch, settlement, storage-backend, promise-state, or green-claim authority, and does not assert that any real remote checkpoint store was read back — only that each admitted receipt is what it claims to be.'

/**
 * Aggregate an untrusted list of published durable-seal receipts into a
 * public-safe feed.
 *
 * Pure and total: never throws. Each receipt is decoded and verified by the
 * read-side verifier; malformed, unverified, and duplicate-ref receipts are dropped
 * and counted with their reasons, and the accepted entries are returned ordered by
 * receipt ref.
 */
export const buildDurableCheckpointSealReceiptFeed = (
  receipts: ReadonlyArray<unknown>,
): DurableCheckpointSealReceiptFeed => {
  const acceptedByRef = new Map<string, DurableCheckpointSealReceiptFeedEntry>()
  const rejectionReasons: Array<DurableCheckpointSealReceiptFeedRejectionReason> =
    []
  let rejectedReceiptCount = 0

  for (const receipt of receipts) {
    let decoded: DurableCheckpointSealReceiptValue
    try {
      decoded = S.decodeUnknownSync(DurableCheckpointSealReceipt)(receipt)
    } catch {
      rejectedReceiptCount += 1
      rejectionReasons.push('receipt_malformed')
      continue
    }

    const verdict = verifyDurableCheckpointSealReceipt(decoded)
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
      checkpointDigestRef: decoded.checkpointDigestRef,
      readbackRehashReceiptRef: decoded.readbackRehashReceiptRef,
      receiptRef: decoded.receiptRef,
      remoteCheckpointObjectRef: decoded.remoteCheckpointObjectRef,
      remoteCheckpointStoreRef: decoded.remoteCheckpointStoreRef,
      replicationFactor: decoded.replicationFactor,
      storageClass: decoded.storageClass,
      windowRef: decoded.windowRef,
    })
  }

  const acceptedEntries = [...acceptedByRef.values()].sort((a, b) =>
    a.receiptRef < b.receiptRef ? -1 : a.receiptRef > b.receiptRef ? 1 : 0,
  )

  return {
    acceptedEntries,
    acceptedReceiptCount: acceptedEntries.length,
    authorityBoundary: feedAuthorityBoundary,
    blockerRef: DurableCheckpointSealBlocker,
    publicSafe: true,
    rejectedReceiptCount,
    rejectionReasonRefs: [...new Set(rejectionReasons)].sort(),
    schemaVersion: DurableCheckpointSealReceiptFeedSchemaVersion,
  }
}
