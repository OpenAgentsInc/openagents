import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const DebtReceiptSettlementState = S.Literals([
  'blocked',
  'fundable',
  'funded',
  'verified',
  'payable',
  'retired',
  'duplicate_replay',
  'quarantined',
])
export type DebtReceiptSettlementState =
  typeof DebtReceiptSettlementState.Type

export const DebtReceiptSettlementProjection = S.Struct({
  acceptedWorkRefs: S.Array(S.String),
  baselineMetricRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetCapSats: S.Number,
  caveatRefs: S.Array(S.String),
  duplicateFingerprintRefs: S.Array(S.String),
  duplicateReplay: S.Boolean,
  fundingApprovalRefs: S.Array(S.String),
  fundingAuthorityRefs: S.Array(S.String),
  hygieneDeltaRefs: S.Array(S.String),
  manualReviewOnly: S.Boolean,
  noNewEqualOrWorseDebtRefs: S.Array(S.String),
  payableSats: S.Number,
  publicCopyRefs: S.Array(S.String),
  quarantineReasonRefs: S.Array(S.String),
  retiredReceiptRefs: S.Array(S.String),
  reviewDecisionRefs: S.Array(S.String),
  roleRefs: S.Struct({
    fundingAuthorityActorRef: S.NullOr(S.String),
    proposerActorRef: S.NullOr(S.String),
    reviewerActorRef: S.NullOr(S.String),
    settlementAuthorityActorRef: S.NullOr(S.String),
    workerActorRef: S.NullOr(S.String),
  }),
  scopeRefs: S.Array(S.String),
  settlementApprovalRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  settledSats: S.Number,
  sourceRefs: S.Array(S.String),
  spendAuthorityDelegatedToWorker: S.Boolean,
  state: DebtReceiptSettlementState,
  stopConditionRefs: S.Array(S.String),
  targetMetricRefs: S.Array(S.String),
  verificationCommandRefs: S.Array(S.String),
  workerPayoutEligible: S.Boolean,
})
export type DebtReceiptSettlementProjection =
  typeof DebtReceiptSettlementProjection.Type

export type DebtReceiptSettlementInput = Readonly<{
  acceptedWorkRefs?: ReadonlyArray<string> | undefined
  baselineMetricRefs?: ReadonlyArray<string> | undefined
  budgetCapSats?: number | undefined
  duplicateFingerprintRefs?: ReadonlyArray<string> | undefined
  fundingApprovalRefs?: ReadonlyArray<string> | undefined
  fundingAuthorityActorRef?: string | null | undefined
  fundingAuthorityRefs?: ReadonlyArray<string> | undefined
  hygieneDeltaRefs?: ReadonlyArray<string> | undefined
  maxRevisionAttempts?: number | undefined
  noNewEqualOrWorseDebtRefs?: ReadonlyArray<string> | undefined
  payableSats?: number | undefined
  proposerActorRef?: string | null | undefined
  retiredReceiptRefs?: ReadonlyArray<string> | undefined
  reviewDecisionRefs?: ReadonlyArray<string> | undefined
  reviewerActorRef?: string | null | undefined
  revisionAttemptCount?: number | undefined
  scopeRefs?: ReadonlyArray<string> | undefined
  settlementApprovalRefs?: ReadonlyArray<string> | undefined
  settlementAuthorityActorRef?: string | null | undefined
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  settledSats?: number | undefined
  sourceRefs?: ReadonlyArray<string> | undefined
  stopConditionRefs?: ReadonlyArray<string> | undefined
  targetMetricRefs?: ReadonlyArray<string> | undefined
  verificationCommandRefs?: ReadonlyArray<string> | undefined
  workerActorRef?: string | null | undefined
}>

export class DebtReceiptPolicyUnsafe extends S.TaggedErrorClass<DebtReceiptPolicyUnsafe>()(
  'DebtReceiptPolicyUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeProjection = S.decodeUnknownSync(DebtReceiptSettlementProjection)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/
const unsafeDebtReceiptRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|customer|diff|document|email|fixture|invoice|log|payment|payload|prompt|provider|receipt|runner|run[_-]?log|source|source[_-]?archive|usage|webhook)|runner[_-]?log|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const refIsSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !unsafeDebtReceiptRefPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref => !refIsSafe(ref))

  if (unsafe !== undefined) {
    throw new DebtReceiptPolicyUnsafe({
      reason: `${label} must be public-safe refs without raw diffs, prompts, provider, customer, payment, wallet, payout, private repo, secret, or timestamp material.`,
    })
  }

  return normalized
}

