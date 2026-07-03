import { Schema as S } from 'effect'

import {
  buildBusinessQuickWinReceipt,
  type BusinessQuickWinReceipt,
} from './business-quick-win-receipt'

export const QA_SWARM_HOSTED_RUN_RECEIPT_SCHEMA_VERSION =
  'openagents.qa_swarm.hosted_run_receipt.v1'

export const QA_SWARM_HOSTED_RUN_METERING_SCHEMA_VERSION =
  'openagents.qa_swarm.hosted_run_metering.v1'

export const QA_SWARM_ENGAGEMENT_RECEIPT_SCHEMA_VERSION =
  'openagents.qa_swarm.engagement_receipt.v1'

export const QA_SWARM_OWNER_ARM_TOKEN = 'owner.arm.qa_swarm_engagement.v1'

const PUBLIC_REF_PATTERN =
  /^[a-z][a-z0-9_-]*(\.[a-z0-9][a-z0-9_-]*){2,}$/i

const PRIVATE_REF_PATTERN =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|macaroon|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|invoice|log|payment|payload|payout|prompt|provider|record|runner|run[_-]?log|source|state|target|telemetry|text|trace)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

export const QaSwarmHostedRunVerdict = S.Literals([
  'passed',
  'failed',
  'warning',
  'inconclusive',
])
export type QaSwarmHostedRunVerdict = typeof QaSwarmHostedRunVerdict.Type

export const QaSwarmUsageTruth = S.Literals(['exact'])
export type QaSwarmUsageTruth = typeof QaSwarmUsageTruth.Type

export const QaSwarmMeteringSource = S.Literals([
  'runner_report',
  'provider_usage',
  'ledger_reconciliation',
])
export type QaSwarmMeteringSource = typeof QaSwarmMeteringSource.Type

export const QaSwarmHostedRunMeteringRow = S.Struct({
  schemaVersion: S.Literal(QA_SWARM_HOSTED_RUN_METERING_SCHEMA_VERSION),
  rowRef: S.String,
  runRef: S.String,
  provider: S.Literal('qa-swarm-hosted-run'),
  model: S.Literal('openagents/qa-swarm-hosted-run'),
  usageTruth: QaSwarmUsageTruth,
  demandKind: S.Literal('hosted_run'),
  demandSource: S.Literal('qa_swarm_hosted_run'),
  source: QaSwarmMeteringSource,
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  totalTokens: S.Number,
})
export type QaSwarmHostedRunMeteringRow =
  typeof QaSwarmHostedRunMeteringRow.Type

export const QaSwarmHostedRunReceipt = S.Struct({
  schemaVersion: S.Literal(QA_SWARM_HOSTED_RUN_RECEIPT_SCHEMA_VERSION),
  receiptKind: S.Literal('qa_swarm_hosted_run'),
  receiptRef: S.String,
  runRef: S.String,
  projectionRef: S.String,
  verdict: QaSwarmHostedRunVerdict,
  traceRefs: S.Array(S.String),
  coverageRefs: S.Array(S.String),
  videoRefs: S.Array(S.String),
  distilledTestRefs: S.Array(S.String),
  meteringRowRefs: S.Array(S.String),
  exactTokenTotal: S.Number,
  publicSafetyRefs: S.Array(S.String),
  settlement: S.Struct({
    state: S.Literal('inert_owner_armed_required'),
    movedMoney: S.Literal(false),
    ownerArmingRef: S.Literal('NEEDS_OWNER.qa_swarm_hosted_run_engagement_arming'),
  }),
})
export type QaSwarmHostedRunReceipt = typeof QaSwarmHostedRunReceipt.Type

