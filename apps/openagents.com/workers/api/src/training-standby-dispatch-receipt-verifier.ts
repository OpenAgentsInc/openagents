import { Schema as S } from 'effect'

import {
  StandbyDispatchBlocker,
  StandbyDispatchPublicSafeRefPattern,
} from './training-standby-dispatch'
import {
  StandbyDispatchReceipt,
  standbyDispatchReceiptRef,
} from './training-standby-dispatch-receipt'

type StandbyDispatchReceiptValue = typeof StandbyDispatchReceipt.Type

/**
 * Standby-promotion receipt VERIFIER for training.marathon_operations.v1.
 *
 * The receipt emitter (training-standby-dispatch-receipt.ts) produces the
 * public-safe artifact the live runtime publishes once a standby has actually been
 * promoted into a live collective. That is the PRODUCTION side of the receipt
 * contract. This module supplies the missing CONSUMPTION side, mirroring the
 * durable-checkpoint-seal receipt verifier: when a consumer later dereferences a
 * published promotion receipt — which it must treat as untrusted input, not as
 * something it minted itself — it has to confirm the receipt is authentic and
 * self-consistent before relying on it (for example, before any projection flag
 * could ever flip on the strength of one).
 *
 * Decoding alone is not enough. The receipt schema pins the literal `outcome`,
 * `publicSafe`, `blockerRef`, and schema versions, so a decode rejects those, but
 * it does NOT re-check the binding the EMITTER enforced: the deterministic
 * content-addressed `receiptRef` is derived from `runRef` + `standbyContributorRef`,
 * yet the schema stores it as a free string, and the ref fields themselves are
 * unconstrained `S.String`. A forged or tampered receipt can therefore decode
 * cleanly while carrying a `receiptRef` that does not match its run/standby fields,
 * or a non-public-safe `runRef`, `standbyContributorRef`, or `promotedIntoWindowRef`.
 *
 * The verifier re-derives the canonical receipt ref from the receipt's own
 * run/standby fields and confirms it matches, and re-checks that the run, standby,
 * and promoted-window refs are public-safe. It FAILS TOWARD `not_verified` (it
 * never reports a malformed or inconsistent receipt as verified), mirroring the
 * dispatch predicate's fail-toward-HOLD posture.
 *
 * It is contract-level only. A `verified` verdict reports that a published receipt
 * is internally authentic and consistent with the emitter's invariants; it grants
 * no dispatch, settlement, promise-state, or green-claim authority, and it does not
 * assert that any real standby was ever promoted into a live run — only that the
 * receipt is what it claims to be.
 */

export const StandbyDispatchReceiptVerificationSchemaVersion =
  'openagents.training.marathon_operations.standby_dispatch_receipt_verification.v1'
export type StandbyDispatchReceiptVerificationSchemaVersion =
  typeof StandbyDispatchReceiptVerificationSchemaVersion

export type StandbyDispatchReceiptVerificationDecision =
  | 'verified'
  | 'not_verified'

export type StandbyDispatchReceiptVerificationReason =
  | 'receipt_malformed'
  | 'receipt_ref_mismatch'
  | 'run_ref_not_public_safe'
  | 'standby_contributor_ref_not_public_safe'
  | 'promoted_window_ref_not_public_safe'

export type StandbyDispatchReceiptVerificationVerdict = Readonly<{
  authorityBoundary: string
  blockerRef: typeof StandbyDispatchBlocker
  decision: StandbyDispatchReceiptVerificationDecision
  reasons: ReadonlyArray<StandbyDispatchReceiptVerificationReason>
  receiptRef: string | undefined
  schemaVersion: StandbyDispatchReceiptVerificationSchemaVersion
  verified: boolean
}>

const verificationAuthorityBoundary =
  'Standby-promotion receipt verification confirms that a published receipt is internally authentic and consistent with the emitter invariants (canonical content-addressed ref bound to run + standby refs, public-safe run/standby/promoted-window refs). A verified verdict grants no dispatch, settlement, promise-state, or green-claim authority, does not assert any real standby was promoted into a live run, and a not-verified verdict is the safe default.'

/**
 * Verify an already-decoded standby-promotion receipt. A receipt is verified only
 * when every authenticity invariant holds; any failing invariant routes to
 * `not_verified` with the failing reasons enumerated.
 */
export const verifyStandbyDispatchReceipt = (
  receipt: StandbyDispatchReceiptValue,
): StandbyDispatchReceiptVerificationVerdict => {
  const reasons: Array<StandbyDispatchReceiptVerificationReason> = []

  const expectedRef = standbyDispatchReceiptRef(
    receipt.runRef,
    receipt.standbyContributorRef,
  )
  if (receipt.receiptRef !== expectedRef) {
    reasons.push('receipt_ref_mismatch')
  }
  if (!StandbyDispatchPublicSafeRefPattern.test(receipt.runRef)) {
    reasons.push('run_ref_not_public_safe')
  }
  if (
    !StandbyDispatchPublicSafeRefPattern.test(receipt.standbyContributorRef)
  ) {
    reasons.push('standby_contributor_ref_not_public_safe')
  }
  if (!StandbyDispatchPublicSafeRefPattern.test(receipt.promotedIntoWindowRef)) {
    reasons.push('promoted_window_ref_not_public_safe')
  }

  const verified = reasons.length === 0

  return {
    authorityBoundary: verificationAuthorityBoundary,
    blockerRef: StandbyDispatchBlocker,
    decision: verified ? 'verified' : 'not_verified',
    reasons,
    receiptRef: receipt.receiptRef,
    schemaVersion: StandbyDispatchReceiptVerificationSchemaVersion,
    verified,
  }
}

/**
 * Decode an untrusted published receipt and verify it. A receipt that fails to
 * decode (wrong outcome, missing fields, wrong schema version, `publicSafe`
 * absent) yields a `not_verified` verdict — failing toward not-verified rather
 * than trusting an unverifiable artifact.
 */
export const verifyUntrustedStandbyDispatchReceipt = (
  input: unknown,
): StandbyDispatchReceiptVerificationVerdict => {
  let decoded: StandbyDispatchReceiptValue
  try {
    decoded = S.decodeUnknownSync(StandbyDispatchReceipt)(input)
  } catch {
    return {
      authorityBoundary: verificationAuthorityBoundary,
      blockerRef: StandbyDispatchBlocker,
      decision: 'not_verified',
      reasons: ['receipt_malformed'],
      receiptRef: undefined,
      schemaVersion: StandbyDispatchReceiptVerificationSchemaVersion,
      verified: false,
    }
  }
  return verifyStandbyDispatchReceipt(decoded)
}
