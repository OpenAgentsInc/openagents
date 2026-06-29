import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type DebtReceiptKey,
  type DebtReceiptKeyInput,
  type PatchNoveltyKey,
  type PatchNoveltyKeyInput,
  debtReceiptKeyShortRef,
  deriveDebtReceiptKey,
  derivePatchNoveltyKey,
  patchNoveltyKeyShortRef,
} from './debt-receipt-key'

// Debt-receipt settlement policy for the funded, benchmark-verified hygiene lane
// (EPIC #5335). Design and the first policy/test packet are credited to Trigger
// (Codex Loop Guard) on branches codex/debt-receipt-policy and
// codex/study-hygiene-lane. This module incorporates that work and adds:
//   - a typed DebtReceiptKey/PatchNoveltyKey fingerprint model that enforces
//     exactly one accepted settlement per DebtReceiptKey, then retired
//     (duplicate replay against a retired key is not payable); and
//   - a fail-closed studied-knowledge gate: bad/invalid studied-knowledge
//     evidence may not leave a contribution payable, even when it is optional.

export const DebtReceiptSettlementState = S.Literals([
  'blocked',
  'fundable',
  'funded',
  'verified',
  'credit_class',
  'payable',
  'retired',
  'duplicate_replay',
  'quarantined',
])
export type DebtReceiptSettlementState =
  typeof DebtReceiptSettlementState.Type

export const DebtReceiptWorkClass = S.Literals([
  'code_hygiene',
  'documentation_or_journal',
])
export type DebtReceiptWorkClass = typeof DebtReceiptWorkClass.Type

export const DebtReceiptStudiedKnowledgeVerificationSchemaRef =
  'openagents.repo_studied_knowledge_verification.v0' as const

export const DebtReceiptStudiedKnowledgeSource = S.Struct({
  correctnessGatePassed: S.Boolean,
  graphRef: S.String,
  packetRef: S.String,
  rejectedCount: S.Number,
  schemaRef: S.Literal(DebtReceiptStudiedKnowledgeVerificationSchemaRef),
  sourceBoundary: S.Literal('public_refs_only'),
  validatorReviewRefs: S.Array(S.String),
  validatorReviewRequired: S.Boolean,
  verificationRef: S.String,
})
export type DebtReceiptStudiedKnowledgeSource =
  typeof DebtReceiptStudiedKnowledgeSource.Type

export const DebtReceiptSettlementProjection = S.Struct({
  acceptedWorkRefs: S.Array(S.String),
  baselineMetricRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetCapSats: S.Number,
  caveatRefs: S.Array(S.String),
  debtReceiptKey: S.NullOr(S.String),
  duplicateFingerprintRefs: S.Array(S.String),
  duplicateReplay: S.Boolean,
  fundingApprovalRefs: S.Array(S.String),
  fundingAuthorityRefs: S.Array(S.String),
  hygieneDeltaRefs: S.Array(S.String),
  manualReviewOnly: S.Boolean,
  noNewEqualOrWorseDebtRefs: S.Array(S.String),
  patchNoveltyKey: S.NullOr(S.String),
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
  studiedKnowledgeGatePassed: S.Boolean,
  studiedKnowledgeRequired: S.Boolean,
  studiedKnowledgeSource: S.NullOr(DebtReceiptStudiedKnowledgeSource),
  studiedKnowledgeSourceRefs: S.Array(S.String),
  targetMetricRefs: S.Array(S.String),
  verificationCommandRefs: S.Array(S.String),
  workClass: DebtReceiptWorkClass,
  workerPayoutEligible: S.Boolean,
})
export type DebtReceiptSettlementProjection =
  typeof DebtReceiptSettlementProjection.Type

export type DebtReceiptSettlementInput = Readonly<{
  acceptedWorkRefs?: ReadonlyArray<string> | undefined
  baselineMetricRefs?: ReadonlyArray<string> | undefined
  budgetCapSats?: number | undefined
  debtReceiptKeyInput?: DebtReceiptKeyInput | undefined
  duplicateFingerprintRefs?: ReadonlyArray<string> | undefined
  fundingApprovalRefs?: ReadonlyArray<string> | undefined
  fundingAuthorityActorRef?: string | null | undefined
  fundingAuthorityRefs?: ReadonlyArray<string> | undefined
  hygieneDeltaRefs?: ReadonlyArray<string> | undefined
  maxRevisionAttempts?: number | undefined
  noNewEqualOrWorseDebtRefs?: ReadonlyArray<string> | undefined
  patchNoveltyKeyInput?:
    | Omit<PatchNoveltyKeyInput, 'debtReceiptKey'>
    | undefined
  payableSats?: number | undefined
  proposerActorRef?: string | null | undefined
  retiredDebtReceiptKeys?: ReadonlyArray<DebtReceiptKey> | undefined
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
  studiedKnowledgeRequired?: boolean | undefined
  studiedKnowledgeSource?: DebtReceiptStudiedKnowledgeSource | null | undefined
  targetMetricRefs?: ReadonlyArray<string> | undefined
  verificationCommandRefs?: ReadonlyArray<string> | undefined
  workClass?: DebtReceiptWorkClass | undefined
  workerActorRef?: string | null | undefined
}>