export class QaSwarmHostedRunReceiptInvariantError extends S.TaggedErrorClass<QaSwarmHostedRunReceiptInvariantError>()(
  'QaSwarmHostedRunReceiptInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

export type QaSwarmHostedRunMeteringInput = Readonly<{
  rowRef: string
  runRef: string
  source: QaSwarmMeteringSource
  usageTruth: 'exact' | 'estimated' | 'synthetic' | 'missing'
  inputTokens: number
  outputTokens: number
  reasoningTokens?: number
  cacheReadTokens?: number
  totalTokens?: number
}>

export type QaSwarmHostedRunReceiptInput = Readonly<{
  receiptRef: string
  runRef: string
  projectionRef: string
  verdict: QaSwarmHostedRunVerdict
  traceRefs: ReadonlyArray<string>
  coverageRefs: ReadonlyArray<string>
  videoRefs?: ReadonlyArray<string>
  distilledTestRefs?: ReadonlyArray<string>
  meteringRows: ReadonlyArray<QaSwarmHostedRunMeteringRow>
  publicSafetyRefs: ReadonlyArray<string>
}>

export type QaSwarmEngagementReceipt = Readonly<{
  schemaVersion: typeof QA_SWARM_ENGAGEMENT_RECEIPT_SCHEMA_VERSION
  receiptKind: 'qa_swarm_engagement'
  hostedRunReceiptRef: string
  businessQuickWinReceipt: BusinessQuickWinReceipt
  paymentMode: 'inert_until_owner_armed'
  movedMoney: false
}>

export type QaSwarmEngagementReceiptInput = Readonly<{
  signupId: string
  hostedRunReceipt: QaSwarmHostedRunReceipt
  quickWinScopedRef: string
  outcomeAcceptedRef?: string | null
  buyerPaidRef?: string | null
  providerSettledRef?: string | null
  ownerArmToken?: string
}>

const trimmedOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const requirePublicRef = (value: string, field: string): string => {
  const ref = trimmedOrNull(value)
  if (ref === null) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (!PUBLIC_REF_PATTERN.test(ref) || PRIVATE_REF_PATTERN.test(ref)) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `${field} must be a public-safe dereferenceable ref.`,
    })
  }
  return ref
}

const requirePublicRefs = (
  values: ReadonlyArray<string>,
  field: string,
): ReadonlyArray<string> => {
  if (values.length === 0) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `${field} must include at least one dereferenceable ref.`,
    })
  }
  return values.map((value, index) => requirePublicRef(value, `${field}[${index}]`))
}

const requireNonNegativeInteger = (value: number, field: string): number => {
  if (!Number.isInteger(value) || value < 0) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `${field} must be a non-negative integer.`,
    })
  }
  return value
}

export const buildQaSwarmHostedRunMeteringRow = (
  input: QaSwarmHostedRunMeteringInput,
): QaSwarmHostedRunMeteringRow => {
  if (input.usageTruth !== 'exact') {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `hosted QA Swarm metering is exact-only; got ${input.usageTruth}.`,
    })
  }

  const inputTokens = requireNonNegativeInteger(
    input.inputTokens,
    'inputTokens',
  )
  const outputTokens = requireNonNegativeInteger(
    input.outputTokens,
    'outputTokens',
  )
  const reasoningTokens = requireNonNegativeInteger(
    input.reasoningTokens ?? 0,
    'reasoningTokens',
  )
  const cacheReadTokens = requireNonNegativeInteger(
    input.cacheReadTokens ?? 0,
    'cacheReadTokens',
  )
  const computedTotal =
    inputTokens + outputTokens + reasoningTokens + cacheReadTokens
  const totalTokens = requireNonNegativeInteger(
    input.totalTokens ?? computedTotal,
    'totalTokens',
  )

  if (totalTokens !== computedTotal) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: 'totalTokens must equal input + output + reasoning + cacheRead.',
    })
  }

  return {
    schemaVersion: QA_SWARM_HOSTED_RUN_METERING_SCHEMA_VERSION,
    rowRef: requirePublicRef(input.rowRef, 'rowRef'),
    runRef: requirePublicRef(input.runRef, 'runRef'),
    provider: 'qa-swarm-hosted-run',
    model: 'openagents/qa-swarm-hosted-run',
    usageTruth: 'exact',
    demandKind: 'hosted_run',
    demandSource: 'qa_swarm_hosted_run',
    source: input.source,
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    totalTokens,
  }
}

