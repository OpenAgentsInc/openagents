import { describe, expect, it } from 'vitest'

import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'
import {
  Cs336A5GradingJobKind,
  Cs336A5RolloutJobKind,
  Cs336A5SftPackingJobKind,
  buildCs336A5HomeworkPayload,
  cs336A5NoSpendReadiness,
  cs336A5VerificationChallengeRequest,
  publicCs336A5EvalProjection,
} from './cs336-a5-alignment-homework'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

describe('CS336 A5 alignment homework', () => {
  it('builds dispatchable rollout, grading, and SFT packing job contracts', () => {
    expect(
      buildCs336A5HomeworkPayload({
        assignmentRef: 'assignment.cs336.a5.rollout.1',
        jobKind: Cs336A5RolloutJobKind,
      }),
    ).toMatchObject({
      jobKind: 'cs336_a5_rollout_batch',
      psionicLaneRef: 'psion_cs336_a5_alignment_reference_v1',
      updateBoundaryRef: 'issue.github.openagents.4669',
      verificationClass: 'seeded_replication',
    })
    expect(
      buildCs336A5HomeworkPayload({
        assignmentRef: 'assignment.cs336.a5.grading.1',
        jobKind: Cs336A5GradingJobKind,
      }),
    ).toMatchObject({
      jobKind: 'cs336_a5_reward_grading',
      verificationClass: 'deterministic_recompute',
    })
    expect(
      buildCs336A5HomeworkPayload({
        assignmentRef: 'assignment.cs336.a5.sft.1',
        jobKind: Cs336A5SftPackingJobKind,
      }),
    ).toMatchObject({
      jobKind: 'cs336_a5_sft_packing',
      verificationClass: 'deterministic_recompute',
    })
    expect(cs336A5NoSpendReadiness().jobKinds).toHaveLength(3)
  })

  it('creates verification requests for seeded rollouts and recomputed grading', () => {
    expect(
      cs336A5VerificationChallengeRequest({
        closeout: {
          assignmentRef: 'assignment.cs336.a5.rollout.1',
          jobKind: Cs336A5RolloutJobKind,
          outputDigestRef: 'digest.cs336.a5.rollout.output',
          replicatedDigestRef: 'digest.cs336.a5.rollout.output',
          workerReceiptRef: 'receipt.cs336.a5.rollout.worker',
        },
        trainingRunRef: 'training.run.cs336.a5',
        windowRef: 'training.window.cs336.a5.1',
      }),
    ).toMatchObject({
      payload: {
        expectedDigestRef: 'digest.cs336.a5.rollout.output',
        replicatedDigestRef: 'digest.cs336.a5.rollout.output',
      },
      samplingPolicy: 'aggregate',
      verificationClass: 'seeded_replication',
    })
    expect(
      cs336A5VerificationChallengeRequest({
        closeout: {
          assignmentRef: 'assignment.cs336.a5.grading.1',
          jobKind: Cs336A5GradingJobKind,
          outputDigestRef: 'digest.cs336.a5.grading.output',
          recomputedDigestRef: 'digest.cs336.a5.grading.output',
          workerReceiptRef: 'receipt.cs336.a5.grading.worker',
        },
        trainingRunRef: 'training.run.cs336.a5',
        windowRef: 'training.window.cs336.a5.2',
      }),
    ).toMatchObject({
      payload: {
        expectedDigestRef: 'digest.cs336.a5.grading.output',
        recomputedDigestRef: 'digest.cs336.a5.grading.output',
      },
      samplingPolicy: 'per_contribution',
      verificationClass: 'deterministic_recompute',
    })
  })

  it('projects public eval suites as scoped eval evidence', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a5',
      nowIso: '2026-06-10T13:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a5.eval',
      },
    })
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        a5Alignment: {
          evalSuites: [
            {
              evalSuiteRef: 'eval.cs336.a5.gsm8k.seeded.1',
              metric: 'accuracy',
              receiptRefs: ['receipt.cs336.a5.gsm8k.1'],
              sampleCount: 100,
              score: 0.42,
              sourceRefs: ['artifact.cs336.a5.gsm8k.summary.1'],
              splitRef: 'gsm8k.test.public_summary',
              taskSetRef: 'gsm8k',
              verificationRefs: ['challenge.cs336.a5.gsm8k.1'],
              verifiedSampleCount: 100,
            },
          ],
        },
      }),
    }
    const window = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T13:00:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: run.trainingRunRef,
        windowRef: 'training.window.cs336.a5.eval.1',
      },
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'challenge',
      nowIso: '2026-06-10T13:01:00.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a5.gsm8k.1'],
        contributionRef: 'eval.cs336.a5.gsm8k.seeded.1',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          expectedDigestRef: 'digest.cs336.a5.gsm8k.1',
          recomputedDigestRef: 'digest.cs336.a5.gsm8k.1',
        },
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'deterministic_recompute',
        windowRef: window.windowRef,
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: 'lease',
      nowIso: '2026-06-10T13:02:00.000Z',
      request: { validatorRef: 'validator.cs336.a5' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'final',
      nowIso: '2026-06-10T13:03:00.000Z',
      request: { receiptRefs: ['receipt.cs336.a5.verdict.1'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336.a5.gsm8k.1'],
      },
    }).challenge
    const projection = publicCs336A5EvalProjection({
      challenges: [verified],
      leases: [],
      run,
      windows: [window],
    })

    expect(projection.blockerRefs).toEqual([])
    expect(projection.evalSuites).toEqual([
      expect.objectContaining({
        evalSuiteRef: 'eval.cs336.a5.gsm8k.seeded.1',
        metric: 'accuracy',
        score: 0.42,
        taskSetRef: 'gsm8k',
      }),
    ])
    expect(projection.evalSuites[0]?.scopeBoundaryRefs).toContain(
      'scope.cs336_a5.eval_results_not_capability_claims',
    )
  })

  it('rejects raw prompts and answers before publishing eval results', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a5',
      nowIso: '2026-06-10T13:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a5.unsafe',
      },
    })
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        a5Alignment: {
          evalSuites: [
            {
              answer: '42',
              metric: 'accuracy',
              prompt: 'What is six times seven?',
              sampleCount: 1,
              score: 1,
              splitRef: 'gsm8k.test',
              taskSetRef: 'gsm8k',
              verifiedSampleCount: 1,
            },
          ],
        },
      }),
    }

    expect(() =>
      publicCs336A5EvalProjection({
        challenges: [],
        leases: [],
        run,
        windows: [],
      }),
    ).toThrow('raw eval')
  })
})
