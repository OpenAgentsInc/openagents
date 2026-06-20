import { describe, expect, test } from 'vitest'

import { buildTassadarGradientWindowQuarantineRecord } from './tassadar-gradient-window-quarantine-record'
import {
  TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
  verifyTassadarGradientWindowQuarantineRecord,
} from './tassadar-gradient-window-quarantine-record-verify'
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

const builtRecord = () =>
  buildTassadarGradientWindowQuarantineRecord(admissibleSubmission(), {
    admittedAt,
  })

describe('Tassadar gradient window quarantine record verifier', () => {
  test('accepts a record emitted by the canonical builder', () => {
    const verification = verifyTassadarGradientWindowQuarantineRecord(
      builtRecord(),
    )

    expect(verification.valid).toBe(true)
    expect(verification.invalidReasonRefs).toEqual([])
    expect(verification.publicSafe).toBe(true)
    expect(verification.promotionEligible).toBe(false)
    expect(verification.schemaVersion).toBe(
      TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
    )
    expect(verification.windowRef).toBe(
      'training_window.public.tassadar.h1.0001',
    )
    expect(verification.recordRef).toBe(
      'quarantine.public.tassadar_gradient_window.training_window.public.tassadar.h1.0001',
    )
    expect(verification.pendingVerificationStages).toEqual([
      'recomputed',
      'replicated',
      'canary_passed',
      'promoted',
    ])
  })

  test('rejects an unparseable record without throwing', () => {
    const verification = verifyTassadarGradientWindowQuarantineRecord({
      not: 'a record',
    })

    expect(verification.valid).toBe(false)
    expect(verification.recordRef).toBeNull()
    expect(verification.windowRef).toBeNull()
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.quarantine_record_unparsed',
    )
  })

  test('rejects a record whose ref does not derive from its window ref', () => {
    const tampered = {
      ...builtRecord(),
      recordRef:
        'quarantine.public.tassadar_gradient_window.training_window.public.tassadar.h1.9999',
    }

    const verification = verifyTassadarGradientWindowQuarantineRecord(tampered)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.record_ref_mismatch',
    )
  })

  test('rejects a record stripped of its admission evidence', () => {
    const record = builtRecord()
    const stripped = {
      ...record,
      evidenceRefs: {
        ...record.evidenceRefs,
        psionicH1EvidenceRefs: [],
      },
    }

    const verification = verifyTassadarGradientWindowQuarantineRecord(stripped)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.psionic_h1_evidence_refs_missing',
    )
  })

  test('rejects a record whose pending verification debt was tampered', () => {
    const tampered = {
      ...builtRecord(),
      pendingVerificationStages: ['promoted'],
    }

    const verification = verifyTassadarGradientWindowQuarantineRecord(tampered)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.pending_verification_stages_tampered',
    )
  })

  test('rejects a record carrying unsafe material', () => {
    const record = builtRecord()
    const leaky = {
      ...record,
      sourceRefs: [...record.sourceRefs, 'wallet_mnemonic seed phrase leaked'],
    }

    const verification = verifyTassadarGradientWindowQuarantineRecord(leaky)

    expect(verification.valid).toBe(false)
    expect(verification.invalidReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.unsafe_material',
    )
  })
})
