import { Schema as S } from 'effect'

import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAmount,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
  OpenAgentsPylonSettlementBridgeRecord,
} from './pylon-settlement-bridge'
import {
  PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY,
  PylonAcceptedWorkPayoutRowRecord,
} from './pylon-accepted-work-payout-rows'
import {
  PylonMarketplaceAssignmentRecord,
} from './pylon-marketplace-jobs'
import {
  buildTreasuryPaymentSimulationReceipts,
} from './treasury-payment-simulation-adapter'

export class PylonMarketplaceAcceptedWorkPayoutEvidence extends S.Class<PylonMarketplaceAcceptedWorkPayoutEvidence>(
  'PylonMarketplaceAcceptedWorkPayoutEvidence',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  acceptedWorkRefs: S.Array(S.String),
  artifactEvidenceRefs: S.Array(S.String),
  assignmentRef: S.String,
  evidenceRefs: S.Array(S.String),
  jobRef: S.String,
  nexusReceiptRefs: S.Array(S.String),
  payoutCaveatRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  pylonReceiptRefs: S.Array(S.String),
  resultEvidenceRefs: S.Array(S.String),
  treasuryReceiptRefs: S.Array(S.String),
}) {}

export class PylonMarketplacePayoutFlowUnsafe extends S.TaggedErrorClass<PylonMarketplacePayoutFlowUnsafe>()(
  'PylonMarketplacePayoutFlowUnsafe',
  {
    reason: S.String,
  },
) {}

export type PylonMarketplacePayoutFlowRefs = Readonly<{
  artanisDispatchRef: string
  buyerPaymentEvidenceRef: string
  idempotencyRef: string
  ownerUserId: string | null
  payoutTargetApprovalRef: string
  payoutTargetRef: string
  policySnapshotRef: string
  providerRef: string
}>

export type PylonMarketplacePayoutFlowAmounts = Readonly<{
  amount: NexusTreasuryPayoutAmount
  spendCap: NexusTreasuryPayoutAmount
}>

export type PylonMarketplacePayoutFlowRecords = Readonly<{
  acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
  attempt: NexusTreasuryPayoutAttemptRecord
  bridgeTimeline: ReadonlyArray<OpenAgentsPylonSettlementBridgeRecord>
  intent: NexusTreasuryPayoutIntentRecord
  intentCreatedReceipt: NexusPaymentAuthorityReceiptRecord
  payoutRow: PylonAcceptedWorkPayoutRowRecord
  reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
  simulationReceipts: ReadonlyArray<NexusPaymentAuthorityReceiptRecord>
}>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafePayoutFlowRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(archive|customer|dataset|key|model|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref =>
    !safeRefPattern.test(ref) ||
    unsafePayoutFlowRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: `${label} contains private, secret, raw bitcoin payment, wallet, provider, customer, runner, or timestamp material.`,
    })
  }

  return normalized
}

const requiredRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = safeRefs(label, refs)

  if (normalized.length === 0) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: `${label} is required before payout intent creation.`,
    })
  }

  return normalized
}

const stableSuffix = (value: string): string =>
  value
    .trim()
    .replace(/[^A-Za-z0-9_.:/-]/g, '_')
    .replace(/_{2,}/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 120) || 'pylon_marketplace'

const safeJson = (value: unknown): string => JSON.stringify(value)

const assertIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertAmount = (
  label: string,
  amount: NexusTreasuryPayoutAmount,
): void => {
  if (!Number.isInteger(amount.amountMinorUnits) || amount.amountMinorUnits <= 0) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: `${label} must be a positive integer amount.`,
    })
  }

  const expectedDenomination =
    amount.asset === 'bitcoin'
      ? 'bitcoin_millisatoshi'
      : amount.asset === 'usd'
        ? 'usd_cent'
        : 'credit'

  if (amount.denomination !== expectedDenomination) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: `${label} denomination must match its asset.`,
    })
  }
}

