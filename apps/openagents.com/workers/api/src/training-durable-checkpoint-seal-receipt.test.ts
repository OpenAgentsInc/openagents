import { describe, expect, test } from 'vitest'

import {
  type DurableCheckpointSeal,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  DurableCheckpointSealReceiptSchemaVersion,
  DurableCheckpointSealReceiptUnsafe,
  buildDurableCheckpointSealReceipt,
  buildUntrustedDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'

const durableSeal = (): DurableCheckpointSeal => ({
  checkpointDigestRef: `sha256:${'a'.repeat(64)}`,
  replicationFactor: 3,
  retrievalProofRef: 'receipt.training.checkpoint_readback.window.r1.w0007.v1',
  retrievalVerified: true,
  sizeBytes: 4_294_967_296,
  storageClass: 'content_addressed_object_store',
  windowRef: 'training.run.r1.window.0007',
})

describe('durable checkpoint seal receipt emitter', () => {
  test('emits a public-safe receipt for a durable seal', () => {
    const receipt = buildDurableCheckpointSealReceipt(durableSeal())
    expect(receipt.outcome).toBe('seal_on_durable_checkpoint')
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.schemaVersion).toBe(DurableCheckpointSealReceiptSchemaVersion)
    expect(receipt.windowRef).toBe('training.run.r1.window.0007')
    expect(receipt.checkpointDigestRef).toBe(`sha256:${'a'.repeat(64)}`)
    expect(receipt.replicationFactor).toBe(3)
    expect(receipt.storageClass).toBe('content_addressed_object_store')
    expect(receipt.retrievalProofRef).toBe(
      'receipt.training.checkpoint_readback.window.r1.w0007.v1',
    )
    expect(receipt.receiptRef).toBe(
      durableCheckpointSealReceiptRef(
        'training.run.r1.window.0007',
        `sha256:${'a'.repeat(64)}`,
      ),
    )
    expect(receipt.sourceRefs.length).toBeGreaterThan(0)
  })

  test('derives a deterministic receipt ref from the window and digest refs', () => {
    expect(
      durableCheckpointSealReceiptRef(
        durableSeal().windowRef,
        durableSeal().checkpointDigestRef,
      ),
    ).toBe(buildDurableCheckpointSealReceipt(durableSeal()).receiptRef)
  })

  test('refuses to emit when read-back verification has no proof ref', () => {
    const { retrievalProofRef: _ignored, ...withoutProof } = durableSeal()
    expect(() => buildDurableCheckpointSealReceipt(withoutProof)).toThrow(
      DurableCheckpointSealReceiptUnsafe,
    )
  })

  test('refuses to emit when the digest is not content-addressed', () => {
    expect(() =>
      buildDurableCheckpointSealReceipt({
        ...durableSeal(),
        checkpointDigestRef: 'checkpoint.window.0007.latest',
      }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })

  test('refuses to emit when the checkpoint lives on ephemeral storage', () => {
    expect(() =>
      buildDurableCheckpointSealReceipt({
        ...durableSeal(),
        storageClass: 'local_scratch',
      }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })

  test('refuses to emit when replication is below the durable minimum', () => {
    expect(() =>
      buildDurableCheckpointSealReceipt({
        ...durableSeal(),
        replicationFactor: MinDurableReplicationFactor - 1,
      }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })

  test('refuses to emit when the checkpoint was never read back and re-hashed', () => {
    expect(() =>
      buildDurableCheckpointSealReceipt({
        ...durableSeal(),
        retrievalVerified: false,
      }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })

  test('builds from a well-formed untrusted descriptor', () => {
    const receipt = buildUntrustedDurableCheckpointSealReceipt({
      ...durableSeal(),
    })
    expect(receipt.outcome).toBe('seal_on_durable_checkpoint')
  })

  test('refuses to build from an untrusted descriptor with a non-public proof ref', () => {
    expect(() =>
      buildUntrustedDurableCheckpointSealReceipt({
        ...durableSeal(),
        retrievalProofRef: 'x',
      }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })

  test('refuses to build from a malformed untrusted descriptor', () => {
    expect(() =>
      buildUntrustedDurableCheckpointSealReceipt({ windowRef: 42 }),
    ).toThrow(DurableCheckpointSealReceiptUnsafe)
  })
})