export const buildQaSwarmHostedRunReceipt = (
  input: QaSwarmHostedRunReceiptInput,
): QaSwarmHostedRunReceipt => {
  const runRef = requirePublicRef(input.runRef, 'runRef')
  const meteringRows = input.meteringRows
  if (meteringRows.length === 0) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: 'meteringRows must include at least one exact usage row.',
    })
  }
  const mismatched = meteringRows.find(row => row.runRef !== runRef)
  if (mismatched !== undefined) {
    throw new QaSwarmHostedRunReceiptInvariantError({
      reason: `metering row ${mismatched.rowRef} is for ${mismatched.runRef}, not ${runRef}.`,
    })
  }
  const exactTokenTotal = meteringRows.reduce(
    (sum, row) => sum + row.totalTokens,
    0,
  )

  return {
    schemaVersion: QA_SWARM_HOSTED_RUN_RECEIPT_SCHEMA_VERSION,
    receiptKind: 'qa_swarm_hosted_run',
    receiptRef: requirePublicRef(input.receiptRef, 'receiptRef'),
    runRef,
    projectionRef: requirePublicRef(input.projectionRef, 'projectionRef'),
    verdict: input.verdict,
    traceRefs: requirePublicRefs(input.traceRefs, 'traceRefs'),
    coverageRefs: requirePublicRefs(input.coverageRefs, 'coverageRefs'),
    videoRefs: (input.videoRefs ?? []).map((value, index) =>
      requirePublicRef(value, `videoRefs[${index}]`),
    ),
    distilledTestRefs: (input.distilledTestRefs ?? []).map((value, index) =>
      requirePublicRef(value, `distilledTestRefs[${index}]`),
    ),
    meteringRowRefs: meteringRows.map(row => row.rowRef),
    exactTokenTotal,
    publicSafetyRefs: requirePublicRefs(
      input.publicSafetyRefs,
      'publicSafetyRefs',
    ),
    settlement: {
      state: 'inert_owner_armed_required',
      movedMoney: false,
      ownerArmingRef: 'NEEDS_OWNER.qa_swarm_hosted_run_engagement_arming',
    },
  }
}

export const buildQaSwarmEngagementReceipt = (
  input: QaSwarmEngagementReceiptInput,
): QaSwarmEngagementReceipt => {
  const ownerArmed = input.ownerArmToken === QA_SWARM_OWNER_ARM_TOKEN
  const buyerPaidRef = ownerArmed ? input.buyerPaidRef : null
  const providerSettledRef = ownerArmed ? input.providerSettledRef : null
  const outcomeAcceptedRef = trimmedOrNull(input.outcomeAcceptedRef)
  const quickWinInput = {
    signupId: input.signupId,
    offeringPromiseId: 'qa_swarm.hosted_runs.v1',
    quickWinSummary: 'Hosted QA Swarm run engagement.',
    quickWinScopedRef: input.quickWinScopedRef,
    deliveredEvidenceRef: input.hostedRunReceipt.receiptRef,
    ...(outcomeAcceptedRef === null ? {} : { outcomeAcceptedRef }),
    ...(buyerPaidRef === null || buyerPaidRef === undefined
      ? {}
      : { buyerPaidRef }),
    ...(providerSettledRef === null || providerSettledRef === undefined
      ? {}
      : { providerSettledRef }),
    publicCaveatRef:
      'caveat.qa_swarm.engagement_settlement_inert_until_owner_armed',
  }

  return {
    schemaVersion: QA_SWARM_ENGAGEMENT_RECEIPT_SCHEMA_VERSION,
    receiptKind: 'qa_swarm_engagement',
    hostedRunReceiptRef: input.hostedRunReceipt.receiptRef,
    businessQuickWinReceipt: buildBusinessQuickWinReceipt(quickWinInput),
    paymentMode: 'inert_until_owner_armed',
    movedMoney: false,
  }
}

export const publicQaSwarmHostedRunReceiptProjection = (
  receipt: QaSwarmHostedRunReceipt,
) => ({
  receiptKind: receipt.receiptKind,
  receiptRef: receipt.receiptRef,
  runRef: receipt.runRef,
  projectionRef: receipt.projectionRef,
  verdict: receipt.verdict,
  traceRefs: receipt.traceRefs,
  coverageRefs: receipt.coverageRefs,
  meteringRowRefs: receipt.meteringRowRefs,
  exactTokenTotal: receipt.exactTokenTotal,
  publicSafetyRefs: receipt.publicSafetyRefs,
  settlement: receipt.settlement,
})
