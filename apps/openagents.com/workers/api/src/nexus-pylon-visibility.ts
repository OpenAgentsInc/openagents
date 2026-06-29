import { Schema as S } from 'effect'

import {
  exampleArtanisNexusPylonAdminAdapterLedger,
  projectArtanisNexusPylonAdminAdapter,
} from './artanis-nexus-pylon-adapters'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { parseJsonUnknown } from './json-boundary'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
  type NexusReleaseGateRecord,
  type NexusTreasuryPayoutLedgerRecordKind,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import { projectPylonAcceptedWorkPayoutRow } from './pylon-accepted-work-payout-rows'
import {
  examplePylonMarketplaceLedger,
  projectPylonMarketplaceLedger,
} from './pylon-marketplace-jobs'
import { buildPylonMarketplacePayoutFlowRecords } from './pylon-marketplace-payout-flow'
import { projectOpenAgentsPylonSettlementBridge } from './pylon-settlement-bridge'

export type NexusPylonVisibilityMovementMode = 'real_bitcoin' | 'simulation'

export type NexusPylonVisibilityFixture = Readonly<{
  artanisProjection: ReturnType<typeof projectArtanisNexusPylonAdminAdapter>
  bridgeProjections: ReadonlyArray<
    ReturnType<typeof projectOpenAgentsPylonSettlementBridge>
  >
  marketplaceProjection: ReturnType<typeof projectPylonMarketplaceLedger>
  movementMode: NexusPylonVisibilityMovementMode
  payoutAttemptProjection: ReturnType<
    typeof projectNexusTreasuryPayoutLedgerRecord
  >
  payoutIntentProjection: ReturnType<
    typeof projectNexusTreasuryPayoutLedgerRecord
  >
  payoutRowProjection: ReturnType<typeof projectPylonAcceptedWorkPayoutRow>
  realBitcoinMoved: boolean
  receiptProjections: ReadonlyArray<
    ReturnType<typeof projectNexusTreasuryPayoutLedgerRecord>
  >
  receipts: ReadonlyArray<NexusPaymentAuthorityReceiptRecord>
  releaseGateProjections: ReadonlyArray<
    ReturnType<typeof projectNexusTreasuryPayoutLedgerRecord>
  >
  settlementProjection: ReturnType<
    typeof projectOpenAgentsPylonSettlementBridge
  >
}>

export type NexusPylonPublicReceiptDetail = Readonly<{
  schemaVersion: 'openagents.nexus_pylon.public_receipt.v1'
  apiUrl: string
  assignmentRef: string | null
  audience: 'public'
  caveatRefs: ReadonlyArray<string>
  movementMode: NexusPylonVisibilityMovementMode
  payoutAttemptRef: string | null
  payoutIntentRef: string | null
  publicProjection: unknown
  realBitcoinMoved: boolean
  receiptKind: string
  receiptPageUrl: string
  receiptRef: string
  payoutMovement: Readonly<{
    dispatchAccepted: boolean
    terminalResultObserved: boolean
    terminalSettlementClaimAllowed: boolean
  }>
  settlement: Readonly<{
    buyerPaymentEvidencePresent: boolean
    liveWalletSpendAllowed: boolean
    providerRef: string
    settlementMutationAllowed: boolean
    settlementRefs: ReadonlyArray<string>
    state: string
    stateLabel: string
    updatedAtDisplay: string
    walletReadinessStateLabel: string
  }>
  status: string
}>