export const acceptedWorkPayoutEvidenceFromMarketplaceAssignment = (
  assignment: PylonMarketplaceAssignmentRecord,
): PylonMarketplaceAcceptedWorkPayoutEvidence => {
  if (assignment.state !== 'accepted') {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: 'Pylon marketplace payout intent creation requires an accepted assignment.',
    })
  }

  if (assignment.payoutState !== 'accepted_work') {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: 'Pylon marketplace payout intent creation starts from accepted_work payout state.',
    })
  }

  return new PylonMarketplaceAcceptedWorkPayoutEvidence({
    acceptanceCriteriaRefs: requiredRefs(
      'acceptance criteria refs',
      assignment.acceptanceCriteriaRefs,
    ),
    acceptedWorkRefs: requiredRefs(
      'accepted-work refs',
      assignment.acceptedWorkRefs,
    ),
    artifactEvidenceRefs: requiredRefs(
      'artifact evidence refs',
      assignment.artifactEvidenceRefs,
    ),
    assignmentRef: requiredRefs('assignment ref', [assignment.assignmentRef])[0]!,
    evidenceRefs: requiredRefs('result evidence refs', [
      ...assignment.artifactEvidenceRefs,
      ...assignment.resultEvidenceRefs,
    ]),
    jobRef: requiredRefs('job ref', [assignment.jobRef])[0]!,
    nexusReceiptRefs: requiredRefs('Nexus receipt refs', assignment.nexusReceiptRefs),
    payoutCaveatRefs: requiredRefs(
      'payout caveat refs',
      assignment.payoutCaveatRefs,
    ),
    providerRefs: requiredRefs('provider refs', assignment.providerRefs),
    pylonReceiptRefs: requiredRefs('Pylon receipt refs', assignment.pylonReceiptRefs),
    resultEvidenceRefs: requiredRefs(
      'result evidence refs',
      assignment.resultEvidenceRefs,
    ),
    treasuryReceiptRefs: requiredRefs(
      'Treasury receipt refs',
      assignment.treasuryReceiptRefs,
    ),
  })
}

