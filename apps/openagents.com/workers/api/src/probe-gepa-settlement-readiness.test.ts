import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  acceptPylonGepaMetricCallAssignment,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'
import {
  ProbeGepaSettlementAccountingRecord,
  ProbeGepaSettlementAccountingSchemaVersion,
  ProbeGepaSettlementReadinessInput,
  ProbeGepaSettlementReadinessResult,
  ProbeGepaSettlementReadinessUnsafe,
  evaluateProbeGepaSettlementReadiness,
  probeGepaSettlementAssignmentModeReady,
} from './probe-gepa-settlement-readiness'

const nowIso = '2026-06-08T12:00:00.000Z'

const acceptedAssignment = () => {
  const created = createPylonGepaMetricCallAssignment(
    {
      assignmentRef:
        'assignment.public.pylon_gepa_metric_call.stage_0.configure_git_webserver.settlement',
      backendProfileRef: 'backend_profile.probe.apple_fm.local.v1',
      benchmarkSuiteRef: 'benchmark_suite.terminal_bench_2.harbor.retained.v1',
      campaignId: 'probe_gepa.terminal_bench.stage_0_1',
      candidateHash:
        'sha256:1000000000000000000000000000000000000000000000000000000000000001',
      closeoutRequirementRefs: ['probe.benchmark_closeout.v1'],
      expectedArtifactRefs: ['openagents.benchmark_artifact_manifest.v1'],
      expectedProofBundleRefs: ['openagents.benchmark_proof_bundle.v1'],
      paymentMode: 'unpaid_smoke',
      probeCommit: 'probe_commit.ebe108d',
      runtimeRef: 'runtime.probe.benchmark_cloud.v1',
      scorerRef: 'scorer.terminal_bench.binary.v1',
      splitRef:
        'benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1',
      taskRef: 'task.terminal_bench.configure-git-webserver.v1',
      timeoutBudgetRef: 'timeout_budget.probe.retained_smoke.v1',
      verifierRef: 'verifier.terminal_bench.configure_git_webserver.v1',
    },
    nowIso,
  )
  const accepted = acceptPylonGepaMetricCallAssignment(created, {
    leaseRef: 'lease.public.pylon_gepa_metric_call.settlement.1',
    nowIso: '2026-06-08T12:01:00.000Z',
    workerRef: 'pylon.public.demo.alpha',
  })
  const submitted = submitPylonGepaMetricCallResultRefs(accepted, {
    artifactRefs: ['artifact_manifest.probe.configure_git_webserver.1'],
    closeoutResultRefs: [
      'probe_closeout.probe_run.configure_git_webserver.1',
    ],
    nowIso: '2026-06-08T12:02:00.000Z',
    proofBundleRefs: ['proof_bundle.probe.configure_git_webserver.1'],
    resourceUsageRefs: ['resource_usage.probe.configure_git_webserver.1'],
    verifierResultRefs: ['verifier_result.configure_git_webserver.failed.1'],
  })

  return closePylonGepaMetricCallAssignment(submitted, {
    closeoutDecision: 'accepted',
    nowIso: '2026-06-08T12:03:00.000Z',
  })
}

const accountingRecord = (
  overrides: Partial<ProbeGepaSettlementAccountingRecord> = {},
): ProbeGepaSettlementAccountingRecord => {
  const assignment = acceptedAssignment()

  return new ProbeGepaSettlementAccountingRecord({
    accountingRef:
      'operator_accounting.probe_gepa.stage_0.configure_git_webserver.1',
    assignmentRef: assignment.assignmentRef,
    closeoutResultRefs: assignment.closeoutResultRefs,
    operatorRef: 'operator.public.shc.benchmark_pool',
    paymentReceiptRefs: ['credit_receipt.public.probe_gepa.stage_0.1'],
    proofBundleRefs: assignment.proofBundleRefs,
    resourceUsageRefs: assignment.resourceUsageRefs,
    schemaVersion: ProbeGepaSettlementAccountingSchemaVersion,
    settlementReceiptRefs: [],
    verifierResultRefs: assignment.verifierResultRefs,
    ...overrides,
  })
}

const input = (
  overrides: Partial<ProbeGepaSettlementReadinessInput> = {},
): ProbeGepaSettlementReadinessInput =>
  new ProbeGepaSettlementReadinessInput({
    accountingRecords: [],
    assignmentRecords: [acceptedAssignment()],
    batchRef: 'batch.probe_gepa.stage_0.settlement_smoke.1',
    operatorAccountingRefs: [],
    publicClaimState: 'no_spend',
    requestedPaymentMode: 'unpaid_smoke',
    ...overrides,
  })

