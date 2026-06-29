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
  correctnessBlockerRefs: S.Array(S.String),
  correctnessGatePassed: S.Boolean,
  correctnessReceiptRefs: S.Array(S.String),
  correctnessVerificationStatus: S.String,
  dataRevenueCopyAllowed: S.Boolean,
  dedupeReceiptRefs: S.Array(S.String),
  derivedTraceDigest: S.optional(S.String),
  entitlementRefs: S.Array(S.String),
  payoutContractRefs: S.Array(S.String),
  plannerMode: DataTracePlannerMode,
  provenanceReceiptRefs: S.Array(S.String),
  publicCopyRefs: S.Array(S.String),
  publicSafeDataSaleSmokePassed: S.Boolean,
  purchaseReceiptRefs: S.Array(S.String),
  redactionReceiptRefs: S.Array(S.String),
  semanticPlannerRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: DataTraceMarketplaceState,
  traceSubmissionRefs: S.Array(S.String),
  valuationRefs: S.Array(S.String),
  valuationPayoutClaimAllowed: S.Boolean,
  validatorReviewRefs: S.Array(S.String),
  validatorReviewRequired: S.Boolean,
})
export type DataTraceMarketplaceGate = typeof DataTraceMarketplaceGate.Type

export const DataContributionCorrectnessStatus = S.Literals([
  'missing',
  'accepted',
  'rejected',
  'needs_validator_review',
])
export type DataContributionCorrectnessStatus =
  typeof DataContributionCorrectnessStatus.Type

export const DataContributionCorrectnessVerification = S.Struct({
  blockerRefs: S.Array(S.String),
  claimedTraceDigest: S.optional(S.String),
  contributionRef: S.String,
  correctnessGatePassed: S.Boolean,
  correctnessReceiptRefs: S.Array(S.String),
  dedupeReceiptRefs: S.Array(S.String),
  derivedTraceDigest: S.optional(S.String),
  provenanceReceiptRefs: S.Array(S.String),
  sourceDigest: S.optional(S.String),
  sourceRefs: S.Array(S.String),
  status: DataContributionCorrectnessStatus,
  transformRef: S.optional(S.String),
  validatorReviewRefs: S.Array(S.String),
  validatorReviewRequired: S.Boolean,
  verificationRef: S.String,
})
export type DataContributionCorrectnessVerification =
  typeof DataContributionCorrectnessVerification.Type

export type DataContributionDerivedTraceRow = Readonly<Record<string, unknown>>

export type DataContributionCorrectnessInput =
  | Readonly<{
      claimedTraceDigest: string
      contributionRef: string
      derivedTraceRows: ReadonlyArray<DataContributionDerivedTraceRow>
      knownSourceDigests?: ReadonlyArray<string> | undefined
      knownTraceDigests?: ReadonlyArray<string> | undefined
      provenanceRefs?: ReadonlyArray<string> | undefined
      sourceDigest: string
      sourceRefs?: ReadonlyArray<string> | undefined
      transformRef: string
      verificationMode: 'derived_trace_replay'
    }>
  | Readonly<{
      claimedTraceDigest?: string | undefined
      contributionRef: string
      knownSourceDigests?: ReadonlyArray<string> | undefined
      knownTraceDigests?: ReadonlyArray<string> | undefined
      provenanceRefs?: ReadonlyArray<string> | undefined
      sourceDigest?: string | undefined
      sourceRefs?: ReadonlyArray<string> | undefined
      transformRef?: string | undefined
      validatorReviewRefs?: ReadonlyArray<string> | undefined
      verificationMode: 'validator_review_required'
    }>

export type DataTraceMarketplaceGateInput = Readonly<{
  correctnessVerification?: DataContributionCorrectnessVerification | undefined
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
const decodeCorrectnessVerification = S.decodeUnknownSync(
  DataContributionCorrectnessVerification,
)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const sha256Pattern = /^sha256:[a-f0-9]{64}$/
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

const textEncoder = new TextEncoder()

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', textEncoder.encode(value))
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const sha256Ref = async (value: string): Promise<string> =>
  `sha256:${await sha256Hex(value)}`

const shortDigest = (digest: string): string =>
  digest.replace(/^sha256:/, '').slice(0, 16)

const isRecord = (value: unknown): value is Readonly<Record<string, unknown>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const stableValue = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(stableValue)
  }

  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, entryValue]) => entryValue !== undefined)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entryValue]) => [key, stableValue(entryValue)]),
    )
  }

  return value
}

