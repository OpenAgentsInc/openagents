import { describe, expect, test } from 'vitest'

import {
  type DurableCheckpointSeal,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  buildDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'
import {
  DurableCheckpointSealReceiptVerificationSchemaVersion,
  verifyDurableCheckpointSealReceipt,
  verifyUntrustedDurableCheckpointSealReceipt,
} from './training-durable-checkpoint-seal-receipt-verifier'

const durableSeal = (): DurableCheckpointSeal => ({
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
})

const genuineReceipt = () => buildDurableCheckpointSealReceipt(durableSeal())

describe('durable checkpoint seal receipt verifier', () => {
  test('verifies a genuine emitted receipt', () => {
    const verdict = verifyDurableCheckpointSealReceipt(genuineReceipt())
    expect(verdict.verified).toBe(true)
    expect(verdict.decision).toBe('verified')
    expect(verdict.reasons).toEqual([])
    expect(verdict.schemaVersion).toBe(
      DurableCheckpointSealReceiptVerificationSchemaVersion,
    )
    expect(verdict.receiptRef).toBe(genuineReceipt().receiptRef)
  })

  test('verifies a genuine receipt decoded from an untrusted source', () => {
    const verdict = verifyUntrustedDurableCheckpointSealReceipt({
      ...genuineReceipt(),
    })
    expect(verdict.verified).toBe(true)
  })

  test('rejects a receipt whose ref does not match its window/digest fields', () => {
    const verdict = verifyDurableCheckpointSealReceipt({
      ...genuineReceipt(),
      receiptRef: durableCheckpointSealReceiptRef(
        'training.run.r1.window.9999',
        `sha256:${'a'.repeat(64)}`,
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('receipt_ref_mismatch')
  })

  test('rejects a receipt whose digest is not content-addressed', () => {
    const tampered = {
      ...genuineReceipt(),
      checkpointDigestRef: 'checkpoint.window.0007.latest',
    }
    const verdict = verifyDurableCheckpointSealReceipt({
      ...tampered,
      receiptRef: durableCheckpointSealReceiptRef(
        tampered.windowRef,
        tampered.checkpointDigestRef,
      ),
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('checkpoint_digest_not_content_addressed')
  })

  test('rejects a receipt with sub-minimum replication', () => {
    const verdict = verifyDurableCheckpointSealReceipt({
      ...genuineReceipt(),
      replicationFactor: MinDurableReplicationFactor - 1,
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain(
      'replication_factor_below_durable_minimum',
    )
  })

  test('rejects a receipt with invalid remote read-back evidence refs', () => {
    const verdict = verifyDurableCheckpointSealReceipt({
      ...genuineReceipt(),
      readbackRehashReceiptRef: '',
      remoteCheckpointObjectRef: ' raw/object ',
      remoteCheckpointStoreRef: '../private-store',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toContain('readback_rehash_receipt_ref_invalid')
    expect(verdict.reasons).toContain('remote_checkpoint_object_ref_invalid')
    expect(verdict.reasons).toContain('remote_checkpoint_store_ref_invalid')
  })

  test('fails toward not-verified for a malformed untrusted receipt', () => {
    const verdict = verifyUntrustedDurableCheckpointSealReceipt({
      receiptRef: 42,
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
    expect(verdict.receiptRef).toBeUndefined()
  })

  test('fails toward not-verified for a forged outcome that does not decode', () => {
    const verdict = verifyUntrustedDurableCheckpointSealReceipt({
      ...genuineReceipt(),
      outcome: 'seal_on_ephemeral_checkpoint',
    })
    expect(verdict.verified).toBe(false)
    expect(verdict.reasons).toEqual(['receipt_malformed'])
  })
})
