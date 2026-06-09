import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonGepaMetricCallAssignmentRecord,
  PylonGepaMetricCallAssignmentUnsafe,
  PylonGepaMetricCallCoordinatorImport,
  acceptPylonGepaMetricCallAssignment,
  closePylonGepaMetricCallAssignment,
  createPylonGepaMetricCallAssignment,
  pylonGepaMetricCallAcceptedWorkClaimAllowed,
  pylonGepaMetricCallCoordinatorImport,
  pylonGepaMetricCallPayableWorkClaimAllowed,
  pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed,
  reportPylonGepaMetricCallProgress,
  submitPylonGepaMetricCallResultRefs,
} from './pylon-gepa-metric-call-assignments'

const nowIso = '2026-06-08T12:00:00.000Z'

const assignmentInput = () =>
  ({
    assignmentRef:
      'assignment.public.pylon_gepa_metric_call.stage_0.configure_git_webserver.a2a44c21',
    backendProfileRef: 'backend_profile.probe.apple_fm.local.v1',
    benchmarkSuiteRef: 'benchmark_suite.terminal_bench_2.harbor.retained.v1',
    campaignId: 'probe_gepa.terminal_bench.stage_0_1',
    candidateHash:
      'sha256:a2a44c21a08fcba12108786821dc5045a746e72b0d5a7f45374b08f8ba6a6743',
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
  }) as const

const acceptedAssignment = () =>
  acceptPylonGepaMetricCallAssignment(
    createPylonGepaMetricCallAssignment(assignmentInput(), nowIso),
    {
      leaseRef: 'lease.public.pylon_gepa_metric_call.configure_git_webserver.1',
      nowIso: '2026-06-08T12:01:00.000Z',
      workerRef: 'pylon.public.shc_box_1',
    },
  )

const submittedAssignment = () =>
  submitPylonGepaMetricCallResultRefs(
    reportPylonGepaMetricCallProgress(acceptedAssignment(), {
      nowIso: '2026-06-08T12:02:00.000Z',
      progressRefs: ['progress.public.probe.closeout_started'],
    }),
    {
      artifactRefs: ['artifact_manifest.probe.configure_git_webserver.1'],
      closeoutResultRefs: [
        'probe_closeout.probe_run.configure_git_webserver.1',
      ],
      nowIso: '2026-06-08T12:03:00.000Z',
      proofBundleRefs: ['proof_bundle.probe.configure_git_webserver.1'],
      resourceUsageRefs: ['resource_usage.probe.configure_git_webserver.1'],
      verifierResultRefs: ['verifier_result.configure_git_webserver.failed.1'],
    },
  )

