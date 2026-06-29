import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaPaidModeBridgeAttempt,
  ProbeGepaPaidModeCampaignLadderProjection,
  ProbeGepaPaidModeCampaignLadderUnsafe,
  ProbeGepaPaidModeSendReadiness,
  projectProbeGepaPaidModeCampaignLadder,
} from './probe-gepa-paid-mode-ladder'
import {
  ProbeGepaSettlementAccountingRecord,
  ProbeGepaSettlementAccountingSchemaVersion,
  evaluateProbeGepaSettlementReadiness,
} from './probe-gepa-settlement-readiness'
import { projectProbeGepaStage0NoSpendCampaign } from './probe-gepa-stage0-no-spend-campaign'
import {
  type PylonGepaMetricCallAssignmentRecord,
  type PylonGepaMetricCallPaymentMode,
  acceptPylonGepaMetricCallAssignment,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  pylonGepaMetricCallCoordinatorImport,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'

const nowIso = '2026-06-08T12:00:00.000Z'

const assignmentInput = (
  suffix: string,
  paymentMode: PylonGepaMetricCallPaymentMode = 'unpaid_smoke',
) =>
  ({
    assignmentRef: `assignment.public.probe_gepa_paid_ladder.${suffix}`,
    backendProfileRef: 'backend_profile.probe.apple_fm.local.v1',
    benchmarkSuiteRef: 'benchmark_suite.terminal_bench_2.harbor.retained.v1',
    campaignId: 'campaign.probe_gepa.paid_mode_ladder',
    candidateHash:
      'sha256:2000000000000000000000000000000000000000000000000000000000000001',
    closeoutRequirementRefs: ['closeout_requirement.probe_gepa_paid_ladder.v1'],
    expectedArtifactRefs: ['artifact_manifest.expected.probe_gepa_ladder.v1'],
    expectedProofBundleRefs: ['proof_bundle.expected.probe_gepa_ladder.v1'],
    paymentMode,
    probeCommit: 'probe_commit.ebe108d',
    runtimeRef: 'runtime.probe.benchmark_cloud.v1',
    scorerRef: 'scorer.terminal_bench.binary.v1',
    splitRef: 'benchmark_split_manifest.terminal_bench_2.probe_gepa.ladder.v1',
    taskRef: `task.terminal_bench.${suffix}.v1`,
    timeoutBudgetRef: 'timeout_budget.probe.ladder.v1',
    verifierRef: `verifier.terminal_bench.${suffix}.v1`,
  }) as const

const acceptedAssignment = (
  suffix: string,
  workerRef: string,
  closeout: Readonly<{
    paymentMode?: PylonGepaMetricCallPaymentMode
    paymentReceiptRefs?: ReadonlyArray<string>
    settlementReceiptRefs?: ReadonlyArray<string>
  }> = {},
): PylonGepaMetricCallAssignmentRecord => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput(suffix),
    nowIso,
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: `lease.public.probe_gepa_paid_ladder.${suffix}`,
    nowIso: '2026-06-08T12:01:00.000Z',
    workerRef,
  })
  const submitted = submitPylonGepaMetricCallResultRefs(accepted, {
    artifactRefs: [`artifact.public.probe_gepa_paid_ladder.${suffix}`],
    closeoutResultRefs: [
      `closeout.public.probe_gepa_paid_ladder.${suffix}.accepted`,
    ],
    nowIso: '2026-06-08T12:02:00.000Z',
    proofBundleRefs: [`proof.public.probe_gepa_paid_ladder.${suffix}`],
    resourceUsageRefs: [`resource.public.probe_gepa_paid_ladder.${suffix}`],
    verifierResultRefs: [
      `verifier.public.probe_gepa_paid_ladder.${suffix}.accepted`,
    ],
  })

  return closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: 'accepted',
    nowIso: '2026-06-08T12:03:00.000Z',
    ...closeout,
  })
}

const rejectedAssignment = (
  suffix: string,
  workerRef: string,
): PylonGepaMetricCallAssignmentRecord => {
  const created = createPylonGepaMetricCallAssignment(
    assignmentInput(suffix),
    nowIso,
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: `lease.public.probe_gepa_paid_ladder.${suffix}`,
    nowIso: '2026-06-08T12:01:00.000Z',
    workerRef,
  })

  return closePylonGepaMetricCallAssignment(accepted, {
    closeoutDecision: 'rejected',
    closeoutResultRefs: [
      `closeout.public.probe_gepa_paid_ladder.${suffix}.rejected`,
    ],
    nowIso: '2026-06-08T12:03:00.000Z',
  })
}

