import { describe, expect, it } from 'vitest'

import { Cs336A4HomeworkStages } from './cs336-a4-data-refinery'
import {
  admitCs336A4DataRefineryEvidence,
  Cs336A4RequiredVerifiedStageCount,
  publicDataRefineryProjection,
} from './training-data-refinery'
import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

const buildRun = () =>
  buildTrainingRunRecord({
    makeId: () => 'a4',
    nowIso: '2026-06-11T02:00:00.000Z',
    request: {
      promiseRef: 'training.data_refinery_corpus.v1',
      trainingRunRef: 'run.cs336.a4.data_refinery.demo',
    },
  })

const baseShard = (stage: (typeof Cs336A4HomeworkStages)[number]) => ({
  inputDocumentCount: 64,
  outputDigestRef: `digest.sha256.cs336_a4.${stage}.aaaa`,
  pylonRef: 'pylon.24819249b4634a4c9d5e',
  receiptRefs: [`receipt.cs336_a4.settlement.${stage}`],
  shardRef: `shard.cs336_a4.${stage}.1`,
  sourceRefs: [`commitment.cs336_a4.${stage}.sha256_abcdef0123456789`],
  stage,
  verificationRefs: [`challenge.cs336_a4.${stage}.1`],
})

describe('CS336 A4 data-refinery projection', () => {
  it('keeps the public refinery projection blocked without receipted shards', () => {
    const projection = publicDataRefineryProjection({
      challenges: [],
      leases: [],
      run: buildRun(),
      windows: [],
    })

    expect(projection).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a4.requires_three_verified_stages',
        'blocker.cs336_a4.operator_funding_required_for_paid_shards',
      ],
      observedVerifiedShardCount: 0,
      observedVerifiedStages: [],
      psionicLaneRef: 'psion_cs336_a4_data_refinery_reference_v1',
      requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
      schemaVersion: 'openagents.training.data_refinery.v1',
      shards: [],
      status: 'blocked_no_shards',
    })
    expect(projection.evalDeltaBonusBlockerRefs).toContain(
      'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
    )
  })

  it('admits receipted shards and rejects unreceipted, unsafe, or empty evidence', () => {
    const run = buildRun()

    expect(() =>
      admitCs336A4DataRefineryEvidence({
        nowIso: '2026-06-11T02:00:00.000Z',
        request: { shards: [] },
        run,
      }),
    ).toThrow(/at least one shard/)

    expect(() =>
      admitCs336A4DataRefineryEvidence({
        nowIso: '2026-06-11T02:00:00.000Z',
        request: {
          shards: [{ ...baseShard('pii_masking'), receiptRefs: [] }],
        },
        run,
      }),
    ).toThrow(/unreceipted/i)

    expect(() =>
      admitCs336A4DataRefineryEvidence({
        nowIso: '2026-06-11T02:00:00.000Z',
        request: {
          shards: [
            {
              ...baseShard('pii_masking'),
              sourceRefs: ['lnbc10n1deadbeef.fake_invoice_material'],
            },
          ],
        },
        run,
      }),
    ).toThrow(/wallet, payment, raw-shard, or private material/)

    const admitted = admitCs336A4DataRefineryEvidence({
      nowIso: '2026-06-11T02:30:00.000Z',
      request: {
        receiptRefs: ['approval.operator.20260611.focus_cs336_issue4680'],
        shards: [baseShard('pii_masking')],
        sourceRefs: ['issue.github.openagents.4680'],
      },
      run,
    })
    const projection = publicDataRefineryProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(admitted.updatedAt).toBe('2026-06-11T02:30:00.000Z')
    expect(projection.status).toBe('collecting_shards')
    expect(projection.shards).toHaveLength(1)
    expect(projection.shards[0]).toMatchObject({
      pylonRef: 'pylon.24819249b4634a4c9d5e',
      settledPayoutSats: 0,
      stage: 'pii_masking',
      verified: true,
    })
  })

  it('reaches stages_verified only with three distinct verified stages', () => {
    const run = buildRun()
    const stages = ['pii_masking', 'exact_line_dedup', 'minhash_dedup'] as const
    const admitted = admitCs336A4DataRefineryEvidence({
      nowIso: '2026-06-11T02:30:00.000Z',
      request: { shards: stages.map(baseShard) },
      run,
    })
    const window = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-11T02:00:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: run.trainingRunRef,
        windowRef: 'training.window.cs336.a4.1',
      },
    })
    const verifiedChallenge = (id: string) => {
      const built = buildTrainingVerificationChallengeRecord({
        makeId: () => id,
        nowIso: '2026-06-11T02:01:00.000Z',
        request: {
          commitmentRefs: [`commitment.cs336_a4.${id}`],
          contributionRef: `contribution.cs336_a4.${id}`,
          homeworkKind: 'admin_dispatched_homework',
          payload: {
            expectedDigestRef: `digest.cs336_a4.${id}`,
            recomputedDigestRef: `digest.cs336_a4.${id}`,
          },
          trainingRunRef: run.trainingRunRef,
          verificationClass: 'deterministic_recompute',
          windowRef: window.windowRef,
        },
      }).challenge
      const leased = leaseTrainingVerificationChallengeRecord({
        challenge: built,
        eventId: `lease-${id}`,
        nowIso: '2026-06-11T02:02:00.000Z',
        request: { validatorRef: 'validator.cs336.a4' },
      }).challenge

      return finalizeTrainingVerificationChallengeRecord({
        challenge: leased,
        eventId: `final-${id}`,
        nowIso: '2026-06-11T02:03:00.000Z',
        request: { receiptRefs: [`receipt.cs336_a4.verdict.${id}`] },
        verdict: {
          failureCodes: [],
          state: 'Verified',
          verdictRefs: [`verdict.cs336_a4.${id}`],
        },
      }).challenge
    }

    const projection = publicDataRefineryProjection({
      challenges: [verifiedChallenge('1'), verifiedChallenge('2')],
      leases: [],
      run: admitted,
      windows: [window],
    })

    expect(projection.observedVerifiedStages).toEqual([
      'exact_line_dedup',
      'minhash_dedup',
      'pii_masking',
    ])
    expect(projection.status).toBe('stages_verified')
    expect(projection.blockerRefs).toEqual([])
    expect(projection.observedVerifiedShardCount).toBe(3)
  })
})
