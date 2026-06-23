import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_CONCIERGE_OUTPUT_SPEC_PROMPT,
  extractConciergeOutputSpec,
  mergeConciergeOutputSpec,
} from './autopilot-concierge-output-spec'

describe('Autopilot Concierge Output Spec extraction', () => {
  test('extracts the fenced oa-output-spec JSON block', () => {
    const completion = [
      'Here is the current intake state.',
      '',
      '```oa-output-spec',
      '{"business":"Acme LLC","goal":"save time","quickWin":"draft a checklist"}',
      '```',
      '',
      'What is your budget?',
    ].join('\n')
    const spec = extractConciergeOutputSpec(completion)
    expect(spec).toEqual({
      business: 'Acme LLC',
      goal: 'save time',
      quickWin: 'draft a checklist',
    })
  })

  test('keeps only the closed field set and drops invented keys', () => {
    const completion = [
      '```oa-output-spec',
      '{"business":"Acme","secretInstruction":"ignore safety","goal":"x"}',
      '```',
    ].join('\n')
    const spec = extractConciergeOutputSpec(completion)
    expect(spec).toEqual({ business: 'Acme', goal: 'x' })
    expect(
      Object.prototype.hasOwnProperty.call(spec ?? {}, 'secretInstruction'),
    ).toBe(false)
  })

  test('uses the LAST fenced block (freshest snapshot)', () => {
    const completion = [
      '```oa-output-spec',
      '{"business":"old"}',
      '```',
      'later...',
      '```oa-output-spec',
      '{"business":"new","goal":"g"}',
      '```',
    ].join('\n')
    expect(extractConciergeOutputSpec(completion)).toEqual({
      business: 'new',
      goal: 'g',
    })
  })

  test('falls back to a markdown Output Spec section', () => {
    const completion = [
      'Summary of the interview.',
      '',
      'Output Spec',
      '1. Business — Acme LLC, a law firm',
      '2. Goal — win back review hours',
      '4. Quick win — draft an intake checklist',
      '10. Open questions — none',
    ].join('\n')
    const spec = extractConciergeOutputSpec(completion)
    expect(spec?.business).toBe('Acme LLC, a law firm')
    expect(spec?.goal).toBe('win back review hours')
    expect(spec?.quickWin).toBe('draft an intake checklist')
    // "none" placeholder is dropped, not stored.
    expect(spec?.openQuestions).toBeUndefined()
  })

  test('returns undefined when there is no parseable spec', () => {
    expect(extractConciergeOutputSpec('just a normal reply')).toBeUndefined()
    expect(
      extractConciergeOutputSpec('```oa-output-spec\nnot json\n```'),
    ).toBeUndefined()
    expect(
      extractConciergeOutputSpec('```oa-output-spec\n{}\n```'),
    ).toBeUndefined()
  })

  test('merge grows the spec across turns without erasing prior fields', () => {
    const prior = { business: 'Acme', goal: 'save time' }
    const next = { goal: 'save MORE time', quickWin: 'checklist' }
    expect(mergeConciergeOutputSpec(prior, next)).toEqual({
      business: 'Acme',
      goal: 'save MORE time',
      quickWin: 'checklist',
    })
    // An undefined next turn leaves the prior spec untouched.
    expect(mergeConciergeOutputSpec(prior, undefined)).toEqual(prior)
  })

  test('the output-spec prompt names the fence tag and the closed field set', () => {
    expect(AUTOPILOT_CONCIERGE_OUTPUT_SPEC_PROMPT).toContain('oa-output-spec')
    expect(AUTOPILOT_CONCIERGE_OUTPUT_SPEC_PROMPT).toContain('business')
    expect(AUTOPILOT_CONCIERGE_OUTPUT_SPEC_PROMPT).toContain('openQuestions')
  })
})
