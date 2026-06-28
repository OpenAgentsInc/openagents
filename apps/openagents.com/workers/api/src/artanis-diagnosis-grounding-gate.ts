export type ArtanisDiagnosisGateState =
  | 'UNGROUNDED'
  | 'LEDGER_READ'
  | 'DISPATCH_LOG_EXAMINED'
  | 'PROVIDER_VERIFIED'
  | 'GROUNDED'

export type ArtanisDiagnosisClaimKind =
  | 'rate_limited'
  | 'quota_exhausted'
  | 'dispatch_stalled'
  | 'unknown'

export type ArtanisDiagnosisEvidence = Readonly<{
  quotaLedgerReadRef?: string
  supervisorDispatchLog?: Readonly<{
    ref: string
    lastEntriesExamined: number
    outcomes: ReadonlyArray<string>
  }>
  providerRateLimitHeaders?: Readonly<{
    ref: string
    retryAfterSeconds?: number
    xRateLimitReset?: string
  }>
}>

export type ArtanisDiagnosisGateInput = Readonly<{
  claimKind: ArtanisDiagnosisClaimKind
  evidence: ArtanisDiagnosisEvidence
}>

export type ArtanisDiagnosisGate = Readonly<{
  state: ArtanisDiagnosisGateState
  canProposeRemediation: boolean
  requiredRefs: ReadonlyArray<string>
  missingRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

const requiredRefs = [
  'quota-ledger-read',
  'supervisor-dispatch-log',
  'provider-rate-limit-headers',
] as const

const hasText = (value: string | undefined): boolean =>
  value !== undefined && value.trim() !== ''

const providerHeadersVerifyClaim = (
  claimKind: ArtanisDiagnosisClaimKind,
  headers: ArtanisDiagnosisEvidence['providerRateLimitHeaders'],
): boolean => {
  if (claimKind !== 'rate_limited') {
    return true
  }
  return (
    headers !== undefined &&
    hasText(headers.ref) &&
    (typeof headers.retryAfterSeconds === 'number' || hasText(headers.xRateLimitReset))
  )
}

export const evaluateArtanisDiagnosisGroundingGate = (
  input: ArtanisDiagnosisGateInput,
): ArtanisDiagnosisGate => {
  const missingRefs: Array<string> = []
  const blockerRefs: Array<string> = []

  if (!hasText(input.evidence.quotaLedgerReadRef)) {
    missingRefs.push('quota-ledger-read')
  }

  const dispatchLog = input.evidence.supervisorDispatchLog
  if (
    dispatchLog === undefined ||
    !hasText(dispatchLog.ref) ||
    dispatchLog.lastEntriesExamined < 20 ||
    dispatchLog.outcomes.length === 0
  ) {
    missingRefs.push('supervisor-dispatch-log')
  }

  const providerHeaders = input.evidence.providerRateLimitHeaders
  if (!hasText(providerHeaders?.ref)) {
    missingRefs.push('provider-rate-limit-headers')
  }

  if (!providerHeadersVerifyClaim(input.claimKind, providerHeaders)) {
    blockerRefs.push('blocker.artanis.diagnosis.provider_headers_do_not_match_claim')
  }

  const hasLedger = !missingRefs.includes('quota-ledger-read')
  const hasDispatchLog = !missingRefs.includes('supervisor-dispatch-log')
  const hasProviderHeaders = !missingRefs.includes('provider-rate-limit-headers')
  const providerVerified =
    hasProviderHeaders && providerHeadersVerifyClaim(input.claimKind, providerHeaders)

  const state: ArtanisDiagnosisGateState = !hasLedger
    ? 'UNGROUNDED'
    : !hasDispatchLog
      ? 'LEDGER_READ'
      : !hasProviderHeaders
        ? 'DISPATCH_LOG_EXAMINED'
        : !providerVerified
          ? 'PROVIDER_VERIFIED'
          : 'GROUNDED'

  return {
    blockerRefs,
    canProposeRemediation: state === 'GROUNDED',
    missingRefs,
    requiredRefs,
    state,
  }
}

export const artanisDiagnosisGroundingPolicy = (): Readonly<{
  signature: 'autonomous-ops-v1.signature-2.diagnosis-grounding'
  requiredRefs: ReadonlyArray<string>
  stateOrder: ReadonlyArray<ArtanisDiagnosisGateState>
  rule: string
}> => ({
  requiredRefs,
  rule:
    'No root-cause claim or remediation proposal is allowed until quota-ledger-read, supervisor-dispatch-log with the last 20 entries plus outcomes, and provider-rate-limit-headers are present; rate-limited claims must match real Retry-After or X-RateLimit-Reset evidence.',
  signature: 'autonomous-ops-v1.signature-2.diagnosis-grounding',
  stateOrder: [
    'UNGROUNDED',
    'LEDGER_READ',
    'DISPATCH_LOG_EXAMINED',
    'PROVIDER_VERIFIED',
    'GROUNDED',
  ],
})