const accountingFor = (
  assignment: PylonGepaMetricCallAssignmentRecord,
  suffix: string,
): ProbeGepaSettlementAccountingRecord =>
  new ProbeGepaSettlementAccountingRecord({
    accountingRef: `operator_accounting.public.probe_gepa_paid_ladder.${suffix}`,
    assignmentRef: assignment.assignmentRef,
    closeoutResultRefs: assignment.closeoutResultRefs,
    operatorRef: 'operator.public.openagents.probe_gepa',
    paymentReceiptRefs: assignment.paymentReceiptRefs,
    proofBundleRefs: assignment.proofBundleRefs,
    resourceUsageRefs: assignment.resourceUsageRefs,
    schemaVersion: ProbeGepaSettlementAccountingSchemaVersion,
    settlementReceiptRefs: assignment.settlementReceiptRefs,
    verifierResultRefs: assignment.verifierResultRefs,
  })

const stage0Projection = () =>
  projectProbeGepaStage0NoSpendCampaign({
    artanisSummaryRefs: ['summary.public.artanis.probe_gepa_ladder.stage0'],
    campaignRef: 'campaign.probe_gepa.paid_mode_ladder.stage0',
    coordinatorImports: [
      pylonGepaMetricCallCoordinatorImport(
        acceptedAssignment('stage0_accept', 'pylon.public.stage0.one'),
      ),
      pylonGepaMetricCallCoordinatorImport(
        rejectedAssignment('stage0_reject', 'pylon.public.stage0.two'),
      ),
    ],
    probeCloseoutImportRefs: [
      'probe_import.public.probe_gepa_paid_ladder.stage0',
    ],
    psionicImportDryRunRefs: [
      'psionic_import.public.probe_gepa_paid_ladder.stage0.dry_run',
    ],
  })

const ladderFixture = () => {
  const unpaid = acceptedAssignment('unpaid', 'pylon.public.ladder.unpaid')
  const payable = acceptedAssignment('payable', 'pylon.public.ladder.payable', {
    paymentMode: 'payable_pending_settlement',
    paymentReceiptRefs: ['payment_receipt.public.probe_gepa_ladder.payable.1'],
  })
  const settled = acceptedAssignment('settled', 'pylon.public.ladder.settled', {
    paymentMode: 'settled_bitcoin',
    paymentReceiptRefs: ['payment_receipt.public.probe_gepa_ladder.settled.1'],
    settlementReceiptRefs: [
      'settlement_receipt.public.probe_gepa_ladder.settled.1',
    ],
  })
  const unpaidReadiness = evaluateProbeGepaSettlementReadiness({
    accountingRecords: [],
    assignmentRecords: [unpaid],
    batchRef: 'batch.public.probe_gepa_ladder.unpaid',
    operatorAccountingRefs: [],
    publicClaimState: 'no_spend',
    requestedPaymentMode: 'unpaid_smoke',
  })
  const payableReadiness = evaluateProbeGepaSettlementReadiness({
    accountingRecords: [accountingFor(payable, 'payable')],
    assignmentRecords: [payable],
    batchRef: 'batch.public.probe_gepa_ladder.payable',
    operatorAccountingRefs: [
      'operator_accounting_batch.public.probe_gepa_ladder.payable',
    ],
    publicClaimState: 'payable_pending_settlement',
    requestedPaymentMode: 'payable_pending_settlement',
  })
  const settledReadiness = evaluateProbeGepaSettlementReadiness({
    accountingRecords: [accountingFor(settled, 'settled')],
    assignmentRecords: [settled],
    batchRef: 'batch.public.probe_gepa_ladder.settled',
    operatorAccountingRefs: [
      'operator_accounting_batch.public.probe_gepa_ladder.settled',
    ],
    publicClaimState: 'settled_bitcoin',
    requestedPaymentMode: 'settled_bitcoin',
  })

  return {
    payable,
    payableReadiness,
    settled,
    settledReadiness,
    stage0: stage0Projection(),
    unpaid,
    unpaidReadiness,
  }
}

