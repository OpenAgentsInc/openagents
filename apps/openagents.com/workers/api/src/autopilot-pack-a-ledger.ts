import { Match as M, Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const PackALedgerArtifactKind = S.Literals([
  'build_test_log_summary',
  'closeout',
  'decision_action',
  'notification_delivery',
  'redaction_scan',
  'schedule_occurrence',
  'screenshot_companion_evidence',
  'smoke_result',
  'task_output',
  'verification_result',
])
export type PackALedgerArtifactKind = typeof PackALedgerArtifactKind.Type

export const PackALedgerReceiptKind = S.Literals([
  'acceptance_recorded',
  'admission_recorded',
  'blocker_recorded',
  'continuation_queued',
  'decision_requested',
  'decision_resolved',
  'delivery_recorded',
  'notification_delivered',
  'notification_enqueued',
  'notification_failed',
  'review_recorded',
  'schedule_fired',
  'schedule_skipped',
  'settlement_recorded',
  'smoke_failed',
  'smoke_passed',
  'task_cancelled',
  'task_completed',
  'task_failed',
  'usage_budget_stop',
  'usage_threshold_crossed',
  'verification_failed',
  'verification_passed',
])
export type PackALedgerReceiptKind = typeof PackALedgerReceiptKind.Type

export const PackALedgerVisibility = S.Literals([
  'operator',
  'private',
  'public',
  'team',
])
export type PackALedgerVisibility = typeof PackALedgerVisibility.Type

export const PackALedgerRedactionClass = S.Literals([
  'operator_summary',
  'private_ref',
  'public_ref',
  'redacted_summary',
  'team_summary',
])
export type PackALedgerRedactionClass = typeof PackALedgerRedactionClass.Type

export const PackALedgerRetentionPolicy = S.Literals([
  'audit_retained',
  'ephemeral',
  'proof_retained',
  'team_retained',
])
export type PackALedgerRetentionPolicy = typeof PackALedgerRetentionPolicy.Type

export const PackALedgerClaimKind = S.Literals([
  'm9_rate_limit_rotation',
  'm10_overnight_unattended',
  'm14_exit_gate',
  'p4_settlement_bridge',
  'p9_settlement_visibility',
])
export type PackALedgerClaimKind = typeof PackALedgerClaimKind.Type

export class PackALedgerUnsafe extends S.TaggedErrorClass<PackALedgerUnsafe>()(
  'PackALedgerUnsafe',
  {
    reason: S.String,
  },
) {}

export class PackAArtifactRecord extends S.Class<PackAArtifactRecord>(
  'PackAArtifactRecord',
)({
  artifactRef: S.String,
  createdAt: S.String,
  digest: S.String,
  kind: PackALedgerArtifactKind,
  mediaType: S.String,
  payloadRef: S.String,
  producerAdapter: S.String,
  redactionClass: PackALedgerRedactionClass,
  relatedReceiptRefs: S.Array(S.String),
  retentionPolicy: PackALedgerRetentionPolicy,
  sizeBytes: S.Number,
  visibility: PackALedgerVisibility,
}) {}

export class PackAReceiptRecord extends S.Class<PackAReceiptRecord>(
  'PackAReceiptRecord',
)({
  artifactRefs: S.Array(S.String),
  createdAt: S.String,
  idempotencyKey: S.String,
  kind: PackALedgerReceiptKind,
  previousReceiptRefs: S.Array(S.String),
  receiptRef: S.String,
  subjectRef: S.String,
}) {}

export class PackALedgerProjection extends S.Class<PackALedgerProjection>(
  'PackALedgerProjection',
)({
  artifactRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  generatedAt: S.String,
  projectionRef: S.Literal('openagents.autopilot_pack_a_ledger.v1'),
  receiptRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  visibility: PackALedgerVisibility,
}) {}

export class PackAClaimRequirementResult extends S.Class<PackAClaimRequirementResult>(
  'PackAClaimRequirementResult',
)({
  claimKind: PackALedgerClaimKind,
  caveatRefs: S.Array(S.String),
  missingReceiptKinds: S.Array(PackALedgerReceiptKind),
  ready: S.Boolean,
  requiredReceiptKinds: S.Array(PackALedgerReceiptKind),
}) {}

export const PackAUsageEventKind = S.Literals([
  'budget_created',
  'budget_progressed',
  'context_estimated',
  'cost_estimated',
  'max_output_escalated',
  'provider_response_usage',
  'quota_blocked',
  'rate_limit_observed',
  'snapshot_projected',
  'threshold_crossed',
  'unknown_pricing_observed',
])
export type PackAUsageEventKind = typeof PackAUsageEventKind.Type

export const PackABudgetDecisionKind = S.Literals([
  'ask',
  'compact',
  'continue',
  'continue_with_caveat',
  'pause',
  'retry',
  'stop',
  'warn',
])
export type PackABudgetDecisionKind = typeof PackABudgetDecisionKind.Type

export const PackAPricingState = S.Literals(['known', 'unknown'])
export type PackAPricingState = typeof PackAPricingState.Type

export const PackAPaymentState = S.Literals([
  'buyer_credit_debited',
  'free_own_pylon',
  'l402_paid',
  'not_charged',
  'payment_required',
])
export type PackAPaymentState = typeof PackAPaymentState.Type

export class PackAUsageEventRecord extends S.Class<PackAUsageEventRecord>(
  'PackAUsageEventRecord',
)({
  budgetRef: S.String,
  cacheReadTokens: S.Number,
  cacheWriteTokens: S.Number,
  contextEstimateTokens: S.Number,
  costMicros: S.NullOr(S.Number),
  createdAt: S.String,
  currency: S.NullOr(S.String),
  eventRef: S.String,
  externalAdapterSpendMicros: S.NullOr(S.Number),
  inputTokens: S.Number,
  kind: PackAUsageEventKind,
  maxOutputReservationTokens: S.Number,
  outputTokens: S.Number,
  paymentState: PackAPaymentState,
  pricingState: PackAPricingState,
  processRuntimeMs: S.Number,
  providerRef: S.String,
  rateLimitResetRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  scheduleRef: S.NullOr(S.String),
  taskRef: S.String,
  toolCallTokens: S.Number,
  userRequestedTokenTarget: S.NullOr(S.Number),
  wallClockMs: S.Number,
}) {}

export class PackABudgetPolicy extends S.Class<PackABudgetPolicy>(
  'PackABudgetPolicy',
)({
  askAtPercent: S.Number,
  budgetRef: S.String,
  compactAtContextPercent: S.Number,
  hardStopAtPercent: S.Number,
  maxCostMicros: S.Number,
  maxRetryCount: S.Number,
  pauseAtPercent: S.Number,
  warnAtPercent: S.Number,
}) {}

export class PackABudgetDecision extends S.Class<PackABudgetDecision>(
  'PackABudgetDecision',
)({
  blockerRefs: S.Array(S.String),
  decision: PackABudgetDecisionKind,
  receiptKind: S.NullOr(PackALedgerReceiptKind),
  receiptRef: S.NullOr(S.String),
  reasonRef: S.String,
}) {}

export class PackAUsageProjection extends S.Class<PackAUsageProjection>(
  'PackAUsageProjection',
)({
  budgetRef: S.String,
  caveatRefs: S.Array(S.String),
  costMicros: S.NullOr(S.Number),
  currency: S.NullOr(S.String),
  generatedAt: S.String,
  paymentState: PackAPaymentState,
  pricingState: PackAPricingState,
  projectionRef: S.Literal('openagents.autopilot_pack_a_usage.v1'),
  receiptRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  taskRef: S.String,
  totalTokens: S.Number,
}) {}

export class TeamBudgetRecord extends S.Class<TeamBudgetRecord>(
  'TeamBudgetRecord',
)({
  budgetRef: S.String,
  currency: S.String,
  maxMicros: S.Number,
  perMissionCapMicros: S.Number,
  periodRef: S.String,
  teamRef: S.String,
}) {}

export class TeamSpendRecord extends S.Class<TeamSpendRecord>(
  'TeamSpendRecord',
)({
  amountMicros: S.Number,
  artifactRefs: S.Array(S.String),
  ledgerEntryRef: S.String,
  missionRef: S.String,
  receiptRefs: S.Array(S.String),
  spendRef: S.String,
  teamRef: S.String,
}) {}

export class TeamSpendEvidenceJoin extends S.Class<TeamSpendEvidenceJoin>(
  'TeamSpendEvidenceJoin',
)({
  artifactRefs: S.Array(S.String),
  blockedReasonRefs: S.Array(S.String),
  budgetRef: S.String,
  generatedAt: S.String,
  ledgerEntryRefs: S.Array(S.String),
  missionRef: S.String,
  receiptRefs: S.Array(S.String),
  spendAllowed: S.Boolean,
  staleness: PublicProjectionStalenessContract,
  teamRef: S.String,
  totalSpendMicros: S.Number,
}) {}

export const SettlementLedgerStage = S.Literals([
  'accepted_work',
  'buyer_credit_debit',
  'conversion',
  'escrow_hold',
  'settlement',
])
export type SettlementLedgerStage = typeof SettlementLedgerStage.Type

export class SettlementLedgerReceipt extends S.Class<SettlementLedgerReceipt>(
  'SettlementLedgerReceipt',
)({
  amountMinorUnits: S.Number,
  asset: S.Literals(['btc', 'sats', 'usd']),
  conversionRef: S.NullOr(S.String),
  createdAt: S.String,
  duplicateKey: S.String,
  fromReceiptRefs: S.Array(S.String),
  payoutEligible: S.Boolean,
  providerRef: S.String,
  receiptRef: S.String,
  settlementAuthority: S.Literals([
    'accepted_work_only',
    'conversion_only',
    'escrow_only',
    'payment_only',
    'settled_bitcoin',
  ]),
  spendCapRef: S.NullOr(S.String),
  stage: SettlementLedgerStage,
  workOrderRef: S.String,
}) {}

export class SettlementChainProjection extends S.Class<SettlementChainProjection>(
  'SettlementChainProjection',
)({
  caveatRefs: S.Array(S.String),
  duplicateSafe: S.Boolean,
  generatedAt: S.String,
  payoutEligible: S.Boolean,
  projectionRef: S.Literal('openagents.autopilot_settlement_chain.v1'),
  receiptRefs: S.Array(S.String),
  settledBitcoinClaimAllowed: S.Boolean,
  staleness: PublicProjectionStalenessContract,
  workOrderRef: S.String,
}) {}

export const PayoutVisibilityRung = S.Literals([
  'conversion',
  'credited_and_swept',
  'direct_settlement',
  'escrow_hold',
])
export type PayoutVisibilityRung = typeof PayoutVisibilityRung.Type

export const PayoutVisibilitySurface = S.Literals([
  'auditor_aggregate',
  'public_receipt',
  'recipient_view',
])
export type PayoutVisibilitySurface = typeof PayoutVisibilitySurface.Type

export class PayoutVisibilityCell extends S.Class<PayoutVisibilityCell>(
  'PayoutVisibilityCell',
)({
  generatedAt: S.String,
  reasonRef: S.String,
  receiptRef: S.NullOr(S.String),
  rung: PayoutVisibilityRung,
  state: S.Literals(['green', 'typed_absent']),
  staleness: PublicProjectionStalenessContract,
  surface: PayoutVisibilitySurface,
}) {}

export class PayoutVisibilityMatrix extends S.Class<PayoutVisibilityMatrix>(
  'PayoutVisibilityMatrix',
)({
  cells: S.Array(PayoutVisibilityCell),
  generatedAt: S.String,
  matrixRef: S.Literal('openagents.autopilot_payout_visibility_matrix.v1'),
  releaseGateOpen: S.Boolean,
  staleness: PublicProjectionStalenessContract,
}) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

const failUnsafe = (reason: string): never => {
  const error = new PackALedgerUnsafe({ reason })

  throw error
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafeMaterialPattern.test(ref),
  )

  if (unsafe !== undefined) {
    failUnsafe(
      `${label} contains private, secret, provider, wallet, payment, local-path, or raw payload material.`,
    )
  }
}

