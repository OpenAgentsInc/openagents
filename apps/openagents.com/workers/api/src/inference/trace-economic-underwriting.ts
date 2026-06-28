import { Schema as S } from 'effect'

export const TRACE_ECONOMIC_UNDERWRITING_QUOTE_SCHEMA =
  'openagents.trace_economic_underwriting.quote.v1'

export const OutcomeWarrantyCoverageKind = S.Literals([
  'refund_on_rejection',
  'verified_outcome_sla',
])
export type OutcomeWarrantyCoverageKind =
  typeof OutcomeWarrantyCoverageKind.Type

export const OutcomeWarrantyEvidence = S.Struct({
  traceRef: S.NullOr(S.String),
  verdictRef: S.String,
  verdictState: S.Literals(['accepted', 'rejected', 'pending']),
  tokenUsageEventRefs: S.Array(S.String),
  meteringReceiptRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  usageTruth: S.Literals(['exact', 'estimated', 'missing']),
})
export type OutcomeWarrantyEvidence = typeof OutcomeWarrantyEvidence.Type

export const OutcomeWarrantySla = S.Struct({
  coverageKind: OutcomeWarrantyCoverageKind,
  responseWindowSeconds: S.Number,
  verificationWindowSeconds: S.Number,
  refundCapMsat: S.Number,
})
export type OutcomeWarrantySla = typeof OutcomeWarrantySla.Type

export const OutcomeWarrantyQuoteInput = S.Struct({
  assignmentRef: S.String,
  acceptedOutcomeRef: S.String,
  customerAccountRef: S.String,
  evidence: OutcomeWarrantyEvidence,
  sla: OutcomeWarrantySla,
  premiumMarginBps: S.Number,
  rejectionRiskBps: S.Number,
})
export type OutcomeWarrantyQuoteInput =
  typeof OutcomeWarrantyQuoteInput.Type

export const OutcomeWarrantyQuote = S.Struct({
  schemaVersion: S.Literal(TRACE_ECONOMIC_UNDERWRITING_QUOTE_SCHEMA),
  quoteRef: S.String,
  assignmentRef: S.String,
  acceptedOutcomeRef: S.String,
  customerAccountRef: S.String,
  coverageKind: OutcomeWarrantyCoverageKind,
  state: S.Literals(['offered', 'blocked']),
  premiumMsat: S.Number,
  expectedLossMsat: S.Number,
  refundCapMsat: S.Number,
  rejectionRiskBps: S.Number,
  premiumMarginBps: S.Number,
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  refundExecutionAuthority: S.Literal(false),
  settlementMutationAllowed: S.Literal(false),
  publicClaimEligible: S.Literal(false),
})
export type OutcomeWarrantyQuote = typeof OutcomeWarrantyQuote.Type

const decodeInput = S.decodeUnknownSync(OutcomeWarrantyQuoteInput)
const decodeQuote = S.decodeUnknownSync(OutcomeWarrantyQuote)

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '_')
    .replace(/^_+|_+$/g, '')

const fnv1a32 = (value: string): string => {
  let hash = 0x811c9dc5
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash.toString(16).padStart(8, '0')
}

const stableJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null'
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(
    ([a], [b]) => a.localeCompare(b),
  )
  return `{${entries
    .map(([key, entry]) => `${JSON.stringify(key)}:${stableJson(entry)}`)
    .join(',')}}`
}

const quoteRefForInput = (input: OutcomeWarrantyQuoteInput): string =>
  `quote.trace_underwriting.${safeRefSegment(
    input.acceptedOutcomeRef,
  )}.${fnv1a32(
    stableJson({
      assignmentRef: input.assignmentRef,
      coverageKind: input.sla.coverageKind,
      premiumMarginBps: input.premiumMarginBps,
      refundCapMsat: input.sla.refundCapMsat,
      rejectionRiskBps: input.rejectionRiskBps,
      verdictRef: input.evidence.verdictRef,
    }),
  )}`