export const buildPylonMarketplacePayoutIntent = (
  input: Readonly<{
    acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
    amounts: PylonMarketplacePayoutFlowAmounts
    nowIso: string
    refs: PylonMarketplacePayoutFlowRefs
  }>,
): NexusTreasuryPayoutIntentRecord => {
  assertIso('payout intent timestamp', input.nowIso)
  assertAmount('payout amount', input.amounts.amount)
  assertAmount('payout spend cap', input.amounts.spendCap)

  const refValues = safeRefs('payout flow refs', [
    input.refs.artanisDispatchRef,
    input.refs.buyerPaymentEvidenceRef,
    input.refs.idempotencyRef,
    input.refs.payoutTargetApprovalRef,
    input.refs.payoutTargetRef,
    input.refs.policySnapshotRef,
    input.refs.providerRef,
  ])
  const suffix = stableSuffix(input.refs.idempotencyRef)
  const acceptedWorkRefs = requiredRefs(
    'accepted-work refs',
    input.acceptedWork.acceptedWorkRefs,
  )

  return {
    acceptedWorkRefs,
    actorRef: 'agent.artanis',
    adapterKind: 'simulation',
    amount: input.amounts.amount,
    archivedAt: null,
    artanisDispatchRef: input.refs.artanisDispatchRef,
    assignmentRef: input.acceptedWork.assignmentRef,
    buyerPaymentRef: input.refs.buyerPaymentEvidenceRef,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_intent_${suffix}`,
    idempotencyKeyHash: `hash.pylon_marketplace.intent.${suffix}`,
    metadataRefs: uniqueRefs([
      ...refValues,
      ...input.acceptedWork.evidenceRefs,
      'metadata.nexus.pylon_marketplace.accepted_work',
    ]),
    ownerUserId: input.refs.ownerUserId,
    payoutIntentRef: `payout_intent.pylon_marketplace.${suffix}`,
    payoutTargetApprovalRef: input.refs.payoutTargetApprovalRef,
    payoutTargetRef: input.refs.payoutTargetRef,
    policySnapshotRef: input.refs.policySnapshotRef,
    publicProjectionJson: safeJson({
      acceptedWork: true,
      adapter: 'simulation',
      moneyMovement: 'none',
      pylonMarketplaceJob: input.acceptedWork.jobRef,
    }),
    pylonJobRef: input.acceptedWork.jobRef,
    sourceKind: 'pylon_marketplace_assignment',
    spendCap: input.amounts.spendCap,
    status: 'approved',
    updatedAt: input.nowIso,
  }
}

export const buildPylonMarketplacePayoutAttempt = (
  input: Readonly<{
    intent: NexusTreasuryPayoutIntentRecord
    nowIso: string
    refs: PylonMarketplacePayoutFlowRefs
  }>,
): NexusTreasuryPayoutAttemptRecord => {
  assertIso('payout attempt timestamp', input.nowIso)
  const suffix = stableSuffix(input.refs.idempotencyRef)

  return {
    adapterAttemptRef: `adapter_attempt.simulation.pylon_marketplace.${suffix}`,
    adapterKind: input.intent.adapterKind,
    amount: input.intent.amount,
    archivedAt: null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_attempt_${suffix}`,
    idempotencyKeyHash: `hash.pylon_marketplace.attempt.${suffix}`,
    metadataRefs: [
      'metadata.nexus.pylon_marketplace.simulated_dispatch',
      input.intent.payoutIntentRef,
    ],
    payoutAttemptRef: `payout_attempt.pylon_marketplace.${suffix}`,
    payoutIntentRef: input.intent.payoutIntentRef,
    publicProjectionJson: safeJson({
      adapter: 'simulation',
      moneyMovement: 'none',
      pylonMarketplaceJob: input.intent.pylonJobRef,
    }),
    redactedDestinationRef: `destination.redacted.pylon_marketplace.${suffix}`,
    redactedPaymentRef: null,
    status: 'pending',
    updatedAt: input.nowIso,
  }
}

export const buildPylonMarketplacePayoutReconciliationEvent = (
  input: Readonly<{
    attempt: NexusTreasuryPayoutAttemptRecord
    intent: NexusTreasuryPayoutIntentRecord
    nowIso: string
    refs: PylonMarketplacePayoutFlowRefs
  }>,
): NexusTreasuryPayoutReconciliationEventRecord => {
  assertIso('payout reconciliation timestamp', input.nowIso)
  const suffix = stableSuffix(input.refs.idempotencyRef)

  return {
    adapterKind: input.intent.adapterKind,
    archivedAt: null,
    createdAt: input.nowIso,
    eventRef: `reconciliation.pylon_marketplace.${suffix}`,
    externalEventRef: `external_event.simulation.pylon_marketplace.${suffix}`,
    id: `nexus_treasury_reconciliation_${suffix}`,
    idempotencyKeyHash: `hash.pylon_marketplace.reconciliation.${suffix}`,
    metadataRefs: [
      'metadata.nexus.pylon_marketplace.simulated_reconciliation',
      input.attempt.adapterAttemptRef,
    ],
    payoutAttemptRef: input.attempt.payoutAttemptRef,
    payoutIntentRef: input.intent.payoutIntentRef,
    providerRef: 'provider.simulation',
    publicProjectionJson: safeJson({
      adapter: 'simulation',
      moneyMovement: 'none',
      pylonMarketplaceJob: input.intent.pylonJobRef,
    }),
    resultRef: `result.pylon_marketplace.simulation.${suffix}`,
    status: 'observed',
  }
}