const stableJson = (value: unknown): string => JSON.stringify(stableValue(value))

export const deriveDataContributionTraceDigest = async (
  input: Readonly<{
    sourceDigest: string
    traceRows: ReadonlyArray<DataContributionDerivedTraceRow>
    transformRef: string
  }>,
): Promise<string> =>
  sha256Ref(
    stableJson({
      schemaRef: 'openagents.data_contribution.derived_trace.v1',
      sourceDigest: input.sourceDigest,
      traceRows: input.traceRows,
      transformRef: input.transformRef,
    }),
  )

const digestBlockers = (
  digest: string | undefined,
  missingRef: string,
  invalidRef: string,
): ReadonlyArray<string> => {
  if (digest === undefined || digest.trim() === '') {
    return [missingRef]
  }

  return sha256Pattern.test(digest) ? [] : [invalidRef]
}

const dedupeBlockers = (
  input: Readonly<{
    claimedTraceDigest: string | undefined
    knownSourceDigests: ReadonlyArray<string> | undefined
    knownTraceDigests: ReadonlyArray<string> | undefined
    sourceDigest: string | undefined
  }>,
): ReadonlyArray<string> => [
  ...(input.sourceDigest !== undefined &&
  (input.knownSourceDigests ?? []).includes(input.sourceDigest)
    ? ['blocker.public.data_market.duplicate_source_digest']
    : []),
  ...(input.claimedTraceDigest !== undefined &&
  (input.knownTraceDigests ?? []).includes(input.claimedTraceDigest)
    ? ['blocker.public.data_market.duplicate_trace_digest']
    : []),
]

const uniqueBlockers = (
  blockers: ReadonlyArray<string>,
): ReadonlyArray<string> => [...new Set(blockers)].sort()

const buildCorrectnessVerificationRef = async (
  input: Readonly<{
    blockerRefs: ReadonlyArray<string>
    claimedTraceDigest: string | undefined
    contributionRef: string
    derivedTraceDigest: string | undefined
    sourceDigest: string | undefined
    sourceRefs: ReadonlyArray<string>
    status: DataContributionCorrectnessStatus
    transformRef: string | undefined
    validatorReviewRefs: ReadonlyArray<string>
    verificationMode: DataContributionCorrectnessInput['verificationMode']
  }>,
): Promise<string> => {
  const digest = await sha256Ref(
    stableJson({
      blockerRefs: input.blockerRefs,
      claimedTraceDigest: input.claimedTraceDigest,
      contributionRef: input.contributionRef,
      derivedTraceDigest: input.derivedTraceDigest,
      schemaRef: 'openagents.data_contribution.correctness_verification.v1',
      sourceDigest: input.sourceDigest,
      sourceRefs: input.sourceRefs,
      status: input.status,
      transformRef: input.transformRef,
      validatorReviewRefs: input.validatorReviewRefs,
      verificationMode: input.verificationMode,
    }),
  )

  return `verification.public.data_market.correctness.${shortDigest(digest)}`
}