export type NexusPylonOperatorDashboard = Readonly<{
  schemaVersion: 'openagents.nexus_pylon.operator_dashboard.v1'
  artanisRuns: NexusPylonVisibilityFixture['artanisProjection']
  assignments: NexusPylonVisibilityFixture['marketplaceProjection']
  blockedGates: ReadonlyArray<
    NexusPylonVisibilityFixture['artanisProjection']['dispatchRecords'][number]
  >
  movementMode: NexusPylonVisibilityMovementMode
  payoutAttempts: ReadonlyArray<
    NexusPylonVisibilityFixture['payoutAttemptProjection']
  >
  payoutIntents: ReadonlyArray<
    NexusPylonVisibilityFixture['payoutIntentProjection']
  >
  pylonReadiness: NexusPylonVisibilityFixture['settlementProjection']
  realBitcoinMoved: boolean
  receiptProjections: NexusPylonVisibilityFixture['receiptProjections']
  releaseGateEvidence: NexusPylonVisibilityFixture['releaseGateProjections']
  settlementStatus: NexusPylonVisibilityFixture['bridgeProjections']
}>

export class NexusPylonVisibilityNotFound extends S.TaggedErrorClass<NexusPylonVisibilityNotFound>()(
  'NexusPylonVisibilityNotFound',
  {
    receiptRef: S.String,
  },
) {}

export class NexusPylonVisibilityUnsafe extends S.TaggedErrorClass<NexusPylonVisibilityUnsafe>()(
  'NexusPylonVisibilityUnsafe',
  {
    reason: S.String,
  },
) {}

