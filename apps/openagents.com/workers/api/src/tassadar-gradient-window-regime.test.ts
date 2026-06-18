import { describe, expect, test } from 'vitest'

import {
  type TassadarGradientWindowCandidate,
  type TassadarGradientWindowReceiptBundle,
  TassadarGradientWindowUnsafe,
  projectTassadarGradientWindowRegime,
} from './tassadar-gradient-window-regime'

const digest = (char: string): string => char.repeat(64)

const baseCandidate = (): TassadarGradientWindowCandidate => ({
  baseCheckpointDigest: digest('a'),
  compiledCoreGradientTargeted: false,
  compiledCoreRef: 'core.public.psionic.tassadar_alm_numeric_executor.v1',
  constructionReceiptRefs: [
    'receipt.public.tassadar.c1_c5.constructed_compiled_core',
  ],
  contributorRef: 'agent.public.pylon.gradient_window_author',
  curatedDataRefs: ['data.public.tassadar.curated_interface_traces.v1'],
  datasetShardDigest: digest('b'),
  frozenCoreDigestAfter: digest('c'),
  frozenCoreDigestBefore: digest('c'),
  frozenParameterScopes: [
    'compiled_exact_core.tassadar_alm_numeric_executor',
    'compiled_exact_core.digest_pinned_numeric_models',
  ],
  gradientsFlowThroughTrace: true,
  learnedInterfaceDigest: digest('d'),
  modelFamilyRef: 'model.public.tassadar_student.hybrid_h1',
  optimizerStateDigest: digest('e'),
  psionicH1EvidenceRefs: [
    'fixtures/tassadar/w3_student_sweep_20260612/d/eval-report.json',
    'receipt.public.psionic.tassadar_h1.baseline_d_frozen_core',
  ],
  quarantineCheckpointDigest: digest('f'),
  randomSeedRef: 'seed.public.tassadar.h1.window_0001',
  sourceRefs: ['issue.openagents.5332'],
  trainableParameterScopes: [
    'learned_interface.input_limb_assignment',
    'learned_interface.output_limb_assignment',
    'learned_interface.output_routing',
  ],
  trainingConfigDigest: digest('1'),
  updateDigest: digest('2'),
  verificationReceiptRefs: [
    'receipt.public.tassadar.v1_exact_replay',
    'receipt.public.tassadar.v2_composition_verification',
    'receipt.public.tassadar.v3_data_correctness',
  ],
  windowRef: 'training_window.public.tassadar.h1.0001',
})

const passedReceipts = (): TassadarGradientWindowReceiptBundle => ({
  canary: {
    exactRolloutPassAt1: 1,
    outputDigestMatchRate: 1,
    passed: true,
    receiptRefs: ['receipt.public.tassadar.gradient_window.canary.0001'],
    replayAcceptanceRate: 1,
  },
  promotionDecisionRefs: [
    'promotion.public.tassadar.gradient_window.0001.promoted',
  ],
  quarantineReceiptRefs: [
    'receipt.public.tassadar.gradient_window.quarantine.0001',
  ],
  recompute: {
    expectedUpdateDigest: digest('2'),
    passed: true,
    receiptRefs: ['receipt.public.tassadar.gradient_window.recompute.0001'],
    recomputedUpdateDigest: digest('2'),
  },
  replication: {
    passed: true,
    receiptRefs: ['receipt.public.tassadar.gradient_window.replication.0001'],
    replicaUpdateDigests: [digest('2'), digest('2')],
  },
  rollbackRefs: ['rollback.public.tassadar.gradient_window.0001'],
  settlementReceiptRefs: [],
})

describe('Tassadar gradient window regime', () => {
  test('promotes only after quarantine, recompute, replication, and canary with frozen core unchanged', () => {
    const projection = projectTassadarGradientWindowRegime({
      candidate: baseCandidate(),
      receipts: passedReceipts(),
    })

    expect(projection.stage).toBe('promoted')
    expect(projection.promotionAllowed).toBe(true)
    expect(projection.blockerRefs).toEqual([])
    expect(projection.compiledCoreUnchanged).toBe(true)
    expect(projection.authority).toMatchObject({
      canonicalCheckpointMutationAllowed: true,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      quarantineCheckpointMutationAllowed: true,
      settlementMutationAllowed: false,
    })
    expect(projection.settlementEligible).toBe(false)
  })

  test('blocks promotion until recompute and canary pass', () => {
    const receipts = {
      ...passedReceipts(),
      canary: {
        ...passedReceipts().canary,
        exactRolloutPassAt1: 0.99,
        passed: false,
        receiptRefs: [],
      },
      promotionDecisionRefs: [],
      recompute: {
        ...passedReceipts().recompute,
        passed: false,
        receiptRefs: [],
        recomputedUpdateDigest: digest('3'),
      },
    }
    const projection = projectTassadarGradientWindowRegime({
      candidate: baseCandidate(),
      receipts,
    })

    expect(projection.stage).toBe('quarantined')
    expect(projection.promotionAllowed).toBe(false)
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.tassadar_gradient_window.recompute_missing_or_failed',
        'blocker.public.tassadar_gradient_window.canary_missing_or_failed',
        'blocker.public.tassadar_gradient_window.promotion_decision_missing',
      ]),
    )
    expect(projection.authority.canonicalCheckpointMutationAllowed).toBe(false)
  })

  test('blocks a mutated compiled core even when the rest of the window passes', () => {
    const candidate = {
      ...baseCandidate(),
      frozenCoreDigestAfter: digest('9'),
    }
    const projection = projectTassadarGradientWindowRegime({
      candidate,
      receipts: passedReceipts(),
    })

    expect(projection.stage).toBe('blocked')
    expect(projection.promotionAllowed).toBe(false)
    expect(projection.compiledCoreUnchanged).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.tassadar_gradient_window.frozen_core_digest_changed',
    )
    expect(projection.authority.quarantineCheckpointMutationAllowed).toBe(false)
  })

  test('blocks trainable scopes that target the compiled exact core', () => {
    const candidate = {
      ...baseCandidate(),
      trainableParameterScopes: [
        ...baseCandidate().trainableParameterScopes,
        'compiled_exact_core.ffn_bank',
      ],
    }
    const projection = projectTassadarGradientWindowRegime({
      candidate,
      receipts: passedReceipts(),
    })

    expect(projection.stage).toBe('blocked')
    expect(projection.promotionAllowed).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.tassadar_gradient_window.compiled_core_gradient_targeted',
    )
    expect(projection.authority.compiledCoreGradientMutationAllowed).toBe(false)
  })

  test('rejects raw private refs before projection', () => {
    const candidate = {
      ...baseCandidate(),
      sourceRefs: ['/Users/operator/private/raw_trace.json'],
    }

    expect(() =>
      projectTassadarGradientWindowRegime({
        candidate,
        receipts: passedReceipts(),
      }),
    ).toThrow(TassadarGradientWindowUnsafe)
  })
})