const readyInput = () => {
  const fixture = ladderFixture()

  return {
    bridgeAttempts: [
      new ProbeGepaPaidModeBridgeAttempt({
        assignmentRef: fixture.payable.assignmentRef,
        bridgeAttemptRef: 'bridge_attempt.public.probe_gepa_ladder.payable.1',
        decision: 'accepted',
        denialRefs: [],
        idempotencyKeyRef: 'idempotency.public.probe_gepa_ladder.payable.1',
        paymentReceiptRefs: fixture.payable.paymentReceiptRefs,
        replayOfAttemptRef: null,
        requestedPaymentMode: 'payable_pending_settlement',
        settlementReceiptRefs: [],
      }),
      new ProbeGepaPaidModeBridgeAttempt({
        assignmentRef: fixture.settled.assignmentRef,
        bridgeAttemptRef: 'bridge_attempt.public.probe_gepa_ladder.settled.1',
        decision: 'accepted',
        denialRefs: [],
        idempotencyKeyRef: 'idempotency.public.probe_gepa_ladder.settled.1',
        paymentReceiptRefs: fixture.settled.paymentReceiptRefs,
        replayOfAttemptRef: null,
        requestedPaymentMode: 'settled_bitcoin',
        settlementReceiptRefs: fixture.settled.settlementReceiptRefs,
      }),
      new ProbeGepaPaidModeBridgeAttempt({
        assignmentRef: fixture.settled.assignmentRef,
        bridgeAttemptRef:
          'bridge_attempt.public.probe_gepa_ladder.settled.replay',
        decision: 'duplicate_replay',
        denialRefs: ['denial.public.probe_gepa_ladder.duplicate_replay'],
        idempotencyKeyRef: 'idempotency.public.probe_gepa_ladder.settled.1',
        paymentReceiptRefs: [],
        replayOfAttemptRef: 'bridge_attempt.public.probe_gepa_ladder.settled.1',
        requestedPaymentMode: 'settled_bitcoin',
        settlementReceiptRefs: [],
      }),
    ],
    campaignRef: 'campaign.public.probe_gepa_ladder.paid_modes',
    coordinatorImports: [
      pylonGepaMetricCallCoordinatorImport(fixture.unpaid),
      pylonGepaMetricCallCoordinatorImport(fixture.payable),
      pylonGepaMetricCallCoordinatorImport(fixture.settled),
    ],
    liveSmallSatsSmokeRefs: ['smoke.public.probe_gepa_ladder.small_sats.1'],
    sendReadiness: new ProbeGepaPaidModeSendReadiness({
      denialRefs: [],
      outboundLiquidityReady: true,
      payerWalletReady: true,
      sendPreflightRefs: ['send_preflight.public.probe_gepa_ladder.ready.1'],
    }),
    settlementReadinessResults: [
      fixture.unpaidReadiness,
      fixture.payableReadiness,
      fixture.settledReadiness,
    ],
    stage0Projection: fixture.stage0,
  }
}

