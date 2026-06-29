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
  readbackRehashReceiptRef:
    'receipt.training.checkpoint_readback_rehash.window.r1.w0007.v1',
  replicationFactor: 3,
  remoteCheckpointObjectRef:
    'r2.openagents_autopilot_artifacts.training_checkpoint.sha256_aaaaaaaa',
  remoteCheckpointStoreRef: 'r2.openagents_autopilot_artifacts.training',
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

  test('holds when the remote read-back-and-rehash receipt is missing', () => {
    const { readbackRehashReceiptRef: _ignored, ...withoutReceipt } =
      durableSeal
    const gate = evaluateDurableCheckpointSeal(withoutReceipt)
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('readback_rehash_receipt_missing')
  })

  test('holds when the remote checkpoint store/object refs are missing', () => {
    const {
      remoteCheckpointObjectRef: _objectRef,
      remoteCheckpointStoreRef: _storeRef,
      ...withoutRemoteStore
    } = durableSeal
    const gate = evaluateDurableCheckpointSeal(withoutRemoteStore)
    expect(gate.durable).toBe(false)
    expect(gate.reasons).toContain('remote_checkpoint_store_missing')
    expect(gate.reasons).toContain('remote_checkpoint_object_ref_missing')
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