describe('Pylon GEPA metric-call assignment lifecycle', () => {
  test('creates and accepts an explicit unpaid smoke GEPA metric-call assignment', () => {
    const created = createPylonGepaMetricCallAssignment(
      assignmentInput(),
      nowIso,
    )
    const accepted = acceptedAssignment()

    expect(
      S.decodeUnknownSync(PylonGepaMetricCallAssignmentRecord)(created),
    ).toEqual(created)
    expect(created).toMatchObject({
      closeoutDecision: 'open',
      paymentMode: 'unpaid_smoke',
      state: 'created',
      workerRef: null,
    })
    expect(accepted).toMatchObject({
      leaseRef: 'lease.public.pylon_gepa_metric_call.configure_git_webserver.1',
      state: 'accepted',
      workerRef: 'pylon.public.shc_box_1',
    })
    expect(pylonGepaMetricCallAcceptedWorkClaimAllowed(accepted)).toBe(false)
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(accepted)).toBe(false)
    expect(pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(accepted)).toBe(
      false,
    )
  })

  test('reports progress, submits artifact refs, and closes accepted work for GEPA import', () => {
    const submitted = submittedAssignment()
    const closed = closePylonGepaMetricCallAssignment(submitted, {
      closeoutDecision: 'accepted',
      nowIso: '2026-06-08T12:04:00.000Z',
    })
    const coordinatorImport = pylonGepaMetricCallCoordinatorImport(closed)

    expect(closed).toMatchObject({
      closeoutDecision: 'accepted',
      paymentMode: 'unpaid_smoke',
      state: 'accepted_work',
    })
    expect(pylonGepaMetricCallAcceptedWorkClaimAllowed(closed)).toBe(true)
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(closed)).toBe(false)
    expect(pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(closed)).toBe(
      false,
    )
    expect(
      S.decodeUnknownSync(PylonGepaMetricCallCoordinatorImport)(
        coordinatorImport,
      ),
    ).toEqual(coordinatorImport)
    expect(coordinatorImport).toMatchObject({
      acceptedWorkClaimAllowed: true,
      assignmentRef: assignmentInput().assignmentRef,
      candidateHash: assignmentInput().candidateHash,
      noSpendEvidenceRefs: [
        'evidence.public.pylon_gepa_metric_call.no_spend_requested',
      ],
      payableWorkClaimAllowed: false,
      settledBitcoinPayoutClaimAllowed: false,
      taskRef: assignmentInput().taskRef,
      verifierResultRefs: ['verifier_result.configure_git_webserver.failed.1'],
    })
  })

  test('closes rejected work without accepted-work or settled-payout claims', () => {
    const rejected = closePylonGepaMetricCallAssignment(acceptedAssignment(), {
      closeoutDecision: 'rejected',
      closeoutResultRefs: ['probe_closeout.rejected.operator_evaluator.1'],
      nowIso: '2026-06-08T12:04:00.000Z',
    })

    expect(rejected.state).toBe('rejected_work')
    expect(rejected.paymentMode).toBe('rejected_no_pay')
    expect(pylonGepaMetricCallAcceptedWorkClaimAllowed(rejected)).toBe(false)
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(rejected)).toBe(false)
    expect(pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(rejected)).toBe(
      false,
    )
  })

  test('requires submitted artifact proof refs before accepted closeout', () => {
    expect(() =>
      closePylonGepaMetricCallAssignment(acceptedAssignment(), {
        closeoutDecision: 'accepted',
        nowIso: '2026-06-08T12:04:00.000Z',
      }),
    ).toThrow(PylonGepaMetricCallAssignmentUnsafe)
  })

  test('distinguishes payable pending settlement from settled bitcoin payout', () => {
    expect(() =>
      closePylonGepaMetricCallAssignment(submittedAssignment(), {
        closeoutDecision: 'accepted',
        nowIso: '2026-06-08T12:04:00.000Z',
        settlementReceiptRefs: ['settlement.public.pylon_gepa_metric_call.1'],
      }),
    ).toThrow(PylonGepaMetricCallAssignmentUnsafe)

    const payable = closePylonGepaMetricCallAssignment(submittedAssignment(), {
      closeoutDecision: 'accepted',
      nowIso: '2026-06-08T12:04:00.000Z',
      paymentMode: 'payable_pending_settlement',
      paymentReceiptRefs: ['payment_receipt.public.pylon_gepa_metric_call.1'],
    })

    expect(pylonGepaMetricCallAcceptedWorkClaimAllowed(payable)).toBe(true)
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(payable)).toBe(true)
    expect(pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(payable)).toBe(
      false,
    )

    const paidCreated = createPylonGepaMetricCallAssignment(
      {
        ...assignmentInput(),
        assignmentRef: 'assignment.public.pylon_gepa_metric_call.paid.1',
        paymentMode: 'payable_pending_settlement',
      },
      nowIso,
    )
    const paidAccepted = acceptPylonGepaMetricCallAssignment(paidCreated, {
      leaseRef: 'lease.public.pylon_gepa_metric_call.paid.1',
      nowIso: '2026-06-08T12:01:00.000Z',
      workerRef: 'pylon.public.shc_box_1',
    })
    const paidSubmitted = submitPylonGepaMetricCallResultRefs(paidAccepted, {
      artifactRefs: ['artifact_manifest.probe.configure_git_webserver.1'],
      closeoutResultRefs: [
        'probe_closeout.probe_run.configure_git_webserver.1',
      ],
      nowIso: '2026-06-08T12:03:00.000Z',
      proofBundleRefs: ['proof_bundle.probe.configure_git_webserver.1'],
      resourceUsageRefs: ['resource_usage.probe.configure_git_webserver.1'],
      verifierResultRefs: ['verifier_result.configure_git_webserver.failed.1'],
    })
    const paidClosed = closePylonGepaMetricCallAssignment(paidSubmitted, {
      closeoutDecision: 'accepted',
      nowIso: '2026-06-08T12:04:00.000Z',
      paymentMode: 'settled_bitcoin',
      paymentReceiptRefs: ['payment_receipt.public.pylon_gepa_metric_call.1'],
      settlementReceiptRefs: ['settlement.public.pylon_gepa_metric_call.1'],
    })

    expect(pylonGepaMetricCallAcceptedWorkClaimAllowed(paidClosed)).toBe(true)
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(paidClosed)).toBe(true)
    expect(
      pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(paidClosed),
    ).toBe(true)
  })

  test('requires receipts for operator credit and payable modes', () => {
    expect(() =>
      closePylonGepaMetricCallAssignment(submittedAssignment(), {
        closeoutDecision: 'accepted',
        nowIso: '2026-06-08T12:04:00.000Z',
        paymentMode: 'operator_credit',
      }),
    ).toThrow(PylonGepaMetricCallAssignmentUnsafe)

    const credited = closePylonGepaMetricCallAssignment(submittedAssignment(), {
      closeoutDecision: 'accepted',
      nowIso: '2026-06-08T12:04:00.000Z',
      paymentMode: 'operator_credit',
      paymentReceiptRefs: ['credit_receipt.public.pylon_gepa_metric_call.1'],
    })

    expect(credited.paymentMode).toBe('operator_credit')
    expect(pylonGepaMetricCallPayableWorkClaimAllowed(credited)).toBe(false)
    expect(pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(credited)).toBe(
      false,
    )
  })

  test('rejects private refs, raw logs, provider secrets, and payment material', () => {
    expect(() =>
      createPylonGepaMetricCallAssignment(
        {
          ...assignmentInput(),
          expectedArtifactRefs: ['raw_runner_log.private'],
        },
        nowIso,
      ),
    ).toThrow(PylonGepaMetricCallAssignmentUnsafe)

    expect(() =>
      acceptPylonGepaMetricCallAssignment(
        createPylonGepaMetricCallAssignment(assignmentInput(), nowIso),
        {
          leaseRef: 'lease.public.safe',
          nowIso: '2026-06-08T12:01:00.000Z',
          workerRef: 'provider_token.raw_secret',
        },
      ),
    ).toThrow(PylonGepaMetricCallAssignmentUnsafe)
  })
})
