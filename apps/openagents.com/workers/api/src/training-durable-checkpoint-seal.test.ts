import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DurableCheckpointSeal,
  DurableCheckpointSealBlocker,
  MinDurableReplicationFactor,
  evaluateDurableCheckpointSeal,
  evaluateUntrustedDurableCheckpointSeal,
} from './training-durable-checkpoint-seal'

const durableSeal: DurableCheckpointSeal = {
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
  windowRef: 'training.run.r1.window.0007',
}

describe('durable checkpoint seal evaluator', () => {
  test('seals on a content-addressed, replicated, read-back-verified checkpoint', () => {
    const gate = evaluateDurableCheckpointSeal(durableSeal)
    expect(gate.durable).toBe(true)
    expect(gate.decision).toBe('seal_on_durable_checkpoint')
    expect(gate.reasons).toEqual([])
    expect(gate.blockerRef).toBe(DurableCheckpointSealBlocker)
  })

  test('holds when the checkpoint lives on ephemeral storage', () => {
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      storageClass: 'local_scratch',
    })
    expect(gate.durable).toBe(false)
    expect(gate.decision).toBe('hold_for_durable_checkpoint')
    expect(gate.reasons).toContain('storage_class_not_durable_content_addressed')
  })

  test('holds when the digest is not content-addressed', () => {
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      checkpointDigestRef: 'checkpoint.window.0007.latest',
    })
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('checkpoint_digest_not_content_addressed')
  })

  test('holds when replication is below the durable minimum', () => {
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      replicationFactor: MinDurableReplicationFactor - 1,
    })
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('replication_factor_below_durable_minimum')
  })

  test('holds when the checkpoint was never read back and re-hashed', () => {
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      retrievalVerified: false,
    })
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('checkpoint_retrieval_not_verified')
  })

  test('holds when the read-back receipt is missing', () => {
    const { readbackReceipt: _ignored, ...withoutReceipt } = durableSeal
    const gate = evaluateDurableCheckpointSeal(withoutReceipt)
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('readback_receipt_missing')
  })

  test('holds when the read-back receipt does not rehash to the checkpoint digest', () => {
    const readbackReceipt = durableSeal.readbackReceipt!
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      readbackReceipt: {
        ...readbackReceipt,
        readbackDigestRef: `sha256:${'b'.repeat(64)}`,
      },
    })
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('readback_receipt_digest_mismatch')
  })

  test('holds when the read-back receipt size does not match the sealed checkpoint', () => {
    const readbackReceipt = durableSeal.readbackReceipt!
    const gate = evaluateDurableCheckpointSeal({
      ...durableSeal,
      readbackReceipt: {
        ...readbackReceipt,
        sizeBytes: durableSeal.sizeBytes - 1,
      },
    })
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('readback_receipt_size_mismatch')
  })

  test('a malformed descriptor fails toward hold, never toward sealing', () => {
    const gate = evaluateUntrustedDurableCheckpointSeal({
      storageClass: 'unknown_backend',
    })
    expect(gate.durable).toBe(false)
    expect(gate.decision).toBe('hold_for_durable_checkpoint')
    expect(gate.reasons).toEqual(['seal_descriptor_malformed'])
  })

  test('a well-formed untrusted descriptor decodes and evaluates', () => {
    const gate = evaluateUntrustedDurableCheckpointSeal(durableSeal)
    expect(gate.durable).toBe(true)
    expect(S.decodeUnknownSync(DurableCheckpointSeal)(durableSeal)).toEqual(
      durableSeal,
    )
  })
})
