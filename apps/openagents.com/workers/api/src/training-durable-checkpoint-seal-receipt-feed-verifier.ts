import { Schema as S } from 'effect'

import {
  DurableCheckpointDigestPattern,
  DurableCheckpointSealBlocker,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import { durableCheckpointSealReceiptRef } from './training-durable-checkpoint-seal-receipt'
import { DurableCheckpointSealReceiptFeedSchemaVersion } from './training-durable-checkpoint-seal-receipt-feed'

/**
 * Durable-checkpoint-seal receipt FEED verifier for
 * training.marathon_operations.v1.
 *
 * The single-receipt verifier
 * (training-durable-checkpoint-seal-receipt-verifier.ts) validates ONE untrusted
 * read-back receipt, and the feed builder
 * (training-durable-checkpoint-seal-receipt-feed.ts) AGGREGATES a trusted local
 * list into a public-safe feed. But a downstream service does not build the feed
 * itself — it dereferences a feed that some OTHER process published, and must
 * therefore treat the whole feed object as untrusted input and re-validate it
 * before relying on it. Nothing performed that whole-feed validation: the
 * standby-dispatch and curtailment-drill receipts each got a single-receipt
 * verifier, but no lane had a FEED-level verifier. This module supplies the
 * missing consumption side of the FEED contract for the durable-checkpoint-seal
 * lane, mirroring how each receipt emitter gained a receipt verifier.
 *
 * Decoding alone is not enough. The feed schema pins the literal `schemaVersion`,
 * `blockerRef`, and `publicSafe`, so a decode rejects those, but it does NOT
 * re-check the structural invariants the builder is responsible for: that
 * `acceptedReceiptCount` matches the accepted entries, that accepted entries carry
 * no duplicate receipt refs, that they are ordered deterministically by receipt
 * ref, that each entry's deterministic content-addressed `receiptRef` is bound to
 * its `windowRef` + `checkpointDigestRef`, that each digest is content-addressed,
 * that each replication factor meets the durable minimum, and that the rejection
 * tally is internally consistent. A forged or corrupted feed can decode cleanly
 * while violating any of these.
 *
 * The verifier re-derives every canonical entry ref, re-checks the durability
 * invariants per entry, and re-checks the feed-level counting/ordering/dedup
 * invariants. It FAILS TOWARD `not_verified` (it never reports a malformed or
 * inconsistent feed as verified), mirroring the seal predicate's
 * fail-toward-HOLD posture. It is pure and total: it never throws.
 *
 * It is contract-level only. A `verified` verdict reports that a published feed is
 * internally authentic and self-consistent with the builder's invariants; it
 * grants no dispatch, settlement, storage-backend, promise-state, or green-claim
 * authority, and it does not assert that any real remote checkpoint store was ever
 * read back — only that the feed is what it claims to be.
 */

export const DurableCheckpointSealReceiptFeedVerificationSchemaVersion =
  'openagents.training.marathon_operations.durable_checkpoint_seal_receipt_feed_verification.v1'
export type DurableCheckpointSealReceiptFeedVerificationSchemaVersion =
  typeof DurableCheckpointSealReceiptFeedVerificationSchemaVersion

export type DurableCheckpointSealReceiptFeedVerificationDecision =
  | 'verified'
  | 'not_verified'

export type DurableCheckpointSealReceiptFeedVerificationReason =
  | 'feed_malformed'
  | 'accepted_count_mismatch'
  | 'accepted_entries_unordered'
  | 'duplicate_accepted_receipt_ref'
  | 'entry_receipt_ref_mismatch'
  | 'entry_digest_not_content_addressed'
  | 'entry_replication_below_durable_minimum'
  | 'rejection_tally_inconsistent'

export type DurableCheckpointSealReceiptFeedVerificationVerdict = Readonly<{
  acceptedReceiptCount: number | undefined
  authorityBoundary: string
  blockerRef: typeof DurableCheckpointSealBlocker
  decision: DurableCheckpointSealReceiptFeedVerificationDecision
  reasons: ReadonlyArray<DurableCheckpointSealReceiptFeedVerificationReason>
  schemaVersion: DurableCheckpointSealReceiptFeedVerificationSchemaVersion
  verified: boolean
}>

const FeedEntrySchema = S.Struct({
  checkpointDigestRef: S.String,
  receiptRef: S.String,
  replicationFactor: S.Int,
  storageClass: S.String,
  windowRef: S.String,
})

const FeedSchema = S.Struct({
  acceptedEntries: S.Array(FeedEntrySchema),
  acceptedReceiptCount: S.Int,
  authorityBoundary: S.String,
  blockerRef: S.Literal(DurableCheckpointSealBlocker),
  publicSafe: S.Literal(true),
  rejectedReceiptCount: S.Int,
  rejectionReasonRefs: S.Array(
    S.Literals([
      'receipt_malformed',
      'receipt_not_verified',
      'duplicate_receipt_ref',
    ]),
  ),
  schemaVersion: S.Literal(DurableCheckpointSealReceiptFeedSchemaVersion),
})
type FeedValue = typeof FeedSchema.Type

const verificationAuthorityBoundary =
  'Durable-checkpoint-seal receipt feed verification confirms that a published feed is internally authentic and self-consistent with the builder invariants (canonical content-addressed entry refs bound to window + digest, content-addressed digests, durable-minimum replication, accepted-count match, deterministic ref ordering, no duplicate accepted refs, consistent rejection tally). A verified verdict grants no dispatch, settlement, storage-backend, promise-state, or green-claim authority, does not assert any real remote checkpoint store was read back, and a not-verified verdict is the safe default.'

const isAscendingByRef = (
  entries: FeedValue['acceptedEntries'],
): boolean => {
  for (let index = 1; index < entries.length; index += 1) {
    const previous = entries[index - 1]
    const current = entries[index]
    if (previous === undefined || current === undefined) {
      return false
    }
    if (previous.receiptRef > current.receiptRef) {
      return false
    }
  }
  return true
}

/**
 * Verify an already-decoded durable-checkpoint-seal receipt feed. A feed is
 * verified only when every structural and per-entry authenticity invariant holds;
 * any failing invariant routes to `not_verified` with the failing reasons
 * enumerated (deduped and sorted).
 */
export const verifyDurableCheckpointSealReceiptFeed = (
  feed: FeedValue,
): DurableCheckpointSealReceiptFeedVerificationVerdict => {
  const reasons = new Set<DurableCheckpointSealReceiptFeedVerificationReason>()

  if (feed.acceptedReceiptCount !== feed.acceptedEntries.length) {
    reasons.add('accepted_count_mismatch')
  }

  if (!isAscendingByRef(feed.acceptedEntries)) {
    reasons.add('accepted_entries_unordered')
  }

  const seenRefs = new Set<string>()
  for (const entry of feed.acceptedEntries) {
    if (seenRefs.has(entry.receiptRef)) {
      reasons.add('duplicate_accepted_receipt_ref')
    }
    seenRefs.add(entry.receiptRef)

    const expectedRef = durableCheckpointSealReceiptRef(
      entry.windowRef,
      entry.checkpointDigestRef,
    )
    if (entry.receiptRef !== expectedRef) {
      reasons.add('entry_receipt_ref_mismatch')
    }
    if (!DurableCheckpointDigestPattern.test(entry.checkpointDigestRef)) {
      reasons.add('entry_digest_not_content_addressed')
    }
    if (entry.replicationFactor < MinDurableReplicationFactor) {
      reasons.add('entry_replication_below_durable_minimum')
    }
  }

  // The rejection tally must be self-consistent: a non-empty reason list implies
  // at least one rejection was counted, and a positive rejection count implies at
  // least one reason was recorded (reasons are deduped, so the count may exceed
  // the number of distinct reasons, but neither side may be empty while the other
  // is not).
  const hasRejectionReasons = feed.rejectionReasonRefs.length > 0
  const hasRejectionCount = feed.rejectedReceiptCount > 0
  if (
    feed.rejectedReceiptCount < 0 ||
    hasRejectionReasons !== hasRejectionCount ||
    feed.rejectedReceiptCount < feed.rejectionReasonRefs.length
  ) {
    reasons.add('rejection_tally_inconsistent')
  }

  const verified = reasons.size === 0

  return {
    acceptedReceiptCount: feed.acceptedReceiptCount,
    authorityBoundary: verificationAuthorityBoundary,
    blockerRef: DurableCheckpointSealBlocker,
    decision: verified ? 'verified' : 'not_verified',
    reasons: [...reasons].sort(),
    schemaVersion: DurableCheckpointSealReceiptFeedVerificationSchemaVersion,
    verified,
  }
}

/**
 * Decode an untrusted published feed and verify it. A feed that fails to decode
 * (wrong schema version, wrong blocker ref, `publicSafe` absent, malformed
 * entries, unknown rejection reason) yields a `not_verified` verdict — failing
 * toward not-verified rather than trusting an unverifiable artifact.
 */
export const verifyUntrustedDurableCheckpointSealReceiptFeed = (
  input: unknown,
): DurableCheckpointSealReceiptFeedVerificationVerdict => {
  let decoded: FeedValue
  try {
    decoded = S.decodeUnknownSync(FeedSchema)(input)
  } catch {
    return {
      acceptedReceiptCount: undefined,
      authorityBoundary: verificationAuthorityBoundary,
      blockerRef: DurableCheckpointSealBlocker,
      decision: 'not_verified',
      reasons: ['feed_malformed'],
      schemaVersion: DurableCheckpointSealReceiptFeedVerificationSchemaVersion,
      verified: false,
    }
  }
  return verifyDurableCheckpointSealReceiptFeed(decoded)
}
