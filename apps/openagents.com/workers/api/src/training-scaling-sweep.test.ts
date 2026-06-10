import { describe, expect, it } from 'vitest'

import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'
import {
  Cs336A3ScalingSweepJobKind,
  publicScalingSweepProjection,
} from './training-scaling-sweep'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

describe('CS336 A3 scaling sweep projection', () => {
  it('keeps the public IsoFLOP projection blocked without receipt-backed cells', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a3',
      nowIso: '2026-06-10T11:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a3.scaling',
      },
    })
    const projection = publicScalingSweepProjection({
      challenges: [],
      leases: [],
      run,
      windows: [],
    })

    expect(projection).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a3.requires_twenty_verified_cells',
        'blocker.cs336_a3.operator_funding_required_for_paid_cells',
        'blocker.cs336_a3.fit_artifact_not_published',
      ],
      cells: [],
      fitArtifact: null,
      observedVerifiedCellCount: 0,
      psionicLaneRef: 'psion_cs336_a3_scaling_reference_v1',
      requiredVerifiedCellCount: 20,
      status: 'blocked_no_cells',
    })
    expect(Cs336A3ScalingSweepJobKind).toBe('cs336_a3_scaling_sweep')
  })

  it('publishes a fit artifact only from twenty verified public cells', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a3',
      nowIso: '2026-06-10T11:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a3.scaling',
      },
    })
    const cells = Array.from({ length: 20 }, (_, index) => ({
      cellRef: `cell.cs336.a3.${index + 1}`,
      computeBudgetFlops: 1_000_000_000,
      parameterCount: 1_000_000 + index * 10_000,
      pylonRef: `pylon.cs336.a3.${index + 1}`,
      receiptRefs: [`receipt.cs336.a3.cell.${index + 1}`],
      sourceRefs: [`source.cs336.a3.cell.${index + 1}`],
      tokenCount: 4_000_000 - index * 10_000,
      validationLoss: 2.5 - index * 0.01,
      verificationRefs: [`challenge.cs336.a3.${index + 1}`],
    }))
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        a3ScalingSweep: {
          cells,
          fitArtifact: {
            artifactRef: 'artifact.cs336.a3.isoflop.fit.1',
            exponentRefs: ['fit.cs336.a3.exponent.alpha'],
            predictedBestConfig: {
              parameterCount: 1_210_000,
              tokenCount: 3_790_000,
            },
            provenanceLabel:
              'Psionic scaling-law fit over public receipt-backed cells.',
            sourceRefs: ['artifact.cs336.a3.isoflop.fit.1'],
          },
          psionicLaneRef: 'psion_cs336_a3_scaling_reference_v1',
        },
      }),
    }
    const window = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T11:00:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: run.trainingRunRef,
        windowRef: 'training.window.cs336.a3.scaling.1',
      },
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => '1',
      nowIso: '2026-06-10T11:01:00.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a3.cell.1'],
        contributionRef: 'cell.cs336.a3.1',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          expectedDigestRef: 'digest.cs336.a3.cell.1',
          recomputedDigestRef: 'digest.cs336.a3.cell.1',
        },
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'deterministic_recompute',
        windowRef: window.windowRef,
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: 'lease',
      nowIso: '2026-06-10T11:02:00.000Z',
      request: { validatorRef: 'validator.cs336.a3' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'final',
      nowIso: '2026-06-10T11:03:00.000Z',
      request: { receiptRefs: ['receipt.cs336.a3.verdict.1'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336.a3.cell.1'],
      },
    }).challenge
    const projection = publicScalingSweepProjection({
      challenges: [verified],
      leases: [],
      run,
      windows: [window],
    })

    expect(projection.status).toBe('fit_published')
    expect(projection.blockerRefs).toEqual([])
    expect(projection.observedVerifiedCellCount).toBe(20)
    expect(projection.fitArtifact).toMatchObject({
      artifactRef: 'artifact.cs336.a3.isoflop.fit.1',
      predictedBestConfig: {
        parameterCount: 1_210_000,
        tokenCount: 3_790_000,
      },
    })
    expect(projection.cells[0]).toMatchObject({
      cellRef: 'cell.cs336.a3.1',
      settledPayoutSats: 0,
      verified: true,
    })
  })
})
