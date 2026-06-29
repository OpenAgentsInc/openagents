import { describe, expect, it } from 'vitest'

import {
  buildTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
} from './training-verification'
import {
  buildTassadarExecutorTraceDispatch,
  buildTassadarExecutorTracePayload,
  TassadarBoundedProfileRef,
  TassadarExecutorTraceJobKind,
  TassadarExecutorTraceUnsafeProjectionError,
  tassadarExecutorTraceDisclosureChecklist,
  tassadarExecutorTraceReadiness,
  tassadarExecutorTraceVerificationChallengeRequest,
} from './tassadar-executor-trace-homework'

const closeout = {
  assignmentRef: 'assignment.tassadar.article_closeout.1',
  pylonDeviceRef: 'pylon.device.worker.m2',
  replayDigestRef: 'digest.trace.window.abc',
  sampledWindow: { endStep: 120, startStep: 100 },
  sampledWindowRef: 'trace.window.tassadar.100_120',
  traceCommitmentDigestRef: 'digest.trace.window.abc',
  validatorDeviceRef: 'pylon.device.validator.intel',
  workerReceiptRef: 'receipt.tassadar.no_spend.1',
  workloadFamily: 'article_closeout' as const,
}

describe('tassadar executor-trace homework', () => {
  it('builds an internal no-spend dispatch payload without public capability copy', () => {
    const dispatch = buildTassadarExecutorTraceDispatch({
      amountMsats: 0,
      assignmentRef: 'assignment.tassadar.article_closeout.1',
      idempotencyKeyHash: 'sha256.tassadar-article-closeout-1',
      jobId: 'job.tassadar.executor_trace.1',
      nowIso: '2026-06-10T12:00:00.000Z',
      providerPubkeys: ['provider.tassadar.no_spend'],
      workloadFamily: 'article_closeout',
    })
    const content = JSON.parse(dispatch.content)

    expect(content).toMatchObject({
      boundedProfileRef: TassadarBoundedProfileRef,
      jobKind: TassadarExecutorTraceJobKind,
      publicAcceptanceVerdictProjectionAllowed: false,
      publicCapabilityCopyAllowed: false,
      verificationClass: 'exact_trace_replay',
    })

    expect(tassadarExecutorTraceDisclosureChecklist()).toMatchObject({
      agentsCapabilityClaimAllowed: false,
      marketingCopyAllowed: false,
      publicAcceptanceVerdictProjectionAllowed: false,
      publicCapabilityCopyAllowed: false,
      registryCapabilityEditAllowed: false,
    })
  })

  it('creates an exact_trace_replay challenge that verifies sampled-window replay', async () => {
    const request = tassadarExecutorTraceVerificationChallengeRequest({
      closeout,
      trainingRunRef: 'training.run.tassadar.executor_trace',
      windowRef: 'window.tassadar.executor_trace.1',
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'tassadar-challenge-1',
      nowIso: '2026-06-10T12:01:00.000Z',
      request,
    }).challenge

    expect(request).toMatchObject({
      homeworkKind: TassadarExecutorTraceJobKind,
      samplingPolicy: 'per_contribution',
      verificationClass: 'exact_trace_replay',
    })

    await expect(runTrainingVerificationClass({ challenge })).resolves.toMatchObject({
      failureCodes: [],
      state: 'Verified',
    })
  })

  it('rejects same-device replay and mismatched trace digest closeouts', async () => {
    expect(() =>
      tassadarExecutorTraceVerificationChallengeRequest({
        closeout: {
          ...closeout,
          validatorDeviceRef: closeout.pylonDeviceRef,
        },
        trainingRunRef: 'training.run.tassadar.executor_trace',
        windowRef: 'window.tassadar.executor_trace.1',
      }),
    ).toThrow(TassadarExecutorTraceUnsafeProjectionError)

    const request = tassadarExecutorTraceVerificationChallengeRequest({
      closeout: {
        ...closeout,
        replayDigestRef: 'digest.trace.window.mismatch',
      },
      trainingRunRef: 'training.run.tassadar.executor_trace',
      windowRef: 'window.tassadar.executor_trace.1',
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'tassadar-challenge-2',
      nowIso: '2026-06-10T12:02:00.000Z',
      request,
    }).challenge

    await expect(runTrainingVerificationClass({ challenge })).resolves.toMatchObject({
      failureCodes: ['ExecutorTraceMismatch'],
      state: 'Rejected',
    })
  })

  it('keeps paid settlement blocked until operator-funded receipt evidence exists', () => {
    expect(tassadarExecutorTraceReadiness()).toEqual({
      blockerRefs: [
        'blocker.tassadar.requires_psionic_connector_4664_live_endpoint',
        'blocker.tassadar.requires_separate_device_replay_receipt',
        'blocker.tassadar.requires_operator_funded_paid_closeout',
        'blocker.tassadar.public_acceptance_verdicts_suppressed_by_policy',
      ],
      dispatchableJobKind: TassadarExecutorTraceJobKind,
      noSpendDispatchReady: true,
      paidSettlementReady: false,
      separateDeviceReplayRequired: true,
      verificationClass: 'exact_trace_replay',
    })
  })

  it('blocks secret-like material from the internal dispatch payload', () => {
    expect(() =>
      buildTassadarExecutorTracePayload({
        assignmentRef: 'assignment.tassadar.wallet_path.leak',
        workloadFamily: 'article_closeout',
      }),
    ).toThrow(TassadarExecutorTraceUnsafeProjectionError)
  })
})
