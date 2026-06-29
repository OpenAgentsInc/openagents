import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'
import { PylonResourceMode } from './pylon-resource-mode-setup'

export const PylonMarketplaceJobSource = S.Literals([
  'external_agent',
  'external_human',
  'openagents_seeded',
])
export type PylonMarketplaceJobSource =
  typeof PylonMarketplaceJobSource.Type

export const PylonMarketplaceJobKind = S.Literals([
  'artifact_review',
  'benchmark_evaluation',
  'embedding_data_prep',
  'gepa_dspy_optimization',
  'inference',
  'lora_finetuning',
  'training',
  'validation',
])
export type PylonMarketplaceJobKind = typeof PylonMarketplaceJobKind.Type

export const PylonMarketplacePrivacyClass = S.Literals([
  'customer_private',
  'dataset_private',
  'model_private',
  'operator_private',
  'public',
])
export type PylonMarketplacePrivacyClass =
  typeof PylonMarketplacePrivacyClass.Type

export const PylonMarketplaceIntakeState = S.Literals([
  'accepted_for_review',
  'assignment_proposed',
  'blocked',
  'draft',
  'intake_ready',
  'needs_input',
  'policy_gated',
  'rejected',
  'triaged',
])
export type PylonMarketplaceIntakeState =
  typeof PylonMarketplaceIntakeState.Type

export const PylonMarketplaceAssignmentState = S.Literals([
  'accepted',
  'assigned',
  'blocked',
  'cancelled',
  'held_for_authority',
  'proposed',
  'result_submitted',
  'running',
])
export type PylonMarketplaceAssignmentState =
  typeof PylonMarketplaceAssignmentState.Type

export const PylonMarketplacePayoutState = S.Literals([
  'accepted_work',
  'buyer_payment_evidence',
  'not_applicable',
  'planned',
  'payout_dispatched',
  'payout_eligible',
  'payout_verified',
  'reward_intent_recorded',
  'settled',
])
export type PylonMarketplacePayoutState =
  typeof PylonMarketplacePayoutState.Type

export class PylonMarketplaceAuthority extends S.Class<PylonMarketplaceAuthority>(
  'PylonMarketplaceAuthority',
)({
  buyerChargeMutationAllowed: S.Boolean,
  paidAssignmentDispatchAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  proposalAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  triageAllowed: S.Boolean,
}) {}

export class PylonMarketplaceJobIntakeRecord extends S.Class<PylonMarketplaceJobIntakeRecord>(
  'PylonMarketplaceJobIntakeRecord',
)({
  benchmarkRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataRefs: S.Array(S.String),
  eligibilityRequirementRefs: S.Array(S.String),
  evidenceExpectationRefs: S.Array(S.String),
  intakeRef: S.String,
  jobKind: PylonMarketplaceJobKind,
  jobRef: S.String,
  modelRefs: S.Array(S.String),
  policyGateRefs: S.Array(S.String),
  privacyClass: PylonMarketplacePrivacyClass,
  requesterRef: S.String,
  resourceModePreference: PylonResourceMode,
  resourceRequirementRefs: S.Array(S.String),
  resultExpectationRefs: S.Array(S.String),
  source: PylonMarketplaceJobSource,
  sourceRefs: S.Array(S.String),
  spendCaveatRefs: S.Array(S.String),
  state: PylonMarketplaceIntakeState,
  updatedAtIso: S.String,
}) {}

