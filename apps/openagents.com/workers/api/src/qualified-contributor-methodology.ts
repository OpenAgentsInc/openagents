// Qualified-contributor (participant/scale) methodology conformance verifier.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing
//
// This is a pure, side-effect-free enforcement of the WRITTEN participant/scale
// counting rule documented in
//   docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md
// It turns that prose rule into an auditable gate: given the per-contributor
// evidence behind a run's published `qualifiedContributorCount`, it recomputes
// the count under the authoritative rule and confirms the published number is
// neither inflated nor under-counted. It changes no promise state and asserts no
// scale claim; it only verifies that a claimed count CONFORMS to the rule.
//
// The authoritative rule (verbatim prongs): a participant is counted as a
// qualified contributor only if ALL hold —
//   1. admitted (holds/held a real window lease against the run), AND
//   2. produced accepted, replay-verified useful work (a Worker-D1
//      exact_trace_replay verification challenge in state Verified joined to
//      that contributor's lease), AND
//   3. has a public-safe, provider-confirmed settlement receipt ref linked to
//      that run (state settled, providerConfirmed true, realBitcoinMoved true).
//
// Never counted: raw registrations / first-run installs, stale/live heartbeats
// with no accepted verified work, and pending/offered/claimed/wallet-side or
// simulation-only (realBitcoinMoved:false) receipts. Note this closes a gap the
// in-line `qualifiedContributorRefs` join leaves to its caller: it explicitly
// rejects simulation and non-settled receipts rather than trusting that the
// supplied receipt-ref map was pre-filtered.

// The settled terminal state — the only receipt state that can be counted.
export const QUALIFIED_SETTLEMENT_STATE = 'settled' as const

// Reason codes returned per contributor and at the run level. Stable strings so
// callers/tests can assert on them without parsing prose.
export const QualifiedContributorReason = {
  PylonRefEmpty: 'pylon-ref-empty',
  NotAdmittedNoLease: 'not-admitted-no-lease',
  NoReplayVerifiedWork: 'no-accepted-replay-verified-work',
  NoSettlementReceipt: 'no-settlement-receipt',
  SettlementNotSettledState: 'settlement-not-settled-state',
  SettlementNotProviderConfirmed: 'settlement-not-provider-confirmed',
  SettlementSimulationOnly: 'settlement-simulation-only-not-counted',
} as const

export const QualifiedRunReason = {
  ClaimedCountMismatch: 'claimed-count-mismatch',
  DuplicateContributor: 'duplicate-contributor',
} as const

export type QualifiedContributorSettlementEvidence = Readonly<{
  receiptRef: string
  // Receipt lifecycle state; only QUALIFIED_SETTLEMENT_STATE can count.
  state: string
  // Provider (not wallet-side claim) confirmed the settlement.
  providerConfirmed: boolean
  // Real bitcoin actually moved; simulation receipts (false) never count.
  realBitcoinMoved: boolean
}>

export type QualifiedContributorEvidence = Readonly<{
  pylonRef: string
  // Prong 1 — admitted: lease refs the contributor holds/held against the run.
  leaseRefs: ReadonlyArray<string>
  // Prong 2 — accepted, replay-verified useful work: refs of Worker-D1
  // exact_trace_replay verification challenges in state Verified for this run.
  verifiedExactTraceReplayChallengeRefs: ReadonlyArray<string>
  // Prong 3 — settlement receipts linked to this run for this contributor.
  settlementReceipts: ReadonlyArray<QualifiedContributorSettlementEvidence>
}>

export type QualifiedContributorVerdict = Readonly<{
  pylonRef: string
  counts: boolean
  reasons: ReadonlyArray<string>
  // The receipt refs that satisfied prong 3, when counts is true.
  countedSettlementReceiptRefs: ReadonlyArray<string>
}>

export type QualifiedContributorMethodologyVerdict = Readonly<{
  conforms: boolean
  qualifiedContributorCount: number
  verdicts: ReadonlyArray<QualifiedContributorVerdict>
  reasons: ReadonlyArray<string>
}>

