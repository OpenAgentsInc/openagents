import { describe, expect, it } from 'vitest'

import { decodeUnknownWithSchema } from './json-boundary'
import { publicCs336A5EvalProjection } from './cs336-a5-alignment-homework'
import {
  Cs336A5AlignmentEvidenceRequest,
  admitCs336A5AlignmentEvidence,
} from './training-alignment-evals'
import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

const nowIso = '2026-06-11T08:00:00.000Z'

const makeRun = () =>
  buildTrainingRunRecord({
    makeId: () => 'a5-run',
    nowIso,
    request: {
      promiseRef: 'training.post_training_arc.v1',
      trainingRunRef: 'run.cs336.a5.alignment.test',
    },
  })

const baseSuite = {
  evalSuiteRef: 'eval.cs336_a5.synthetic_math.bounded.1',
  metric: 'accuracy' as const,
  receiptRefs: ['receipt.nexus_pylon.settlement.assignment_cs336_a5_test'],
  sampleCount: 256,
  score: 0.66,
  sourceRefs: ['workload.cs336_a5.seeded_rollout_and_reference_grading.v1'],
  splitRef: 'split.cs336_a5.synthetic_math.bounded_combined.v1',
  taskSetRef: 'math' as const,
  verificationRefs: ['training.verification.challenge.a5-test'],
  verifiedSampleCount: 256,
}

const decodeRequest = (value: unknown) =>
  decodeUnknownWithSchema(Cs336A5AlignmentEvidenceRequest, value)

describe('CS336 A5 alignment evidence admission', () => {
  it('admits receipted eval suites and work shards into the run projection', () => {
    const request = decodeRequest({
      evalSuites: [baseSuite],
      shards: [
        {
          jobKind: 'cs336_a5_rollout_batch',
          outputDigestRef: 'digest.cs336_a5.rollout_batch.split_a.sha256_abc',
          pylonRef: 'pylon.24819249b4634a4c9d5e',
          receiptRefs: [
            'receipt.nexus_pylon.settlement.assignment_cs336_a5_rollout_a',
          ],
          rolloutCount: 128,
          splitRef: 'split_a',
          verificationRefs: ['training.verification.challenge.a5-rollout-a'],
        },
        {
          jobKind: 'cs336_a5_reward_grading',
          outputDigestRef: 'digest.cs336_a5.reward_grading.split_a.sha256_def',
          receiptRefs: [
            'receipt.nexus_pylon.settlement.assignment_cs336_a5_grading_a',
          ],
        },
      ],
      sourceRefs: ['issue.github.openagents.4682'],
    })
    const admitted = admitCs336A5AlignmentEvidence({
      nowIso,
      request,
      run: makeRun(),
    })
    const stored = JSON.parse(admitted.publicProjectionJson) as {
      a5Alignment: { evalSuites: unknown[]; shards: unknown[] }
    }

    expect(stored.a5Alignment.evalSuites).toHaveLength(1)
    expect(stored.a5Alignment.shards).toHaveLength(2)
    expect(stored.a5Alignment).toMatchObject({
      psionicLaneRef: 'psion_cs336_a5_alignment_reference_v1',
      updateBoundaryRef: 'issue.github.openagents.4669',
    })
  })

  it('serves admitted suites through the public A5 eval projection with verified challenges', () => {
    const request = decodeRequest({ evalSuites: [baseSuite] })
    const run = admitCs336A5AlignmentEvidence({
      nowIso,
      request,
      run: makeRun(),
    })
    const window = buildTrainingWindowRecord({
      makeId: () => 'a5-window',
      nowIso,
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: run.trainingRunRef,
        windowRef: 'training.window.cs336_a5.test.w1',
      },
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'a5-challenge',
      nowIso,
      request: {
        commitmentRefs: ['commitment.cs336_a5.reward_grading.split_a'],
        contributionRef: 'contribution.cs336_a5.grading.split_a',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          expectedDigestRef: 'digest.cs336_a5.reward_grading.split_a',
          recomputedDigestRef: 'digest.cs336_a5.reward_grading.split_a',
        },
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'deterministic_recompute',
        windowRef: window.windowRef,
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: 'a5-lease',
      nowIso,
      request: { validatorRef: 'validator.cs336_a5.test' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'a5-final',
      nowIso,
      request: { receiptRefs: ['issue.github.openagents.4682'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336_a5.test.1'],
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
        evalSuiteRef: 'eval.cs336_a5.synthetic_math.bounded.1',
        sampleCount: 256,
        score: 0.66,
        taskSetRef: 'math',
      }),
    ])
  })

  it('rejects unreceipted eval suites', () => {
    const request = decodeRequest({
      evalSuites: [{ ...baseSuite, receiptRefs: [] }],
    })

    expect(() =>
      admitCs336A5AlignmentEvidence({ nowIso, request, run: makeRun() }),
    ).toThrow('at least one receipt ref')
  })

  it('rejects accuracy scores outside [0, 1]', () => {
    const request = decodeRequest({
      evalSuites: [{ ...baseSuite, score: 1.2 }],
    })

    expect(() =>
      admitCs336A5AlignmentEvidence({ nowIso, request, run: makeRun() }),
    ).toThrow('within [0, 1]')
  })

  it('rejects verified sample counts above the sample count', () => {
    const request = decodeRequest({
      evalSuites: [{ ...baseSuite, verifiedSampleCount: 999 }],
    })

    expect(() =>
      admitCs336A5AlignmentEvidence({ nowIso, request, run: makeRun() }),
    ).toThrow('between zero and the sample count')
  })

  it('rejects unreceipted work shards', () => {
    const request = decodeRequest({
      evalSuites: [baseSuite],
      shards: [
        {
          jobKind: 'cs336_a5_rollout_batch',
          outputDigestRef: 'digest.cs336_a5.rollout_batch.split_a.sha256_abc',
          receiptRefs: [],
        },
      ],
    })

    expect(() =>
      admitCs336A5AlignmentEvidence({ nowIso, request, run: makeRun() }),
    ).toThrow('unreceipted shards are not admissible')
  })

  it('rejects wallet and payment material at admission time', () => {
    const request = decodeRequest({
      evalSuites: [
        {
          ...baseSuite,
          sourceRefs: ['lnbc1500n1exampleinvoicematerial'],
        },
      ],
    })

    expect(() =>
      admitCs336A5AlignmentEvidence({ nowIso, request, run: makeRun() }),
    ).toThrow('raw eval, wallet, payment, or private material')
  })
})
