import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Array as Arr, Schema as S } from 'effect'

export const DataTraceMarketplaceState = S.Literals([
  'blocked',
  'submitted',
  'redacted',
  'valued',
  'purchased',
  'entitled',
  'payable',
  'settled',
])
export type DataTraceMarketplaceState = typeof DataTraceMarketplaceState.Type

export const DataTracePlannerMode = S.Literals([
  'typed_semantic_selector',
  'cosine_embedding_search',
  'structured_query_planner',
  'keyword_route',
])
export type DataTracePlannerMode = typeof DataTracePlannerMode.Type

export const DataTraceMarketplaceGate = S.Struct({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  correctnessGatePassed: S.Boolean,
  correctnessReceiptRefs: S.Array(S.String),
  dataRevenueCopyAllowed: S.Boolean,
  entitlementRefs: S.Array(S.String),
  payoutContractRefs: S.Array(S.String),
  plannerMode: DataTracePlannerMode,
  publicCopyRefs: S.Array(S.String),
  publicSafeDataSaleSmokePassed: S.Boolean,
  purchaseReceiptRefs: S.Array(S.String),
  redactionReceiptRefs: S.Array(S.String),
  semanticPlannerRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  state: DataTraceMarketplaceState,
  traceSubmissionRefs: S.Array(S.String),
  valuationRefs: S.Array(S.String),
  valuationPayoutClaimAllowed: S.Boolean,
  validatorReviewRefs: S.Array(S.String),
  validatorReviewRequired: S.Boolean,
})
export type DataTraceMarketplaceGate = typeof DataTraceMarketplaceGate.Type

export type DataTraceMarketplaceGateInput = Readonly<{
  correctnessReceiptRefs?: ReadonlyArray<string> | undefined
  entitlementRefs?: ReadonlyArray<string> | undefined
  payoutContractRefs?: ReadonlyArray<string> | undefined
  plannerMode: DataTracePlannerMode
  purchaseReceiptRefs?: ReadonlyArray<string> | undefined
  redactionReceiptRefs?: ReadonlyArray<string> | undefined
  semanticPlannerRefs?: ReadonlyArray<string> | undefined
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  traceSubmissionRefs?: ReadonlyArray<string> | undefined
  valuationRefs?: ReadonlyArray<string> | undefined
  validatorReviewRefs?: ReadonlyArray<string> | undefined
}>

export class DataTraceMarketplaceGateUnsafe extends S.TaggedErrorClass<DataTraceMarketplaceGateUnsafe>()(
  'DataTraceMarketplaceGateUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeGate = S.decodeUnknownSync(DataTraceMarketplaceGate)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const rawMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|dataset|key|repo|source|trace|wallet)|prompt[_-]?(raw|text|full)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|payment|payload|prompt|provider|repo|runner|run[_-]?log|source|telemetry|text|trace|usage|webhook)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|trace[_-]?(raw|full|private|payload)|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(
    ref =>
      !safeRefPattern.test(ref) ||
      containsProviderSecretMaterial(ref) ||
      rawMaterialPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new DataTraceMarketplaceGateUnsafe({
      reason: `${label} must be public-safe refs without raw traces, prompts, private repo/source content, provider payloads, customer data, wallet/payment material, secrets, or timestamps.`,
    })
  }

  return normalized
}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  Arr.isReadonlyArrayNonEmpty(refs)

