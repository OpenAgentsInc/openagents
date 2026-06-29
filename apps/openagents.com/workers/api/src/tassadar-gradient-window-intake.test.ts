import { describe, expect, test } from 'vitest'

import {
  TassadarGradientWindowIntakeSchemaVersion,
  admitTassadarGradientWindowToQuarantine,
} from './tassadar-gradient-window-intake'
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

describe('Tassadar gradient window intake admission', () => {
  test('admits a well-formed, frozen-core-respecting submission to quarantine only', () => {
    const decision = admitTassadarGradientWindowToQuarantine(
      admissibleSubmission(),
    )

    expect(decision.admitted).toBe(true)
    expect(decision.stage).toBe('quarantined')
    expect(decision.schemaVersion).toBe(
      TassadarGradientWindowIntakeSchemaVersion,
    )
    expect(decision.rejectionReasonRefs).toEqual([])
    expect(decision.quarantineRecordRef).toBe(
      'quarantine.public.tassadar_gradient_window.training_window.public.tassadar.h1.0001',
    )
    expect(decision.authority).toEqual({
      canonicalCheckpointMutationAllowed: false,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      promotionAllowed: false,
      quarantineAdmissionGranted: true,
      settlementMutationAllowed: false,
    })
  })

  test('rejects a submission that targets the compiled core', () => {
    const decision = admitTassadarGradientWindowToQuarantine({
      ...admissibleSubmission(),
      compiledCoreGradientTargeted: true,
      trainableParameterScopes: ['compiled_exact_core.tassadar_alm_numeric_executor'],
    })

    expect(decision.admitted).toBe(false)
    expect(decision.stage).toBe('rejected')
    expect(decision.quarantineRecordRef).toBeNull()
    expect(decision.authority.quarantineAdmissionGranted).toBe(false)
    expect(decision.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.intake.compiled_core_gradient_targeted',
    )
  })

  test('rejects a submission that mutates the frozen core digest', () => {
    const decision = admitTassadarGradientWindowToQuarantine({
      ...admissibleSubmission(),
      frozenCoreDigestAfter: digest('9'),
    })

    expect(decision.admitted).toBe(false)
    expect(decision.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.intake.frozen_core_digest_changed',
    )
  })

  test('rejects a submission missing required evidence refs', () => {
    const decision = admitTassadarGradientWindowToQuarantine({
      ...admissibleSubmission(),
      constructionReceiptRefs: [],
      psionicH1EvidenceRefs: [],
    })

    expect(decision.admitted).toBe(false)
    expect(decision.rejectionReasonRefs).toEqual([
      'blocker.public.tassadar_gradient_window.intake.construction_substrate_missing',
      'blocker.public.tassadar_gradient_window.intake.psionic_h1_evidence_missing',
    ])
  })

  test('never throws on a malformed submission; it returns a rejection', () => {
    const decision = admitTassadarGradientWindowToQuarantine({
      not: 'a candidate',
    })

    expect(decision.admitted).toBe(false)
    expect(decision.stage).toBe('rejected')
    expect(decision.windowRef).toBe(
      'window.public.tassadar_gradient_window.unparsed',
    )
    expect(decision.rejectionReasonRefs).toEqual([
      'blocker.public.tassadar_gradient_window.intake.malformed_submission',
    ])
  })
})