export const verifyDataContributionCorrectness = async (
  input: DataContributionCorrectnessInput,
): Promise<DataContributionCorrectnessVerification> => {
  const contributionRefs = safeRefs('Data contribution ref', [
    input.contributionRef,
  ])
  const contributionRef = contributionRefs[0] ?? ''
  const sourceRefs = safeRefs('Data contribution source refs', input.sourceRefs)
  const provenanceRefs = safeRefs(
    'Data contribution provenance refs',
    input.provenanceRefs,
  )
  const transformRefs = safeRefs(
    'Data contribution transform ref',
    input.transformRef === undefined ? [] : [input.transformRef],
  )
  const validatorReviewRefs = safeRefs(
    'Data contribution validator review refs',
    input.verificationMode === 'validator_review_required'
      ? input.validatorReviewRefs
      : [],
  )
  const sourceDigest = input.sourceDigest
  const claimedTraceDigest = input.claimedTraceDigest
  const digestEvidenceBlockers = [
    ...digestBlockers(
      sourceDigest,
      'blocker.public.data_market.source_digest_missing',
      'blocker.public.data_market.source_digest_invalid',
    ),
    ...digestBlockers(
      claimedTraceDigest,
      'blocker.public.data_market.claimed_trace_digest_missing',
      'blocker.public.data_market.claimed_trace_digest_invalid',
    ),
  ]
  const baseBlockers = [
    ...(contributionRef === ''
      ? ['blocker.public.data_market.contribution_ref_missing']
      : []),
    ...(!hasRefs(sourceRefs)
      ? ['blocker.public.data_market.provenance_source_missing']
      : []),
    ...(!hasRefs(provenanceRefs)
      ? ['blocker.public.data_market.provenance_ref_missing']
      : []),
    ...(!hasRefs(transformRefs)
      ? ['blocker.public.data_market.transform_ref_missing']
      : []),
    ...digestEvidenceBlockers,
    ...dedupeBlockers({
      claimedTraceDigest,
      knownSourceDigests: input.knownSourceDigests,
      knownTraceDigests: input.knownTraceDigests,
      sourceDigest,
    }),
  ]
  const derivedTraceDigest =
    input.verificationMode === 'derived_trace_replay' &&
    sourceDigest !== undefined &&
    sha256Pattern.test(sourceDigest) &&
    hasRefs(transformRefs)
      ? await deriveDataContributionTraceDigest({
          sourceDigest,
          traceRows: input.derivedTraceRows,
          transformRef: transformRefs[0] ?? '',
        })
      : undefined
  const replayBlockers =
    input.verificationMode === 'derived_trace_replay'
      ? [
          ...(input.derivedTraceRows.length === 0
            ? ['blocker.public.data_market.derived_trace_rows_missing']
            : []),
          ...(derivedTraceDigest !== undefined &&
          claimedTraceDigest !== undefined &&
          sha256Pattern.test(claimedTraceDigest) &&
          derivedTraceDigest !== claimedTraceDigest
            ? ['blocker.public.data_market.derived_trace_digest_mismatch']
            : []),
        ]
      : [
          ...(!hasRefs(validatorReviewRefs)
            ? ['blocker.public.data_market.validator_review_ref_missing']
            : ['blocker.public.data_market.validator_review_required']),
        ]
  const blockerRefs = uniqueBlockers([...baseBlockers, ...replayBlockers])
  const status: DataContributionCorrectnessStatus =
    blockerRefs.length > 0 &&
    input.verificationMode === 'validator_review_required' &&
    blockerRefs.every(
      ref => ref === 'blocker.public.data_market.validator_review_required',
    )
      ? 'needs_validator_review'
      : blockerRefs.length === 0
        ? 'accepted'
        : 'rejected'
  const correctnessGatePassed = status === 'accepted'
  const evidenceDigest =
    derivedTraceDigest ??
    (claimedTraceDigest !== undefined && sha256Pattern.test(claimedTraceDigest)
      ? claimedTraceDigest
      : undefined)
  const provenanceReceiptDigest =
    hasRefs(sourceRefs) &&
    hasRefs(provenanceRefs) &&
    sourceDigest !== undefined &&
    sha256Pattern.test(sourceDigest)
      ? await sha256Ref(
          stableJson({
            provenanceRefs,
            schemaRef: 'openagents.data_contribution.provenance.v1',
            sourceDigest,
            sourceRefs,
          }),
        )
      : undefined
  const provenanceReceiptRefs =
    provenanceReceiptDigest === undefined
      ? []
      : [`provenance.public.data_market.${shortDigest(provenanceReceiptDigest)}`]
  const dedupeReceiptRefs =
    status === 'accepted' || status === 'needs_validator_review'
      ? [
          ...(sourceDigest !== undefined && sha256Pattern.test(sourceDigest)
            ? [`dedupe.public.data_market.source.${shortDigest(sourceDigest)}`]
            : []),
          ...(evidenceDigest !== undefined
            ? [`dedupe.public.data_market.trace.${shortDigest(evidenceDigest)}`]
            : []),
        ].sort()
      : []
  const verificationRef = await buildCorrectnessVerificationRef({
    blockerRefs,
    claimedTraceDigest,
    contributionRef,
    derivedTraceDigest,
    sourceDigest,
    sourceRefs,
    status,
    transformRef: transformRefs[0],
    validatorReviewRefs,
    verificationMode: input.verificationMode,
  })
  const correctnessReceiptRefs =
    status === 'accepted'
      ? [
          `correctness.public.data_market.derived_trace.${shortDigest(
            evidenceDigest ?? verificationRef,
          )}`,
          verificationRef,
        ]
      : []

  return decodeCorrectnessVerification({
    blockerRefs,
    claimedTraceDigest,
    contributionRef,
    correctnessGatePassed,
    correctnessReceiptRefs,
    dedupeReceiptRefs,
    derivedTraceDigest,
    provenanceReceiptRefs,
    sourceDigest,
    sourceRefs,
    status,
    transformRef: transformRefs[0],
    validatorReviewRefs,
    validatorReviewRequired: status === 'needs_validator_review',
    verificationRef,
  })
}

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
  const correctnessVerification =
    input.correctnessVerification === undefined
      ? undefined
      : decodeCorrectnessVerification(input.correctnessVerification)
  const correctnessBlockerRefs = safeRefs(
    'Data market correctness blocker refs',
    correctnessVerification?.blockerRefs,
  )
  const verifierCorrectnessReceiptRefs =
    correctnessVerification?.status === 'accepted'
      ? correctnessVerification.correctnessReceiptRefs
      : []
  const correctnessReceiptRefs = safeRefs(
    'Data market correctness receipt refs',
    [...(input.correctnessReceiptRefs ?? []), ...verifierCorrectnessReceiptRefs],
  )
  const dedupeReceiptRefs = safeRefs(
    'Data market dedupe receipt refs',
    correctnessVerification?.dedupeReceiptRefs,
  )
  const entitlementRefs = safeRefs(
    'Data market entitlement refs',
    input.entitlementRefs,
  )
  const payoutContractRefs = safeRefs(
    'Data market payout contract refs',
    input.payoutContractRefs,
  )
  const provenanceReceiptRefs = safeRefs(
    'Data market provenance receipt refs',
    correctnessVerification?.provenanceReceiptRefs,
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
  const sourceRefs = safeRefs(
    'Data market correctness source refs',
    correctnessVerification?.sourceRefs,
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
    [
      ...(input.validatorReviewRefs ?? []),
      ...(correctnessVerification?.validatorReviewRefs ?? []),
    ],
  )
  const keywordRoutingBlocked = input.plannerMode === 'keyword_route'
  const correctnessVerificationStatus: DataContributionCorrectnessStatus =
    correctnessVerification?.status ??
    (hasRefs(correctnessReceiptRefs)
      ? 'accepted'
      : hasRefs(validatorReviewRefs)
        ? 'needs_validator_review'
        : 'missing')
  const correctnessGatePassed =
    hasRefs(correctnessReceiptRefs) &&
    correctnessVerificationStatus === 'accepted'
  const validatorReviewRequired =
    correctnessVerificationStatus === 'needs_validator_review' ||
    (hasRefs(validatorReviewRefs) && !correctnessGatePassed)
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
  const blockerRefs = uniqueBlockers([
    ...(keywordRoutingBlocked
      ? ['blocker.public.data_market.keyword_routing_disallowed']
      : []),
    ...missingBlockerRefs,
    ...(correctnessVerificationStatus === 'rejected'
      ? ['blocker.public.data_market.correctness_verdict_rejected']
      : []),
    ...(validatorReviewRequired
      ? ['blocker.public.data_market.validator_review_required']
      : []),
    ...correctnessBlockerRefs,
  ])

  return decodeGate({
    blockerRefs,
    caveatRefs: baseCaveatRefs,
    correctnessBlockerRefs,
    correctnessGatePassed,
    correctnessReceiptRefs,
    correctnessVerificationStatus,
    dataRevenueCopyAllowed: publicSafeDataSaleSmokePassed,
    dedupeReceiptRefs,
    derivedTraceDigest: correctnessVerification?.derivedTraceDigest,
    entitlementRefs,
    payoutContractRefs,
    plannerMode: input.plannerMode,
    provenanceReceiptRefs,
    publicCopyRefs: publicSafeDataSaleSmokePassed
      ? ['copy.public.data_market.sale_settlement_receipts_visible']
      : ['copy.public.data_market.revenue_claim_blocked'],
    publicSafeDataSaleSmokePassed,
    purchaseReceiptRefs,
    redactionReceiptRefs,
    semanticPlannerRefs,
    settlementClaimAllowed: settled,
    settlementReceiptRefs,
    sourceRefs,
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
    ...gate.correctnessBlockerRefs,
    ...gate.correctnessReceiptRefs,
    gate.correctnessVerificationStatus,
    ...(gate.derivedTraceDigest === undefined ? [] : [gate.derivedTraceDigest]),
    ...gate.dedupeReceiptRefs,
    ...gate.entitlementRefs,
    ...gate.payoutContractRefs,
    ...gate.provenanceReceiptRefs,
    ...gate.publicCopyRefs,
    ...gate.purchaseReceiptRefs,
    ...gate.redactionReceiptRefs,
    ...gate.semanticPlannerRefs,
    ...gate.settlementReceiptRefs,
    ...gate.sourceRefs,
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