export class PylonMarketplaceAssignmentRecord extends S.Class<PylonMarketplaceAssignmentRecord>(
  'PylonMarketplaceAssignmentRecord',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  acceptedWorkRefs: S.Array(S.String),
  artifactEvidenceRefs: S.Array(S.String),
  assignmentAuthorityRefs: S.Array(S.String),
  assignmentRef: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  intakeRef: S.String,
  jobRef: S.String,
  nexusReceiptRefs: S.Array(S.String),
  payoutCaveatRefs: S.Array(S.String),
  payoutState: PylonMarketplacePayoutState,
  providerEligibilityRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  pylonReceiptRefs: S.Array(S.String),
  resourceMode: PylonResourceMode,
  resultEvidenceRefs: S.Array(S.String),
  state: PylonMarketplaceAssignmentState,
  treasuryReceiptRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonMarketplaceLedgerRecord extends S.Class<PylonMarketplaceLedgerRecord>(
  'PylonMarketplaceLedgerRecord',
)({
  agentRef: S.String,
  assignmentRecords: S.Array(PylonMarketplaceAssignmentRecord),
  authority: PylonMarketplaceAuthority,
  caveatRefs: S.Array(S.String),
  intakeRecords: S.Array(PylonMarketplaceJobIntakeRecord),
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonMarketplaceJobIntakeProjection extends S.Class<PylonMarketplaceJobIntakeProjection>(
  'PylonMarketplaceJobIntakeProjection',
)({
  benchmarkRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dataRefs: S.Array(S.String),
  eligibilityRequirementRefs: S.Array(S.String),
  evidenceExpectationRefs: S.Array(S.String),
  intakeRef: S.String,
  jobKind: PylonMarketplaceJobKind,
  jobRef: S.String,
  modelRefs: S.Array(S.String),
  policyGateRefs: S.Array(S.String),
  privacyClass: PylonMarketplacePrivacyClass,
  requesterRef: S.String,
  resourceModePreference: PylonResourceMode,
  resourceRequirementRefs: S.Array(S.String),
  resultExpectationRefs: S.Array(S.String),
  source: PylonMarketplaceJobSource,
  sourceRefs: S.Array(S.String),
  spendCaveatRefs: S.Array(S.String),
  state: PylonMarketplaceIntakeState,
  updatedAtDisplay: S.String,
}) {}

export class PylonMarketplaceAssignmentProjection extends S.Class<PylonMarketplaceAssignmentProjection>(
  'PylonMarketplaceAssignmentProjection',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  artifactEvidenceRefs: S.Array(S.String),
  assignmentAuthorityRefs: S.Array(S.String),
  assignmentRef: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  intakeRef: S.String,
  jobRef: S.String,
  nexusReceiptRefs: S.Array(S.String),
  paidAssignmentClaimAllowed: S.Boolean,
  payoutCaveatRefs: S.Array(S.String),
  payoutState: PylonMarketplacePayoutState,
  payoutStateLabel: S.String,
  providerEligibilityRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  pylonReceiptRefs: S.Array(S.String),
  resourceMode: PylonResourceMode,
  resultEvidenceRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  state: PylonMarketplaceAssignmentState,
  stateLabel: S.String,
  treasuryReceiptRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class PylonMarketplaceLedgerProjection extends S.Class<PylonMarketplaceLedgerProjection>(
  'PylonMarketplaceLedgerProjection',
)({
  agentRef: S.String,
  assignmentCount: S.Number,
  assignmentRecords: S.Array(PylonMarketplaceAssignmentProjection),
  audience: OmniProjectionAudience,
  authority: PylonMarketplaceAuthority,
  caveatRefs: S.Array(S.String),
  externalPolicyGatedCount: S.Number,
  intakeCount: S.Number,
  intakeRecords: S.Array(PylonMarketplaceJobIntakeProjection),
  ledgerRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class PylonMarketplaceUnsafe extends S.TaggedErrorClass<PylonMarketplaceUnsafe>()(
  'PylonMarketplaceUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_MARKETPLACE_JOB_KINDS:
  ReadonlyArray<PylonMarketplaceJobKind> = [
    'artifact_review',
    'benchmark_evaluation',
    'embedding_data_prep',
    'gepa_dspy_optimization',
    'inference',
    'lora_finetuning',
    'training',
    'validation',
  ]

export const PYLON_MARKETPLACE_NO_SPEND_AUTHORITY:
  PylonMarketplaceAuthority = {
    buyerChargeMutationAllowed: false,
    paidAssignmentDispatchAllowed: false,
    payoutMutationAllowed: false,
    proposalAllowed: true,
    settlementMutationAllowed: false,
    triageAllowed: true,
  }

const assignmentStateRank:
  Readonly<Record<PylonMarketplaceAssignmentState, number>> = {
    accepted: 4,
    assigned: 1,
    blocked: -1,
    cancelled: -1,
    held_for_authority: 0,
    proposed: 0,
    result_submitted: 3,
    running: 2,
  }

const payoutStateRank: Readonly<Record<PylonMarketplacePayoutState, number>> = {
  accepted_work: 4,
  buyer_payment_evidence: 1,
  not_applicable: -1,
  payout_dispatched: 7,
  payout_eligible: 6,
  payout_verified: 8,
  planned: 0,
  reward_intent_recorded: 5,
  settled: 9,
}

const assignmentStateLabel:
  Readonly<Record<PylonMarketplaceAssignmentState, string>> = {
    accepted: 'Accepted',
    assigned: 'Assigned',
    blocked: 'Blocked',
    cancelled: 'Cancelled',
    held_for_authority: 'Held for authority',
    proposed: 'Proposed',
    result_submitted: 'Result submitted',
    running: 'Running',
  }

const payoutStateLabel: Readonly<Record<PylonMarketplacePayoutState, string>> = {
  accepted_work: 'Accepted work',
  buyer_payment_evidence: 'Buyer payment evidence',
  not_applicable: 'Not applicable',
  payout_dispatched: 'Payout dispatched',
  payout_eligible: 'Payout eligible',
  payout_verified: 'Payout verified',
  planned: 'Planned',
  reward_intent_recorded: 'Reward intent recorded',
  settled: 'Settled',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMarketplaceRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset[._-]?(raw|private|secret|payload)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[._-]?(artifact|raw|secret|weights)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|model|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(artifact\.private|authority\.operator|buyer[_-]?payment|evidence\.private|model\.private|nexus\.private|operator\.|pylon\.private|requester\.private|treasury\.private)/i
const payoutBasisForbiddenPattern =
  /(omega[_-]?forum[_-]?reward|forum[_-]?reward|generic[_-]?job[_-]?creation)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assignmentAtLeast = (
  state: PylonMarketplaceAssignmentState,
  threshold: PylonMarketplaceAssignmentState,
): boolean => assignmentStateRank[state] >= assignmentStateRank[threshold]

const payoutAtLeast = (
  state: PylonMarketplacePayoutState,
  threshold: PylonMarketplacePayoutState,
): boolean => payoutStateRank[state] >= payoutStateRank[threshold]

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new PylonMarketplaceUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeMarketplaceRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonMarketplaceUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, raw model, raw dataset, raw prompt, raw artifact, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator' || audience === 'private') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const requesterForAudience = (
  intake: PylonMarketplaceJobIntakeRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  assertSafeRefs('Pylon marketplace requester ref', [intake.requesterRef])

  if (
    intake.privacyClass === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return intake.requesterRef
  }

  return 'requester.redacted'
}

const providerRefsForAudience = (
  assignment: PylonMarketplaceAssignmentRecord,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  if (audience === 'operator' || audience === 'private') {
    return refsForAudience(
      'Pylon marketplace provider refs',
      assignment.providerRefs,
      audience,
    )
  }

  return refsForAudience(
    'Pylon marketplace provider refs',
    assignment.providerRefs,
    audience,
  ).filter(ref => !ref.includes('.private'))
}

const assertNoSpendAuthority = (
  authority: PylonMarketplaceAuthority,
): void => {
  if (
    authority.buyerChargeMutationAllowed ||
    authority.paidAssignmentDispatchAllowed ||
    authority.payoutMutationAllowed ||
    authority.settlementMutationAllowed
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Pylon marketplace records do not grant buyer-charge, paid-assignment dispatch, payout, or settlement mutation authority.',
    })
  }
}

const assertIntake = (
  intake: PylonMarketplaceJobIntakeRecord,
): void => {
  assertValidIso('intake.createdAtIso', intake.createdAtIso)
  assertValidIso('intake.updatedAtIso', intake.updatedAtIso)
  assertSafeRefs('Pylon marketplace intake identity refs', [
    intake.intakeRef,
    intake.jobRef,
    intake.requesterRef,
  ])
  assertSafeRefs('Pylon marketplace benchmark refs', intake.benchmarkRefs)
  assertSafeRefs('Pylon marketplace blocker refs', intake.blockerRefs)
  assertSafeRefs('Pylon marketplace budget refs', intake.budgetRefs)
  assertSafeRefs('Pylon marketplace caveat refs', intake.caveatRefs)
  assertSafeRefs('Pylon marketplace data refs', intake.dataRefs)
  assertSafeRefs(
    'Pylon marketplace eligibility requirement refs',
    intake.eligibilityRequirementRefs,
  )
  assertSafeRefs(
    'Pylon marketplace evidence expectation refs',
    intake.evidenceExpectationRefs,
  )
  assertSafeRefs('Pylon marketplace model refs', intake.modelRefs)
  assertSafeRefs('Pylon marketplace policy gate refs', intake.policyGateRefs)
  assertSafeRefs(
    'Pylon marketplace resource requirement refs',
    intake.resourceRequirementRefs,
  )
  assertSafeRefs(
    'Pylon marketplace result expectation refs',
    intake.resultExpectationRefs,
  )
  assertSafeRefs('Pylon marketplace source refs', intake.sourceRefs)
  assertSafeRefs('Pylon marketplace spend caveat refs', intake.spendCaveatRefs)

  if (!hasAny(intake.sourceRefs)) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace intake requires source refs.',
    })
  }

  if (
    intake.source !== 'openagents_seeded' &&
    !hasAny(intake.policyGateRefs)
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'External Pylon marketplace jobs require policy gate refs before triage.',
    })
  }

  if (intake.state === 'blocked' && !hasAny(intake.blockerRefs)) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Blocked Pylon marketplace intakes require blocker refs.',
    })
  }

  if (
    (intake.state === 'needs_input' || intake.state === 'rejected') &&
    !hasAny(intake.blockerRefs)
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Needs-input and rejected Pylon marketplace intakes require blocker refs.',
    })
  }

  if (
    !hasAny(intake.resourceRequirementRefs) ||
    !hasAny(intake.eligibilityRequirementRefs) ||
    !hasAny(intake.resultExpectationRefs) ||
    !hasAny(intake.evidenceExpectationRefs)
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Pylon marketplace intakes require resource, eligibility, result, and evidence expectations.',
    })
  }
}