export const assertPackAArtifactPublicSafe = (
  artifact: PackAArtifactRecord,
): PackAArtifactRecord => {
  assertSafeRefs('artifact refs', [
    artifact.artifactRef,
    artifact.digest,
    artifact.payloadRef,
    artifact.producerAdapter,
    ...artifact.relatedReceiptRefs,
  ])

  if (
    artifact.visibility === 'public' &&
    artifact.redactionClass === 'private_ref'
  ) {
    failUnsafe('public artifacts cannot carry private redaction refs.')
  }

  return artifact
}

export const appendPackAReceiptIdempotent = (
  receipts: ReadonlyArray<PackAReceiptRecord>,
  receipt: PackAReceiptRecord,
): Readonly<{
  inserted: boolean
  receipts: ReadonlyArray<PackAReceiptRecord>
}> => {
  assertSafeRefs('receipt refs', [
    receipt.receiptRef,
    receipt.idempotencyKey,
    receipt.subjectRef,
    ...receipt.artifactRefs,
    ...receipt.previousReceiptRefs,
  ])

  const existing = receipts.find(
    candidate => candidate.idempotencyKey === receipt.idempotencyKey,
  )

  if (existing !== undefined) {
    return { inserted: false, receipts }
  }

  return { inserted: true, receipts: [...receipts, receipt] }
}