const blockerRefsForMissingEvidence = (
  input: Readonly<{
    correctnessReceiptRefs: ReadonlyArray<string>
    entitlementRefs: ReadonlyArray<string>
    payoutContractRefs: ReadonlyArray<string>
    purchaseReceiptRefs: ReadonlyArray<string>
    redactionReceiptRefs: ReadonlyArray<string>
    semanticPlannerRefs: ReadonlyArray<string>
    settlementReceiptRefs: ReadonlyArray<string>
    traceSubmissionRefs: ReadonlyArray<string>
    valuationRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(!hasRefs(input.traceSubmissionRefs)
    ? ['blocker.public.data_market.trace_submission_missing']
    : []),
  ...(!hasRefs(input.redactionReceiptRefs)
    ? ['blocker.public.data_market.redaction_receipt_missing']
    : []),
  ...(!hasRefs(input.semanticPlannerRefs)
    ? ['blocker.public.data_market.semantic_planner_missing']
    : []),
  ...(!hasRefs(input.correctnessReceiptRefs)
    ? ['blocker.public.data_market.correctness_verdict_missing']
    : []),
  ...(!hasRefs(input.valuationRefs)
    ? ['blocker.public.data_market.valuation_missing']
    : []),
  ...(!hasRefs(input.purchaseReceiptRefs)
    ? ['blocker.public.data_market.purchase_receipt_missing']
    : []),
  ...(!hasRefs(input.entitlementRefs)
    ? ['blocker.public.data_market.entitlement_missing']
    : []),
  ...(!hasRefs(input.payoutContractRefs)
    ? ['blocker.public.data_market.payout_contract_missing']
    : []),
  ...(!hasRefs(input.settlementReceiptRefs)
    ? ['blocker.public.data_market.settlement_receipt_missing']
    : []),
]

const baseCaveatRefs = [
  'caveat.public.data_market.trace_material_requires_redaction',
  'caveat.public.data_market.correctness_verdict_required',
  'caveat.public.data_market.validator_review_for_nondeterministic_remainder',
  'caveat.public.data_market.valuation_is_not_payout',
  'caveat.public.data_market.purchase_is_not_settlement',
  'caveat.public.data_market.semantic_planner_required',
]

export const projectDataTraceMarketplaceGate = (
  input: DataTraceMarketplaceGateInput,
): DataTraceMarketplaceGate => {
  const correctnessReceiptRefs = safeRefs(
    'Data market correctness receipt refs',
    input.correctnessReceiptRefs,
  )
  const entitlementRefs = safeRefs(
    'Data market entitlement refs',
    input.entitlementRefs,
  )
  const payoutContractRefs = safeRefs(
    'Data market payout contract refs',
    input.payoutContractRefs,
  )
  const purchaseReceiptRefs = safeRefs(
    'Data market purchase receipt refs',
    input.purchaseReceiptRefs,
  )
  const redactionReceiptRefs = safeRefs(
    'Data market redaction receipt refs',
    input.redactionReceiptRefs,
  )
  const semanticPlannerRefs = safeRefs(
    'Data market semantic planner refs',
    input.semanticPlannerRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'Data market settlement receipt refs',
    input.settlementReceiptRefs,
  )
  const traceSubmissionRefs = safeRefs(
    'Data market trace submission refs',
    input.traceSubmissionRefs,
  )
  const valuationRefs = safeRefs(
    'Data market valuation refs',
    input.valuationRefs,
  )
  const validatorReviewRefs = safeRefs(
    'Data market validator review refs',
    input.validatorReviewRefs,
  )
  const keywordRoutingBlocked = input.plannerMode === 'keyword_route'
  const correctnessGatePassed = hasRefs(correctnessReceiptRefs)
  const validatorReviewRequired =
    hasRefs(validatorReviewRefs) && !correctnessGatePassed
  const missingBlockerRefs = blockerRefsForMissingEvidence({
    correctnessReceiptRefs,
    entitlementRefs,
    payoutContractRefs,
    purchaseReceiptRefs,
    redactionReceiptRefs,
    semanticPlannerRefs,
    settlementReceiptRefs,
    traceSubmissionRefs,
    valuationRefs,
  })
  const submitted = hasRefs(traceSubmissionRefs) && !keywordRoutingBlocked
  const redacted = submitted && hasRefs(redactionReceiptRefs)
  const valued =
    redacted &&
    !keywordRoutingBlocked &&
    correctnessGatePassed &&
    hasRefs(semanticPlannerRefs) &&
    hasRefs(valuationRefs)
  const purchased = valued && hasRefs(purchaseReceiptRefs)
  const entitled = purchased && hasRefs(entitlementRefs)
  const payable = entitled && hasRefs(payoutContractRefs)
  const settled = payable && hasRefs(settlementReceiptRefs)
  const publicSafeDataSaleSmokePassed = settled
  const state: DataTraceMarketplaceState = keywordRoutingBlocked
    ? 'blocked'
    : settled
      ? 'settled'
      : payable
        ? 'payable'
        : entitled
          ? 'entitled'
          : purchased
            ? 'purchased'
            : valued
              ? 'valued'
              : redacted
                ? 'redacted'
                : submitted
                  ? 'submitted'
                  : 'blocked'
  const blockerRefs = [
    ...(keywordRoutingBlocked
      ? ['blocker.public.data_market.keyword_routing_disallowed']
      : []),
    ...missingBlockerRefs,
  ].sort()

  return decodeGate({
    blockerRefs,
    caveatRefs: baseCaveatRefs,
    correctnessGatePassed,
    correctnessReceiptRefs,
    dataRevenueCopyAllowed: publicSafeDataSaleSmokePassed,
    entitlementRefs,
    payoutContractRefs,
    plannerMode: input.plannerMode,
    publicCopyRefs: publicSafeDataSaleSmokePassed
      ? ['copy.public.data_market.sale_settlement_receipts_visible']
      : ['copy.public.data_market.revenue_claim_blocked'],
    publicSafeDataSaleSmokePassed,
    purchaseReceiptRefs,
    redactionReceiptRefs,
    semanticPlannerRefs,
    settlementClaimAllowed: settled,
    settlementReceiptRefs,
    state,
    traceSubmissionRefs,
    valuationRefs,
    valuationPayoutClaimAllowed: false,
    validatorReviewRefs,
    validatorReviewRequired,
  })
}

export const dataTraceMarketplaceGateHasPrivateMaterial = (
  gate: DataTraceMarketplaceGate,
): boolean => {
  const publicValues = [
    gate.plannerMode,
    gate.state,
    ...gate.blockerRefs,
    ...gate.caveatRefs,
    ...gate.correctnessReceiptRefs,
    ...gate.entitlementRefs,
    ...gate.payoutContractRefs,
    ...gate.publicCopyRefs,
    ...gate.purchaseReceiptRefs,
    ...gate.redactionReceiptRefs,
    ...gate.semanticPlannerRefs,
    ...gate.settlementReceiptRefs,
    ...gate.traceSubmissionRefs,
    ...gate.valuationRefs,
    ...gate.validatorReviewRefs,
  ]

  return publicValues.some(
    value =>
      containsProviderSecretMaterial(value) ||
      rawMaterialPattern.test(value) ||
      rawTimestampPattern.test(value),
  )
}