describe('Probe GEPA settlement readiness', () => {
  test('allows no-spend batches without payment or settlement refs', () => {
    const result = evaluateProbeGepaSettlementReadiness(input())

    expect(S.decodeUnknownSync(ProbeGepaSettlementReadinessResult)(result)).toEqual(
      result,
    )
    expect(result).toMatchObject({
      decision: 'ready',
      noSpendBatchClaimAllowed: true,
      payableWorkClaimAllowed: false,
      publicSummaryLabel: 'unpaid smoke complete; no payout claim',
      readinessState: 'unpaid_smoke_complete',
      requestedPaymentMode: 'unpaid_smoke',
      settledBitcoinClaimAllowed: false,
    })
  })

  test('requires accounting refs before operator credit readiness', () => {
    const missingAccounting = evaluateProbeGepaSettlementReadiness(
      input({
        publicClaimState: 'operator_credit',
        requestedPaymentMode: 'operator_credit',
      }),
    )

    expect(missingAccounting.decision).toBe('rejected')
    expect(missingAccounting.blockerRefs).toContain(
      'blocker.probe_gepa.settlement.operator_accounting_refs_missing',
    )
    expect(missingAccounting.blockerRefs).toContain(
      'blocker.probe_gepa.settlement.accounting_missing.assignment.public.pylon_gepa_metric_call.stage_0.configure_git_webserver.settlement',
    )

    const ready = evaluateProbeGepaSettlementReadiness(
      input({
        accountingRecords: [accountingRecord()],
        operatorAccountingRefs: [
          'operator_accounting_batch.probe_gepa.stage_0.credit.1',
        ],
        publicClaimState: 'operator_credit',
        requestedPaymentMode: 'operator_credit',
      }),
    )

    expect(ready).toMatchObject({
      decision: 'ready',
      payableWorkClaimAllowed: false,
      publicSummaryLabel: 'operator credit recorded; no bitcoin settlement claim',
      readinessState: 'operator_credit_ready',
      settledBitcoinClaimAllowed: false,
    })
    expect(probeGepaSettlementAssignmentModeReady(ready, 'operator_credit')).toBe(
      true,
    )
  })

  test('requires payment receipts before payable pending settlement readiness', () => {
    const result = evaluateProbeGepaSettlementReadiness(
      input({
        accountingRecords: [accountingRecord({ paymentReceiptRefs: [] })],
        operatorAccountingRefs: [
          'operator_accounting_batch.probe_gepa.stage_0.payable.1',
        ],
        publicClaimState: 'payable_pending_settlement',
        requestedPaymentMode: 'payable_pending_settlement',
      }),
    )

    expect(result.decision).toBe('rejected')
    expect(result.blockerRefs).toContain(
      'blocker.probe_gepa.settlement.payment_receipt_missing.assignment.public.pylon_gepa_metric_call.stage_0.configure_git_webserver.settlement',
    )
  })

  test('requires settlement receipts before settled bitcoin readiness', () => {
    const missingSettlement = evaluateProbeGepaSettlementReadiness(
      input({
        accountingRecords: [accountingRecord()],
        operatorAccountingRefs: [
          'operator_accounting_batch.probe_gepa.stage_0.settled.1',
        ],
        publicClaimState: 'settled_bitcoin',
        requestedPaymentMode: 'settled_bitcoin',
      }),
    )

    expect(missingSettlement.decision).toBe('rejected')
    expect(missingSettlement.blockerRefs).toContain(
      'blocker.probe_gepa.settlement.settlement_receipt_missing.assignment.public.pylon_gepa_metric_call.stage_0.configure_git_webserver.settlement',
    )

    const settled = evaluateProbeGepaSettlementReadiness(
      input({
        accountingRecords: [
          accountingRecord({
            settlementReceiptRefs: [
              'settlement_receipt.public.probe_gepa.stage_0.bitcoin.1',
            ],
          }),
        ],
        operatorAccountingRefs: [
          'operator_accounting_batch.probe_gepa.stage_0.settled.1',
        ],
        publicClaimState: 'settled_bitcoin',
        requestedPaymentMode: 'settled_bitcoin',
      }),
    )

    expect(settled).toMatchObject({
      decision: 'ready',
      payableWorkClaimAllowed: true,
      readinessState: 'settled_bitcoin_ready',
      settledBitcoinClaimAllowed: true,
    })
  })

  test('rejects public overclaims and unsafe settlement refs', () => {
    const overclaim = evaluateProbeGepaSettlementReadiness(
      input({
        publicClaimState: 'settled_bitcoin',
        requestedPaymentMode: 'operator_credit',
      }),
    )

    expect(overclaim.decision).toBe('rejected')
    expect(overclaim.blockerRefs).toContain(
      'blocker.probe_gepa.settlement.public_claim_overstates_mode',
    )

    expect(() =>
      evaluateProbeGepaSettlementReadiness(
        input({
          accountingRecords: [
            accountingRecord({
              paymentReceiptRefs: ['payment_preimage.private'],
            }),
          ],
          operatorAccountingRefs: [
            'operator_accounting_batch.probe_gepa.stage_0.private.1',
          ],
          publicClaimState: 'operator_credit',
          requestedPaymentMode: 'operator_credit',
        }),
      ),
    ).toThrow(ProbeGepaSettlementReadinessUnsafe)
  })
})