export const projectPackALedger = (
  input: Readonly<{
    artifacts: ReadonlyArray<PackAArtifactRecord>
    generatedAt: string
    receipts: ReadonlyArray<PackAReceiptRecord>
    visibility: PackALedgerVisibility
  }>,
): PackALedgerProjection => {
  input.artifacts.forEach(assertPackAArtifactPublicSafe)
  input.receipts.forEach(receipt => {
    assertSafeRefs('receipt projection refs', [
      receipt.receiptRef,
      receipt.subjectRef,
      ...receipt.artifactRefs,
    ])
  })

  const visibleArtifacts = input.artifacts.filter(
    artifact =>
      artifact.visibility === input.visibility ||
      (input.visibility === 'public' && artifact.visibility === 'public') ||
      (input.visibility === 'team' &&
        (artifact.visibility === 'team' || artifact.visibility === 'public')) ||
      input.visibility === 'operator',
  )
  const narrowedCount = input.artifacts.length - visibleArtifacts.length

  return new PackALedgerProjection({
    artifactRefs: uniqueRefs(
      visibleArtifacts.map(artifact => artifact.artifactRef),
    ),
    caveatRefs:
      narrowedCount > 0 ? ['caveat.pack_a_ledger.visibility_narrowed'] : [],
    generatedAt: input.generatedAt,
    projectionRef: 'openagents.autopilot_pack_a_ledger.v1',
    receiptRefs: uniqueRefs(input.receipts.map(receipt => receipt.receiptRef)),
    staleness: liveAtReadStaleness([
      'pack_a_artifact_recorded',
      'pack_a_receipt_appended',
      'pack_a_visibility_policy_changed',
    ]),
    visibility: input.visibility,
  })
}

