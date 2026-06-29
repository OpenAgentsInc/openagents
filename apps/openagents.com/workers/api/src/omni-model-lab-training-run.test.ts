import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_TRAINING_RUN_READ_ONLY_AUTHORITY,
  OmniTrainingRunProjection,
  OmniTrainingRunRecord,
  OmniTrainingRunUnsafe,
  exampleOmniTrainingRun,
  omniTrainingRunProjectionHasPrivateMaterial,
  projectOmniTrainingRun,
} from './omni-model-lab-training-run'

const nowIso = '2026-06-06T23:30:00.000Z'

const trainingRecord = (
  overrides: Partial<OmniTrainingRunRecord> = {},
): OmniTrainingRunRecord =>
  S.decodeUnknownSync(OmniTrainingRunRecord)({
    ...exampleOmniTrainingRun(),
    ...overrides,
  })

describe('Omni Model Lab training run', () => {
  test('projects a reviewed training run without launch, provider, adapter, spend, runtime, routing, payout, settlement, or public-claim authority', () => {
    const projection = projectOmniTrainingRun(
      exampleOmniTrainingRun(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniTrainingRunProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      adapterInstallAllowed: false,
      createdAtDisplay: '25 minutes ago',
      hyperparameterCount: 1,
      metricCount: 1,
      modelTrainingLaunchAllowed: false,
      paymentSpendAllowed: false,
      payoutMutationAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      rawDatasetCopyAllowed: false,
      readiness: 'complete',
      readinessLabel: 'Complete',
      routingMutationAllowed: false,
      runtimePromotionAllowed: false,
      settlementMutationAllowed: false,
      state: 'reviewed',
      stateLabel: 'Reviewed evidence',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(OMNI_TRAINING_RUN_READ_ONLY_AUTHORITY)
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniTrainingRunProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('keeps status progression, metric summary, and budget caveats explicit', () => {
    expect(projectOmniTrainingRun(
      trainingRecord({ operatorReviewReceiptRefs: [], state: 'completed' }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'needs_review',
      readinessLabel: 'Needs review',
      metricCount: 1,
    })
    expect(projectOmniTrainingRun(
      trainingRecord({ state: 'running' }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'running',
      providerRefs: ['provider.public.psionic_lab'],
      runnerRefs: ['runner.public.model_lab_sandbox'],
    })
    expect(projectOmniTrainingRun(
      trainingRecord({
        artifactRefs: [],
        benchmarkRefs: [],
        evalRerunRefs: [],
        metrics: [],
        state: 'imported',
      }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'imported',
      metricCount: 0,
    })
    expect(projectOmniTrainingRun(
      trainingRecord({
        failureRefs: ['failure.public.loss_spike'],
        state: 'failed',
      }),
      'operator',
      nowIso,
    )).toMatchObject({
      readiness: 'failed',
      failureRefs: ['failure.public.loss_spike'],
    })
  })

  test('requires source/evidence refs, artifact linkage, metric evidence, cost caveats, data refs, and review receipts', () => {
    for (const badRecord of [
      trainingRecord({ sourceRefs: [] }),
      trainingRecord({ evidenceRefs: [] }),
      trainingRecord({
        dataPackageRefs: [],
        kind: 'fine_tune',
      }),
      trainingRecord({
        hyperparameters: [
          {
            ...exampleOmniTrainingRun().hyperparameters[0]!,
            evidenceRefs: [],
          },
        ],
      }),
      trainingRecord({
        metrics: [
          {
            ...exampleOmniTrainingRun().metrics[0]!,
            evidenceRefs: [],
          },
        ],
      }),
      trainingRecord({
        budget: {
          ...exampleOmniTrainingRun().budget,
          actualCostCents: 10,
          creditRefs: [],
        },
      }),
      trainingRecord({
        artifactRefs: [],
        state: 'completed',
      }),
      trainingRecord({
        evalRerunRefs: [],
        benchmarkRefs: [],
        state: 'completed',
      }),
      trainingRecord({
        operatorReviewReceiptRefs: [],
        state: 'reviewed',
      }),
      trainingRecord({
        failureRefs: [],
        state: 'failed',
      }),
      trainingRecord({
        providerRefs: [],
        state: 'running',
      }),
      trainingRecord({
        caveatRefs: [],
        state: 'blocked',
      }),
    ]) {
      expect(() =>
        projectOmniTrainingRun(badRecord, 'operator', nowIso),
      ).toThrow(OmniTrainingRunUnsafe)
    }
  })

  test('redacts private prompts, datasets, provider, runner, budget, metrics, source, and workroom refs publicly', () => {
    const projection = projectOmniTrainingRun(
      trainingRecord({
        artifactRefs: [
          'artifact.public.otect_layout_adapter_v1',
          'artifact.private.operator_adapter',
        ],
        budget: {
          ...exampleOmniTrainingRun().budget,
          budgetRef: 'budget.private.operator_budget',
          caveatRefs: [
            'caveat.public.cost_imported',
            'caveat.private.operator_cost_note',
          ],
          creditRefs: [
            'credit.public.operator_lab_budget',
            'credit.private.operator_payment_ref',
          ],
        },
        dataPackageRefs: [
          'data_package.public.feedback_refs',
          'data_package.private.raw_feedback',
        ],
        hyperparameters: [
          {
            ...exampleOmniTrainingRun().hyperparameters[0]!,
            evidenceRefs: [
              'evidence.public.hyperparameter_manifest',
              'hyperparam.private.operator_note',
            ],
            paramRef: 'hyperparam.private.operator_learning_rate',
          },
        ],
        metrics: [
          {
            ...exampleOmniTrainingRun().metrics[0]!,
            evidenceRefs: [
              'evidence.public.metric_manifest',
              'metric.private.operator_eval',
            ],
            metricRef: 'metric.private.operator_score',
          },
        ],
        providerRefs: [
          'provider.public.psionic_lab',
          'provider.private.operator_gpu',
        ],
        runRef: 'run.private.operator_run',
        runnerRefs: [
          'runner.public.model_lab_sandbox',
          'runner.private.operator_runner',
        ],
        sourceRefs: [
          'source.public.training_summary',
          'source.private.operator_archive',
        ],
        triggerWorkroomRefs: [
          'workroom.public.otect_revision_two',
          'workroom.private.operator_room',
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.runRef).toBe('run.redacted.training_run')
    expect(projection.providerRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.dataPackageRefs).toEqual([
      'data_package.public.feedback_refs',
    ])
    expect(projection.budget.budgetRef).toBe('budget.redacted.training_run')
    expect(projection.budget.creditRefs).toEqual([
      'credit.public.operator_lab_budget',
    ])
    expect(projection.metrics[0]!.metricRef).toBe('metric.redacted.training_run')
    expect(projection.hyperparameters[0]!.paramRef).toBe(
      'hyperparam.redacted.training_run',
    )
    expect(serialized).not.toContain('private')
    expect(serialized).not.toContain('operator_gpu')
    expect(omniTrainingRunProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('rejects private datasets, raw logs, model weights, provider payloads, secrets, payment material, raw timestamps, and mutable authority', () => {
    for (const badRecord of [
      trainingRecord({ dataPackageRefs: ['dataset.raw.customer_feedback'] }),
      trainingRecord({ evidenceRefs: ['raw_run_log.operator'] }),
      trainingRecord({ artifactRefs: ['weights.safetensors'] }),
      trainingRecord({ providerRefs: ['provider_payload.raw'] }),
      trainingRecord({ caveatRefs: ['secret.model_lab_token'] }),
      trainingRecord({ caveatRefs: ['payment_hash.raw'] }),
      trainingRecord({ sourceRefs: ['source.public.2026-06-06T23:00:00'] }),
      trainingRecord({
        budget: {
          ...exampleOmniTrainingRun().budget,
          paymentSpendAllowed: true,
        },
      }),
      trainingRecord({
        authority: {
          ...OMNI_TRAINING_RUN_READ_ONLY_AUTHORITY,
          noModelTrainingLaunch: false,
        },
      }),
    ]) {
      expect(() =>
        projectOmniTrainingRun(badRecord, 'operator', nowIso),
      ).toThrow(OmniTrainingRunUnsafe)
    }
  })
})