export const buildPylonMarketplacePayoutIntentCreatedReceipt = (
  input: Readonly<{
    intent: NexusTreasuryPayoutIntentRecord
    nowIso: string
    refs: PylonMarketplacePayoutFlowRefs
  }>,
): NexusPaymentAuthorityReceiptRecord => {
  assertIso('intent receipt timestamp', input.nowIso)
  const suffix = stableSuffix(input.refs.idempotencyRef)

  return {
    archivedAt: null,
    audience: 'public',
    createdAt: input.nowIso,
    eventRef: null,
    id: `nexus_payment_authority_receipt_intent_${suffix}`,
    metadataRefs: [
      'metadata.nexus.pylon_marketplace.intent_created',
      input.intent.assignmentRef ?? input.intent.payoutIntentRef,
    ],
    payoutAttemptRef: null,
    payoutIntentRef: input.intent.payoutIntentRef,
    publicProjectionJson: safeJson({
      adapter: input.intent.adapterKind,
      moneyMovement: 'none',
      policyProofOnly: true,
      pylonMarketplaceJob: input.intent.pylonJobRef,
      state: 'intent_created',
    }),
    receiptKind: 'intent_created',
    receiptRef: `receipt.nexus.pylon_marketplace.intent.${suffix}`,
  }
}

const bridgeRecord = (
  input: Readonly<{
    acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
    attempt: NexusTreasuryPayoutAttemptRecord
    blockerRefs?: ReadonlyArray<string>
    createdAtIso: string
    evidenceRefs: ReadonlyArray<string>
    idSuffix: string
    intent: NexusTreasuryPayoutIntentRecord
    payoutConfirmationRefs?: ReadonlyArray<string>
    payoutDispatchRefs?: ReadonlyArray<string>
    payoutEligibilityRefs?: ReadonlyArray<string>
    payoutVerificationRefs?: ReadonlyArray<string>
    providerRef: string
    rewardIntentRefs?: ReadonlyArray<string>
    settlementRefs?: ReadonlyArray<string>
    state: OpenAgentsPylonSettlementBridgeRecord['state']
    updatedAtIso: string
  }>,
): OpenAgentsPylonSettlementBridgeRecord =>
  new OpenAgentsPylonSettlementBridgeRecord({
    acceptedWorkRefs: input.acceptedWork.acceptedWorkRefs,
    authority: OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_EVIDENCE_ONLY_AUTHORITY,
    blockerRefs: safeRefs('bridge blocker refs', input.blockerRefs ?? []),
    buyerPaymentEvidenceRefs: input.intent.buyerPaymentRef === null
      ? []
      : [input.intent.buyerPaymentRef],
    capabilitySnapshotRefs: [
      `capability_snapshot.pylon_marketplace.${input.idSuffix}`,
    ],
    caveatRefs: [
      'caveat.pylon.marketplace_bridge_evidence_only',
      ...input.acceptedWork.payoutCaveatRefs,
    ],
    createdAtIso: input.createdAtIso,
    evidenceRefs: uniqueRefs([
      ...input.evidenceRefs,
      input.intent.payoutIntentRef,
      input.attempt.adapterAttemptRef,
      input.intent.artanisDispatchRef ?? '',
    ]),
    id: `pylon_settlement_bridge.pylon_marketplace.${input.idSuffix}`,
    operatorDiagnosticRefs: [
      `diagnostic.operator.pylon_marketplace.${input.idSuffix}`,
    ],
    payoutConfirmationRefs: safeRefs(
      'bridge payout confirmation refs',
      input.payoutConfirmationRefs ?? [],
    ),
    payoutDispatchRefs: safeRefs(
      'bridge payout dispatch refs',
      input.payoutDispatchRefs ?? [],
    ),
    payoutEligibilityRefs: safeRefs(
      'bridge payout eligibility refs',
      input.payoutEligibilityRefs ?? [],
    ),
    payoutVerificationRefs: safeRefs(
      'bridge payout verification refs',
      input.payoutVerificationRefs ?? [],
    ),
    providerAssignmentRefs: [input.acceptedWork.assignmentRef],
    providerJobRefs: [input.acceptedWork.jobRef],
    providerRef: input.providerRef,
    providerVisibility: 'public',
    rewardIntentRefs: safeRefs(
      'bridge reward intent refs',
      input.rewardIntentRefs ?? [],
    ),
    settlementRefs: safeRefs('bridge settlement refs', input.settlementRefs ?? []),
    state: input.state,
    updatedAtIso: input.updatedAtIso,
    walletReadinessRefs: [
      `readiness_summary.pylon_marketplace.${input.idSuffix}`,
    ],
    walletReadinessState: 'receive_ready',
    workroomRefs: [`workroom.pylon_marketplace.${input.idSuffix}`],
  })