describe('Probe GEPA paid-mode campaign ladder', () => {
  test('projects a receipt-backed campaign from no-spend to payable and settled bitcoin', () => {
    const projection = projectProbeGepaPaidModeCampaignLadder(readyInput())

    expect(
      S.decodeUnknownSync(ProbeGepaPaidModeCampaignLadderProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      aggregatePaymentMode: 'settled_bitcoin',
      duplicateReplayDoubleSettlementBlocked: true,
      ladderState: 'settled_bitcoin_ready',
      paidPylonWorkClaimAllowed: true,
      payablePendingSettlementClaimAllowed: true,
      sendReadinessReady: true,
      settledBitcoinCampaignClaimAllowed: true,
      stage0Green: true,
      unpaidSmokeClaimAllowed: true,
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.paymentReceiptRefs).toEqual([
      'payment_receipt.public.probe_gepa_ladder.payable.1',
      'payment_receipt.public.probe_gepa_ladder.settled.1',
    ])
    expect(projection.publicCopySettlementReceiptRefs).toEqual([
      'settlement_receipt.public.probe_gepa_ladder.settled.1',
    ])
    expect(
      projection.assignmentPaymentModes.map(assignment => [
        assignment.assignmentRef,
        assignment.paymentMode,
      ]),
    ).toEqual([
      [
        'assignment.public.probe_gepa_paid_ladder.payable',
        'payable_pending_settlement',
      ],
      ['assignment.public.probe_gepa_paid_ladder.settled', 'settled_bitcoin'],
      ['assignment.public.probe_gepa_paid_ladder.unpaid', 'unpaid_smoke'],
    ])
  })

  test('stops at payable mode when settled-bitcoin readiness is missing', () => {
    const input = readyInput()
    const projection = projectProbeGepaPaidModeCampaignLadder({
      ...input,
      settlementReadinessResults: input.settlementReadinessResults.filter(
        result => result.requestedPaymentMode !== 'settled_bitcoin',
      ),
    })

    expect(projection).toMatchObject({
      aggregatePaymentMode: 'payable_pending_settlement',
      ladderState: 'payable_pending_settlement_ready',
      paidPylonWorkClaimAllowed: true,
      settledBitcoinCampaignClaimAllowed: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa.paid_ladder.settled_bitcoin_readiness_missing',
    )
  })

  test('blocks duplicate settlement attempts that would mint new receipts', () => {
    const input = readyInput()
    const projection = projectProbeGepaPaidModeCampaignLadder({
      ...input,
      bridgeAttempts: [
        ...(input.bridgeAttempts ?? []),
        new ProbeGepaPaidModeBridgeAttempt({
          assignmentRef: 'assignment.public.probe_gepa_paid_ladder.settled',
          bridgeAttemptRef:
            'bridge_attempt.public.probe_gepa_ladder.settled.double_spend',
          decision: 'accepted',
          denialRefs: [],
          idempotencyKeyRef:
            'idempotency.public.probe_gepa_ladder.settled.double_spend',
          paymentReceiptRefs: [
            'payment_receipt.public.probe_gepa_ladder.settled.duplicate',
          ],
          replayOfAttemptRef: null,
          requestedPaymentMode: 'settled_bitcoin',
          settlementReceiptRefs: [
            'settlement_receipt.public.probe_gepa_ladder.settled.duplicate',
          ],
        }),
      ],
    })

    expect(projection.settledBitcoinCampaignClaimAllowed).toBe(false)
    expect(projection.duplicateReplayDoubleSettlementBlocked).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa.paid_ladder.duplicate_settlement_assignment.assignment.public.probe_gepa_paid_ladder.settled',
    )
  })

  test('blocks settled-bitcoin claims when wallet send-readiness or liquidity is denied', () => {
    const projection = projectProbeGepaPaidModeCampaignLadder({
      ...readyInput(),
      sendReadiness: new ProbeGepaPaidModeSendReadiness({
        denialRefs: ['denial.public.probe_gepa_ladder.no_outbound_liquidity'],
        outboundLiquidityReady: false,
        payerWalletReady: true,
        sendPreflightRefs: ['send_preflight.public.probe_gepa_ladder.denied.1'],
      }),
    })

    expect(projection).toMatchObject({
      aggregatePaymentMode: 'payable_pending_settlement',
      ladderState: 'payable_pending_settlement_ready',
      sendReadinessReady: false,
      settledBitcoinCampaignClaimAllowed: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.probe_gepa.paid_ladder.send_readiness_or_liquidity_missing',
    )
  })

  test('rejects private payment and wallet material in public ladder refs', () => {
    expect(() =>
      projectProbeGepaPaidModeCampaignLadder({
        ...readyInput(),
        bridgeAttempts: [
          new ProbeGepaPaidModeBridgeAttempt({
            assignmentRef: 'assignment.public.probe_gepa_paid_ladder.settled',
            bridgeAttemptRef:
              'bridge_attempt.public.probe_gepa_ladder.settled.private',
            decision: 'accepted',
            denialRefs: [],
            idempotencyKeyRef:
              'idempotency.public.probe_gepa_ladder.settled.private',
            paymentReceiptRefs: ['payment_preimage.private'],
            replayOfAttemptRef: null,
            requestedPaymentMode: 'settled_bitcoin',
            settlementReceiptRefs: [
              'settlement_receipt.public.probe_gepa_ladder.settled.private',
            ],
          }),
        ],
      }),
    ).toThrow(ProbeGepaPaidModeCampaignLadderUnsafe)
  })
})
