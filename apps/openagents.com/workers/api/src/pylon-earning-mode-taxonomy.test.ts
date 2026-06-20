import { describe, expect, test } from 'vitest'

import {
  canonicalizeEarningModeFamily,
  distinctEarningModeFamilies,
  isSameEarningModeFamily,
} from './pylon-earning-mode-taxonomy'

describe('pylon earning-mode family canonicalizer (#5527)', () => {
  test('a plain label is its own family', () => {
    expect(canonicalizeEarningModeFamily('training')).toBe('training')
    expect(canonicalizeEarningModeFamily('forum_tips')).toBe('forum_tips')
    expect(canonicalizeEarningModeFamily('compute')).toBe('compute')
  })

  test('drops trailing version/variant segments to one family', () => {
    expect(canonicalizeEarningModeFamily('training_v2')).toBe('training')
    expect(canonicalizeEarningModeFamily('training.v02')).toBe('training')
    expect(canonicalizeEarningModeFamily('forum_tips_2')).toBe('forum_tips')
    expect(canonicalizeEarningModeFamily('compute-alt')).toBe('compute')
    expect(canonicalizeEarningModeFamily('training_new')).toBe('training')
  })

  test('strips a within-segment numeric/version suffix', () => {
    expect(canonicalizeEarningModeFamily('training2')).toBe('training')
    expect(canonicalizeEarningModeFamily('trainingv2')).toBe('training')
  })

  test('is case- and separator-insensitive', () => {
    expect(canonicalizeEarningModeFamily('Training/V2')).toBe('training')
    expect(canonicalizeEarningModeFamily('FORUM_TIPS')).toBe('forum_tips')
  })

  test('keeps genuinely distinct stems distinct (no false collapse)', () => {
    expect(canonicalizeEarningModeFamily('training')).not.toBe(
      canonicalizeEarningModeFamily('forum_tips'),
    )
    expect(canonicalizeEarningModeFamily('mowing')).not.toBe(
      canonicalizeEarningModeFamily('painting'),
    )
  })

  test('does not reduce a short token to noise', () => {
    // No meaningful stem to keep; the token survives intact.
    expect(canonicalizeEarningModeFamily('v2')).toBe('v2')
  })

  test('isSameEarningModeFamily detects label-split spellings', () => {
    expect(isSameEarningModeFamily('training', 'training_v2')).toBe(true)
    expect(isSameEarningModeFamily('training', 'forum_tips')).toBe(false)
  })

  test('distinctEarningModeFamilies collapses splits, preserves order', () => {
    expect(
      distinctEarningModeFamilies([
        'training',
        'training_v2',
        'forum_tips',
        'forum_tips_2',
        'compute',
      ]),
    ).toEqual(['training', 'forum_tips', 'compute'])
  })

  test('two labels of one mode count as ONE family', () => {
    expect(
      distinctEarningModeFamilies(['training', 'training_v2']),
    ).toHaveLength(1)
  })
})