export class DebtReceiptPolicyUnsafe extends S.TaggedErrorClass<DebtReceiptPolicyUnsafe>()(
  'DebtReceiptPolicyUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeProjection = S.decodeUnknownSync(DebtReceiptSettlementProjection)
const decodeStudiedKnowledgeSource = S.decodeUnknownSync(
  DebtReceiptStudiedKnowledgeSource,
)

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

const safeWorkClass = (
  value: DebtReceiptWorkClass | undefined,
): DebtReceiptWorkClass => value ?? 'code_hygiene'

const safeStudiedKnowledgeSource = (
  source: DebtReceiptStudiedKnowledgeSource | null | undefined,
): DebtReceiptStudiedKnowledgeSource | null => {
  if (source === null || source === undefined) {
    return null
  }

  const decoded = decodeStudiedKnowledgeSource(source)
  const packetRef = safeRefs('Debt receipt studied packet ref', [
    decoded.packetRef,
  ])[0]!
  const graphRef = safeRefs('Debt receipt studied graph ref', [
    decoded.graphRef,
  ])[0]!
  const verificationRef = safeRefs('Debt receipt studied verification ref', [
    decoded.verificationRef,
  ])[0]!
  const validatorReviewRefs = safeRefs(
    'Debt receipt studied validator review refs',
    decoded.validatorReviewRefs,
  )
  const rejectedCount = safeNonNegativeInteger(
    'studiedKnowledgeSource.rejectedCount',
    decoded.rejectedCount,
  )

  return {
    ...decoded,
    graphRef,
    packetRef,
    rejectedCount,
    validatorReviewRefs,
    verificationRef,
  }
}

// A studied-knowledge source is internally valid only when its own correctness
// gate passed with no rejected claims and no pending validator review.
const studiedKnowledgeSourceIsValid = (
  source: DebtReceiptStudiedKnowledgeSource,
): boolean =>
  source.correctnessGatePassed &&
  source.rejectedCount === 0 &&
  !source.validatorReviewRequired

const studiedKnowledgeBlockers = (
  source: DebtReceiptStudiedKnowledgeSource | null,
  required: boolean,
): ReadonlyArray<string> => [
  ...(required && source === null
    ? ['blocker.public.debt_receipt.studied_knowledge_source_missing']
    : []),
  ...(source !== null && !source.correctnessGatePassed
    ? ['blocker.public.debt_receipt.studied_knowledge_correctness_failed']
    : []),
  ...(source !== null && source.rejectedCount > 0
    ? ['blocker.public.debt_receipt.studied_knowledge_rejected_claims']
    : []),
  ...(source !== null && source.validatorReviewRequired
    ? ['blocker.public.debt_receipt.studied_knowledge_validator_review_required']
    : []),
]

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
      blockers.push(
        `blocker.public.debt_receipt.role_overlap.${firstLabel}_${label}`,
      )
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
  const workClass = safeWorkClass(input.workClass)
  const documentationCreditOnly = workClass === 'documentation_or_journal'
  const maxRevisionAttempts = safeNonNegativeInteger(
    'maxRevisionAttempts',
    input.maxRevisionAttempts ?? 3,
  )
  const requestedPayableSats = safeNonNegativeInteger(
    'payableSats',
    input.payableSats,
  )
  const payableSats = documentationCreditOnly ? 0 : requestedPayableSats
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

  // Typed fingerprint keys (EPIC #5335 dup/novelty fingerprint comment).
  const debtReceiptKey =
    input.debtReceiptKeyInput === undefined
      ? null
      : deriveDebtReceiptKey(input.debtReceiptKeyInput)
  const patchNoveltyKey =
    debtReceiptKey === null || input.patchNoveltyKeyInput === undefined
      ? null
      : derivePatchNoveltyKey({
          ...input.patchNoveltyKeyInput,
          debtReceiptKey,
        })
  const retiredDebtReceiptKeys = new Set(input.retiredDebtReceiptKeys ?? [])

  const studiedKnowledgeRequired = input.studiedKnowledgeRequired ?? false
  const studiedKnowledgeSource = safeStudiedKnowledgeSource(
    input.studiedKnowledgeSource,
  )
  const studiedKnowledgeSourceRefs =
    studiedKnowledgeSource === null
      ? []
      : [
          studiedKnowledgeSource.graphRef,
          studiedKnowledgeSource.packetRef,
          studiedKnowledgeSource.verificationRef,
          ...studiedKnowledgeSource.validatorReviewRefs,
        ].sort()
  // Fail closed: a present studied-knowledge source must itself be valid, even
  // when studied knowledge is optional. A required source must additionally be
  // present. This prevents bad optional evidence from attaching blockers while
  // still allowing a payable state.
  const studiedKnowledgeGatePassed =
    studiedKnowledgeSource === null
      ? !studiedKnowledgeRequired
      : studiedKnowledgeSourceIsValid(studiedKnowledgeSource)
  const studiedKnowledgeBlockerRefs = studiedKnowledgeBlockers(
    studiedKnowledgeSource,
    studiedKnowledgeRequired,
  )

  if (requestedPayableSats > budgetCapSats) {
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

  // One accepted settlement per DebtReceiptKey: a patch whose DebtReceiptKey is
  // already in the retired set is a duplicate replay. The typed key is the
  // authority; loose retired/fingerprint refs remain supplementary evidence.
  const typedDuplicateReplay =
    debtReceiptKey !== null && retiredDebtReceiptKeys.has(debtReceiptKey)
  const looseDuplicateReplay =
    hasRefs(retiredReceiptRefs) && hasRefs(duplicateFingerprintRefs)
  const duplicateReplay = typedDuplicateReplay || looseDuplicateReplay
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
    hasRefs(noNewEqualOrWorseDebtRefs) &&
    studiedKnowledgeGatePassed
  const payable =
    verified &&
    hasRefs(settlementApprovalRefs) &&
    payableSats > 0 &&
    !duplicateReplay &&
    !manualReviewOnly &&
    !documentationCreditOnly
  const retired =
    payable &&
    hasRefs(settlementReceiptRefs) &&
    settledSats === payableSats
  const creditClass =
    documentationCreditOnly &&
    verified &&
    !duplicateReplay &&
    !manualReviewOnly
  const state: DebtReceiptSettlementState = duplicateReplay
    ? 'duplicate_replay'
    : manualReviewOnly
      ? 'quarantined'
      : retired
        ? 'retired'
        : payable
          ? 'payable'
          : creditClass
            ? 'credit_class'
            : verified
              ? 'verified'
              : funded
                ? 'funded'
                : defined
                  ? 'fundable'
                  : 'blocked'

  const duplicateBlockerRefs = duplicateReplay
    ? [
        'blocker.public.debt_receipt.duplicate_replay',
        ...(typedDuplicateReplay && debtReceiptKey !== null
          ? [
              `blocker.public.debt_receipt.duplicate_replay.${debtReceiptKeyShortRef(
                debtReceiptKey,
              )}`,
            ]
          : []),
      ]
    : []
  const creditClassBlockerRefs = documentationCreditOnly
    ? ['blocker.public.debt_receipt.documentation_or_journal_credit_not_payable']
    : []
  const missingBlockerRefs = missingEvidenceBlockers({
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
  }).filter(
    ref =>
      !documentationCreditOnly ||
      (ref !== 'blocker.public.debt_receipt.settlement_approval_missing' &&
        ref !== 'blocker.public.debt_receipt.settlement_receipt_missing'),
  )

  return decodeProjection({
    acceptedWorkRefs,
    baselineMetricRefs,
    blockerRefs: [
      ...missingBlockerRefs,
      ...roleBlockers,
      ...studiedKnowledgeBlockerRefs,
      ...duplicateBlockerRefs,
      ...creditClassBlockerRefs,
      ...(manualReviewOnly
        ? ['blocker.public.debt_receipt.manual_review_only']
        : []),
    ].sort(),
    budgetCapSats,
    caveatRefs: [
      ...baseCaveatRefs,
      ...(documentationCreditOnly
        ? [
            'caveat.public.debt_receipt.documentation_or_journal_not_size_scaled',
          ]
        : []),
    ],
    debtReceiptKey,
    duplicateFingerprintRefs,
    duplicateReplay,
    fundingApprovalRefs,
    fundingAuthorityRefs,
    hygieneDeltaRefs,
    manualReviewOnly,
    noNewEqualOrWorseDebtRefs,
    patchNoveltyKey,
    payableSats,
    publicCopyRefs: creditClass
      ? ['copy.public.debt_receipt.credit_class_not_payable']
      : retired
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
    studiedKnowledgeGatePassed,
    studiedKnowledgeRequired,
    studiedKnowledgeSource,
    studiedKnowledgeSourceRefs,
    targetMetricRefs,
    verificationCommandRefs,
    workClass,
    workerPayoutEligible: payable || retired,
  })
}

export const debtReceiptSettlementHasPrivateMaterial = (
  projection: DebtReceiptSettlementProjection,
): boolean => {
  const publicValues = [
    projection.state,
    projection.workClass,
    ...(projection.debtReceiptKey === null ? [] : [projection.debtReceiptKey]),
    ...(projection.patchNoveltyKey === null ? [] : [projection.patchNoveltyKey]),
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
    ...projection.studiedKnowledgeSourceRefs,
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

export type {
  DebtReceiptKey,
  DebtReceiptKeyInput,
  PatchNoveltyKey,
  PatchNoveltyKeyInput,
}
export {
  debtReceiptKeyShortRef,
  deriveDebtReceiptKey,
  derivePatchNoveltyKey,
  patchNoveltyKeyShortRef,
}
