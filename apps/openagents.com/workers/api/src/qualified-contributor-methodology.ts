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
  // The SAME provider-confirmed real-bitcoin settlement receipt was used to
  // satisfy prong 3 for two or more counted contributors. Distinct pylonRefs are
  // not enough: a single real settlement cannot back two "distinct real-paid
  // contributors" without inflating the real-paid count.
  SharedSettlementReceipt: 'shared-settlement-receipt-across-contributors',
  // The SAME window-lease ref satisfied prong 1 (admitted) for two or more
  // counted contributors. A lease identifies one contributor's admitted window;
  // reusing it across distinct pylonRefs inflates the admitted-contributor count
  // exactly as a shared settlement inflates the real-paid count.
  SharedLease: 'shared-lease-across-contributors',
  // The SAME replay-verified exact_trace challenge satisfied prong 2 for two or
  // more counted contributors. One piece of verified useful work cannot be
  // credited to two "distinct independent contributors".
  SharedVerifiedWork: 'shared-verified-work-across-contributors',
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
  // The lease refs that satisfied prong 1, when counts is true.
  countedLeaseRefs: ReadonlyArray<string>
  // The replay-verified challenge refs that satisfied prong 2, when counts is true.
  countedVerifiedWorkRefs: ReadonlyArray<string>
  // The receipt refs that satisfied prong 3, when counts is true.
  countedSettlementReceiptRefs: ReadonlyArray<string>
}>

export type QualifiedContributorMethodologyVerdict = Readonly<{
  conforms: boolean
  qualifiedContributorCount: number
  verdicts: ReadonlyArray<QualifiedContributorVerdict>
  reasons: ReadonlyArray<string>
}>