const assertReceiptBackedPayout = (
  assignment: PylonMarketplaceAssignmentRecord,
): void => {
  const receiptRefs = [
    ...assignment.nexusReceiptRefs,
    ...assignment.pylonReceiptRefs,
    ...assignment.treasuryReceiptRefs,
  ]

  assertSafeRefs('Pylon marketplace payout receipt refs', receiptRefs)

  if (
    [
      ...receiptRefs,
      ...assignment.acceptedWorkRefs,
      ...assignment.payoutCaveatRefs,
    ].some(ref => payoutBasisForbiddenPattern.test(ref))
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Pylon marketplace payouts must be tied to Nexus, Treasury, or Pylon receipts, not Forum rewards or generic job creation.',
    })
  }

  if (
    payoutAtLeast(assignment.payoutState, 'accepted_work') &&
    (!hasAny(assignment.acceptedWorkRefs) ||
      !hasAny(assignment.nexusReceiptRefs) ||
      !hasAny(assignment.pylonReceiptRefs) ||
      !hasAny(assignment.treasuryReceiptRefs) ||
      !hasAny(assignment.payoutCaveatRefs))
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Accepted-work payout states require accepted-work refs plus Nexus, Pylon, Treasury, and payout-caveat refs.',
    })
  }
}

const assertAssignment = (
  assignment: PylonMarketplaceAssignmentRecord,
): void => {
  assertValidIso('assignment.updatedAtIso', assignment.updatedAtIso)
  assertSafeRefs('Pylon marketplace assignment identity refs', [
    assignment.assignmentRef,
    assignment.intakeRef,
    assignment.jobRef,
  ])
  assertSafeRefs(
    'Pylon marketplace acceptance criteria refs',
    assignment.acceptanceCriteriaRefs,
  )
  assertSafeRefs(
    'Pylon marketplace accepted work refs',
    assignment.acceptedWorkRefs,
  )
  assertSafeRefs(
    'Pylon marketplace artifact evidence refs',
    assignment.artifactEvidenceRefs,
  )
  assertSafeRefs(
    'Pylon marketplace assignment authority refs',
    assignment.assignmentAuthorityRefs,
  )
  assertSafeRefs('Pylon marketplace blocker refs', assignment.blockerRefs)
  assertSafeRefs('Pylon marketplace caveat refs', assignment.caveatRefs)
  assertSafeRefs('Pylon marketplace nexus receipt refs', assignment.nexusReceiptRefs)
  assertSafeRefs(
    'Pylon marketplace payout caveat refs',
    assignment.payoutCaveatRefs,
  )
  assertSafeRefs(
    'Pylon marketplace provider eligibility refs',
    assignment.providerEligibilityRefs,
  )
  assertSafeRefs('Pylon marketplace provider refs', assignment.providerRefs)
  assertSafeRefs('Pylon marketplace Pylon receipt refs', assignment.pylonReceiptRefs)
  assertSafeRefs(
    'Pylon marketplace result evidence refs',
    assignment.resultEvidenceRefs,
  )
  assertSafeRefs(
    'Pylon marketplace Treasury receipt refs',
    assignment.treasuryReceiptRefs,
  )
  assertReceiptBackedPayout(assignment)

  if (
    assignmentAtLeast(assignment.state, 'assigned') &&
    (!hasAny(assignment.assignmentAuthorityRefs) ||
      !hasAny(assignment.providerEligibilityRefs) ||
      !hasAny(assignment.pylonReceiptRefs))
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Assigned Pylon marketplace jobs require assignment authority, provider eligibility, and Pylon assignment receipt refs.',
    })
  }

  if (
    assignmentAtLeast(assignment.state, 'result_submitted') &&
    (!hasAny(assignment.artifactEvidenceRefs) ||
      !hasAny(assignment.resultEvidenceRefs))
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Result-submitted Pylon marketplace jobs require artifact and result evidence refs.',
    })
  }

  if (
    assignment.state === 'accepted' &&
    (!hasAny(assignment.acceptanceCriteriaRefs) ||
      !hasAny(assignment.acceptedWorkRefs))
  ) {
    throw new PylonMarketplaceUnsafe({
      reason:
        'Accepted Pylon marketplace jobs require acceptance criteria and accepted-work refs.',
    })
  }

  if (assignment.state === 'blocked' && !hasAny(assignment.blockerRefs)) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Blocked Pylon marketplace assignments require blocker refs.',
    })
  }
}