const isNonEmptyRef = (value: string): boolean =>
  typeof value === 'string' && value.trim().length > 0

const countingSettlementReceiptRefs = (
  receipts: ReadonlyArray<QualifiedContributorSettlementEvidence>,
): ReadonlyArray<string> =>
  receipts
    .filter(
      receipt =>
        isNonEmptyRef(receipt.receiptRef) &&
        receipt.state === QUALIFIED_SETTLEMENT_STATE &&
        receipt.providerConfirmed === true &&
        receipt.realBitcoinMoved === true,
    )
    .map(receipt => receipt.receiptRef)

const settlementReasons = (
  receipts: ReadonlyArray<QualifiedContributorSettlementEvidence>,
): ReadonlyArray<string> => {
  if (receipts.length === 0) {
    return [QualifiedContributorReason.NoSettlementReceipt]
  }

  const reasons: Array<string> = []
  // Surface the most specific exclusion that applies across the receipt set.
  if (receipts.some(receipt => receipt.realBitcoinMoved !== true)) {
    reasons.push(QualifiedContributorReason.SettlementSimulationOnly)
  }
  if (receipts.some(receipt => receipt.state !== QUALIFIED_SETTLEMENT_STATE)) {
    reasons.push(QualifiedContributorReason.SettlementNotSettledState)
  }
  if (receipts.some(receipt => receipt.providerConfirmed !== true)) {
    reasons.push(QualifiedContributorReason.SettlementNotProviderConfirmed)
  }
  return reasons
}

// Verify a single contributor against the three-prong rule.
export const verifyQualifiedContributor = (
  evidence: QualifiedContributorEvidence,
): QualifiedContributorVerdict => {
  const reasons: Array<string> = []

  if (!isNonEmptyRef(evidence.pylonRef)) {
    reasons.push(QualifiedContributorReason.PylonRefEmpty)
  }

  const admitted = evidence.leaseRefs.some(isNonEmptyRef)
  if (!admitted) {
    reasons.push(QualifiedContributorReason.NotAdmittedNoLease)
  }

  const replayVerified =
    evidence.verifiedExactTraceReplayChallengeRefs.some(isNonEmptyRef)
  if (!replayVerified) {
    reasons.push(QualifiedContributorReason.NoReplayVerifiedWork)
  }

  const countedSettlementReceiptRefs = countingSettlementReceiptRefs(
    evidence.settlementReceipts,
  )
  if (countedSettlementReceiptRefs.length === 0) {
    for (const reason of settlementReasons(evidence.settlementReceipts)) {
      reasons.push(reason)
    }
  }

  return {
    pylonRef: evidence.pylonRef,
    counts: reasons.length === 0,
    reasons,
    countedSettlementReceiptRefs:
      reasons.length === 0 ? countedSettlementReceiptRefs : [],
  }
}

// Verify a whole run's published qualifiedContributorCount against the rule.
// `conforms` is true only when every counted contributor passes all prongs, no
// contributor is double-counted, and the recomputed count equals the claim.
export const verifyQualifiedContributorMethodology = (
  input: Readonly<{
    claimedQualifiedContributorCount: number
    contributors: ReadonlyArray<QualifiedContributorEvidence>
  }>,
): QualifiedContributorMethodologyVerdict => {
  const verdicts = input.contributors.map(verifyQualifiedContributor)
  const reasons: Array<string> = []

  const countedRefs = verdicts
    .filter(verdict => verdict.counts)
    .map(verdict => verdict.pylonRef)
  const distinctCountedRefs = new Set(countedRefs)
  if (distinctCountedRefs.size !== countedRefs.length) {
    reasons.push(QualifiedRunReason.DuplicateContributor)
  }

  const qualifiedContributorCount = distinctCountedRefs.size
  if (qualifiedContributorCount !== input.claimedQualifiedContributorCount) {
    reasons.push(QualifiedRunReason.ClaimedCountMismatch)
  }

  return {
    conforms: reasons.length === 0,
    qualifiedContributorCount,
    verdicts,
    reasons,
  }
}