// The full input a run-level verification consumes. Exported so a parser can
// produce exactly this shape from an untrusted document.
export type QualifiedContributorMethodologyInput = Readonly<{
  claimedQualifiedContributorCount: number
  contributors: ReadonlyArray<QualifiedContributorEvidence>
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

  const countedLeaseRefs = evidence.leaseRefs.filter(isNonEmptyRef)
  if (countedLeaseRefs.length === 0) {
    reasons.push(QualifiedContributorReason.NotAdmittedNoLease)
  }

  const countedVerifiedWorkRefs =
    evidence.verifiedExactTraceReplayChallengeRefs.filter(isNonEmptyRef)
  if (countedVerifiedWorkRefs.length === 0) {
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

  const counts = reasons.length === 0
  return {
    pylonRef: evidence.pylonRef,
    counts,
    reasons,
    countedLeaseRefs: counts ? countedLeaseRefs : [],
    countedVerifiedWorkRefs: counts ? countedVerifiedWorkRefs : [],
    countedSettlementReceiptRefs: counts ? countedSettlementReceiptRefs : [],
  }
}

// Verify a whole run's published qualifiedContributorCount against the rule.
// `conforms` is true only when every counted contributor passes all prongs, no
// contributor is double-counted, and the recomputed count equals the claim.
export const verifyQualifiedContributorMethodology = (
  input: QualifiedContributorMethodologyInput,
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

  // Cross-contributor evidence integrity: each counted contributor must be
  // backed by its OWN distinct evidence for every prong. A single lease, a single
  // replay-verified work challenge, or a single real-bitcoin settlement reused
  // across two counted contributors inflates the qualified count even when their
  // pylonRefs differ, so the run does not conform to the rule. Distinct pylonRefs
  // are necessary but not sufficient — the underlying work must be distinct too.
  //
  // The shared-ref test must compare refs ACROSS contributors, so it operates on
  // each contributor's DISTINCT refs (deduped within that contributor) before
  // flattening. Otherwise a single legitimate contributor whose own evidence
  // harmlessly lists the same ref twice (e.g. the same lease recorded from two
  // evidence sources, or one receipt cited twice) would trip the cross-contributor
  // check and a real, conforming run would be falsely flagged
  // `*-across-contributors`. Deduping per contributor keeps genuine cross-
  // contributor sharing caught (each sharer still contributes one copy of the
  // shared ref → a duplicate across the flattened set) while collapsing harmless
  // within-contributor repeats.
  const counted = verdicts.filter(verdict => verdict.counts)
  const hasSharedRef = (refs: ReadonlyArray<string>): boolean =>
    new Set(refs).size !== refs.length
  const flattenPerContributorDistinct = (
    perContributor: ReadonlyArray<ReadonlyArray<string>>,
  ): ReadonlyArray<string> =>
    perContributor.flatMap(refs => [...new Set(refs)])
  if (
    hasSharedRef(
      flattenPerContributorDistinct(counted.map(v => v.countedLeaseRefs)),
    )
  ) {
    reasons.push(QualifiedRunReason.SharedLease)
  }
  if (
    hasSharedRef(
      flattenPerContributorDistinct(counted.map(v => v.countedVerifiedWorkRefs)),
    )
  ) {
    reasons.push(QualifiedRunReason.SharedVerifiedWork)
  }
  if (
    hasSharedRef(
      flattenPerContributorDistinct(
        counted.map(v => v.countedSettlementReceiptRefs),
      ),
    )
  ) {
    reasons.push(QualifiedRunReason.SharedSettlementReceipt)
  }

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

// ---------------------------------------------------------------------------
// Untrusted-input parse boundary.
//
// `verifyQualifiedContributorMethodology` trusts its input is already typed
// `QualifiedContributorMethodologyInput`. But the documented remaining step for
// `consumer_compute_self_serve_scale_methodology_missing` is running the verifier
// against the LIVE run's real evidence — which an auditor loads from an untrusted
// JSON document. Passing such a document straight to the verifier is unsafe:
// missing/mistyped fields would silently misbehave (e.g. a numeric `state`, a
// `contributors` that is an object, a count that is a float), and a leak-prone
// extra field (raw address, balance, internal id) could ride along into a
// published evidence artifact.
//
// `parseQualifiedContributorMethodologyInput` is the deterministic, pure gate
// that closes that hole — mirroring the closed key allowlist + path-qualified
// errors already used by the Spark-helper autostart receipt verifier. It returns
// the typed input ONLY when the document is structurally sound and public-safe;
// otherwise it returns the precise reasons. It performs NO counting and asserts
// NO scale claim — it only makes the existing verifier runnable against a real
// captured JSON document.
// ---------------------------------------------------------------------------

// Closed key allowlists. Any key outside these is rejected: an unknown field is
// exactly how a raw target/balance/credential could be smuggled into a published
// evidence document.
const ALLOWED_DOCUMENT_KEYS: ReadonlySet<string> = new Set<string>([
  'claimedQualifiedContributorCount',
  'contributors',
])

const ALLOWED_CONTRIBUTOR_KEYS: ReadonlySet<string> = new Set<string>([
  'pylonRef',
  'leaseRefs',
  'verifiedExactTraceReplayChallengeRefs',
  'settlementReceipts',
])

const ALLOWED_RECEIPT_KEYS: ReadonlySet<string> = new Set<string>([
  'receiptRef',
  'state',
  'providerConfirmed',
  'realBitcoinMoved',
])

export type QualifiedContributorMethodologyParse =
  | Readonly<{ ok: true; value: QualifiedContributorMethodologyInput }>
  | Readonly<{ ok: false; errors: ReadonlyArray<string> }>

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const collectUnexpectedKeys = (
  record: Record<string, unknown>,
  allowed: ReadonlySet<string>,
  path: string,
  errors: Array<string>,
): void => {
  for (const key of Object.keys(record)) {
    if (!allowed.has(key)) errors.push(`unexpected-key:${path}.${key}`)
  }
}

const parseStringArray = (
  value: unknown,
  path: string,
  errors: Array<string>,
): ReadonlyArray<string> => {
  if (!Array.isArray(value)) {
    errors.push(`not-an-array:${path}`)
    return []
  }
  const out: Array<string> = []
  value.forEach((item, index) => {
    if (typeof item !== 'string') {
      errors.push(`not-a-string:${path}[${index}]`)
      return
    }
    out.push(item)
  })
  return out
}

const parseSettlementReceipt = (
  value: unknown,
  path: string,
  errors: Array<string>,
): QualifiedContributorSettlementEvidence | null => {
  if (!isPlainObject(value)) {
    errors.push(`not-an-object:${path}`)
    return null
  }
  collectUnexpectedKeys(value, ALLOWED_RECEIPT_KEYS, path, errors)

  const before = errors.length
  if (typeof value.receiptRef !== 'string') {
    errors.push(`not-a-string:${path}.receiptRef`)
  }
  if (typeof value.state !== 'string') {
    errors.push(`not-a-string:${path}.state`)
  }
  if (typeof value.providerConfirmed !== 'boolean') {
    errors.push(`not-a-boolean:${path}.providerConfirmed`)
  }
  if (typeof value.realBitcoinMoved !== 'boolean') {
    errors.push(`not-a-boolean:${path}.realBitcoinMoved`)
  }
  if (errors.length !== before) return null

  return {
    receiptRef: value.receiptRef as string,
    state: value.state as string,
    providerConfirmed: value.providerConfirmed as boolean,
    realBitcoinMoved: value.realBitcoinMoved as boolean,
  }
}

const parseContributor = (
  value: unknown,
  path: string,
  errors: Array<string>,
): QualifiedContributorEvidence | null => {
  if (!isPlainObject(value)) {
    errors.push(`not-an-object:${path}`)
    return null
  }
  collectUnexpectedKeys(value, ALLOWED_CONTRIBUTOR_KEYS, path, errors)

  const before = errors.length
  if (typeof value.pylonRef !== 'string') {
    errors.push(`not-a-string:${path}.pylonRef`)
  }
  const leaseRefs = parseStringArray(
    value.leaseRefs,
    `${path}.leaseRefs`,
    errors,
  )
  const verifiedExactTraceReplayChallengeRefs = parseStringArray(
    value.verifiedExactTraceReplayChallengeRefs,
    `${path}.verifiedExactTraceReplayChallengeRefs`,
    errors,
  )

  let settlementReceipts: Array<QualifiedContributorSettlementEvidence> = []
  if (!Array.isArray(value.settlementReceipts)) {
    errors.push(`not-an-array:${path}.settlementReceipts`)
  } else {
    settlementReceipts = value.settlementReceipts
      .map((receipt, index) =>
        parseSettlementReceipt(
          receipt,
          `${path}.settlementReceipts[${index}]`,
          errors,
        ),
      )
      .filter((r): r is QualifiedContributorSettlementEvidence => r !== null)
  }

  if (errors.length !== before) return null

  return {
    pylonRef: value.pylonRef as string,
    leaseRefs,
    verifiedExactTraceReplayChallengeRefs,
    settlementReceipts,
  }
}

/**
 * Parse and validate an untrusted methodology evidence document (e.g. JSON an
 * auditor loaded from a file) into the typed input the run-level verifier
 * consumes. Pure and side-effect-free. Returns `{ ok: true, value }` only when
 * the document is structurally sound and carries no key outside the closed
 * allowlists; otherwise `{ ok: false, errors }` with path-qualified reasons.
 *
 * This neither counts contributors nor asserts any scale claim — it is solely the
 * safe boundary that lets `verifyQualifiedContributorMethodology` be run against
 * real captured evidence.
 */
export const parseQualifiedContributorMethodologyInput = (
  candidate: unknown,
): QualifiedContributorMethodologyParse => {
  const errors: Array<string> = []

  if (!isPlainObject(candidate)) {
    return { ok: false, errors: ['not-an-object:$'] }
  }
  collectUnexpectedKeys(candidate, ALLOWED_DOCUMENT_KEYS, '$', errors)

  const count = candidate.claimedQualifiedContributorCount
  if (
    typeof count !== 'number' ||
    !Number.isInteger(count) ||
    count < 0
  ) {
    errors.push('not-a-non-negative-integer:$.claimedQualifiedContributorCount')
  }

  let contributors: Array<QualifiedContributorEvidence> = []
  if (!Array.isArray(candidate.contributors)) {
    errors.push('not-an-array:$.contributors')
  } else {
    contributors = candidate.contributors
      .map((contributor, index) =>
        parseContributor(contributor, `$.contributors[${index}]`, errors),
      )
      .filter((c): c is QualifiedContributorEvidence => c !== null)
  }

  if (errors.length > 0) {
    return { ok: false, errors }
  }

  return {
    ok: true,
    value: {
      claimedQualifiedContributorCount: count as number,
      contributors,
    },
  }
}

// ---------------------------------------------------------------------------
// Safe single entry: parse -> verify, fused.
//
// The documented remaining step for this blocker is "run the verifier against
// the live run's real evidence". That real evidence arrives as an UNTRUSTED JSON
// document, so the correct flow is two steps: parse it through the boundary, then
// (only if it parsed) verify the typed value. Exposing both halves separately
// leaves a footgun — a caller can skip the parse boundary entirely by
// type-asserting the raw document straight into `verifyQualifiedContributorMethodology`,
// silently defeating the closed key allowlist and type checks the boundary
// exists to enforce. `verifyQualifiedContributorMethodologyDocument` removes that
// footgun: it is the single public entry an auditor runs against a real captured
// document, and the parse boundary is unbypassable by construction. It performs
// NO counting beyond what the existing verifier already does and asserts NO scale
// claim — it only fuses the two steps so "run against real evidence" is one call.
// ---------------------------------------------------------------------------

export type QualifiedContributorMethodologyDocumentResult =
  // The document failed the untrusted-input parse boundary; nothing was verified.
  | Readonly<{ ok: false; errors: ReadonlyArray<string> }>
  // The document parsed; `verdict` is the run-level conformance result.
  | Readonly<{ ok: true; verdict: QualifiedContributorMethodologyVerdict }>

/**
 * Parse an untrusted methodology evidence document (e.g. JSON an auditor loaded
 * from a file) and, only if it is structurally sound and public-safe, run the
 * run-level conformance verifier over it. Pure and side-effect-free.
 *
 * Returns `{ ok: false, errors }` with path-qualified parse reasons when the
 * document does not pass the boundary (nothing is verified in that case), or
 * `{ ok: true, verdict }` with the conformance verdict otherwise. This is the
 * single safe entry point for the documented "run the verifier against the live
 * run's real evidence" step — it makes the parse boundary unbypassable. It
 * neither counts beyond the existing rule nor asserts any scale claim.
 */
export const verifyQualifiedContributorMethodologyDocument = (
  candidate: unknown,
): QualifiedContributorMethodologyDocumentResult => {
  const parsed = parseQualifiedContributorMethodologyInput(candidate)
  if (!parsed.ok) {
    return { ok: false, errors: parsed.errors }
  }
  return {
    ok: true,
    verdict: verifyQualifiedContributorMethodology(parsed.value),
  }
}