const uniqueSorted = (
  values: ReadonlyArray<string | null>,
): ReadonlyArray<string> =>
  Array.from(
    new Set(
      values
        .filter((value): value is string => value !== null)
        .map(value => value.trim())
        .filter(value => value !== ''),
    ),
  ).sort()

const warrantyBlockers = (
  input: OutcomeWarrantyQuoteInput,
): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (
    input.evidence.traceRef === null ||
    input.evidence.traceRef.trim() === ''
  ) {
    blockers.push('blocker.trace_underwriting.public_safe_trace_missing')
  }
  if (input.evidence.verdictState !== 'accepted') {
    blockers.push('blocker.trace_underwriting.accepted_verdict_missing')
  }
  if (input.evidence.usageTruth !== 'exact') {
    blockers.push('blocker.trace_underwriting.exact_usage_missing')
  }
  if (input.evidence.tokenUsageEventRefs.length === 0) {
    blockers.push('blocker.trace_underwriting.token_usage_event_missing')
  }
  if (input.evidence.meteringReceiptRefs.length === 0) {
    blockers.push('blocker.trace_underwriting.metering_receipt_missing')
  }
  if (input.sla.refundCapMsat <= 0) {
    blockers.push('blocker.trace_underwriting.refund_cap_missing')
  }
  if (input.rejectionRiskBps < 0 || input.rejectionRiskBps > 10_000) {
    blockers.push('blocker.trace_underwriting.risk_out_of_range')
  }
  if (input.premiumMarginBps < 0) {
    blockers.push('blocker.trace_underwriting.margin_out_of_range')
  }
  return uniqueSorted(blockers)
}

const warrantyCaveats = (
  input: OutcomeWarrantyQuoteInput,
): ReadonlyArray<string> =>
  uniqueSorted([
    'caveat.trace_underwriting.inert_quote_only',
    'caveat.trace_underwriting.refund_execution_requires_separate_authority',
    input.evidence.settlementReceiptRefs.length === 0
      ? 'caveat.trace_underwriting.settlement_receipt_not_attached'
      : null,
  ])

export const priceOutcomeWarrantyQuote = (
  unknownInput: OutcomeWarrantyQuoteInput,
): OutcomeWarrantyQuote => {
  const input = decodeInput(unknownInput)
  const blockers = warrantyBlockers(input)
  const expectedLossMsat = Math.ceil(
    (input.sla.refundCapMsat * input.rejectionRiskBps) / 10_000,
  )
  const premiumMsat =
    blockers.length > 0
      ? 0
      : Math.ceil(expectedLossMsat * (1 + input.premiumMarginBps / 10_000))

  return decodeQuote({
    acceptedOutcomeRef: input.acceptedOutcomeRef,
    assignmentRef: input.assignmentRef,
    caveatRefs: warrantyCaveats(input),
    coverageKind: input.sla.coverageKind,
    customerAccountRef: input.customerAccountRef,
    evidenceRefs: uniqueSorted([
      input.evidence.traceRef,
      input.evidence.verdictRef,
      ...input.evidence.tokenUsageEventRefs,
      ...input.evidence.meteringReceiptRefs,
      ...input.evidence.settlementReceiptRefs,
    ]),
    expectedLossMsat: blockers.length > 0 ? 0 : expectedLossMsat,
    premiumMarginBps: input.premiumMarginBps,
    premiumMsat,
    publicClaimEligible: false,
    quoteRef: quoteRefForInput(input),
    refundCapMsat: input.sla.refundCapMsat,
    refundExecutionAuthority: false,
    rejectionRiskBps: input.rejectionRiskBps,
    schemaVersion: TRACE_ECONOMIC_UNDERWRITING_QUOTE_SCHEMA,
    settlementMutationAllowed: false,
    state: blockers.length === 0 ? 'offered' : 'blocked',
    blockerRefs: blockers,
  })
}