export const buildPylonMarketplaceSettlementBridgeTimeline = (
  input: Readonly<{
    acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
    attempt: NexusTreasuryPayoutAttemptRecord
    createdAtIso: string
    intent: NexusTreasuryPayoutIntentRecord
    providerRef: string
    reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
    receipts: ReadonlyArray<NexusPaymentAuthorityReceiptRecord>
    refs: PylonMarketplacePayoutFlowRefs
    updatedAtIso: string
  }>,
): ReadonlyArray<OpenAgentsPylonSettlementBridgeRecord> => {
  assertIso('settlement bridge created timestamp', input.createdAtIso)
  assertIso('settlement bridge updated timestamp', input.updatedAtIso)
  const suffix = stableSuffix(input.refs.idempotencyRef)
  const receiptRefForKind = (
    kind: NexusPaymentAuthorityReceiptRecord['receiptKind'],
  ): string | undefined =>
    input.receipts.find(receipt => receipt.receiptKind === kind)?.receiptRef

  const intentReceiptRef = receiptRefForKind('intent_created') ??
    `receipt.nexus.pylon_marketplace.intent.${suffix}`
  const dispatchReceiptRef = receiptRefForKind('dispatch_recorded') ??
    input.attempt.payoutAttemptRef
  const confirmationReceiptRef =
    receiptRefForKind('confirmation_recorded') ??
      input.reconciliationEvent.eventRef
  const verificationReceiptRef =
    receiptRefForKind('verification_recorded') ??
      input.reconciliationEvent.resultRef
  const settlementReceiptRef = receiptRefForKind('settlement_recorded') ??
    `settlement.public_receipt.pylon_marketplace.${suffix}`
  const base = {
    acceptedWork: input.acceptedWork,
    attempt: input.attempt,
    createdAtIso: input.createdAtIso,
    evidenceRefs: input.acceptedWork.evidenceRefs,
    idSuffix: suffix,
    intent: input.intent,
    providerRef: input.providerRef,
    updatedAtIso: input.updatedAtIso,
  }

  return [
    bridgeRecord({
      ...base,
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      state: 'reward_intent',
    }),
    bridgeRecord({
      ...base,
      payoutEligibilityRefs: [
        `payout_eligibility.pylon_marketplace.${suffix}`,
      ],
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      state: 'payout_eligible',
    }),
    bridgeRecord({
      ...base,
      payoutDispatchRefs: [
        input.attempt.payoutAttemptRef,
        input.attempt.adapterAttemptRef,
        dispatchReceiptRef,
      ],
      payoutEligibilityRefs: [
        `payout_eligibility.pylon_marketplace.${suffix}`,
      ],
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      state: 'payout_dispatched',
    }),
    bridgeRecord({
      ...base,
      payoutConfirmationRefs: [
        input.reconciliationEvent.eventRef,
        confirmationReceiptRef,
      ],
      payoutDispatchRefs: [
        input.attempt.payoutAttemptRef,
        input.attempt.adapterAttemptRef,
        dispatchReceiptRef,
      ],
      payoutEligibilityRefs: [
        `payout_eligibility.pylon_marketplace.${suffix}`,
      ],
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      state: 'payout_confirmed',
    }),
    bridgeRecord({
      ...base,
      payoutConfirmationRefs: [
        input.reconciliationEvent.eventRef,
        confirmationReceiptRef,
      ],
      payoutDispatchRefs: [
        input.attempt.payoutAttemptRef,
        input.attempt.adapterAttemptRef,
        dispatchReceiptRef,
      ],
      payoutEligibilityRefs: [
        `payout_eligibility.pylon_marketplace.${suffix}`,
      ],
      payoutVerificationRefs: [
        input.reconciliationEvent.resultRef,
        verificationReceiptRef,
      ],
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      state: 'payout_verified',
    }),
    bridgeRecord({
      ...base,
      payoutConfirmationRefs: [
        input.reconciliationEvent.eventRef,
        confirmationReceiptRef,
      ],
      payoutDispatchRefs: [
        input.attempt.payoutAttemptRef,
        input.attempt.adapterAttemptRef,
        dispatchReceiptRef,
      ],
      payoutEligibilityRefs: [
        `payout_eligibility.pylon_marketplace.${suffix}`,
      ],
      payoutVerificationRefs: [
        input.reconciliationEvent.resultRef,
        verificationReceiptRef,
      ],
      rewardIntentRefs: [input.intent.payoutIntentRef, intentReceiptRef],
      settlementRefs: [settlementReceiptRef],
      state: 'settled',
    }),
  ]
}

