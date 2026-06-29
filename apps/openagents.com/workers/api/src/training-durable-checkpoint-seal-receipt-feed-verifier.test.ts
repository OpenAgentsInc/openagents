import { describe, expect, test } from 'vitest'

import {
  type DurableCheckpointSeal,
  DurableCheckpointSealBlocker,
} from './training-durable-checkpoint-seal'
import {
  buildDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'
import { buildDurableCheckpointSealReceiptFeed } from './training-durable-checkpoint-seal-receipt-feed'
import {
  DurableCheckpointSealReceiptFeedVerificationSchemaVersion,
  verifyDurableCheckpointSealReceiptFeed,
  verifyUntrustedDurableCheckpointSealReceiptFeed,
} from './training-durable-checkpoint-seal-receipt-feed-verifier'

const durableSeal = (windowRef: string): DurableCheckpointSeal => ({
  checkpointDigestRef: `sha256:${'a'.repeat(64)}`,
  readbackRehashReceiptRef: `receipt.training.checkpoint_readback_rehash.${windowRef}`,
  replicationFactor: 3,
  remoteCheckpointObjectRef: `r2.training_checkpoint.${windowRef}`,
  remoteCheckpointStoreRef: 'r2.openagents_autopilot_artifacts.training',
  retrievalProofRef: 'receipt.training.checkpoint_readback.window.r1.w0007.v1',
  retrievalVerified: true,
  sizeBytes: 4_294_967_296,
  storageClass: 'content_addressed_object_store',
  windowRef,
})

const feedFor = (...windowRefs: ReadonlyArray<string>) =>
  buildDurableCheckpointSealReceiptFeed(
    windowRefs.map(ref => buildDurableCheckpointSealReceipt(durableSeal(ref))),
  )

describe('durable checkpoint seal receipt feed verifier', () => {
  test('verifies a genuine built feed (trusted + untrusted-decode paths)', () => {
    const feed = feedFor(
      'training.run.r1.window.0008',
      'training.run.r1.window.0007',
    )
    expect(feed.acceptedReceiptCount).toBe(2)

    const direct = verifyDurableCheckpointSealReceiptFeed(feed)
    expect(direct.verified).toBe(true)
    expect(direct.decision).toBe('verified')
    expect(direct.reasons).toEqual([])
    expect(direct.acceptedReceiptCount).toBe(2)
    expect(direct.blockerRef).toBe(DurableCheckpointSealBlocker)
    expect(direct.schemaVersion).toBe(
      DurableCheckpointSealReceiptFeedVerificationSchemaVersion,
    )

    const fromJson = verifyUntrustedDurableCheckpointSealReceiptFeed(
      JSON.parse(JSON.stringify(feed)),
    )
    expect(fromJson.verified).toBe(true)
  })

  test('an empty feed verifies', () => {
    const verdict = verifyDurableCheckpointSealReceiptFeed(feedFor())
    expect(verdict.verified).toBe(true)
    expect(verdict.acceptedReceiptCount).toBe(0)
  })

  test('rejects an accepted-count mismatch', () => {
    const feed = { ...feedFor('training.run.r1.window.0007') }
    const tampered = { ...feed, acceptedReceiptCount: feed.acceptedReceiptCount + 1 }
    const verdict = verifyDurableCheckpointSealReceiptFeed(tampered)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('accepted_count_mismatch')
  })

  test('rejects out-of-order accepted entries', () => {
    const feed = feedFor(
      'training.run.r1.window.0007',
      'training.run.r1.window.0008',
    )
    const reversed = {
      ...feed,
      acceptedEntries: [...feed.acceptedEntries].reverse(),
    }
    const verdict = verifyDurableCheckpointSealReceiptFeed(reversed)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('accepted_entries_unordered')
  })

  test('rejects a duplicate accepted receipt ref', () => {
    const feed = feedFor('training.run.r1.window.0007')
    const entry = feed.acceptedEntries[0]
    if (entry === undefined) {
      throw new Error('expected one accepted entry')
    }
    const dupe = { ...feed, acceptedEntries: [entry, entry], acceptedReceiptCount: 2 }
    const verdict = verifyDurableCheckpointSealReceiptFeed(dupe)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('duplicate_accepted_receipt_ref')
  })

  test('rejects an entry whose ref is not bound to its window + digest', () => {
    const feed = feedFor('training.run.r1.window.0007')
    const entry = feed.acceptedEntries[0]
    if (entry === undefined) {
      throw new Error('expected one accepted entry')
    }
    const tampered = {
      ...feed,
      acceptedEntries: [
        {
          ...entry,
          receiptRef: durableCheckpointSealReceiptRef(
            'training.run.r1.window.9999',
            `sha256:${'a'.repeat(64)}`,
          ),
        },
      ],
    }
    const verdict = verifyDurableCheckpointSealReceiptFeed(tampered)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('entry_receipt_ref_mismatch')
  })

  test('rejects an entry with sub-minimum replication', () => {
    const feed = feedFor('training.run.r1.window.0007')
    const entry = feed.acceptedEntries[0]
    if (entry === undefined) {
      throw new Error('expected one accepted entry')
    }
    const tampered = {
      ...feed,
      acceptedEntries: [{ ...entry, replicationFactor: 1 }],
    }
    const verdict = verifyDurableCheckpointSealReceiptFeed(tampered)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('entry_replication_below_durable_minimum')
  })

  test('rejects an inconsistent rejection tally', () => {
    const feed = feedFor('training.run.r1.window.0007')
    const tampered = {
      ...feed,
      rejectedReceiptCount: 3,
      rejectionReasonRefs: [],
    }
    const verdict = verifyDurableCheckpointSealReceiptFeed(tampered)
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('rejection_tally_inconsistent')
  })

  test('a malformed feed fails toward not_verified without throwing', () => {
    const verdict = verifyUntrustedDurableCheckpointSealReceiptFeed({
      not: 'a feed',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['feed_malformed'])
    expect(verdict.acceptedReceiptCount).toBeUndefined()
  })
})