const requiredReceiptsByClaim: Readonly<
  Record<PackALedgerClaimKind, ReadonlyArray<PackALedgerReceiptKind>>
> = {
  m10_overnight_unattended: [
    'schedule_fired',
    'task_completed',
    'notification_delivered',
    'review_recorded',
    'verification_passed',
    'delivery_recorded',
  ],
  m14_exit_gate: [
    'smoke_passed',
    'review_recorded',
    'acceptance_recorded',
    'usage_budget_stop',
  ],
  m9_rate_limit_rotation: [
    'smoke_passed',
    'verification_passed',
    'usage_threshold_crossed',
  ],
  p4_settlement_bridge: [
    'delivery_recorded',
    'acceptance_recorded',
    'settlement_recorded',
  ],
  p9_settlement_visibility: ['settlement_recorded'],
}

export const checkPackAClaimRequirements = (
  claimKind: PackALedgerClaimKind,
  receipts: ReadonlyArray<PackAReceiptRecord>,
): PackAClaimRequirementResult => {
  const present = new Set(receipts.map(receipt => receipt.kind))
  const requiredReceiptKinds = requiredReceiptsByClaim[claimKind]
  const missingReceiptKinds = requiredReceiptKinds.filter(
    kind => !present.has(kind),
  )

  return new PackAClaimRequirementResult({
    claimKind,
    caveatRefs:
      missingReceiptKinds.length > 0
        ? [`caveat.${claimKind}.missing_required_receipts`]
        : [],
    missingReceiptKinds,
    ready: missingReceiptKinds.length === 0,
    requiredReceiptKinds: [...requiredReceiptKinds],
  })
}