export const buildPylonMarketplaceSettlementBridgePauseRecord = (
  input: Readonly<{
    acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
    attempt: NexusTreasuryPayoutAttemptRecord
    blockerRefs: ReadonlyArray<string>
    createdAtIso: string
    intent: NexusTreasuryPayoutIntentRecord
    providerRef: string
    refs: PylonMarketplacePayoutFlowRefs
    updatedAtIso: string
  }>,
): OpenAgentsPylonSettlementBridgeRecord =>
  bridgeRecord({
    acceptedWork: input.acceptedWork,
    attempt: input.attempt,
    blockerRefs: requiredRefs('pause blocker refs', input.blockerRefs),
    createdAtIso: input.createdAtIso,
    evidenceRefs: input.acceptedWork.evidenceRefs,
    idSuffix: `${stableSuffix(input.refs.idempotencyRef)}.paused`,
    intent: input.intent,
    providerRef: input.providerRef,
    state: 'blocked',
    updatedAtIso: input.updatedAtIso,
  })

export const buildPylonMarketplaceAcceptedWorkPayoutRow = (
  input: Readonly<{
    acceptedWork: PylonMarketplaceAcceptedWorkPayoutEvidence
    bridgeTimeline: ReadonlyArray<OpenAgentsPylonSettlementBridgeRecord>
    createdAtIso: string
    intent: NexusTreasuryPayoutIntentRecord
    providerRef: string
    refs: PylonMarketplacePayoutFlowRefs
    updatedAtIso: string
  }>,
): PylonAcceptedWorkPayoutRowRecord => {
  const suffix = stableSuffix(input.refs.idempotencyRef)
  const settledBridge = input.bridgeTimeline.find(
    record => record.state === 'settled',
  )
  const verifiedBridge = input.bridgeTimeline.find(
    record => record.state === 'payout_verified',
  )
  const dispatchedBridge = input.bridgeTimeline.find(
    record => record.state === 'payout_dispatched',
  )
  const eligibleBridge = input.bridgeTimeline.find(
    record => record.state === 'payout_eligible',
  )
  const rewardBridge = input.bridgeTimeline.find(
    record => record.state === 'reward_intent',
  )

  if (settledBridge === undefined || verifiedBridge === undefined) {
    throw new PylonMarketplacePayoutFlowUnsafe({
      reason: 'Accepted-work payout row requires verified and settled bridge records.',
    })
  }

  return new PylonAcceptedWorkPayoutRowRecord({
    acceptedWorkRefs: input.acceptedWork.acceptedWorkRefs,
    authority: PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: [
      'caveat.public.row_links_settlement_receipt_only',
      ...input.acceptedWork.payoutCaveatRefs,
    ],
    confirmationRefs: settledBridge.payoutConfirmationRefs,
    createdAtIso: input.createdAtIso,
    dispatchRefs: dispatchedBridge?.payoutDispatchRefs ?? [],
    eligibilityRefs: eligibleBridge?.payoutEligibilityRefs ?? [],
    evidenceRefs: input.acceptedWork.evidenceRefs,
    id: `payout_row.pylon_marketplace.${suffix}`,
    linkRefs: [
      input.acceptedWork.jobRef,
      input.acceptedWork.assignmentRef,
      input.intent.payoutIntentRef,
    ],
    payoutBasis: 'accepted_work_reward',
    payoutClass: 'settled_payout',
    progressClass: 'settled',
    providerRef: input.providerRef,
    providerVisibility: 'public',
    rewardIntentRefs: rewardBridge?.rewardIntentRefs ?? [],
    rowRef: `row.pylon_marketplace.${suffix}`,
    settlementRefs: settledBridge.settlementRefs,
    settlementState: 'settled',
    sourceRefs: [
      'docs/nexus/2026-06-07-pylon-marketplace-payout-flow-runbook.md',
    ],
    surfaceRefs: [
      'surface.openagents.nexus_pylon_marketplace',
      'surface.openagents.payment_authority_receipts',
    ],
    updatedAtIso: input.updatedAtIso,
    verificationRefs: verifiedBridge.payoutVerificationRefs,
    workClass: 'pylon_compute',
  })
}

