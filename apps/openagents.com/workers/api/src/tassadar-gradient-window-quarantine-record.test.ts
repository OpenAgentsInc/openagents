import { describe, expect, test } from 'vitest'

import {
  TassadarGradientWindowPendingVerificationStages,
  TassadarGradientWindowQuarantineRecordSchemaVersion,
  TassadarGradientWindowQuarantineRecordUnsafe,
  buildTassadarGradientWindowQuarantineRecord,
  tassadarGradientWindowQuarantineRecordRef,
} from './tassadar-gradient-window-quarantine-record'
import type { TassadarGradientWindowCandidate } from './tassadar-gradient-window-regime'

const digest = (char: string): string => char.repeat(64)

const admissibleSubmission = (): TassadarGradientWindowCandidate => ({
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

const admittedAt = '2026-06-20T00:00:00.000Z'

describe('Tassadar gradient window quarantine record', () => {
  test('builds a deterministic, quarantine-only record for an admitted submission', () => {
    const record = buildTassadarGradientWindowQuarantineRecord(
      admissibleSubmission(),
      { admittedAt },
    )

    expect(record.stage).toBe('quarantined')
    expect(record.schemaVersion).toBe(
      TassadarGradientWindowQuarantineRecordSchemaVersion,
    )
    expect(record.admittedAt).toBe(admittedAt)
    expect(record.publicSafe).toBe(true)
    expect(record.compiledCoreUnchanged).toBe(true)
    expect(record.recordRef).toBe(
      tassadarGradientWindowQuarantineRecordRef(
        'training_window.public.tassadar.h1.0001',
      ),
    )
    expect(record.pendingVerificationStages).toEqual([
      ...TassadarGradientWindowPendingVerificationStages,
    ])
    expect(record.authority).toEqual({
      canonicalCheckpointMutationAllowed: false,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      promotionAllowed: false,
      quarantineResidencyGranted: true,
      settlementMutationAllowed: false,
    })
    expect(record.candidateDigests.updateDigest).toBe(digest('2'))
  })

  test('derives the same record ref as the intake admission decision', () => {
    const record = buildTassadarGradientWindowQuarantineRecord(
      admissibleSubmission(),
      { admittedAt },
    )

    expect(record.recordRef).toBe(
      'quarantine.public.tassadar_gradient_window.training_window.public.tassadar.h1.0001',
    )
  })

  test('refuses to build a record for a compiled-core-targeting submission', () => {
    expect(() =>
      buildTassadarGradientWindowQuarantineRecord({
        ...admissibleSubmission(),
        compiledCoreGradientTargeted: true,
        trainableParameterScopes: [
          'compiled_exact_core.tassadar_alm_numeric_executor',
        ],
      }),
    ).toThrow(TassadarGradientWindowQuarantineRecordUnsafe)
  })

  test('refuses to build a record for a frozen-core-mutating submission', () => {
    expect(() =>
      buildTassadarGradientWindowQuarantineRecord({
        ...admissibleSubmission(),
        frozenCoreDigestAfter: digest('9'),
      }),
    ).toThrow(TassadarGradientWindowQuarantineRecordUnsafe)
  })

  test('refuses to build a record for a malformed submission', () => {
    expect(() =>
      buildTassadarGradientWindowQuarantineRecord({ not: 'a candidate' }),
    ).toThrow(TassadarGradientWindowQuarantineRecordUnsafe)
  })

  test('carries the rejection reasons on refusal', () => {
    try {
      buildTassadarGradientWindowQuarantineRecord({
        ...admissibleSubmission(),
        constructionReceiptRefs: [],
        psionicH1EvidenceRefs: [],
      })
      throw new Error('expected refusal')
    } catch (error) {
      expect(error).toBeInstanceOf(TassadarGradientWindowQuarantineRecordUnsafe)
      const unsafe = error as TassadarGradientWindowQuarantineRecordUnsafe
      expect(unsafe.rejectionReasonRefs).toEqual([
        'blocker.public.tassadar_gradient_window.intake.construction_substrate_missing',
        'blocker.public.tassadar_gradient_window.intake.psionic_h1_evidence_missing',
      ])
    }
  })
})