const assertLedger = (ledger: PylonMarketplaceLedgerRecord): void => {
  assertValidIso('ledger.updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Pylon marketplace ledger refs', [
    ledger.agentRef,
    ledger.ledgerRef,
  ])
  assertSafeRefs('Pylon marketplace ledger caveat refs', ledger.caveatRefs)
  assertSafeRefs('Pylon marketplace ledger source refs', ledger.sourceRefs)
  assertNoSpendAuthority(ledger.authority)

  if (ledger.agentRef !== 'agent_artanis') {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace ledgers must be administered by agent_artanis.',
    })
  }

  if (!ledger.authority.triageAllowed || !ledger.authority.proposalAllowed) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace ledgers must allow Artanis triage and proposal.',
    })
  }

  if (!hasAny(ledger.intakeRecords)) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace ledgers require at least one intake record.',
    })
  }

  ledger.intakeRecords.forEach(assertIntake)
  ledger.assignmentRecords.forEach(assertAssignment)

  const intakeKeys = new Set(
    ledger.intakeRecords.map(intake => `${intake.intakeRef}:${intake.jobRef}`),
  )
  const orphan = ledger.assignmentRecords.find(
    assignment => !intakeKeys.has(`${assignment.intakeRef}:${assignment.jobRef}`),
  )

  if (orphan !== undefined) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace assignments must link to a known intake/job.',
    })
  }
}