const safeActorRef = (
  label: string,
  ref: string | null | undefined,
): string | null => {
  const normalized = ref?.trim() ?? ''

  if (normalized === '') {
    return null
  }

  if (!refIsSafe(normalized)) {
    throw new DebtReceiptPolicyUnsafe({
      reason: `${label} must be a public-safe actor ref.`,
    })
  }

  return normalized
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const safeNonNegativeInteger = (
  label: string,
  value: number | undefined,
): number => {
  const normalized = value ?? 0

  if (!Number.isInteger(normalized) || normalized < 0) {
    throw new DebtReceiptPolicyUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }

  return normalized
}

const distinctPresentActors = (
  refs: ReadonlyArray<Readonly<[string, string | null]>>,
): ReadonlyArray<string> => {
  const seen = new Map<string, string>()
  const blockers: Array<string> = []

  refs.forEach(([label, ref]) => {
    if (ref === null) {
      return
    }
    const firstLabel = seen.get(ref)
    if (firstLabel !== undefined) {
      blockers.push(`blocker.public.debt_receipt.role_overlap.${firstLabel}_${label}`)
      return
    }
    seen.set(ref, label)
  })

  return blockers.sort()
}

const missingEvidenceBlockers = (input: {
  acceptedWorkRefs: ReadonlyArray<string>
  baselineMetricRefs: ReadonlyArray<string>
  budgetCapSats: number
  fundingApprovalRefs: ReadonlyArray<string>
  fundingAuthorityRefs: ReadonlyArray<string>
  hygieneDeltaRefs: ReadonlyArray<string>
  noNewEqualOrWorseDebtRefs: ReadonlyArray<string>
  reviewDecisionRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  settlementApprovalRefs: ReadonlyArray<string>
  settlementReceiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  stopConditionRefs: ReadonlyArray<string>
  targetMetricRefs: ReadonlyArray<string>
  verificationCommandRefs: ReadonlyArray<string>
}): ReadonlyArray<string> => [
  ...(!hasRefs(input.sourceRefs)
    ? ['blocker.public.debt_receipt.source_missing']
    : []),
  ...(!hasRefs(input.baselineMetricRefs)
    ? ['blocker.public.debt_receipt.baseline_metric_missing']
    : []),
  ...(!hasRefs(input.targetMetricRefs)
    ? ['blocker.public.debt_receipt.target_metric_missing']
    : []),
  ...(!hasRefs(input.scopeRefs)
    ? ['blocker.public.debt_receipt.scope_missing']
    : []),
  ...(!hasRefs(input.stopConditionRefs)
    ? ['blocker.public.debt_receipt.stop_condition_missing']
    : []),
  ...(input.budgetCapSats === 0
    ? ['blocker.public.debt_receipt.budget_cap_missing']
    : []),
  ...(!hasRefs(input.fundingApprovalRefs)
    ? ['blocker.public.debt_receipt.funding_approval_missing']
    : []),
  ...(!hasRefs(input.fundingAuthorityRefs)
    ? ['blocker.public.debt_receipt.funding_authority_missing']
    : []),
  ...(!hasRefs(input.verificationCommandRefs)
    ? ['blocker.public.debt_receipt.verification_command_missing']
    : []),
  ...(!hasRefs(input.acceptedWorkRefs)
    ? ['blocker.public.debt_receipt.accepted_work_missing']
    : []),
  ...(!hasRefs(input.reviewDecisionRefs)
    ? ['blocker.public.debt_receipt.review_decision_missing']
    : []),
  ...(!hasRefs(input.hygieneDeltaRefs)
    ? ['blocker.public.debt_receipt.hygiene_delta_missing']
    : []),
  ...(!hasRefs(input.noNewEqualOrWorseDebtRefs)
    ? ['blocker.public.debt_receipt.no_equal_or_worse_debt_check_missing']
    : []),
  ...(!hasRefs(input.settlementApprovalRefs)
    ? ['blocker.public.debt_receipt.settlement_approval_missing']
    : []),
  ...(!hasRefs(input.settlementReceiptRefs)
    ? ['blocker.public.debt_receipt.settlement_receipt_missing']
    : []),
]

const baseCaveatRefs = [
  'caveat.public.debt_receipt.discovery_is_not_spend',
  'caveat.public.debt_receipt.worker_cannot_mint_payable_followups',
  'caveat.public.debt_receipt.payment_requires_verified_delta',
  'caveat.public.debt_receipt.settle_once_then_retire',
  'caveat.public.debt_receipt.duplicate_replay_not_payable',
]

export const projectDebtReceiptSettlement = (
  input: DebtReceiptSettlementInput,
): DebtReceiptSettlementProjection => {
  const acceptedWorkRefs = safeRefs(
    'Debt receipt accepted work refs',
    input.acceptedWorkRefs,
  )
  const baselineMetricRefs = safeRefs(
    'Debt receipt baseline metric refs',
    input.baselineMetricRefs,
  )
  const duplicateFingerprintRefs = safeRefs(
    'Debt receipt duplicate fingerprint refs',
    input.duplicateFingerprintRefs,
  )
  const fundingApprovalRefs = safeRefs(
    'Debt receipt funding approval refs',
    input.fundingApprovalRefs,
  )
  const fundingAuthorityRefs = safeRefs(
    'Debt receipt funding authority refs',
    input.fundingAuthorityRefs,
  )
  const hygieneDeltaRefs = safeRefs(
    'Debt receipt hygiene delta refs',
    input.hygieneDeltaRefs,
  )
  const noNewEqualOrWorseDebtRefs = safeRefs(
    'Debt receipt no-new-debt refs',
    input.noNewEqualOrWorseDebtRefs,
  )
  const retiredReceiptRefs = safeRefs(
    'Debt receipt retired receipt refs',
    input.retiredReceiptRefs,
  )
  const reviewDecisionRefs = safeRefs(
    'Debt receipt review decision refs',
    input.reviewDecisionRefs,
  )
  const scopeRefs = safeRefs('Debt receipt scope refs', input.scopeRefs)
  const settlementApprovalRefs = safeRefs(
    'Debt receipt settlement approval refs',
    input.settlementApprovalRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'Debt receipt settlement receipt refs',
    input.settlementReceiptRefs,
  )
  const sourceRefs = safeRefs('Debt receipt source refs', input.sourceRefs)
  const stopConditionRefs = safeRefs(
    'Debt receipt stop condition refs',
    input.stopConditionRefs,
  )
  const targetMetricRefs = safeRefs(
    'Debt receipt target metric refs',
    input.targetMetricRefs,
  )
  const verificationCommandRefs = safeRefs(
    'Debt receipt verification command refs',
    input.verificationCommandRefs,
  )
  const budgetCapSats = safeNonNegativeInteger(
    'budgetCapSats',
    input.budgetCapSats,
  )
  const maxRevisionAttempts = safeNonNegativeInteger(
    'maxRevisionAttempts',
    input.maxRevisionAttempts ?? 3,
  )
  const payableSats = safeNonNegativeInteger('payableSats', input.payableSats)
  const revisionAttemptCount = safeNonNegativeInteger(
    'revisionAttemptCount',
    input.revisionAttemptCount,
  )
  const settledSats = safeNonNegativeInteger('settledSats', input.settledSats)
  const roleRefs = {
    fundingAuthorityActorRef: safeActorRef(
      'Debt receipt funding authority actor ref',
      input.fundingAuthorityActorRef,
    ),
    proposerActorRef: safeActorRef(
      'Debt receipt proposer actor ref',
      input.proposerActorRef,
    ),
    reviewerActorRef: safeActorRef(
      'Debt receipt reviewer actor ref',
      input.reviewerActorRef,
    ),
    settlementAuthorityActorRef: safeActorRef(
      'Debt receipt settlement authority actor ref',
      input.settlementAuthorityActorRef,
    ),
    workerActorRef: safeActorRef(
      'Debt receipt worker actor ref',
      input.workerActorRef,
    ),
  }

  if (payableSats > budgetCapSats) {
    throw new DebtReceiptPolicyUnsafe({
      reason: 'Debt receipt payable sats cannot exceed the budget cap.',
    })
  }

  if (settledSats > payableSats) {
    throw new DebtReceiptPolicyUnsafe({
      reason: 'Debt receipt settled sats cannot exceed payable sats.',
    })
  }

  const roleBlockers = distinctPresentActors([
    ['funder', roleRefs.fundingAuthorityActorRef],
    ['proposer', roleRefs.proposerActorRef],
    ['reviewer', roleRefs.reviewerActorRef],
    ['settlement', roleRefs.settlementAuthorityActorRef],
    ['worker', roleRefs.workerActorRef],
  ])
  const duplicateReplay =
    hasRefs(retiredReceiptRefs) && hasRefs(duplicateFingerprintRefs)
  const manualReviewOnly =
    maxRevisionAttempts > 0 && revisionAttemptCount >= maxRevisionAttempts
  const quarantineReasonRefs = manualReviewOnly
    ? ['quarantine.public.debt_receipt.revision_attempt_limit_reached']
    : []
  const defined =
    hasRefs(sourceRefs) &&
    hasRefs(baselineMetricRefs) &&
    hasRefs(targetMetricRefs) &&
    hasRefs(scopeRefs) &&
    hasRefs(stopConditionRefs) &&
    budgetCapSats > 0
  const funded =
    defined &&
    hasRefs(fundingApprovalRefs) &&
    hasRefs(fundingAuthorityRefs) &&
    roleBlockers.length === 0
  const verified =
    funded &&
    hasRefs(verificationCommandRefs) &&
    hasRefs(acceptedWorkRefs) &&
    hasRefs(reviewDecisionRefs) &&
    hasRefs(hygieneDeltaRefs) &&
    hasRefs(noNewEqualOrWorseDebtRefs)
  const payable =
    verified &&
    hasRefs(settlementApprovalRefs) &&
    payableSats > 0 &&
    !duplicateReplay &&
    !manualReviewOnly
  const retired =
    payable &&
    hasRefs(settlementReceiptRefs) &&
    settledSats === payableSats
  const state: DebtReceiptSettlementState = duplicateReplay
    ? 'duplicate_replay'
    : manualReviewOnly
      ? 'quarantined'
      : retired
        ? 'retired'
        : payable
          ? 'payable'
          : verified
            ? 'verified'
            : funded
              ? 'funded'
              : defined
                ? 'fundable'
                : 'blocked'

  return decodeProjection({
    acceptedWorkRefs,
    baselineMetricRefs,
    blockerRefs: [
      ...missingEvidenceBlockers({
        acceptedWorkRefs,
        baselineMetricRefs,
        budgetCapSats,
        fundingApprovalRefs,
        fundingAuthorityRefs,
        hygieneDeltaRefs,
        noNewEqualOrWorseDebtRefs,
        reviewDecisionRefs,
        scopeRefs,
        settlementApprovalRefs,
        settlementReceiptRefs,
        sourceRefs,
        stopConditionRefs,
        targetMetricRefs,
        verificationCommandRefs,
      }),
      ...roleBlockers,
      ...(duplicateReplay
        ? ['blocker.public.debt_receipt.duplicate_replay']
        : []),
      ...(manualReviewOnly
        ? ['blocker.public.debt_receipt.manual_review_only']
        : []),
    ].sort(),
    budgetCapSats,
    caveatRefs: baseCaveatRefs,
    duplicateFingerprintRefs,
    duplicateReplay,
    fundingApprovalRefs,
    fundingAuthorityRefs,
    hygieneDeltaRefs,
    manualReviewOnly,
    noNewEqualOrWorseDebtRefs,
    payableSats,
    publicCopyRefs: retired
      ? ['copy.public.debt_receipt.retired_with_settlement_receipt']
      : payable
        ? ['copy.public.debt_receipt.payable_pending_settlement']
        : defined
          ? ['copy.public.debt_receipt.defined_not_settled']
          : ['copy.public.debt_receipt.blocked'],
    quarantineReasonRefs,
    retiredReceiptRefs,
    reviewDecisionRefs,
    roleRefs,
    scopeRefs,
    settlementApprovalRefs,
    settlementClaimAllowed: retired,
    settlementReceiptRefs,
    settledSats,
    sourceRefs,
    spendAuthorityDelegatedToWorker: false,
    state,
    stopConditionRefs,
    targetMetricRefs,
    verificationCommandRefs,
    workerPayoutEligible: payable || retired,
  })
}

export const debtReceiptSettlementHasPrivateMaterial = (
  projection: DebtReceiptSettlementProjection,
): boolean => {
  const publicValues = [
    projection.state,
    ...projection.acceptedWorkRefs,
    ...projection.baselineMetricRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.duplicateFingerprintRefs,
    ...projection.fundingApprovalRefs,
    ...projection.fundingAuthorityRefs,
    ...projection.hygieneDeltaRefs,
    ...projection.noNewEqualOrWorseDebtRefs,
    ...projection.publicCopyRefs,
    ...projection.quarantineReasonRefs,
    ...projection.retiredReceiptRefs,
    ...projection.reviewDecisionRefs,
    ...projection.scopeRefs,
    ...projection.settlementApprovalRefs,
    ...projection.settlementReceiptRefs,
    ...projection.sourceRefs,
    ...projection.stopConditionRefs,
    ...projection.targetMetricRefs,
    ...projection.verificationCommandRefs,
    ...Object.values(projection.roleRefs).filter(ref => ref !== null),
  ]

  return publicValues.some(
    value =>
      containsProviderSecretMaterial(value) ||
      unsafeDebtReceiptRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )
}
