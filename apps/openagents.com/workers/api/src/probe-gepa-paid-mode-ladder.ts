import { Array as Arr, Schema as S } from 'effect'

import {
  type ProbeGepaSettlementReadinessResult,
  ProbeGepaSettlementRequestedPaymentMode,
} from './probe-gepa-settlement-readiness'
import type { ProbeGepaStage0NoSpendCampaignProjection } from './probe-gepa-stage0-no-spend-campaign'
import {
  PylonGepaMetricCallPaymentMode,
  assertPylonGepaMetricCallPublicRefs,
} from './pylon-gepa-metric-call-assignments'
import type { PylonGepaMetricCallCoordinatorImport } from './pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const ProbeGepaPaidModeCampaignLadderSchemaVersion =
  'omega.probe_gepa_paid_mode_campaign_ladder.v1'

export const ProbeGepaPaidModeCampaignLadderState = S.Literals([
  'blocked',
  'payable_pending_settlement_ready',
  'settled_bitcoin_ready',
  'unpaid_smoke_ready',
])
export type ProbeGepaPaidModeCampaignLadderState =
  typeof ProbeGepaPaidModeCampaignLadderState.Type

export const ProbeGepaPaidModeAggregatePaymentMode = S.Literals([
  'none',
  'payable_pending_settlement',
  'settled_bitcoin',
  'unpaid_smoke',
])
export type ProbeGepaPaidModeAggregatePaymentMode =
  typeof ProbeGepaPaidModeAggregatePaymentMode.Type

export const ProbeGepaPaidModeBridgeAttemptDecision = S.Literals([
  'accepted',
  'denied_liquidity',
  'denied_send_readiness',
  'duplicate_replay',
])
export type ProbeGepaPaidModeBridgeAttemptDecision =
  typeof ProbeGepaPaidModeBridgeAttemptDecision.Type

export const ProbeGepaPaidModeBridgeAttemptPaymentMode = S.Literals([
  'payable_pending_settlement',
  'settled_bitcoin',
])
export type ProbeGepaPaidModeBridgeAttemptPaymentMode =
  typeof ProbeGepaPaidModeBridgeAttemptPaymentMode.Type

export class ProbeGepaPaidModeBridgeAttempt extends S.Class<ProbeGepaPaidModeBridgeAttempt>(
  'ProbeGepaPaidModeBridgeAttempt',
)({
  assignmentRef: S.String,
  bridgeAttemptRef: S.String,
  decision: ProbeGepaPaidModeBridgeAttemptDecision,
  denialRefs: S.Array(S.String),
  idempotencyKeyRef: S.String,
  paymentReceiptRefs: S.Array(S.String),
  replayOfAttemptRef: S.NullOr(S.String),
  requestedPaymentMode: ProbeGepaPaidModeBridgeAttemptPaymentMode,
  settlementReceiptRefs: S.Array(S.String),
}) {}

export class ProbeGepaPaidModeSendReadiness extends S.Class<ProbeGepaPaidModeSendReadiness>(
  'ProbeGepaPaidModeSendReadiness',
)({
  denialRefs: S.Array(S.String),
  outboundLiquidityReady: S.Boolean,
  payerWalletReady: S.Boolean,
  sendPreflightRefs: S.Array(S.String),
}) {}

export const ProbeGepaPaidModeAssignmentPaymentProjection = S.Struct({
  acceptedWorkClaimAllowed: S.Boolean,
  assignmentRef: S.String,
  payableWorkClaimAllowed: S.Boolean,
  paymentMode: PylonGepaMetricCallPaymentMode,
  paymentReceiptRefs: S.Array(S.String),
  settledBitcoinPayoutClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  workerRef: S.NullOr(S.String),
})
export type ProbeGepaPaidModeAssignmentPaymentProjection =
  typeof ProbeGepaPaidModeAssignmentPaymentProjection.Type

export class ProbeGepaPaidModeCampaignLadderProjection extends S.Class<ProbeGepaPaidModeCampaignLadderProjection>(
  'ProbeGepaPaidModeCampaignLadderProjection',
)({
  aggregatePaymentMode: ProbeGepaPaidModeAggregatePaymentMode,
  assignmentPaymentModes: S.Array(ProbeGepaPaidModeAssignmentPaymentProjection),
  blockerRefs: S.Array(S.String),
  bridgeAttemptRefs: S.Array(S.String),
  campaignRef: S.String,
  duplicateReplayDoubleSettlementBlocked: S.Boolean,
  ladderState: ProbeGepaPaidModeCampaignLadderState,
  liveSmallSatsSmokeRefs: S.Array(S.String),
  paidPylonWorkClaimAllowed: S.Boolean,
  payablePendingSettlementClaimAllowed: S.Boolean,
  paymentReceiptRefs: S.Array(S.String),
  publicCopySettlementReceiptRefs: S.Array(S.String),
  readinessDecisionRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaPaidModeCampaignLadderSchemaVersion),
  sendReadinessReady: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  settledBitcoinCampaignClaimAllowed: S.Boolean,
  stage0Green: S.Boolean,
  unpaidSmokeClaimAllowed: S.Boolean,
}) {}

