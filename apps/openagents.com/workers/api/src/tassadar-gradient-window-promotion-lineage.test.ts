import { describe, expect, test } from 'vitest'

import { buildTassadarGradientWindowPromotionReceipt } from './tassadar-gradient-window-promotion-receipt'
import {
  TassadarGradientWindowPromotionLineageSchemaVersion,
  verifyTassadarGradientWindowPromotionLineage,
} from './tassadar-gradient-window-promotion-lineage'
import { buildTassadarGradientWindowQuarantineRecord } from './tassadar-gradient-window-quarantine-record'
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

const recordFor = (candidate: TassadarGradientWindowCandidate) =>
  buildTassadarGradientWindowQuarantineRecord(candidate, {
    admittedAt: '2026-06-20T00:00:00.000Z',
  })

const receiptFor = (candidate: TassadarGradientWindowCandidate) =>
  buildTassadarGradientWindowPromotionReceipt(
    projectTassadarGradientWindowRegime({
      candidate,
      receipts: passedReceipts(),
    }),
  )

describe('Tassadar gradient window promotion lineage', () => {
  test('confirms continuity for a record and receipt of the same window', () => {
    const candidate = baseCandidate()
    const decision = verifyTassadarGradientWindowPromotionLineage(
      recordFor(candidate),
      receiptFor(candidate),
    )

    expect(decision.continuous).toBe(true)
    expect(decision.breakReasonRefs).toEqual([])
    expect(decision.publicSafe).toBe(true)
    expect(decision.schemaVersion).toBe(
      TassadarGradientWindowPromotionLineageSchemaVersion,
    )
    expect(decision.windowRef).toBe('training_window.public.tassadar.h1.0001')
    expect(decision.recordRef).toBe(
      'quarantine.public.tassadar_gradient_window.training_window.public.tassadar.h1.0001',
    )
    expect(decision.receiptRef).toBe(
      'receipt.public.tassadar_gradient_window.promoted.training_window.public.tassadar.h1.0001',
    )
  })

  test('breaks lineage when the receipt is for a different window', () => {
    const record = recordFor(baseCandidate())
    const otherReceipt = receiptFor({
      ...baseCandidate(),
      windowRef: 'training_window.public.tassadar.h1.9999',
    })

    const decision = verifyTassadarGradientWindowPromotionLineage(
      record,
      otherReceipt,
    )

    expect(decision.continuous).toBe(false)
    expect(decision.breakReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_lineage.window_ref_mismatch',
    )
  })

  test('breaks lineage when admission evidence is dropped before promotion', () => {
    const candidate = baseCandidate()
    const record = recordFor(candidate)
    const receipt = receiptFor(candidate)
    const evidenceStrippedReceipt = { ...receipt, curatedDataRefs: [] }

    const decision = verifyTassadarGradientWindowPromotionLineage(
      record,
      evidenceStrippedReceipt,
    )

    expect(decision.continuous).toBe(false)
    expect(decision.breakReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.promotion_lineage.curated_data_refs_not_carried',
    )
  })

  test('is total on malformed input rather than throwing', () => {
    const decision = verifyTassadarGradientWindowPromotionLineage(
      { not: 'a record' },
      42,
    )

    expect(decision.continuous).toBe(false)
    expect(decision.breakReasonRefs).toEqual([
      'blocker.public.tassadar_gradient_window.promotion_lineage.promotion_receipt_unparsed',
      'blocker.public.tassadar_gradient_window.promotion_lineage.quarantine_record_unparsed',
    ])
    expect(decision.recordRef).toBeNull()
    expect(decision.receiptRef).toBeNull()
  })
})
