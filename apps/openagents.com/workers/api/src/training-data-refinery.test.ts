import { describe, expect, it } from 'vitest'

import { Cs336A4HomeworkStages } from './cs336-a4-data-refinery'
import { buildCs336A4ProvenanceReceipt } from './cs336-a4-provenance'
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

const shardWithProvenance = async (
  stage: (typeof Cs336A4HomeworkStages)[number],
) => {
  const shard = baseShard(stage)
  const corpusProvenanceReceipt = await buildCs336A4ProvenanceReceipt({
    assignmentRef: `assignment.cs336_a4.${stage}.1`,
    finalOutputDigestRef: shard.outputDigestRef,
    inputShardRef: shard.shardRef,
    provenance: {
      acquisitionMode: 'bounded_synthetic_corpus',
      licenseRef: 'license.public.cc0.synthetic_corpus_v1',
      snapshotRef: `snapshot.cs336_a4.${stage}.v1`,
      sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
    },
    sourceInputDigestRef: `digest.cs336_a4.${stage}.source`,
    transformChain: [
      {
        codeVersionRef: `psionic.refinery.v1.${stage}`,
        inputDigestRef: `digest.cs336_a4.${stage}.source`,
        outputDigestRef: shard.outputDigestRef,
        recomputedDigestRef: shard.outputDigestRef,
        stage,
      },
    ],
  })

  return { ...shard, corpusProvenanceReceipt }
}

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
        'blocker.cs336_a4.operator_funding_required_for_paid_shards',
        'blocker.cs336_a4.requires_corpus_provenance_receipts',
        'blocker.cs336_a4.requires_three_verified_stages',
      ],
      observedVerifiedShardCount: 0,
      observedVerifiedStages: [],
      corpusProvenanceReceiptBlockerRefs: [
        'blocker.cs336_a4.requires_corpus_provenance_receipts',
      ],
      corpusProvenanceReceiptRefs: [],
      corpusProvenanceReceiptStatus: 'missing',
      psionicLaneRef: 'psion_cs336_a4_data_refinery_reference_v1',
      requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
      schemaVersion: 'openagents.training.data_refinery.v1',
      shards: [],
      status: 'blocked_no_shards',
    })
    expect(projection.evalDeltaBonusBlockerRefs).toContain(
      'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
    )
    expect(projection.evalDeltaPaymentGate).toMatchObject({
      fixedTrainerEvalMeasurementAvailable: false,
      greenGateSatisfied: false,
      leaderboardLane: 'a4_eval_delta',
      operatorFundingParametersAvailable: false,
      payableSettlementCount: 0,
      paymentComputationAvailable: true,
      paymentSchemaVersion:
        'openagents.training.data_refinery.eval_delta_payment.v1',
      remainingProductBlockerRefs: [
        'blocker.product_promises.eval_delta_payment_missing',
      ],
      settlementReceiptAvailable: false,
      settledBonusSats: 0,
      verifiedMeasurementRowCount: 0,
    })
    expect(projection.evalDeltaPaymentGate.blockerRefs).toEqual([
      'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
      'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
      'blocker.cs336_a4.psionic_classifier_adapters_partial',
    ])
  })

  it('projects verified eval-delta rows without fabricating payment', () => {
    const projection = publicDataRefineryProjection({
      challenges: [],
      leases: [],
      run: {
        ...buildRun(),
        publicProjectionJson: JSON.stringify({
          a4DataRefinery: {
            leaderboardRows: [
              {
                contributorRef: 'pylon.public.a4.eval_delta.1',
                evalDelta: 0.12,
                receiptRefs: ['receipt.cs336_a4.base_shard.1'],
                sourceRefs: ['measurement.cs336_a4.fixed_reference.1'],
                verificationRefs: ['challenge.cs336_a4.eval_delta.1'],
              },
            ],
          },
        }),
      },
      windows: [],
    })

    expect(projection.evalDeltaPaymentGate).toMatchObject({
      fixedTrainerEvalMeasurementAvailable: true,
      greenGateSatisfied: false,
      operatorFundingParametersAvailable: false,
      payableSettlementCount: 0,
      paymentComputationAvailable: true,
      settlementReceiptAvailable: false,
      settledBonusSats: 0,
      verifiedMeasurementRowCount: 1,
    })
    expect(projection.evalDeltaPaymentGate.blockerRefs).toEqual([
      'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
      'blocker.cs336_a4.psionic_classifier_adapters_partial',
    ])
  })

  it('admits receipted shards and rejects unreceipted, unsafe, empty, or unprovenanced evidence', async () => {
    const run = buildRun()
    const shard = await shardWithProvenance('pii_masking')

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
          shards: [{ ...shard, receiptRefs: [] }],
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
              ...shard,
              sourceRefs: ['lnbc10n1deadbeef.fake_invoice_material'],
            },
          ],
        },
        run,
      }),
    ).toThrow(/wallet, payment, raw-shard, or private material/)

    expect(() =>
      admitCs336A4DataRefineryEvidence({
        nowIso: '2026-06-11T02:00:00.000Z',
        request: {
          shards: [
            {
              ...baseShard('pii_masking'),
            } as unknown as Awaited<ReturnType<typeof shardWithProvenance>>,
          ],
        },
        run,
      }),
    ).toThrow(/corpusProvenanceReceipt/)

    expect(() =>
      admitCs336A4DataRefineryEvidence({
        nowIso: '2026-06-11T02:00:00.000Z',
        request: {
          shards: [
            {
              ...shard,
              outputDigestRef: 'digest.sha256.cs336_a4.pii_masking.WRONG',
            },
          ],
        },
        run,
      }),
    ).toThrow(/final output digest/)

    const admitted = admitCs336A4DataRefineryEvidence({
      nowIso: '2026-06-11T02:30:00.000Z',
      request: {
        receiptRefs: ['approval.operator.20260611.focus_cs336_issue4680'],
        shards: [shard],
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
      corpusProvenanceReceiptRef: shard.corpusProvenanceReceipt.receiptRef,
      corpusProvenanceVerified: true,
      pylonRef: 'pylon.24819249b4634a4c9d5e',
      settledPayoutSats: 0,
      stage: 'pii_masking',
      verified: true,
    })
    expect(projection.corpusProvenanceReceiptStatus).toBe('available')
    expect(projection.corpusProvenanceReceiptBlockerRefs).toEqual([])
  })

  it('reaches stages_verified only with three distinct verified stages', async () => {
    const run = buildRun()
    const stages = ['pii_masking', 'exact_line_dedup', 'minhash_dedup'] as const
    const shards = await Promise.all(stages.map(shardWithProvenance))
    const admitted = admitCs336A4DataRefineryEvidence({
      nowIso: '2026-06-11T02:30:00.000Z',
      request: { shards },
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
    expect(projection.corpusProvenanceReceiptStatus).toBe('available')
    expect(projection.observedVerifiedShardCount).toBe(3)
  })
})
