import { Schema as S } from 'effect'

import {
  DurableCheckpointDigestPattern,
  DurableCheckpointSealBlocker,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  DurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'

type DurableCheckpointSealReceiptValue =
  typeof DurableCheckpointSealReceipt.Type

/**
 * Durable-checkpoint-seal receipt VERIFIER for training.marathon_operations.v1.
 *
 * The receipt emitter (training-durable-checkpoint-seal-receipt.ts) produces the
 * public-safe artifact the live runtime publishes once a window has actually been
 * sealed on a durable content-addressed checkpoint. That is the PRODUCTION side of
 * the receipt contract. This module supplies the missing CONSUMPTION side: when a
 * consumer later dereferences a published receipt — which it must treat as
 * untrusted input, not as something it minted itself — it has to confirm the
 * receipt is authentic and self-consistent before relying on it (for example,
 * before any projection flag could ever flip on the strength of one).
 *
 * Decoding alone is not enough. The receipt schema pins the literal `outcome`,
 * `publicSafe`, `blockerRef`, and schema versions, so a decode rejects those, but
 * it does NOT re-check the durability invariants the EMITTER enforced: the
 * `checkpointDigestRef` is just a string in the receipt, the `replicationFactor`
 * is just an int, and — critically — the deterministic content-addressed
 * `receiptRef` is not bound to its `windowRef` + `checkpointDigestRef` by the
 * schema. A forged or tampered receipt can decode cleanly while carrying a
 * mismatched ref, a non-content-addressed digest, or sub-minimum replication.
 *
 * The verifier re-derives the canonical receipt ref from the receipt's own
 * window/digest fields and confirms it matches, re-checks that the digest is
 * content-addressed, and re-checks that replication meets the durable minimum. It
 * FAILS TOWARD `not_verified` (it never reports a malformed or inconsistent
 * receipt as verified), mirroring the seal predicate's fail-toward-HOLD posture.
 *
 * It is contract-level only. A `verified` verdict reports that a published receipt
 * is internally authentic and consistent with the emitter's invariants; it grants
 * no dispatch, settlement, storage-backend, promise-state, or green-claim
 * authority, and it does not assert that a real remote checkpoint store was ever
 * read back — only that the receipt is what it claims to be.
 */

export const DurableCheckpointSealReceiptVerificationSchemaVersion =
  'openagents.training.marathon_operations.durable_checkpoint_seal_receipt_verification.v1'
export type DurableCheckpointSealReceiptVerificationSchemaVersion =
  typeof DurableCheckpointSealReceiptVerificationSchemaVersion

export type DurableCheckpointSealReceiptVerificationDecision =
  | 'verified'
  | 'not_verified'

export type DurableCheckpointSealReceiptVerificationReason =
  | 'receipt_malformed'
  | 'receipt_ref_mismatch'
  | 'checkpoint_digest_not_content_addressed'
  | 'replication_factor_below_durable_minimum'
  | 'readback_receipt_digest_mismatch'
  | 'readback_receipt_size_mismatch'

export type DurableCheckpointSealReceiptVerificationVerdict = Readonly<{
  authorityBoundary: string
  blockerRef: typeof DurableCheckpointSealBlocker
  decision: DurableCheckpointSealReceiptVerificationDecision
  reasons: ReadonlyArray<DurableCheckpointSealReceiptVerificationReason>
  receiptRef: string | undefined
  schemaVersion: DurableCheckpointSealReceiptVerificationSchemaVersion
  verified: boolean
}>

const verificationAuthorityBoundary =
  'Durable-checkpoint-seal receipt verification confirms that a published receipt is internally authentic and consistent with the emitter invariants (canonical content-addressed ref, content-addressed digest, durable-minimum replication). A verified verdict grants no dispatch, settlement, storage-backend, promise-state, or green-claim authority, does not assert any real remote checkpoint store was read back, and a not-verified verdict is the safe default.'

/**
 * Verify an already-decoded durable-checkpoint-seal receipt. A receipt is verified
 * only when every authenticity invariant holds; any failing invariant routes to
 * `not_verified` with the failing reasons enumerated.
 */
export const verifyDurableCheckpointSealReceipt = (
  receipt: DurableCheckpointSealReceiptValue,
): DurableCheckpointSealReceiptVerificationVerdict => {
  const reasons: Array<DurableCheckpointSealReceiptVerificationReason> = []

  const expectedRef = durableCheckpointSealReceiptRef(
    receipt.windowRef,
    receipt.checkpointDigestRef,
  )
  if (receipt.receiptRef !== expectedRef) {
    reasons.push('receipt_ref_mismatch')
  }
  if (!DurableCheckpointDigestPattern.test(receipt.checkpointDigestRef)) {
    reasons.push('checkpoint_digest_not_content_addressed')
  }
  if (receipt.replicationFactor < MinDurableReplicationFactor) {
    reasons.push('replication_factor_below_durable_minimum')
  }
  if (
    receipt.storedDigestRef !== receipt.checkpointDigestRef ||
    receipt.readbackDigestRef !== receipt.checkpointDigestRef
  ) {
    reasons.push('readback_receipt_digest_mismatch')
  }
  if (receipt.sizeBytes < 1) {
    reasons.push('readback_receipt_size_mismatch')
  }

  const verified = reasons.length === 0

  return {
    authorityBoundary: verificationAuthorityBoundary,
    blockerRef: DurableCheckpointSealBlocker,
    decision: verified ? 'verified' : 'not_verified',
    reasons,
    receiptRef: receipt.receiptRef,
    schemaVersion: DurableCheckpointSealReceiptVerificationSchemaVersion,
    verified,
  }
}

/**
 * Decode an untrusted published receipt and verify it. A receipt that fails to
 * decode (wrong outcome, missing fields, wrong schema version, `publicSafe`
 * absent) yields a `not_verified` verdict — failing toward not-verified rather
 * than trusting an unverifiable artifact.
 */
export const verifyUntrustedDurableCheckpointSealReceipt = (
  input: unknown,
): DurableCheckpointSealReceiptVerificationVerdict => {
  let decoded: DurableCheckpointSealReceiptValue
  try {
    decoded = S.decodeUnknownSync(DurableCheckpointSealReceipt)(input)
  } catch {
    return {
      authorityBoundary: verificationAuthorityBoundary,
      blockerRef: DurableCheckpointSealBlocker,
      decision: 'not_verified',
      reasons: ['receipt_malformed'],
      receiptRef: undefined,
      schemaVersion: DurableCheckpointSealReceiptVerificationSchemaVersion,
      verified: false,
    }
  }
  return verifyDurableCheckpointSealReceipt(decoded)
}
