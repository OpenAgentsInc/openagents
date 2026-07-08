import { describe, expect, test } from 'vitest'

import {
  assessVerificationIntegrity,
  inferVerificationModelFamily,
} from './verification-integrity'

describe('verification integrity — model-family independence (#6423)', () => {
  test('normalizes served model ids into coarse model families', () => {
    expect(inferVerificationModelFamily('accounts/fireworks/models/kimi-k2p7-code')).toBe(
      'model_family.kimi',
    )
    expect(inferVerificationModelFamily('vertex/gemini-3.5-flash')).toBe(
      'model_family.gemini',
    )
    expect(inferVerificationModelFamily('deterministic/browser-acceptance-runner')).toBe(
      'model_family.deterministic_acceptance_runner',
    )
  })

  test('blocks a verifier panel member from the worker model family', () => {
    const report = assessVerificationIntegrity({
      panel: [
        { model: 'openai/gpt-4.1', verifierRef: 'verifier.public.gpt_4_1.a' },
        { model: 'anthropic/claude-opus-4', verifierRef: 'verifier.public.claude.b' },
      ],
      workerModel: 'openai/gpt-4.1-mini',
    })

    expect(report.passed).toBe(false)
    expect(report.workerModelFamily).toBe('model_family.openai')
    expect(report.effectiveIndependentVotes).toBe(1)
    expect(report.blockerRefs).toContain(
      'blocker.public.verification_integrity.same_model_family_verifier',
    )
  })

  test('measures nominal judges versus effective independent votes by family', () => {
    const report = assessVerificationIntegrity({
      minimumEffectiveIndependentVotes: 2,
      panel: [
        { model: 'google/gemini-2.5-pro', verifierRef: 'verifier.public.gemini.a' },
        { model: 'vertex/gemini-3.5-flash', verifierRef: 'verifier.public.gemini.b' },
        { model: 'anthropic/claude-opus-4', verifierRef: 'verifier.public.claude.c' },
        { model: 'deepseek/deepseek-v4', verifierRef: 'verifier.public.deepseek.d' },
      ],
      workerModel: 'openai/gpt-oss-120b',
    })

    expect(report.passed).toBe(true)
    expect(report.nominalVotes).toBe(4)
    expect(report.effectiveIndependentVotes).toBe(3)
    expect(report.panelDiversityLabel).toBe(
      '4 nominal judge(s), 3 effective independent vote(s)',
    )
  })

  test('requires paraphrase, cross-model, and steganography channel defenses', () => {
    const report = assessVerificationIntegrity({
      channelDefenses: {
        crossModelReview: false,
        paraphraseBeforeReview: false,
        steganographyScan: false,
      },
      panel: [{ model: 'anthropic/claude-opus-4', verifierRef: 'verifier.public.claude' }],
      workerModel: 'openai/gpt-4.1-mini',
    })

    expect(report.passed).toBe(false)
    expect(report.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.verification_integrity.paraphrase_defense_missing',
        'blocker.public.verification_integrity.cross_model_review_missing',
        'blocker.public.verification_integrity.steganography_scan_missing',
      ]),
    )
  })
})