const fixtureCreatedAtIso = '2026-06-07T06:45:00.000Z'
const fixtureUpdatedAtIso = '2026-06-07T07:05:00.000Z'
const fixtureRefs = {
  artanisDispatchRef: 'artanis.dispatch.pylon_marketplace.gepa_autopilot_001',
  buyerPaymentEvidenceRef:
    'buyer_payment_evidence.public.pylon_marketplace.gepa_autopilot_001',
  idempotencyRef: 'gepa_autopilot_001',
  ownerUserId: 'user_openagents_operator',
  payoutTargetApprovalRef:
    'approval.nexus_payout_target.pylon_marketplace.gepa_autopilot_001',
  payoutTargetRef: 'payout_target.pylon_marketplace.gepa_autopilot_001',
  policySnapshotRef: 'policy_snapshot.nexus.pylon_marketplace.spend_cap_001',
  providerRef: 'provider.public.pylon_demo_runner',
} as const
const fixtureAmounts = {
  amount: {
    amountMinorUnits: 1_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  spendCap: {
    amountMinorUnits: 2_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
} as const
const unsafePublicKeyPattern =
  /(access[_-]?token|bearer|cookie|customer[_-]?(email|name)|email[_-]?(address|body|html|raw|text)|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|operatorRefs|payment[_-]?(hash|invoice|preimage|raw|secret)|payoutTargetRef|preimage|private|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|redactedDestinationRef|redactedPaymentRef|secret|sk-[A-Za-z0-9]|wallet[_-]?(config|key|material|mnemonic|secret|seed|state))/i
const unsafePublicValuePattern =
  /(@|bearer\s+|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|raw[_-]?(invoice|payment|prompt|runner|run[_-]?log)|secret|sk-[A-Za-z0-9]|wallet[_-]?(config|key|material|mnemonic|secret|seed|state))/i

const receiptRecordKinds: Readonly<
  Record<string, NexusTreasuryPayoutLedgerRecordKind>
> = {
  confirmation_recorded: 'receipt',
  dispatch_recorded: 'receipt',
  intent_created: 'receipt',
  settlement_recorded: 'receipt',
  verification_recorded: 'receipt',
}

const stableReleaseGate = (
  input: Readonly<{
    blockerRefs?: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
    gateKind: NexusReleaseGateRecord['gateKind']
    gateRef: string
    status: NexusReleaseGateRecord['status']
  }>,
): NexusReleaseGateRecord => ({
  archivedAt: null,
  blockerRefs: [...(input.blockerRefs ?? [])],
  createdAt: fixtureUpdatedAtIso,
  evidenceRefs: [...input.evidenceRefs],
  gateKind: input.gateKind,
  gateRef: input.gateRef,
  id: input.gateRef.replaceAll('.', '_'),
  idempotencyKeyHash: `hash.${input.gateRef}`,
  publicProjectionJson: JSON.stringify({
    evidenceOnly: true,
    moneyMovement: 'none',
    simulation: true,
  }),
  status: input.status,
  updatedAt: fixtureUpdatedAtIso,
})

const parsePublicProjectionJson = (value: string): unknown => {
  try {
    return parseJsonUnknown(value)
  } catch {
    return {}
  }
}

const publicProjectionRecord = (
  value: string,
): Readonly<Record<string, unknown>> => {
  const parsed = parsePublicProjectionJson(value)

  return parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)
    ? parsed as Readonly<Record<string, unknown>>
    : {}
}

const stringFromPublicProjection = (
  projection: Readonly<Record<string, unknown>>,
  key: string,
): string | undefined => {
  const value = projection[key]

  return typeof value === 'string' && value.trim() !== ''
    ? value
    : undefined
}

const publicSettlementStateLabel = (
  label: string,
  realBitcoinMoved: boolean,
): string => (realBitcoinMoved ? label : `${label} (simulation only)`)

const normalizeReceiptRef = (receiptRef: string): string =>
  decodeURIComponent(receiptRef).trim()

const selectedReceipt = (
  receipts: ReadonlyArray<NexusPaymentAuthorityReceiptRecord>,
  receiptRef: string,
): NexusPaymentAuthorityReceiptRecord | undefined =>
  receipts.find(
    receipt => receipt.receiptRef === normalizeReceiptRef(receiptRef),
  )

const scanPublicResponse = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return unsafePublicValuePattern.test(value) ? path.join('.') : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) => scanPublicResponse(item, [...path, String(index)]))
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) =>
      unsafePublicKeyPattern.test(key)
        ? [...path, key].join('.')
        : scanPublicResponse(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

export const assertNexusPylonPublicSafe = (
  label: string,
  value: unknown,
): void => {
  const unsafePath = scanPublicResponse(value)

  if (unsafePath !== undefined) {
    throw new NexusPylonVisibilityUnsafe({
      reason: `${label} contains private payment, wallet, operator, customer, or raw runner material at ${unsafePath}.`,
    })
  }
}

export const exampleNexusPylonVisibilityFixture = (
  nowIso: string,
): NexusPylonVisibilityFixture => {
  const marketplace = examplePylonMarketplaceLedger()
  const assignment = marketplace.assignmentRecords[0]

  if (assignment === undefined) {
    throw new NexusPylonVisibilityUnsafe({
      reason:
        'Nexus/Pylon visibility fixture requires a marketplace assignment.',
    })
  }

  const flow = buildPylonMarketplacePayoutFlowRecords({
    amounts: fixtureAmounts,
    assignment,
    createdAtIso: fixtureCreatedAtIso,
    refs: fixtureRefs,
    updatedAtIso: fixtureUpdatedAtIso,
  })
  const receipts = [flow.intentCreatedReceipt, ...flow.simulationReceipts]
  const bridgeProjections = flow.bridgeTimeline.map(record =>
    projectOpenAgentsPylonSettlementBridge(record, 'operator', nowIso),
  )
  const releaseGateProjections = [
    stableReleaseGate({
      evidenceRefs: [
        flow.intentCreatedReceipt.receiptRef,
        flow.simulationReceipts.at(-1)?.receiptRef ??
          'receipt.nexus.pylon_marketplace.settlement_missing',
      ],
      gateKind: 'public_receipt',
      gateRef: 'gate.nexus_pylon.public_receipt.gepa_autopilot_001',
      status: 'passed',
    }),
    stableReleaseGate({
      evidenceRefs: [
        flow.intent.payoutIntentRef,
        flow.attempt.payoutAttemptRef,
      ],
      gateKind: 'operator_dashboard',
      gateRef: 'gate.nexus_pylon.operator_dashboard.gepa_autopilot_001',
      status: 'passed',
    }),
  ].map(record =>
    projectNexusTreasuryPayoutLedgerRecord('release_gate', record, 'operator'),
  )

  return {
    artanisProjection: projectArtanisNexusPylonAdminAdapter(
      exampleArtanisNexusPylonAdminAdapterLedger(),
      'operator',
      nowIso,
    ),
    bridgeProjections,
    marketplaceProjection: projectPylonMarketplaceLedger(
      marketplace,
      'operator',
      nowIso,
    ),
    movementMode: 'simulation',
    payoutAttemptProjection: projectNexusTreasuryPayoutLedgerRecord(
      'attempt',
      flow.attempt,
      'operator',
    ),
    payoutIntentProjection: projectNexusTreasuryPayoutLedgerRecord(
      'intent',
      flow.intent,
      'operator',
    ),
    payoutRowProjection: projectPylonAcceptedWorkPayoutRow(
      flow.payoutRow,
      'operator',
      nowIso,
    ),
    realBitcoinMoved: false,
    receiptProjections: receipts.map(receipt =>
      projectNexusTreasuryPayoutLedgerRecord(
        receiptRecordKinds[receipt.receiptKind] ?? 'receipt',
        receipt,
        'operator',
      ),
    ),
    receipts,
    releaseGateProjections,
    settlementProjection:
      bridgeProjections.at(-1) ??
      projectOpenAgentsPylonSettlementBridge(
        flow.bridgeTimeline[0]!,
        'operator',
        nowIso,
      ),
  }
}

export const nexusPylonPublicReceiptDetail = (
  input: Readonly<{
    appUrl: string
    nowIso: string
    receiptRef: string
  }>,
): NexusPylonPublicReceiptDetail => {
  const fixture = exampleNexusPylonVisibilityFixture(input.nowIso)
  const receipt = selectedReceipt(fixture.receipts, input.receiptRef)

  if (receipt === undefined) {
    throw new NexusPylonVisibilityNotFound({
      receiptRef: normalizeReceiptRef(input.receiptRef),
    })
  }

  const publicReceipt = projectNexusTreasuryPayoutLedgerRecord(
    'receipt',
    receipt,
    'public',
  )
  const settlement = projectOpenAgentsPylonSettlementBridge(
    buildPylonMarketplacePayoutFlowRecords({
      amounts: fixtureAmounts,
      assignment: examplePylonMarketplaceLedger().assignmentRecords[0]!,
      createdAtIso: fixtureCreatedAtIso,
      refs: fixtureRefs,
      updatedAtIso: fixtureUpdatedAtIso,
    }).bridgeTimeline.at(-1)!,
    'public',
    input.nowIso,
  )
  const detail: NexusPylonPublicReceiptDetail = {
    schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
    apiUrl: `${input.appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(
      receipt.receiptRef,
    )}`,
    assignmentRef: publicReceipt.assignmentRef,
    audience: 'public',
    caveatRefs: [
      'caveat.public.nexus_pylon.simulation_receipt',
      'caveat.public.no_private_payment_material',
    ],
    movementMode: fixture.movementMode,
    payoutAttemptRef: publicReceipt.payoutAttemptRef,
    payoutIntentRef: publicReceipt.payoutIntentRef,
    publicProjection: parsePublicProjectionJson(
      publicReceipt.publicProjectionJson,
    ),
    realBitcoinMoved: fixture.realBitcoinMoved,
    receiptKind: receipt.receiptKind,
    receiptPageUrl: `${input.appUrl}/nexus-pylon/receipts/${encodeURIComponent(
      receipt.receiptRef,
    )}`,
    receiptRef: receipt.receiptRef,
    payoutMovement: {
      dispatchAccepted:
        receipt.receiptKind === 'dispatch_recorded' ||
        receipt.receiptKind === 'confirmation_recorded' ||
        receipt.receiptKind === 'verification_recorded' ||
        receipt.receiptKind === 'settlement_recorded',
      terminalResultObserved:
        receipt.receiptKind === 'confirmation_recorded' ||
        receipt.receiptKind === 'verification_recorded' ||
        receipt.receiptKind === 'settlement_recorded',
      terminalSettlementClaimAllowed:
        fixture.realBitcoinMoved && settlement.settlementClaimAllowed,
    },
    settlement: {
      buyerPaymentEvidencePresent: settlement.buyerPaymentEvidencePresent,
      liveWalletSpendAllowed: settlement.liveWalletSpendAllowed,
      providerRef: settlement.providerRef,
      settlementMutationAllowed: settlement.settlementMutationAllowed,
      settlementRefs: settlement.settlementRefs,
      state: settlement.state,
      stateLabel: publicSettlementStateLabel(
        settlement.stateLabel,
        fixture.realBitcoinMoved,
      ),
      updatedAtDisplay: settlement.updatedAtDisplay,
      walletReadinessStateLabel: settlement.walletReadinessStateLabel,
    },
    status: publicReceipt.status,
  }

  assertNexusPylonPublicSafe('Nexus/Pylon public receipt detail', detail)

  return detail
}

export const nexusPylonPublicReceiptDetailFromLedger = (
  input: Readonly<{
    appUrl: string
    nowIso: string
    receipt: NexusPaymentAuthorityReceiptRecord
    attempt?: NexusTreasuryPayoutAttemptRecord | undefined
    event?: NexusTreasuryPayoutReconciliationEventRecord | undefined
    intent?: NexusTreasuryPayoutIntentRecord | undefined
  }>,
): NexusPylonPublicReceiptDetail => {
  const publicReceipt = projectNexusTreasuryPayoutLedgerRecord(
    'receipt',
    input.receipt,
    'public',
  )
  const receiptProjection = publicProjectionRecord(
    publicReceipt.publicProjectionJson,
  )
  const intentProjection = input.intent === undefined
    ? {}
    : publicProjectionRecord(input.intent.publicProjectionJson)
  const attemptProjection = input.attempt === undefined
    ? {}
    : publicProjectionRecord(input.attempt.publicProjectionJson)
  const eventProjection = input.event === undefined
    ? {}
    : publicProjectionRecord(input.event.publicProjectionJson)
  const movementMode: NexusPylonVisibilityMovementMode =
    receiptProjection.moneyMovement === 'real_bitcoin' ||
      attemptProjection.moneyMovement === 'real_bitcoin' ||
      eventProjection.moneyMovement === 'real_bitcoin' ||
      intentProjection.moneyMovement === 'real_bitcoin'
      ? 'real_bitcoin'
      : 'simulation'
  const realBitcoinMoved =
    movementMode === 'real_bitcoin' &&
    input.receipt.receiptKind === 'settlement_recorded' &&
    input.event?.status === 'matched'
  const providerRef =
    input.event?.providerRef ??
    stringFromPublicProjection(receiptProjection, 'providerRef') ??
    stringFromPublicProjection(eventProjection, 'providerRef') ??
    'provider.public.nexus_pylon'
  const settlementState =
    input.receipt.receiptKind === 'settlement_recorded'
      ? 'settled'
      : input.receipt.receiptKind === 'verification_recorded'
        ? 'payout_verified'
        : input.receipt.receiptKind === 'confirmation_recorded'
          ? 'payout_confirmed'
          : input.receipt.receiptKind === 'dispatch_recorded'
            ? 'payout_dispatched'
            : 'reward_intent'
  const settlementStateLabel =
    settlementState === 'settled'
      ? 'Settled'
      : settlementState === 'payout_verified'
        ? 'Payout verified'
        : settlementState === 'payout_confirmed'
          ? 'Payout confirmed'
          : settlementState === 'payout_dispatched'
            ? 'Payout dispatched'
            : 'Reward intent recorded'
  const settlementRefs = [
    ...(
      input.event?.resultRef === undefined ? [] : [input.event.resultRef]
    ),
    input.receipt.receiptRef,
  ]
  const assignmentRef =
    input.intent?.assignmentRef ??
    publicReceipt.assignmentRef ??
    stringFromPublicProjection(receiptProjection, 'assignmentRef') ??
    stringFromPublicProjection(intentProjection, 'assignmentRef') ??
    null
  const detail: NexusPylonPublicReceiptDetail = {
    schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
    apiUrl: `${input.appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(
      input.receipt.receiptRef,
    )}`,
    assignmentRef,
    audience: 'public',
    caveatRefs: [
      movementMode === 'real_bitcoin'
        ? 'caveat.public.nexus_pylon.real_bitcoin_receipt'
        : 'caveat.public.nexus_pylon.simulation_receipt',
      'caveat.public.no_private_payment_material',
    ],
    movementMode,
    payoutAttemptRef: publicReceipt.payoutAttemptRef,
    payoutIntentRef: publicReceipt.payoutIntentRef,
    publicProjection: receiptProjection,
    realBitcoinMoved,
    receiptKind: input.receipt.receiptKind,
    receiptPageUrl: `${input.appUrl}/nexus-pylon/receipts/${encodeURIComponent(
      input.receipt.receiptRef,
    )}`,
    receiptRef: input.receipt.receiptRef,
    payoutMovement: {
      dispatchAccepted:
        input.receipt.receiptKind === 'dispatch_recorded' ||
        input.receipt.receiptKind === 'confirmation_recorded' ||
        input.receipt.receiptKind === 'verification_recorded' ||
        input.receipt.receiptKind === 'settlement_recorded',
      terminalResultObserved:
        input.receipt.receiptKind === 'confirmation_recorded' ||
        input.receipt.receiptKind === 'verification_recorded' ||
        input.receipt.receiptKind === 'settlement_recorded',
      terminalSettlementClaimAllowed: realBitcoinMoved,
    },
    settlement: {
      buyerPaymentEvidencePresent:
        input.intent !== undefined && input.intent.buyerPaymentRef !== null,
      liveWalletSpendAllowed: false,
      providerRef,
      settlementMutationAllowed: false,
      settlementRefs,
      state: settlementState,
      stateLabel: publicSettlementStateLabel(
        settlementStateLabel,
        realBitcoinMoved,
      ),
      updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
        input.receipt.createdAt,
        input.nowIso,
      ),
      walletReadinessStateLabel: 'Receive ready',
    },
    status: publicReceipt.status,
  }

  assertNexusPylonPublicSafe(
    'Nexus/Pylon persisted public receipt detail',
    detail,
  )

  return detail
}

export const nexusPylonOperatorDashboard = (
  nowIso: string,
): NexusPylonOperatorDashboard => {
  const fixture = exampleNexusPylonVisibilityFixture(nowIso)

  return {
    schemaVersion: 'openagents.nexus_pylon.operator_dashboard.v1',
    artanisRuns: fixture.artanisProjection,
    assignments: fixture.marketplaceProjection,
    blockedGates: fixture.artanisProjection.dispatchRecords.filter(
      dispatch =>
        dispatch.paymentAuthorityBlocked ||
        dispatch.state === 'blocked' ||
        dispatch.blockerRefs.length > 0,
    ),
    movementMode: fixture.movementMode,
    payoutAttempts: [fixture.payoutAttemptProjection],
    payoutIntents: [fixture.payoutIntentProjection],
    pylonReadiness: fixture.settlementProjection,
    realBitcoinMoved: fixture.realBitcoinMoved,
    receiptProjections: fixture.receiptProjections,
    releaseGateEvidence: fixture.releaseGateProjections,
    settlementStatus: fixture.bridgeProjections,
  }
}
