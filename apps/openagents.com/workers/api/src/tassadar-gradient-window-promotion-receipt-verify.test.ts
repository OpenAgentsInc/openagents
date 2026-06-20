import { describe, expect, test } from 'vitest'

import { buildTassadarGradientWindowPromotionReceipt } from './tassadar-gradient-window-promotion-receipt'
import {
  TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
  verifyTassadarGradientWindowPromotionReceipt,
} from './tassadar-gradient-window-promotion-receipt-verify'
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
  frozenParameterScopes: ['compiled_exact_core.tassadar_alm_numeric_executor'],
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

const promotedReceipt = () =>
  buildTassadarGradientWindowPromotionReceipt(
    projectTassadarGradientWindowRegime({
      candidate: baseCandidate(),
      receipts: passedReceipts(),
    }),
  )

describe('Tassadar gradient window promotion receipt verifier', () => {
  test('accepts a receipt emitted by the canonical builder', () => {
    const verification = verifyTassadarGradientWindowPromotionReceipt(
      promotedReceipt(),
    )

    expect(verification.valid).toBe(true)
    expect(verification.invalidReasonRefs).toEqual([])
    expect(verification.publicSafe).toBe(true)
    expect(verification.schemaVersion).toBe(
      TassadarGradientWindowPromotionReceiptVerificationSchemaVersion,
    )
    expect(verification.windowRef).toBe(
      'training_window.public.tassadar.h1.0001',
    )
    expect(verification.receiptRef).toBe(
      'receipt.public.tassadar_gradient_window.promoted.training_window.public.tassadar.h1.0001',
    )
  })

  test('rejects an unparseable receipt without throwing', () => {
    const verification = verifyTassadarGradientWindowPromotionReceipt({
      not: 'a receipt',
    })

    expect(verification.valid).toBe(false)
    expect(verification.receiptRef).toBeNull()
    expect(verification.windowRef).toBeNull()
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_verification.promotion_receipt_unparsed',
    )
  })

  test('rejects a receipt whose ref does not derive from its window ref', () => {
    const tampered = {
      ...promotedReceipt(),
      receiptRef:
        'receipt.public.tassadar_gradient_window.promoted.training_window.public.tassadar.h1.9999',
    }

    const verification = verifyTassadarGradientWindowPromotionReceipt(tampered)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_verification.receipt_ref_mismatch',
    )
  })

  test('rejects a receipt with missing recompute lineage', () => {
    const stripped = {
      ...promotedReceipt(),
      recomputeReceiptRefs: [],
    }

    const verification = verifyTassadarGradientWindowPromotionReceipt(stripped)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_verification.recompute_receipt_refs_missing',
    )
  })

  test('rejects a receipt carrying unsafe material', () => {
    const leaky = {
      ...promotedReceipt(),
      sourceRefs: [
        ...promotedReceipt().sourceRefs,
        'wallet_mnemonic seed phrase leaked',
      ],
    }

    const verification = verifyTassadarGradientWindowPromotionReceipt(leaky)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_verification.unsafe_material',
    )
  })
})
