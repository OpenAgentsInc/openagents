/**
 * Blueprint Signature 2 — `diagnosis-grounding`
 *
 * No root-cause claim reaches remediation without the evidence behind it.
 *
 * Ordered state machine:
 *   UNGROUNDED -> LEDGER_READ -> DISPATCH_LOG_EXAMINED -> PROVIDER_VERIFIED -> GROUNDED
 *
 * Only GROUNDED unlocks proposing a remediation.
 */

export const DIAGNOSIS_GROUNDING_STATES = [
  "UNGROUNDED",
  "LEDGER_READ",
  "DISPATCH_LOG_EXAMINED",
  "PROVIDER_VERIFIED",
  "GROUNDED",
] as const

export type DiagnosisGroundingState =
  (typeof DIAGNOSIS_GROUNDING_STATES)[number]

export const DIAGNOSIS_GROUNDING_EVIDENCE = {
  quotaLedgerRead: "evidence://diagnosis/quota-ledger-read",
  supervisorDispatchLog: "evidence://diagnosis/supervisor-dispatch-log",
  providerRateLimitHeaders: "evidence://diagnosis/provider-rate-limit-headers",
} as const

export type DiagnosisGroundingEvidenceRef =
  (typeof DIAGNOSIS_GROUNDING_EVIDENCE)[keyof typeof DIAGNOSIS_GROUNDING_EVIDENCE]

export interface ProviderRateLimitHeaders {
  readonly retryAfter?: string | null
  readonly xRateLimitReset?: string | null
  readonly statusCode?: number | null
}

export interface DiagnosisGroundingInputs {
  readonly claimedRootCause: string
  /** evidence://diagnosis/quota-ledger-read */
  readonly quotaLedgerSnapshot: unknown
  /** evidence://diagnosis/supervisor-dispatch-log */
  readonly supervisorDispatchLog: ReadonlyArray<unknown>
  /** evidence://diagnosis/provider-rate-limit-headers */
  readonly accountRateLimitHeaders: ProviderRateLimitHeaders | null
}

export interface DiagnosisGroundingResult {
  readonly state: DiagnosisGroundingState
  readonly canProposeRemediation: boolean
  readonly identity: Readonly<{ readonly claimedRootCause: string }>
  readonly satisfiedEvidence: ReadonlyArray<DiagnosisGroundingEvidenceRef>
  readonly missingEvidence: ReadonlyArray<DiagnosisGroundingEvidenceRef>
  readonly locked: boolean
  readonly lockedAt: DiagnosisGroundingState | null
  readonly blockedReason: string | null
}

const rateLimitClaimPattern = /\b(rate[- ]?limit(?:ed|ing)?|429|retry[- ]?after)\b/i

const hasEvidence = (value: unknown): boolean =>
  value !== null && value !== undefined

const hasProviderRateLimitHeaders = (
  headers: ProviderRateLimitHeaders | null,
): boolean => {
  if (headers === null) {
    return false
  }
  return (
    headers.statusCode === 429 ||
    (typeof headers.retryAfter === "string" && headers.retryAfter.trim().length > 0) ||
    (typeof headers.xRateLimitReset === "string" && headers.xRateLimitReset.trim().length > 0)
  )
}

export function diagnosisClaimsRateLimit(claimedRootCause: string): boolean {
  return rateLimitClaimPattern.test(claimedRootCause)
}

export function evaluateDiagnosisGrounding(
  inputs: DiagnosisGroundingInputs,
): DiagnosisGroundingResult {
  const satisfied: Array<DiagnosisGroundingEvidenceRef> = []
  const identity = { claimedRootCause: inputs.claimedRootCause }

  const lock = (
    state: DiagnosisGroundingState,
    lockedAt: DiagnosisGroundingState,
    blockedReason: string,
    missing: ReadonlyArray<DiagnosisGroundingEvidenceRef>,
  ): DiagnosisGroundingResult => ({
    state,
    canProposeRemediation: false,
    identity,
    satisfiedEvidence: satisfied,
    missingEvidence: missing,
    locked: true,
    lockedAt,
    blockedReason,
  })

  if (!hasEvidence(inputs.quotaLedgerSnapshot)) {
    return lock("UNGROUNDED", "LEDGER_READ", "quota ledger snapshot was not read", [
      DIAGNOSIS_GROUNDING_EVIDENCE.quotaLedgerRead,
      DIAGNOSIS_GROUNDING_EVIDENCE.supervisorDispatchLog,
      DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders,
    ])
  }
  satisfied.push(DIAGNOSIS_GROUNDING_EVIDENCE.quotaLedgerRead)

  if (!Array.isArray(inputs.supervisorDispatchLog) || inputs.supervisorDispatchLog.length === 0) {
    return lock(
      "LEDGER_READ",
      "DISPATCH_LOG_EXAMINED",
      "supervisor dispatch log evidence is missing",
      [
        DIAGNOSIS_GROUNDING_EVIDENCE.supervisorDispatchLog,
        DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders,
      ],
    )
  }
  satisfied.push(DIAGNOSIS_GROUNDING_EVIDENCE.supervisorDispatchLog)

  const claimsRateLimit = diagnosisClaimsRateLimit(inputs.claimedRootCause)
  if (claimsRateLimit && !hasProviderRateLimitHeaders(inputs.accountRateLimitHeaders)) {
    return lock(
      "DISPATCH_LOG_EXAMINED",
      "PROVIDER_VERIFIED",
      "rate-limit diagnosis requires actual provider 429 / Retry-After / X-RateLimit-Reset headers",
      [DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders],
    )
  }
  satisfied.push(DIAGNOSIS_GROUNDING_EVIDENCE.providerRateLimitHeaders)

  return {
    state: "GROUNDED",
    canProposeRemediation: true,
    identity,
    satisfiedEvidence: satisfied,
    missingEvidence: [],
    locked: false,
    lockedAt: null,
    blockedReason: null,
  }
}
