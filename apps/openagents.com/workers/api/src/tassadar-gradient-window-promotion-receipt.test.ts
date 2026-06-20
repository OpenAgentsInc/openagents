import { describe, expect, test } from 'vitest'

import {
  TassadarGradientWindowPromotionReceiptSchemaVersion,
  TassadarGradientWindowPromotionReceiptUnsafe,
  buildTassadarGradientWindowPromotionReceipt,
  tassadarGradientWindowPromotionReceiptRef,
} from './tassadar-gradient-window-promotion-receipt'
import {
  type TassadarGradientWindowCandidate,
  type TassadarGradientWindowReceiptBundle,
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
  ],
  gradientsFlowThroughTrace: true,
  learnedInterfaceDigest: digest('d'),
  modelFamilyRef: 'model.public.tassadar_student.hybrid_h1',
  optimizerStateDigest: digest('e'),
  psionicH1EvidenceRefs: [
    'receipt.public.psionic.tassadar_h1.baseline_d_frozen_core',
  ],
  quarantineCheckpointDigest: digest('f'),
  randomSeedRef: 'seed.public.tassadar.h1.window_0001',
  sourceRefs: ['issue.openagents.5332'],
  trainableParameterScopes: ['learned_interface.output_routing'],
  trainingConfigDigest: digest('1'),
  updateDigest: digest('2'),
  verificationReceiptRefs: ['receipt.public.tassadar.v1_exact_replay'],
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

const promotedProjection = () =>
  projectTassadarGradientWindowRegime({
    candidate: baseCandidate(),
    receipts: passedReceipts(),
  })

describe('Tassadar gradient window promotion receipt', () => {
  test('emits a public-safe receipt from a fully promoted projection', () => {
    const receipt = buildTassadarGradientWindowPromotionReceipt(
      promotedProjection(),
    )

    expect(receipt.schemaVersion).toBe(
      TassadarGradientWindowPromotionReceiptSchemaVersion,
    )
    expect(receipt.stage).toBe('promoted')
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.compiledCoreUnchanged).toBe(true)
    expect(receipt.receiptRef).toBe(
      'receipt.public.tassadar_gradient_window.promoted.training_window.public.tassadar.h1.0001',
    )
    expect(receipt.canaryReceiptRefs.length).toBeGreaterThan(0)
    expect(receipt.recomputeReceiptRefs.length).toBeGreaterThan(0)
    expect(receipt.replicationReceiptRefs.length).toBeGreaterThan(0)
    expect(receipt.promotionDecisionRefs.length).toBeGreaterThan(0)
    expect(receipt.rollbackRefs.length).toBeGreaterThan(0)
    expect(receipt.authority).toMatchObject({
      canonicalCheckpointMutationAllowed: true,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      settlementMutationAllowed: false,
    })
  })

  test('derives a stable receipt ref from the window ref', () => {
    expect(
      tassadarGradientWindowPromotionReceiptRef(
        'training_window.public.tassadar.h1.0007',
      ),
    ).toBe(
      'receipt.public.tassadar_gradient_window.promoted.training_window.public.tassadar.h1.0007',
    )
  })

  test('refuses to emit a receipt for a window that did not promote', () => {
    const blockedProjection = projectTassadarGradientWindowRegime({
      candidate: baseCandidate(),
      receipts: {
        ...passedReceipts(),
        canary: {
          ...passedReceipts().canary,
          passed: false,
          receiptRefs: [],
        },
        promotionDecisionRefs: [],
      },
    })

    expect(blockedProjection.stage).not.toBe('promoted')
    expect(() =>
      buildTassadarGradientWindowPromotionReceipt(blockedProjection),
    ).toThrow(TassadarGradientWindowPromotionReceiptUnsafe)
  })

  test('refuses to emit a receipt when the compiled core changed', () => {
    const mutatedProjection = projectTassadarGradientWindowRegime({
      candidate: { ...baseCandidate(), frozenCoreDigestAfter: digest('9') },
      receipts: passedReceipts(),
    })

    expect(mutatedProjection.stage).toBe('blocked')
    expect(() =>
      buildTassadarGradientWindowPromotionReceipt(mutatedProjection),
    ).toThrow(TassadarGradientWindowPromotionReceiptUnsafe)
  })
})