const projectIntake = (
  intake: PylonMarketplaceJobIntakeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonMarketplaceJobIntakeProjection => ({
  benchmarkRefs: refsForAudience(
    'Pylon marketplace benchmark refs',
    intake.benchmarkRefs,
    audience,
  ),
  blockerRefs: refsForAudience(
    'Pylon marketplace blocker refs',
    intake.blockerRefs,
    audience,
  ),
  budgetRefs: refsForAudience(
    'Pylon marketplace budget refs',
    intake.budgetRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Pylon marketplace caveat refs',
    intake.caveatRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    intake.createdAtIso,
    nowIso,
  ),
  dataRefs: refsForAudience(
    'Pylon marketplace data refs',
    intake.dataRefs,
    audience,
  ),
  eligibilityRequirementRefs: refsForAudience(
    'Pylon marketplace eligibility requirement refs',
    intake.eligibilityRequirementRefs,
    audience,
  ),
  evidenceExpectationRefs: refsForAudience(
    'Pylon marketplace evidence expectation refs',
    intake.evidenceExpectationRefs,
    audience,
  ),
  intakeRef: refsForAudience(
    'Pylon marketplace intake ref',
    [intake.intakeRef],
    audience,
  )[0] ?? 'intake.redacted',
  jobKind: intake.jobKind,
  jobRef: refsForAudience(
    'Pylon marketplace job ref',
    [intake.jobRef],
    audience,
  )[0] ?? 'job.redacted',
  modelRefs: refsForAudience(
    'Pylon marketplace model refs',
    intake.modelRefs,
    audience,
  ),
  policyGateRefs: refsForAudience(
    'Pylon marketplace policy gate refs',
    intake.policyGateRefs,
    audience,
  ),
  privacyClass: intake.privacyClass,
  requesterRef: requesterForAudience(intake, audience),
  resourceModePreference: intake.resourceModePreference,
  resourceRequirementRefs: refsForAudience(
    'Pylon marketplace resource requirement refs',
    intake.resourceRequirementRefs,
    audience,
  ),
  resultExpectationRefs: refsForAudience(
    'Pylon marketplace result expectation refs',
    intake.resultExpectationRefs,
    audience,
  ),
  source: intake.source,
  sourceRefs: refsForAudience(
    'Pylon marketplace source refs',
    intake.sourceRefs,
    audience,
  ),
  spendCaveatRefs: refsForAudience(
    'Pylon marketplace spend caveat refs',
    intake.spendCaveatRefs,
    audience,
  ),
  state: intake.state,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    intake.updatedAtIso,
    nowIso,
  ),
})