export type ProbeGepaPaidModeCampaignLadderInput = Readonly<{
  bridgeAttempts?: ReadonlyArray<ProbeGepaPaidModeBridgeAttempt> | undefined
  campaignRef: string
  coordinatorImports: ReadonlyArray<PylonGepaMetricCallCoordinatorImport>
  liveSmallSatsSmokeRefs?: ReadonlyArray<string> | undefined
  sendReadiness: ProbeGepaPaidModeSendReadiness
  settlementReadinessResults: ReadonlyArray<ProbeGepaSettlementReadinessResult>
  stage0Projection: ProbeGepaStage0NoSpendCampaignProjection
}>

export class ProbeGepaPaidModeCampaignLadderUnsafe extends S.TaggedErrorClass<ProbeGepaPaidModeCampaignLadderUnsafe>()(
  'ProbeGepaPaidModeCampaignLadderUnsafe',
  {
    reason: S.String,
  },
) {}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  Arr.isReadonlyArrayNonEmpty(refs)

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)

  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new ProbeGepaPaidModeCampaignLadderUnsafe({
      reason:
        error instanceof Error
          ? error.message
          : `${label} contains unsafe refs.`,
    })
  }

  return normalized
}

const modeReady = (
  resultsByMode: Map<
    ProbeGepaSettlementRequestedPaymentMode,
    ProbeGepaSettlementReadinessResult
  >,
  requestedPaymentMode: ProbeGepaSettlementRequestedPaymentMode,
): boolean => {
  const result = resultsByMode.get(requestedPaymentMode)

  return (
    result !== undefined &&
    result.decision === 'ready' &&
    result.requestedPaymentMode === requestedPaymentMode
  )
}

const duplicateRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const duplicates = new Set<string>()

  refs.forEach(ref => {
    if (seen.has(ref)) {
      duplicates.add(`${label}.${publicRefSegment(ref, 'record')}`)
    }
    seen.add(ref)
  })

  return [...duplicates].sort()
}

const bridgeReplayBlockers = (
  attempts: ReadonlyArray<ProbeGepaPaidModeBridgeAttempt>,
): ReadonlyArray<string> => {
  const acceptedSettledAttempts = attempts.filter(
    attempt =>
      attempt.decision === 'accepted' &&
      attempt.requestedPaymentMode === 'settled_bitcoin',
  )
  const duplicateReplayAttempts = attempts.filter(
    attempt => attempt.decision === 'duplicate_replay',
  )

  return uniqueRefs([
    ...duplicateRefs(
      'blocker.probe_gepa.paid_ladder.duplicate_settlement_assignment',
      acceptedSettledAttempts.map(attempt => attempt.assignmentRef),
    ),
    ...duplicateRefs(
      'blocker.probe_gepa.paid_ladder.duplicate_bridge_idempotency_key',
      attempts
        .filter(attempt => attempt.decision === 'accepted')
        .map(attempt => attempt.idempotencyKeyRef),
    ),
    ...duplicateReplayAttempts.flatMap(attempt => [
      ...(attempt.replayOfAttemptRef === null
        ? [
            `blocker.probe_gepa.paid_ladder.duplicate_replay_missing_source.${publicRefSegment(
              attempt.bridgeAttemptRef,
              'record',
            )}`,
          ]
        : []),
      ...(hasRefs(attempt.paymentReceiptRefs) ||
      hasRefs(attempt.settlementReceiptRefs)
        ? [
            `blocker.probe_gepa.paid_ladder.duplicate_replay_has_new_receipts.${publicRefSegment(
              attempt.bridgeAttemptRef,
              'record',
            )}`,
          ]
        : []),
    ]),
  ])
}

