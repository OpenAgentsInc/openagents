import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingAblationManifestDelta,
  TrainingAblationOneDeltaHarnessError,
  TrainingAblationOneDeltaHarnessRef,
  TrainingAblationOneDeltaManifestInput,
  TrainingAblationDeriskingLedgerEndpoint,
  TrainingAblationDeriskingLedgerProjection,
  projectTrainingAblationDeriskingLedger,
  verifyTrainingAblationOneDeltaManifest,
} from './training-ablation-derisking-ledger'
import { handleTrainingAblationDeriskingLedgerApi } from './training-ablation-derisking-ledger-routes'

type TrainingAblationLedgerBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    publicProjectionAvailable: boolean
    greenGateSatisfied: boolean
  }>
  promiseRef: string
}>

describe('training ablation derisking ledger projection', () => {
  test('publishes a public-safe one-delta manifest ledger without claiming ablation execution', () => {
    const projection = projectTrainingAblationDeriskingLedger({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingAblationDeriskingLedgerProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.promiseRef).toBe('promise:training.ablation_system.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.status).toBe('candidate_ledger_projection')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toMatchObject({
      ablationHarnessAvailable: true,
      evalSuiteReproductionAvailable: true,
      greenGateSatisfied: false,
      paidAblationDispatchAvailable: true,
      publicProjectionAvailable: true,
    })
    expect(projection.gate.clearsBlockerRefs).toContain(
      'blocker.product_promises.ablation_ledger_projection_missing',
    )
    expect(projection.gate.clearsBlockerRefs).toContain(
      'blocker.product_promises.ablation_harness_missing',
    )
    expect(projection.gate.clearsBlockerRefs).toContain(
      'blocker.product_promises.eval_suite_reproduction_missing',
    )
    expect(projection.gate.clearsBlockerRefs).toContain(
      'blocker.product_promises.paid_ablation_dispatch_missing',
    )
    expect(projection.gate.remainingBlockerRefs).toEqual([
      'blocker.product_promises.seeded_ablation_replication_missing',
      'blocker.product_promises.owner_signed_green_transition_missing',
    ])
    expect(projection.ledgerSummary).toMatchObject({
      acceptedVerdictCount: 1,
      entryCount: 3,
      evalSuiteReproductionReceiptCount: 1,
      paidAblationCount: 1,
      reproducedEvalCount: 3,
      verifiedManifestCount: 3,
    })
    expect(projection.paidDispatchReceipts[0]).toMatchObject({
      accepted: true,
      amountSats: 21,
      assignmentRef:
        'assignment.public.training_ablation.wsd_schedule.one_delta_paid.v1',
      dispatchState: 'settled',
      manifestRef: 'manifest.training_ablation.wsd_schedule.one_delta.v1',
      receiptRef:
        'receipt.training_ablation.paid_dispatch.wsd_schedule.one_delta.v1',
      settlementReceiptRef:
        'settlement.public.training_ablation.wsd_schedule.one_delta_paid.v1',
      verdictReceiptRef:
        'verdict.training_ablation.wsd_schedule.one_delta_paid.accepted.v1',
    })
    expect(projection.evalReproductionReceipts[0]).toMatchObject({
      aggregatePassRateBps: 10000,
      aggregateScoreBps: 8532,
      benchmarkPackageRef:
        'benchmark://psion/actual_pretraining/checkpoint_eval@2026.04.02',
      decisionState: 'continue',
      metricGateCount: 4,
      passedMetricGateCount: 4,
      receiptRef:
        'receipt.training_ablation.eval_reproduction.psion_actual_checkpoint_eval.v1',
      sourceSchemaVersion: 'psion.actual_pretraining_checkpoint_eval_decision.v1',
    })
    expect(
      projection.entries.every(
        entry =>
          entry.manifestRef.startsWith('manifest.training_ablation.') &&
          entry.oneDeltaManifestState === 'manifest_verified' &&
          entry.evalReproductionState === 'reproduced',
      ),
    ).toBe(true)
    expect(
      projection.entries.filter(entry => entry.paidDispatchState === 'settled'),
    ).toHaveLength(1)
    expect(
      projection.entries.filter(entry => entry.verdictState === 'accepted'),
    ).toHaveLength(1)
    expect(
      projection.entries.every(
        entry =>
          !entry.blockerRefs.includes(
            'blocker.product_promises.eval_suite_reproduction_missing',
          ),
      ),
    ).toBe(true)
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingAblationDeriskingLedger({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain(
      'Do not claim the ablation system is green',
    )
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public ledger route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTrainingAblationDeriskingLedgerApi(
        new Request(
          `https://openagents.com${TrainingAblationDeriskingLedgerEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as TrainingAblationLedgerBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingAblationDeriskingLedgerEndpoint)
    expect(body.promiseRef).toBe('promise:training.ablation_system.v1')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.greenGateSatisfied).toBe(false)
  })

  test('verifies exactly one public-safe ablation delta', () => {
    const manifest = new TrainingAblationOneDeltaManifestInput({
      baselineRef: 'baseline.psion.r1_reference_optimizer',
      caveatRefs: ['caveat.training_ablation.test'],
      candidateRef: 'ablation.derisking.test_candidate',
      deltas: [
        new TrainingAblationManifestDelta({
          deltaRef: 'delta.training.test_schedule',
          kind: 'optimizer_schedule',
          sourceRefs: ['docs/training/2026-06-19-model-ladder-rung-economics.md'],
          summary: 'Change only the optimizer schedule.',
          targetRef: 'target.training.optimizer_schedule',
        }),
      ],
      evaluationPlanRefs: ['eval_plan.psion.r1.fixed_suite'],
      frozenRefSet: ['frozen.training.r1_reference_corpus'],
      manifestRef: 'manifest.training_ablation.test.one_delta.v1',
      sourceRefs: ['docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md'],
    })

    const verification = verifyTrainingAblationOneDeltaManifest(manifest)

    expect(verification).toMatchObject({
      accepted: true,
      changedDeltaCount: 1,
      harnessRef: TrainingAblationOneDeltaHarnessRef,
      manifestRef: 'manifest.training_ablation.test.one_delta.v1',
    })
    expect(verification.clearsBlockerRefs).toEqual([
      'blocker.product_promises.ablation_harness_missing',
    ])
    expect(verification.authorityBoundary).toContain('grants no')
  })

  test('rejects multi-delta manifests before projection', () => {
    const delta = new TrainingAblationManifestDelta({
      deltaRef: 'delta.training.test_schedule',
      kind: 'optimizer_schedule',
      sourceRefs: ['docs/training/2026-06-19-model-ladder-rung-economics.md'],
      summary: 'Change only the optimizer schedule.',
      targetRef: 'target.training.optimizer_schedule',
    })
    const manifest = new TrainingAblationOneDeltaManifestInput({
      baselineRef: 'baseline.psion.r1_reference_optimizer',
      caveatRefs: ['caveat.training_ablation.test'],
      candidateRef: 'ablation.derisking.test_candidate',
      deltas: [
        delta,
        new TrainingAblationManifestDelta({
          deltaRef: 'delta.training.second_change',
          kind: 'runtime_config',
          sourceRefs: ['docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md'],
          summary: 'Change only the runtime config.',
          targetRef: 'target.training.runtime_config',
        }),
      ],
      evaluationPlanRefs: ['eval_plan.psion.r1.fixed_suite'],
      frozenRefSet: ['frozen.training.r1_reference_corpus'],
      manifestRef: 'manifest.training_ablation.test.multi_delta.v1',
      sourceRefs: ['docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md'],
    })

    expect(() => verifyTrainingAblationOneDeltaManifest(manifest)).toThrow(
      TrainingAblationOneDeltaHarnessError,
    )
  })

  test('rejects private material in a manifest', () => {
    expect(() =>
      verifyTrainingAblationOneDeltaManifest({
        baselineRef: 'baseline.psion.r1_reference_optimizer',
        caveatRefs: ['caveat.training_ablation.test'],
        candidateRef: 'ablation.derisking.test_candidate',
        deltas: [
          {
            deltaRef: 'delta.training.test_schedule',
            kind: 'optimizer_schedule',
            sourceRefs: ['docs/training/2026-06-19-model-ladder-rung-economics.md'],
            summary: 'Uses /home/operator/raw_prompt.txt',
            targetRef: 'target.training.optimizer_schedule',
          },
        ],
        evaluationPlanRefs: ['eval_plan.psion.r1.fixed_suite'],
        frozenRefSet: ['frozen.training.r1_reference_corpus'],
        manifestRef: 'manifest.training_ablation.test.private.v1',
        sourceRefs: ['docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md'],
      }),
    ).toThrow(TrainingAblationOneDeltaHarnessError)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingAblationDeriskingLedgerApi(
        new Request(
          `https://openagents.com${TrainingAblationDeriskingLedgerEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
