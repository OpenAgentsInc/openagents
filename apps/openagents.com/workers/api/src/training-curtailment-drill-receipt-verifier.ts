import { Schema as S } from 'effect'

import {
  CurtailmentDrillBlocker,
  CurtailmentDrillPublicSafeRefPattern,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
} from './training-curtailment-drill'
import {
  CurtailmentDrillReceipt,
  curtailmentDrillReceiptRef,
} from './training-curtailment-drill-receipt'

type CurtailmentDrillReceiptValue = typeof CurtailmentDrillReceipt.Type

/**
 * Curtailment-drill receipt VERIFIER for training.marathon_operations.v1.
 *
 * The receipt emitter (training-curtailment-drill-receipt.ts) produces the
 * public-safe artifact the live runtime publishes once a scheduled curtailment
 * drill has actually passed. That is the PRODUCTION side of the receipt contract.
 * This module supplies the missing CONSUMPTION side, mirroring the
 * durable-checkpoint-seal and standby-promotion receipt verifiers: when a consumer
 * later dereferences a published drill receipt — which it must treat as untrusted
 * input, not as something it minted itself — it has to confirm the receipt is
 * authentic and self-consistent before relying on it (for example, before any
 * projection flag could ever flip on the strength of one).
 *
 * Decoding alone is not enough. The receipt schema pins the literal `outcome`,
 * `publicSafe`, `blockerRef`, schema versions, and the `ackSlaMs` / `haltSlaMs`
 * SLA literals, so a decode rejects those, but it does NOT re-check the invariants
 * the EMITTER enforced: the deterministic content-addressed `receiptRef` is derived
 * from `drillRef`, yet the schema stores it as a free string; `drillRef` and
 * `runRef` are unconstrained `S.String`; and the measured `ackLatencyMs` /
 * `haltLatencyMs` are plain ints that the schema never compares against the SLA
 * literals. A forged or tampered receipt can therefore decode cleanly while
 * carrying a `receiptRef` that does not match its drill ref, a non-public-safe
 * `drillRef` or `runRef`, or latencies that actually BREACH the very SLAs the
 * receipt claims it met.
 *
 * The verifier re-derives the canonical receipt ref from the receipt's own drill
 * ref and confirms it matches, re-checks that the drill and run refs are
 * public-safe, and re-checks that the measured ack/halt latencies are within their
 * SLAs. It FAILS TOWARD `not_verified` (it never reports a malformed or
 * inconsistent receipt as verified), mirroring the drill predicate's
 * fail-toward-INCOMPLETE posture.
 *
 * It is contract-level only. A `verified` verdict reports that a published receipt
 * is internally authentic and consistent with the emitter's invariants; it grants
 * no dispatch, settlement, flexible-load-market, promise-state, or green-claim
 * authority, and it does not assert that any real scheduled curtailment drill was
 * ever run — only that the receipt is what it claims to be.
 */

export const CurtailmentDrillReceiptVerificationSchemaVersion =
  'openagents.training.marathon_operations.curtailment_drill_receipt_verification.v1'
export type CurtailmentDrillReceiptVerificationSchemaVersion =
  typeof CurtailmentDrillReceiptVerificationSchemaVersion

export type CurtailmentDrillReceiptVerificationDecision =
  | 'verified'
  | 'not_verified'

export type CurtailmentDrillReceiptVerificationReason =
  | 'receipt_malformed'
  | 'receipt_ref_mismatch'
  | 'drill_ref_not_public_safe'
  | 'run_ref_not_public_safe'
  | 'ack_latency_exceeded'
  | 'halt_latency_exceeded'

export type CurtailmentDrillReceiptVerificationVerdict = Readonly<{
  authorityBoundary: string
  blockerRef: typeof CurtailmentDrillBlocker
  decision: CurtailmentDrillReceiptVerificationDecision
  reasons: ReadonlyArray<CurtailmentDrillReceiptVerificationReason>
  receiptRef: string | undefined
  schemaVersion: CurtailmentDrillReceiptVerificationSchemaVersion
  verified: boolean
}>

const verificationAuthorityBoundary =
  'Curtailment-drill receipt verification confirms that a published receipt is internally authentic and consistent with the emitter invariants (canonical content-addressed ref bound to the drill ref, public-safe drill/run refs, measured ack/halt latencies within their SLAs). A verified verdict grants no dispatch, settlement, flexible-load-market, promise-state, or green-claim authority, does not assert any real scheduled curtailment drill was run, and a not-verified verdict is the safe default.'

/**
 * Verify an already-decoded curtailment-drill receipt. A receipt is verified only
 * when every authenticity invariant holds; any failing invariant routes to
 * `not_verified` with the failing reasons enumerated.
 */
export const verifyCurtailmentDrillReceipt = (
  receipt: CurtailmentDrillReceiptValue,
): CurtailmentDrillReceiptVerificationVerdict => {
  const reasons: Array<CurtailmentDrillReceiptVerificationReason> = []

  const expectedRef = curtailmentDrillReceiptRef(receipt.drillRef)
  if (receipt.receiptRef !== expectedRef) {
    reasons.push('receipt_ref_mismatch')
  }
  if (!CurtailmentDrillPublicSafeRefPattern.test(receipt.drillRef)) {
    reasons.push('drill_ref_not_public_safe')
  }
  if (!CurtailmentDrillPublicSafeRefPattern.test(receipt.runRef)) {
    reasons.push('run_ref_not_public_safe')
  }
  if (receipt.ackLatencyMs > MaxCurtailmentAckLatencyMs) {
    reasons.push('ack_latency_exceeded')
  }
  if (receipt.haltLatencyMs > MaxCurtailmentHaltLatencyMs) {
    reasons.push('halt_latency_exceeded')
  }

  const verified = reasons.length === 0

  return {
    authorityBoundary: verificationAuthorityBoundary,
    blockerRef: CurtailmentDrillBlocker,
    decision: verified ? 'verified' : 'not_verified',
    reasons,
    receiptRef: receipt.receiptRef,
    schemaVersion: CurtailmentDrillReceiptVerificationSchemaVersion,
    verified,
  }
}

/**
 * Decode an untrusted published receipt and verify it. A receipt that fails to
 * decode (wrong outcome, missing fields, wrong schema version, `publicSafe`
 * absent, mismatched SLA literals) yields a `not_verified` verdict — failing
 * toward not-verified rather than trusting an unverifiable artifact.
 */
export const verifyUntrustedCurtailmentDrillReceipt = (
  input: unknown,
): CurtailmentDrillReceiptVerificationVerdict => {
  let decoded: CurtailmentDrillReceiptValue
  try {
    decoded = S.decodeUnknownSync(CurtailmentDrillReceipt)(input)
  } catch {
    return {
      authorityBoundary: verificationAuthorityBoundary,
      blockerRef: CurtailmentDrillBlocker,
      decision: 'not_verified',
      reasons: ['receipt_malformed'],
      receiptRef: undefined,
      schemaVersion: CurtailmentDrillReceiptVerificationSchemaVersion,
      verified: false,
    }
  }
  return verifyCurtailmentDrillReceipt(decoded)
}