const assignmentPaymentModes = (
  coordinatorImports: ReadonlyArray<PylonGepaMetricCallCoordinatorImport>,
): ReadonlyArray<ProbeGepaPaidModeAssignmentPaymentProjection> =>
  coordinatorImports
    .map(coordinatorImport => ({
      acceptedWorkClaimAllowed: coordinatorImport.acceptedWorkClaimAllowed,
      assignmentRef: coordinatorImport.assignmentRef,
      payableWorkClaimAllowed: coordinatorImport.payableWorkClaimAllowed,
      paymentMode: coordinatorImport.paymentMode,
      paymentReceiptRefs: uniqueRefs(coordinatorImport.paymentReceiptRefs),
      settledBitcoinPayoutClaimAllowed:
        coordinatorImport.settledBitcoinPayoutClaimAllowed,
      settlementReceiptRefs: uniqueRefs(
        coordinatorImport.settlementReceiptRefs,
      ),
      workerRef: coordinatorImport.workerRef,
    }))
    .sort((left, right) =>
      left.assignmentRef.localeCompare(right.assignmentRef),
    )

const readinessResultBlockers = (
  results: ReadonlyArray<ProbeGepaSettlementReadinessResult>,
): ReadonlyArray<string> =>
  uniqueRefs(
    results.flatMap(result =>
      result.decision === 'ready' ? [] : result.blockerRefs,
    ),
  )

