// Paid-privacy / confidential-compute capture opt-OUT (openagents #6295,
// child of #6293, epic #6206).
//
// POLICY (Episode 243, the data-market thesis): the free API is captured by
// default and may train the next generation of models; BUT "if you want super
// privacy of your data, you have to pay for that, and/or configure the
// confidential compute module." Paid-for-privacy / confidential-compute callers
// are the OPT-OUT: their traffic is NEVER captured.
//
// This is the inverse fail-safe of the free-tier gate. The free-tier gate is
// fail-closed-TO-PAID (a read error => treat as paid => normal 402). This
// resolver is fail-closed-TO-PRIVATE: if we cannot determine the privacy
// entitlement SAFELY (a read error, an unconfigured seam), we treat the caller
// as paying-for-privacy and do NOT capture. Over-excluding from capture is the
// safe failure mode — we never capture a caller we are unsure about.
//
// The capture decision at the chat seam is:
//     captureDefault = freeTier.free && !paidPrivacy.enabled
// so a paid-privacy caller is excluded regardless of the global capture flag,
// the free-tier signal, or the per-request state.
//
// SIGNALS (any one marks the caller paid-privacy):
//   1. An account-level privacy entitlement row
//      (`inference_privacy_entitlements`, keyed by account_ref). This is the
//      durable, per-account "zero-retention / confidential" marker.
//   2. The confidential-compute module being configured for the deployment
//      (env `INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED`). When the deployment is
//      running in confidential-compute mode, NOTHING is captured — the explicit
//      exclusion signal named in the issue.
//
// PUBLIC-SAFE: this module reads only an account_ref and bounded flags; it never
// reads, logs, or returns prompts, completions, wallet/payment material, raw
// tokens, or secrets. The decision is a single boolean; no account/payment
// material is surfaced in any ref.

import type { InferenceEntitlementsGateReads } from '../inference-entitlements-store'

const ON_TOKENS = new Set(['1', 'on', 'true', 'yes'])

// Fail-closed flag read. Absent / non-string / any non-on value => disabled.
export const isConfidentialComputeEnabled = (value: unknown): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())

export const INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED_ENV_KEY =
  'INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED' as const

export type PaidPrivacyDecision = Readonly<{
  // True => the caller is paying for privacy / confidential-compute => NEVER
  // captured. Fail-closed: an unsafe/unknown determination resolves to true.
  enabled: boolean
  // Public-safe reason ref for observability (never an account/payment value).
  reasonRef: string
}>

export const PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE =
  'reason.privacy.confidential_compute_mode' as const
export const PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT =
  'reason.privacy.account_entitlement' as const
export const PAID_PRIVACY_REASON_READ_ERROR =
  'reason.privacy.read_error_fail_closed' as const
export const PAID_PRIVACY_REASON_NONE = 'reason.privacy.none' as const

// Plain-async read of whether an account carries the privacy entitlement.
// FAIL-CLOSED-TO-PRIVATE: any read error returns TRUE (treat as paid-privacy =>
// do not capture). A missing row (no entitlement) returns FALSE (capturable).
export const readAccountPaidPrivacy = async (
  db: D1Database,
  accountRef: string,
): Promise<PaidPrivacyDecision> => {
  try {
    const row = await db
      .prepare(
        `SELECT account_ref FROM inference_privacy_entitlements WHERE account_ref = ? LIMIT 1`,
      )
      .bind(accountRef)
      .first<{ account_ref: string }>()
    return row !== null
      ? { enabled: true, reasonRef: PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT }
      : { enabled: false, reasonRef: PAID_PRIVACY_REASON_NONE }
  } catch {
    // Fail-closed-to-private: we could not determine the entitlement, so we do
    // NOT capture (the inverse of the free-tier gate's fail-closed-to-paid).
    return { enabled: true, reasonRef: PAID_PRIVACY_REASON_READ_ERROR }
  }
}

export type PaidPrivacyResolver = (
  accountRef: string,
) => Promise<PaidPrivacyDecision>

export type PaidPrivacyResolverDeps = Readonly<{
  db: D1Database
  // When true (the deployment runs in confidential-compute mode), EVERY caller
  // is treated as paid-privacy and nothing is captured, regardless of the
  // per-account row. This is the deployment-wide explicit exclusion signal.
  confidentialComputeEnabled: boolean
  // KS-8.9 (#8320): routed enforcement read (compare/postgres modes).
  // Absent => the untouched inline D1 read. Errors stay FAIL-CLOSED-TO-
  // PRIVATE, exactly like readAccountPaidPrivacy.
  gateReads?:
    | Pick<InferenceEntitlementsGateReads, 'privacyEntitlementExists'>
    | undefined
}>

// Build the seam the chat route calls alongside `checkFreeTier`. Returns the
// paid-privacy decision for an account. Confidential-compute mode short-circuits
// to enabled=true (exclude everyone); otherwise the per-account row decides,
// fail-closed-to-private on a read error.
export const makePaidPrivacyResolver = (
  deps: PaidPrivacyResolverDeps,
): PaidPrivacyResolver => {
  return async (accountRef: string): Promise<PaidPrivacyDecision> => {
    if (deps.confidentialComputeEnabled) {
      return {
        enabled: true,
        reasonRef: PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE,
      }
    }
    if (deps.gateReads !== undefined) {
      try {
        const exists =
          await deps.gateReads.privacyEntitlementExists(accountRef)
        return exists
          ? {
              enabled: true,
              reasonRef: PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT,
            }
          : { enabled: false, reasonRef: PAID_PRIVACY_REASON_NONE }
      } catch {
        // Fail-closed-to-private (same as readAccountPaidPrivacy).
        return { enabled: true, reasonRef: PAID_PRIVACY_REASON_READ_ERROR }
      }
    }
    return readAccountPaidPrivacy(deps.db, accountRef)
  }
}
