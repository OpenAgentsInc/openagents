import { describe, expect, test } from 'vitest'

import {
  TassadarGradientWindowPromotionReceiptFeedSchemaVersion,
  buildTassadarGradientWindowPromotionReceiptFeed,
} from './tassadar-gradient-window-promotion-receipt-feed'
import { buildTassadarGradientWindowPromotionReceipt } from './tassadar-gradient-window-promotion-receipt'
import {
  type TassadarGradientWindowCandidate,
  type TassadarGradientWindowReceiptBundle,
  projectTassadarGradientWindowRegime,
} from './tassadar-gradient-window-regime'

const digest = (char: string): string => char.repeat(64)

const baseCandidate = (
  windowRef: string,
): TassadarGradientWindowCandidate => ({
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
  windowRef,
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

const promotedReceipt = (windowRef: string) =>
  buildTassadarGradientWindowPromotionReceipt(
    projectTassadarGradientWindowRegime({
      candidate: baseCandidate(windowRef),
      receipts: passedReceipts(),
    }),
  )

describe('Tassadar gradient window promotion receipt feed', () => {
  test('an empty input yields an empty feed', () => {
    const feed = buildTassadarGradientWindowPromotionReceiptFeed([])

    expect(feed.schemaVersion).toBe(
      TassadarGradientWindowPromotionReceiptFeedSchemaVersion,
    )
    expect(feed.publicSafe).toBe(true)
    expect(feed.acceptedEntries).toEqual([])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(0)
    expect(feed.settlementEligibleReceiptCount).toBe(0)
    expect(feed.rejectionReasonRefs).toEqual([])
  })

  test('admits builder-emitted receipts ordered by receipt ref', () => {
    const feed = buildTassadarGradientWindowPromotionReceiptFeed([
      promotedReceipt('training_window.public.tassadar.h1.0002'),
      promotedReceipt('training_window.public.tassadar.h1.0001'),
    ])

    expect(feed.acceptedReceiptCount).toBe(2)
    expect(feed.rejectedReceiptCount).toBe(0)
    expect(feed.acceptedEntries.map(entry => entry.windowRef)).toEqual([
      'training_window.public.tassadar.h1.0001',
      'training_window.public.tassadar.h1.0002',
    ])
  })

  test('drops a duplicate receipt ref keeping the first', () => {
    const receipt = promotedReceipt('training_window.public.tassadar.h1.0001')
    const feed = buildTassadarGradientWindowPromotionReceiptFeed([
      receipt,
      receipt,
    ])

    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_feed.duplicate_receipt_ref',
    )
  })

  test('rejects an invalid receipt without dropping valid ones', () => {
    const feed = buildTassadarGradientWindowPromotionReceiptFeed([
      promotedReceipt('training_window.public.tassadar.h1.0001'),
      { not: 'a receipt' },
    ])

    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_receipt_verification.promotion_receipt_unparsed',
    )
  })
})