export const buildPylonMarketplacePayoutFlowRecords = (
  input: Readonly<{
    amounts: PylonMarketplacePayoutFlowAmounts
    assignment: PylonMarketplaceAssignmentRecord
    createdAtIso: string
    refs: PylonMarketplacePayoutFlowRefs
    updatedAtIso: string
  }>,
): PylonMarketplacePayoutFlowRecords => {
  const acceptedWork =
    acceptedWorkPayoutEvidenceFromMarketplaceAssignment(input.assignment)
  const intent = buildPylonMarketplacePayoutIntent({
    acceptedWork,
    amounts: input.amounts,
    nowIso: input.createdAtIso,
    refs: input.refs,
  })
  const attempt = buildPylonMarketplacePayoutAttempt({
    intent,
    nowIso: input.updatedAtIso,
    refs: input.refs,
  })
  const reconciliationEvent = buildPylonMarketplacePayoutReconciliationEvent({
    attempt,
    intent,
    nowIso: input.updatedAtIso,
    refs: input.refs,
  })
  const intentCreatedReceipt = buildPylonMarketplacePayoutIntentCreatedReceipt({
    intent,
    nowIso: input.createdAtIso,
    refs: input.refs,
  })
  const simulationReceipts = buildTreasuryPaymentSimulationReceipts({
    attempt,
    createdAt: input.updatedAtIso,
    event: reconciliationEvent,
    intent,
  })
  const bridgeTimeline = buildPylonMarketplaceSettlementBridgeTimeline({
    acceptedWork,
    attempt,
    createdAtIso: input.createdAtIso,
    intent,
    providerRef: input.refs.providerRef,
    reconciliationEvent,
    receipts: [intentCreatedReceipt, ...simulationReceipts],
    refs: input.refs,
    updatedAtIso: input.updatedAtIso,
  })
  const payoutRow = buildPylonMarketplaceAcceptedWorkPayoutRow({
    acceptedWork,
    bridgeTimeline,
    createdAtIso: input.createdAtIso,
    intent,
    providerRef: input.refs.providerRef,
    refs: input.refs,
    updatedAtIso: input.updatedAtIso,
  })

  return {
    acceptedWork,
    attempt,
    bridgeTimeline,
    intent,
    intentCreatedReceipt,
    payoutRow,
    reconciliationEvent,
    simulationReceipts,
  }
}