const projectAssignment = (
  assignment: PylonMarketplaceAssignmentRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonMarketplaceAssignmentProjection => {
  const acceptedWorkClaimAllowed =
    assignment.payoutState === 'accepted_work' ||
    payoutAtLeast(assignment.payoutState, 'reward_intent_recorded')
  const paidAssignmentClaimAllowed =
    assignmentAtLeast(assignment.state, 'assigned') &&
    hasAny(assignment.assignmentAuthorityRefs) &&
    hasAny(assignment.providerEligibilityRefs) &&
    hasAny(assignment.pylonReceiptRefs)
  const settlementClaimAllowed =
    assignment.payoutState === 'settled' &&
    hasAny(assignment.nexusReceiptRefs) &&
    hasAny(assignment.pylonReceiptRefs) &&
    hasAny(assignment.treasuryReceiptRefs)

  return {
    acceptanceCriteriaRefs: refsForAudience(
      'Pylon marketplace acceptance criteria refs',
      assignment.acceptanceCriteriaRefs,
      audience,
    ),
    acceptedWorkClaimAllowed,
    acceptedWorkRefs: refsForAudience(
      'Pylon marketplace accepted work refs',
      assignment.acceptedWorkRefs,
      audience,
    ),
    artifactEvidenceRefs: refsForAudience(
      'Pylon marketplace artifact evidence refs',
      assignment.artifactEvidenceRefs,
      audience,
    ),
    assignmentAuthorityRefs: refsForAudience(
      'Pylon marketplace assignment authority refs',
      assignment.assignmentAuthorityRefs,
      audience,
    ),
    assignmentRef: refsForAudience(
      'Pylon marketplace assignment ref',
      [assignment.assignmentRef],
      audience,
    )[0] ?? 'assignment.redacted',
    blockerRefs: refsForAudience(
      'Pylon marketplace assignment blocker refs',
      assignment.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Pylon marketplace assignment caveat refs',
      assignment.caveatRefs,
      audience,
    ),
    intakeRef: refsForAudience(
      'Pylon marketplace assignment intake ref',
      [assignment.intakeRef],
      audience,
    )[0] ?? 'intake.redacted',
    jobRef: refsForAudience(
      'Pylon marketplace assignment job ref',
      [assignment.jobRef],
      audience,
    )[0] ?? 'job.redacted',
    nexusReceiptRefs: refsForAudience(
      'Pylon marketplace Nexus receipt refs',
      assignment.nexusReceiptRefs,
      audience,
    ),
    paidAssignmentClaimAllowed,
    payoutCaveatRefs: refsForAudience(
      'Pylon marketplace payout caveat refs',
      assignment.payoutCaveatRefs,
      audience,
    ),
    payoutState: assignment.payoutState,
    payoutStateLabel: payoutStateLabel[assignment.payoutState],
    providerEligibilityRefs: refsForAudience(
      'Pylon marketplace provider eligibility refs',
      assignment.providerEligibilityRefs,
      audience,
    ),
    providerRefs: providerRefsForAudience(assignment, audience),
    pylonReceiptRefs: refsForAudience(
      'Pylon marketplace Pylon receipt refs',
      assignment.pylonReceiptRefs,
      audience,
    ),
    resourceMode: assignment.resourceMode,
    resultEvidenceRefs: refsForAudience(
      'Pylon marketplace result evidence refs',
      assignment.resultEvidenceRefs,
      audience,
    ),
    settlementClaimAllowed,
    state: assignment.state,
    stateLabel: assignmentStateLabel[assignment.state],
    treasuryReceiptRefs: refsForAudience(
      'Pylon marketplace Treasury receipt refs',
      assignment.treasuryReceiptRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      assignment.updatedAtIso,
      nowIso,
    ),
  }
}

export const pylonMarketplaceProjectionHasPrivateMaterial = (
  projection: PylonMarketplaceLedgerProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return containsProviderSecretMaterial(serialized) ||
    unsafeMarketplaceRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized) ||
    (
      projection.audience !== 'operator' &&
      projection.audience !== 'private' &&
      publicUnsafeRefPattern.test(serialized)
    )
}