export const projectProbeGepaPaidModeCampaignLadder = (
  input: ProbeGepaPaidModeCampaignLadderInput,
): ProbeGepaPaidModeCampaignLadderProjection => {
  const campaignRef =
    assertPublicRefs('Probe GEPA paid ladder campaign refs', [
      input.campaignRef,
    ])[0] ?? 'campaign.public.probe_gepa.paid_ladder.redacted'
  const bridgeAttempts = (input.bridgeAttempts ?? []).map(
    attempt =>
      new ProbeGepaPaidModeBridgeAttempt({
        ...attempt,
        denialRefs: uniqueRefs(attempt.denialRefs),
        paymentReceiptRefs: uniqueRefs(attempt.paymentReceiptRefs),
        settlementReceiptRefs: uniqueRefs(attempt.settlementReceiptRefs),
      }),
  )
  const sendReadiness = new ProbeGepaPaidModeSendReadiness({
    ...input.sendReadiness,
    denialRefs: uniqueRefs(input.sendReadiness.denialRefs),
    sendPreflightRefs: uniqueRefs(input.sendReadiness.sendPreflightRefs),
  })
  const liveSmallSatsSmokeRefs = assertPublicRefs(
    'Probe GEPA paid ladder live-small-sats smoke refs',
    input.liveSmallSatsSmokeRefs ?? [],
  )
  const bridgeAttemptRefs = assertPublicRefs(
    'Probe GEPA paid ladder bridge attempt refs',
    bridgeAttempts.flatMap(attempt => [
      attempt.assignmentRef,
      attempt.bridgeAttemptRef,
      attempt.idempotencyKeyRef,
      ...(attempt.replayOfAttemptRef === null
        ? []
        : [attempt.replayOfAttemptRef]),
      ...attempt.denialRefs,
      ...attempt.paymentReceiptRefs,
      ...attempt.settlementReceiptRefs,
    ]),
  )
  assertPublicRefs('Probe GEPA paid ladder send-readiness refs', [
    ...sendReadiness.denialRefs,
    ...sendReadiness.sendPreflightRefs,
  ])

  const coordinatorImports = input.coordinatorImports
  const paymentReceiptRefs = assertPublicRefs(
    'Probe GEPA paid ladder payment receipt refs',
    [
      ...coordinatorImports.flatMap(
        coordinatorImport => coordinatorImport.paymentReceiptRefs,
      ),
      ...bridgeAttempts.flatMap(attempt => attempt.paymentReceiptRefs),
    ],
  )
  const settlementReceiptRefs = assertPublicRefs(
    'Probe GEPA paid ladder settlement receipt refs',
    [
      ...coordinatorImports.flatMap(
        coordinatorImport => coordinatorImport.settlementReceiptRefs,
      ),
      ...bridgeAttempts.flatMap(attempt => attempt.settlementReceiptRefs),
    ],
  )
  const readinessDecisionRefs = assertPublicRefs(
    'Probe GEPA paid ladder readiness refs',
    input.settlementReadinessResults.flatMap(result => [
      result.batchRef,
      result.readinessDecisionRef,
      ...result.acceptedAssignmentRefs,
      ...result.accountingRefs,
      ...result.operatorAccountingRefs,
      ...result.blockerRefs,
    ]),
  )
  const resultsByMode = new Map<
    ProbeGepaSettlementRequestedPaymentMode,
    ProbeGepaSettlementReadinessResult
  >(
    input.settlementReadinessResults.map(result => [
      result.requestedPaymentMode,
      result,
    ]),
  )
  const duplicateModeBlockers = duplicateRefs(
    'blocker.probe_gepa.paid_ladder.duplicate_readiness_mode',
    input.settlementReadinessResults.map(result => result.requestedPaymentMode),
  )
  const stage0Green =
    input.stage0Projection.state === 'green' &&
    input.stage0Projection.noSpendCampaignClaimAllowed &&
    input.stage0Projection.paidModesBlocked
  const unpaidSmokeReady =
    stage0Green && modeReady(resultsByMode, 'unpaid_smoke')
  const payablePendingSettlementReady =
    unpaidSmokeReady &&
    modeReady(resultsByMode, 'payable_pending_settlement') &&
    hasRefs(paymentReceiptRefs)
  const sendReadinessReady =
    sendReadiness.payerWalletReady &&
    sendReadiness.outboundLiquidityReady &&
    hasRefs(sendReadiness.sendPreflightRefs) &&
    !hasRefs(sendReadiness.denialRefs)
  const replayBlockers = bridgeReplayBlockers(bridgeAttempts)
  const duplicateReplayDoubleSettlementBlocked = replayBlockers.length === 0
  const settledBitcoinReady =
    payablePendingSettlementReady &&
    modeReady(resultsByMode, 'settled_bitcoin') &&
    hasRefs(settlementReceiptRefs) &&
    sendReadinessReady &&
    duplicateReplayDoubleSettlementBlocked &&
    hasRefs(liveSmallSatsSmokeRefs)
  const ladderState: ProbeGepaPaidModeCampaignLadderState = settledBitcoinReady
    ? 'settled_bitcoin_ready'
    : payablePendingSettlementReady
      ? 'payable_pending_settlement_ready'
      : unpaidSmokeReady
        ? 'unpaid_smoke_ready'
        : 'blocked'
  const aggregatePaymentMode: ProbeGepaPaidModeAggregatePaymentMode =
    ladderState === 'settled_bitcoin_ready'
      ? 'settled_bitcoin'
      : ladderState === 'payable_pending_settlement_ready'
        ? 'payable_pending_settlement'
        : ladderState === 'unpaid_smoke_ready'
          ? 'unpaid_smoke'
          : 'none'
  const blockerRefs = uniqueRefs([
    ...(!stage0Green
      ? ['blocker.probe_gepa.paid_ladder.stage0_no_spend_not_green']
      : []),
    ...(!modeReady(resultsByMode, 'unpaid_smoke')
      ? ['blocker.probe_gepa.paid_ladder.unpaid_smoke_readiness_missing']
      : []),
    ...(!modeReady(resultsByMode, 'payable_pending_settlement')
      ? ['blocker.probe_gepa.paid_ladder.payable_readiness_missing']
      : []),
    ...(!modeReady(resultsByMode, 'settled_bitcoin')
      ? ['blocker.probe_gepa.paid_ladder.settled_bitcoin_readiness_missing']
      : []),
    ...(!hasRefs(paymentReceiptRefs)
      ? ['blocker.probe_gepa.paid_ladder.payment_receipts_missing']
      : []),
    ...(!hasRefs(settlementReceiptRefs)
      ? ['blocker.probe_gepa.paid_ladder.settlement_receipts_missing']
      : []),
    ...(!sendReadinessReady
      ? ['blocker.probe_gepa.paid_ladder.send_readiness_or_liquidity_missing']
      : []),
    ...(!hasRefs(liveSmallSatsSmokeRefs)
      ? ['blocker.probe_gepa.paid_ladder.live_small_sats_smoke_missing']
      : []),
    ...duplicateModeBlockers,
    ...replayBlockers,
    ...readinessResultBlockers(input.settlementReadinessResults),
  ])

  return new ProbeGepaPaidModeCampaignLadderProjection({
    aggregatePaymentMode,
    assignmentPaymentModes: [...assignmentPaymentModes(coordinatorImports)],
    blockerRefs,
    bridgeAttemptRefs,
    campaignRef,
    duplicateReplayDoubleSettlementBlocked,
    ladderState,
    liveSmallSatsSmokeRefs,
    paidPylonWorkClaimAllowed: payablePendingSettlementReady,
    payablePendingSettlementClaimAllowed: payablePendingSettlementReady,
    paymentReceiptRefs,
    publicCopySettlementReceiptRefs: settlementReceiptRefs,
    readinessDecisionRefs,
    schemaVersion: ProbeGepaPaidModeCampaignLadderSchemaVersion,
    sendReadinessReady,
    settlementReceiptRefs,
    settledBitcoinCampaignClaimAllowed: settledBitcoinReady,
    stage0Green,
    unpaidSmokeClaimAllowed: unpaidSmokeReady,
  })
}