export const decidePackABudget = (
  input: Readonly<{
    contextWindowTokens: number
    retryCount: number
    usage: PackAUsageEventRecord
    policy: PackABudgetPolicy
  }>,
): PackABudgetDecision => {
  const costPercent =
    input.usage.costMicros === null
      ? null
      : (input.usage.costMicros / input.policy.maxCostMicros) * 100
  const contextPercent =
    (input.usage.contextEstimateTokens / input.contextWindowTokens) * 100

  if (input.usage.kind === 'quota_blocked') {
    return new PackABudgetDecision({
      blockerRefs: ['blocker.pack_a_usage.quota_blocked'],
      decision: 'stop',
      receiptKind: 'usage_budget_stop',
      receiptRef: `receipt.${input.usage.eventRef}.quota_blocked`,
      reasonRef: 'budget.quota_blocked',
    })
  }

  if (input.usage.kind === 'rate_limit_observed') {
    return new PackABudgetDecision({
      blockerRefs:
        input.retryCount >= input.policy.maxRetryCount
          ? ['blocker.pack_a_usage.retry_budget_exhausted']
          : [],
      decision:
        input.retryCount >= input.policy.maxRetryCount ? 'stop' : 'retry',
      receiptKind:
        input.retryCount >= input.policy.maxRetryCount
          ? 'usage_budget_stop'
          : 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.rate_limit`,
      reasonRef:
        input.retryCount >= input.policy.maxRetryCount
          ? 'budget.retry_budget_exhausted'
          : 'budget.rate_limit_retry_allowed',
    })
  }

  if (input.usage.pricingState === 'unknown') {
    return new PackABudgetDecision({
      blockerRefs: [],
      decision: 'continue_with_caveat',
      receiptKind: 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.unknown_pricing`,
      reasonRef: 'budget.pricing_unknown_not_zero',
    })
  }

  if (costPercent !== null && costPercent >= input.policy.hardStopAtPercent) {
    return new PackABudgetDecision({
      blockerRefs: ['blocker.pack_a_usage.hard_stop'],
      decision: 'stop',
      receiptKind: 'usage_budget_stop',
      receiptRef: `receipt.${input.usage.eventRef}.hard_stop`,
      reasonRef: 'budget.hard_stop_reached',
    })
  }

  if (contextPercent >= input.policy.compactAtContextPercent) {
    return new PackABudgetDecision({
      blockerRefs: [],
      decision: 'compact',
      receiptKind: 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.context_compact`,
      reasonRef: 'budget.context_threshold_crossed',
    })
  }

  if (costPercent !== null && costPercent >= input.policy.pauseAtPercent) {
    return new PackABudgetDecision({
      blockerRefs: [],
      decision: 'pause',
      receiptKind: 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.pause`,
      reasonRef: 'budget.pause_threshold_crossed',
    })
  }

  if (costPercent !== null && costPercent >= input.policy.askAtPercent) {
    return new PackABudgetDecision({
      blockerRefs: [],
      decision: 'ask',
      receiptKind: 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.ask`,
      reasonRef: 'budget.ask_threshold_crossed',
    })
  }

  if (costPercent !== null && costPercent >= input.policy.warnAtPercent) {
    return new PackABudgetDecision({
      blockerRefs: [],
      decision: 'warn',
      receiptKind: 'usage_threshold_crossed',
      receiptRef: `receipt.${input.usage.eventRef}.warn`,
      reasonRef: 'budget.warn_threshold_crossed',
    })
  }

  return new PackABudgetDecision({
    blockerRefs: [],
    decision: 'continue',
    receiptKind: null,
    receiptRef: null,
    reasonRef: 'budget.within_policy',
  })
}

export const projectPackAUsage = (
  usage: PackAUsageEventRecord,
  generatedAt: string,
): PackAUsageProjection => {
  assertSafeRefs('usage refs', [
    usage.budgetRef,
    usage.eventRef,
    usage.providerRef,
    usage.rateLimitResetRef ?? '',
    usage.scheduleRef ?? '',
    usage.taskRef,
    ...usage.receiptRefs,
  ])

  const totalTokens =
    usage.inputTokens +
    usage.outputTokens +
    usage.cacheReadTokens +
    usage.cacheWriteTokens +
    usage.toolCallTokens

  return new PackAUsageProjection({
    budgetRef: usage.budgetRef,
    caveatRefs:
      usage.pricingState === 'unknown'
        ? ['caveat.pack_a_usage.unknown_pricing_not_zero']
        : [],
    costMicros: usage.pricingState === 'unknown' ? null : usage.costMicros,
    currency: usage.pricingState === 'unknown' ? null : usage.currency,
    generatedAt,
    paymentState: usage.paymentState,
    pricingState: usage.pricingState,
    projectionRef: 'openagents.autopilot_pack_a_usage.v1',
    receiptRefs: uniqueRefs(usage.receiptRefs),
    staleness: liveAtReadStaleness([
      'pack_a_usage_event_recorded',
      'pack_a_budget_policy_changed',
      'pack_a_cost_projection_rebuilt',
    ]),
    taskRef: usage.taskRef,
    totalTokens,
  })
}

export const joinTeamSpendEvidence = (
  input: Readonly<{
    budget: TeamBudgetRecord
    generatedAt: string
    missionRef: string
    proposedSpendMicros: number
    spendRecords: ReadonlyArray<TeamSpendRecord>
  }>,
): TeamSpendEvidenceJoin => {
  const missionSpends = input.spendRecords.filter(
    spend =>
      spend.teamRef === input.budget.teamRef &&
      spend.missionRef === input.missionRef,
  )
  const totalSpendMicros = missionSpends.reduce(
    (sum, spend) => sum + spend.amountMicros,
    0,
  )
  const overMissionCap =
    totalSpendMicros + input.proposedSpendMicros >
    input.budget.perMissionCapMicros
  const overTeamBudget =
    input.spendRecords
      .filter(spend => spend.teamRef === input.budget.teamRef)
      .reduce((sum, spend) => sum + spend.amountMicros, 0) +
      input.proposedSpendMicros >
    input.budget.maxMicros

  return new TeamSpendEvidenceJoin({
    artifactRefs: uniqueRefs(
      missionSpends.flatMap(spend => spend.artifactRefs),
    ),
    blockedReasonRefs: [
      ...(overMissionCap ? ['blocker.team_budget.per_mission_cap'] : []),
      ...(overTeamBudget ? ['blocker.team_budget.team_period_cap'] : []),
    ],
    budgetRef: input.budget.budgetRef,
    generatedAt: input.generatedAt,
    ledgerEntryRefs: uniqueRefs(
      missionSpends.map(spend => spend.ledgerEntryRef),
    ),
    missionRef: input.missionRef,
    receiptRefs: uniqueRefs(missionSpends.flatMap(spend => spend.receiptRefs)),
    spendAllowed: !overMissionCap && !overTeamBudget,
    staleness: liveAtReadStaleness([
      'team_budget_changed',
      'team_spend_recorded',
      'mission_artifact_recorded',
    ]),
    teamRef: input.budget.teamRef,
    totalSpendMicros,
  })
}

export const projectSettlementChain = (
  input: Readonly<{
    generatedAt: string
    receipts: ReadonlyArray<SettlementLedgerReceipt>
    workOrderRef: string
  }>,
): SettlementChainProjection => {
  input.receipts.forEach(receipt => {
    assertSafeRefs('settlement refs', [
      receipt.conversionRef ?? '',
      receipt.duplicateKey,
      receipt.providerRef,
      receipt.receiptRef,
      receipt.spendCapRef ?? '',
      receipt.workOrderRef,
      ...receipt.fromReceiptRefs,
    ])
  })

  const stageSet = new Set(input.receipts.map(receipt => receipt.stage))
  const duplicateSafe =
    new Set(input.receipts.map(receipt => receipt.duplicateKey)).size ===
    input.receipts.length
  const hasSettlement = input.receipts.some(
    receipt => receipt.settlementAuthority === 'settled_bitcoin',
  )
  const hasUsdAsBitcoin = input.receipts.some(
    receipt =>
      receipt.asset === 'usd' &&
      receipt.settlementAuthority === 'settled_bitcoin',
  )
  const payoutEligible =
    stageSet.has('buyer_credit_debit') &&
    stageSet.has('conversion') &&
    stageSet.has('escrow_hold') &&
    stageSet.has('accepted_work')

  return new SettlementChainProjection({
    caveatRefs: [
      ...(!duplicateSafe ? ['caveat.settlement.duplicate_key_detected'] : []),
      ...(hasUsdAsBitcoin ? ['caveat.settlement.usd_not_bitcoin'] : []),
      ...(!hasSettlement ? ['caveat.settlement.not_settled'] : []),
    ],
    duplicateSafe,
    generatedAt: input.generatedAt,
    payoutEligible,
    projectionRef: 'openagents.autopilot_settlement_chain.v1',
    receiptRefs: uniqueRefs(input.receipts.map(receipt => receipt.receiptRef)),
    settledBitcoinClaimAllowed:
      duplicateSafe && !hasUsdAsBitcoin && payoutEligible && hasSettlement,
    staleness: liveAtReadStaleness([
      'buyer_credit_debited',
      'settlement_conversion_recorded',
      'labor_escrow_held',
      'accepted_work_recorded',
      'settlement_recorded',
    ]),
    workOrderRef: input.workOrderRef,
  })
}

export const payoutVisibilityMatrix = (
  input: Readonly<{
    cells: ReadonlyArray<PayoutVisibilityCell>
    generatedAt: string
  }>,
): PayoutVisibilityMatrix => {
  input.cells.forEach(cell => {
    assertSafeRefs('payout visibility refs', [
      cell.reasonRef,
      cell.receiptRef ?? '',
    ])
  })

  const requiredPairs = PayoutVisibilityRung.literals.flatMap(rung =>
    PayoutVisibilitySurface.literals.map(surface => `${rung}:${surface}`),
  )
  const greenPairs = new Set(
    input.cells
      .filter(cell => cell.state === 'green')
      .map(cell => `${cell.rung}:${cell.surface}`),
  )
  const releaseGateOpen = requiredPairs.every(pair => greenPairs.has(pair))

  return new PayoutVisibilityMatrix({
    cells: [...input.cells],
    generatedAt: input.generatedAt,
    matrixRef: 'openagents.autopilot_payout_visibility_matrix.v1',
    releaseGateOpen,
    staleness: liveAtReadStaleness([
      'settlement_visibility_cell_recorded',
      'payout_receipt_published',
      'recipient_visibility_projection_rebuilt',
    ]),
  })
}

export const settlementStageAuthorityLabel = (
  stage: SettlementLedgerStage,
): string =>
  M.value(stage).pipe(
    M.withReturnType<string>(),
    M.when('buyer_credit_debit', () => 'USD credit debit only'),
    M.when('conversion', () => 'Conversion ref only'),
    M.when('escrow_hold', () => 'Held escrow only'),
    M.when('accepted_work', () => 'Accepted-work eligibility only'),
    M.when('settlement', () => 'Settled bitcoin only with settlement receipt'),
    M.exhaustive,
  )