export const projectPylonMarketplaceLedger = (
  ledger: PylonMarketplaceLedgerRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonMarketplaceLedgerProjection => {
  assertLedger(ledger)

  const projection: PylonMarketplaceLedgerProjection = {
    agentRef: ledger.agentRef,
    assignmentCount: ledger.assignmentRecords.length,
    assignmentRecords: ledger.assignmentRecords.map(assignment =>
      projectAssignment(assignment, audience, nowIso),
    ),
    audience,
    authority: ledger.authority,
    caveatRefs: refsForAudience(
      'Pylon marketplace ledger caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    externalPolicyGatedCount: ledger.intakeRecords.filter(
      intake =>
        intake.source !== 'openagents_seeded' &&
        hasAny(intake.policyGateRefs),
    ).length,
    intakeCount: ledger.intakeRecords.length,
    intakeRecords: ledger.intakeRecords.map(intake =>
      projectIntake(intake, audience, nowIso),
    ),
    ledgerRef: refsForAudience(
      'Pylon marketplace ledger ref',
      [ledger.ledgerRef],
      audience,
    )[0] ?? 'ledger.redacted',
    sourceRefs: refsForAudience(
      'Pylon marketplace ledger source refs',
      ledger.sourceRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  }

  if (pylonMarketplaceProjectionHasPrivateMaterial(projection)) {
    throw new PylonMarketplaceUnsafe({
      reason: 'Pylon marketplace projection contains private material.',
    })
  }

  return projection
}

const intake = (
  input: Readonly<{
    intakeRef: string
    jobKind: PylonMarketplaceJobKind
    jobRef: string
    policyGateRefs?: ReadonlyArray<string> | undefined
    privacyClass: PylonMarketplacePrivacyClass
    requesterRef: string
    resourceModePreference: PylonResourceMode
    source: PylonMarketplaceJobSource
    state: PylonMarketplaceIntakeState
  }>,
): PylonMarketplaceJobIntakeRecord => ({
  benchmarkRefs: [`benchmark.public.${input.jobKind}`],
  blockerRefs: input.state === 'blocked'
    ? ['blocker.public.pylon_marketplace.policy_not_ready']
    : [],
  budgetRefs: ['budget.public.openagents_seeded_marketplace_initial'],
  caveatRefs: ['caveat.public.marketplace_assignment_not_payment'],
  createdAtIso: '2026-06-07T01:00:00.000Z',
  dataRefs: [`dataset.public.${input.jobKind}.manifest`],
  eligibilityRequirementRefs: [
    'eligibility.public.pylon_provider_registered',
    'eligibility.public.resource_mode_supported',
  ],
  evidenceExpectationRefs: [
    'evidence_expectation.public.redacted_artifact_manifest',
  ],
  intakeRef: input.intakeRef,
  jobKind: input.jobKind,
  jobRef: input.jobRef,
  modelRefs: [`model.public.${input.jobKind}.target`],
  policyGateRefs: [...(input.policyGateRefs ?? [])],
  privacyClass: input.privacyClass,
  requesterRef: input.requesterRef,
  resourceModePreference: input.resourceModePreference,
  resourceRequirementRefs: [
    `resource.public.pylon.${input.resourceModePreference}`,
  ],
  resultExpectationRefs: [
    `result_expectation.public.${input.jobKind}.summary`,
  ],
  source: input.source,
  sourceRefs: [`source.public.${input.source}.${input.jobKind}`],
  spendCaveatRefs: ['spend.public.no_live_spend_without_nexus_authority'],
  state: input.state,
  updatedAtIso: '2026-06-07T01:05:00.000Z',
})

export const examplePylonMarketplaceLedger = ():
  PylonMarketplaceLedgerRecord => ({
  agentRef: 'agent_artanis',
  assignmentRecords: [
    {
      acceptanceCriteriaRefs: [
        'acceptance.public.gepa_autopilot_benchmark_delta',
      ],
      acceptedWorkRefs: [
        'accepted_work.public.gepa_autopilot_smoke_patch',
      ],
      artifactEvidenceRefs: [
        'artifact.public.gepa_autopilot_redacted_manifest',
      ],
      assignmentAuthorityRefs: [
        'authority.public.nexus.pylon_assignment.approved',
      ],
      assignmentRef: 'assignment.public.pylon.gepa_autopilot_001',
      blockerRefs: [],
      caveatRefs: ['caveat.public.pylon_marketplace_assignment_evidence_only'],
      intakeRef: 'intake.public.openagents.gepa_autopilot_001',
      jobRef: 'job.public.pylon.gepa_autopilot_001',
      nexusReceiptRefs: ['receipt.public.nexus.assignment.gepa_autopilot_001'],
      payoutCaveatRefs: [
        'caveat.public.payout_waits_for_treasury_and_pylon_receipts',
      ],
      payoutState: 'accepted_work',
      providerEligibilityRefs: [
        'eligibility.public.provider.capability_snapshot_ok',
      ],
      providerRefs: ['provider.public.pylon_demo_runner'],
      pylonReceiptRefs: ['receipt.public.pylon.assignment.gepa_autopilot_001'],
      resourceMode: 'overnight_full',
      resultEvidenceRefs: ['result.public.gepa_autopilot_eval_summary'],
      state: 'accepted',
      treasuryReceiptRefs: [
        'receipt.public.treasury.reward_intent.gepa_autopilot_001',
      ],
      updatedAtIso: '2026-06-07T01:12:00.000Z',
    },
    {
      acceptanceCriteriaRefs: [
        'acceptance.public.external_agent_inference_summary',
      ],
      acceptedWorkRefs: [],
      artifactEvidenceRefs: [],
      assignmentAuthorityRefs: [],
      assignmentRef: 'assignment.public.pylon.external_agent_inference_001',
      blockerRefs: [],
      caveatRefs: ['caveat.public.external_jobs_policy_gated'],
      intakeRef: 'intake.public.external_agent.inference_001',
      jobRef: 'job.public.pylon.external_agent_inference_001',
      nexusReceiptRefs: [],
      payoutCaveatRefs: ['caveat.public.no_payout_before_acceptance_receipts'],
      payoutState: 'planned',
      providerEligibilityRefs: [],
      providerRefs: [],
      pylonReceiptRefs: [],
      resourceMode: 'balanced',
      resultEvidenceRefs: [],
      state: 'held_for_authority',
      treasuryReceiptRefs: [],
      updatedAtIso: '2026-06-07T01:14:00.000Z',
    },
  ],
  authority: PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
  caveatRefs: [
    'caveat.public.openagents_seeded_first',
    'caveat.public.external_jobs_policy_gated',
  ],
  intakeRecords: [
    intake({
      intakeRef: 'intake.public.openagents.gepa_autopilot_001',
      jobKind: 'gepa_dspy_optimization',
      jobRef: 'job.public.pylon.gepa_autopilot_001',
      privacyClass: 'public',
      requesterRef: 'requester.public.openagents',
      resourceModePreference: 'overnight_full',
      source: 'openagents_seeded',
      state: 'assignment_proposed',
    }),
    intake({
      intakeRef: 'intake.public.external_agent.inference_001',
      jobKind: 'inference',
      jobRef: 'job.public.pylon.external_agent_inference_001',
      policyGateRefs: ['policy.public.marketplace.external_agent_review'],
      privacyClass: 'customer_private',
      requesterRef: 'requester.private.external_agent_redacted',
      resourceModePreference: 'balanced',
      source: 'external_agent',
      state: 'policy_gated',
    }),
  ],
  ledgerRef: 'ledger.public.artanis.pylon_marketplace_jobs',
  sourceRefs: [
    'docs/artanis/2026-06-06-work-routing-contract.md',
    'docs/artanis/2026-06-06-pylon-resource-mode-setup.md',
    'docs/pylon/2026-06-06-pylon-provider-settlement-bridge.md',
  ],
  updatedAtIso: '2026-06-07T01:15:00.000Z',
})
