import { describe, expect, test } from 'vitest'

import {
  type DurableCheckpointSeal,
  DurableCheckpointSealBlocker,
} from './training-durable-checkpoint-seal'
import {
  buildDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'
import {
  DurableCheckpointSealReceiptFeedSchemaVersion,
  buildDurableCheckpointSealReceiptFeed,
} from './training-durable-checkpoint-seal-receipt-feed'

const durableSeal = (windowRef: string): DurableCheckpointSeal => ({
  checkpointDigestRef: `sha256:${'a'.repeat(64)}`,
  readbackReceipt: {
    objectKey: `checkpoints/sha256:${'a'.repeat(64)}`,
    readbackDigestRef: `sha256:${'a'.repeat(64)}`,
    receiptRef: 'receipt.training.checkpoint_readback.window.r1.w0007.v1',
    sizeBytes: 4_294_967_296,
    storeClass: 'r2',
    storedDigestRef: `sha256:${'a'.repeat(64)}`,
  },
  replicationFactor: 3,
  retrievalProofRef: 'receipt.training.checkpoint_readback.window.r1.w0007.v1',
  retrievalVerified: true,
  sizeBytes: 4_294_967_296,
  storageClass: 'content_addressed_object_store',
  windowRef,
})

const receiptFor = (windowRef: string) =>
  buildDurableCheckpointSealReceipt(durableSeal(windowRef))

describe('durable checkpoint seal receipt feed', () => {
  test('an empty list yields an empty public-safe feed', () => {
    const feed = buildDurableCheckpointSealReceiptFeed([])
    expect(feed.acceptedEntries).toEqual([])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(0)
    expect(feed.publicSafe).toBe(true)
    expect(feed.blockerRef).toBe(DurableCheckpointSealBlocker)
    expect(feed.schemaVersion).toBe(
      DurableCheckpointSealReceiptFeedSchemaVersion,
    )
  })

  test('admits genuine receipts ordered by receipt ref', () => {
    const a = receiptFor('training.run.r1.window.0007')
    const b = receiptFor('training.run.r1.window.0008')
    const feed = buildDurableCheckpointSealReceiptFeed([b, a])
    expect(feed.acceptedReceiptCount).toBe(2)
    expect(feed.acceptedEntries.map(e => e.receiptRef)).toEqual(
      [a.receiptRef, b.receiptRef].sort(),
    )
    expect(feed.acceptedEntries[0]?.storageClass).toBe(
      'content_addressed_object_store',
    )
    expect(feed.acceptedEntries[0]?.replicationFactor).toBe(3)
    expect(feed.rejectedReceiptCount).toBe(0)
  })

  test('drops duplicate receipt refs keeping the first', () => {
    const a = receiptFor('training.run.r1.window.0007')
    const feed = buildDurableCheckpointSealReceiptFeed([a, { ...a }])
    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('duplicate_receipt_ref')
  })

  test('rejects a malformed receipt without throwing', () => {
    const feed = buildDurableCheckpointSealReceiptFeed([{ not: 'a receipt' }])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_malformed')
  })

  test('rejects a decodable but unverifiable (ref-mismatched) receipt', () => {
    const tampered = {
      ...receiptFor('training.run.r1.window.0007'),
      receiptRef: durableCheckpointSealReceiptRef(
        'training.run.r1.window.9999',
        `sha256:${'a'.repeat(64)}`,
      ),
    }
    const feed = buildDurableCheckpointSealReceiptFeed([tampered])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_not_verified')
  })

  test('mixes accepted and rejected receipts deterministically', () => {
    const good = receiptFor('training.run.r1.window.0007')
    const feed = buildDurableCheckpointSealReceiptFeed([
      good,
      { garbage: true },
      { ...good },
    ])
    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(2)
    expect([...feed.rejectionReasonRefs]).toEqual(
      [...feed.rejectionReasonRefs].sort(),
    )
  })
})
