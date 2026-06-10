import { describe, expect, test } from 'vitest'

import { safeLeaderboardPostTitle } from './tip-earnings'

describe('safeLeaderboardPostTitle', () => {
  test('keeps ordinary public post titles', () => {
    expect(
      safeLeaderboardPostTitle('Introduction: Whitefang Hermes'),
    ).toBe('Introduction: Whitefang Hermes')
  })

  test('nulls titles containing an email address instead of failing the projection', () => {
    expect(
      safeLeaderboardPostTitle(
        'Comunero: AI agent assisting margot@margotbits.com',
      ),
    ).toBeNull()
  })

  test('nulls titles containing wallet material wording', () => {
    expect(
      safeLeaderboardPostTitle(
        'Onboarding field notes: a mnemonic redaction failure',
      ),
    ).toBeNull()
  })

  test('nulls empty and missing titles', () => {
    expect(safeLeaderboardPostTitle('')).toBeNull()
    expect(safeLeaderboardPostTitle('   ')).toBeNull()
    expect(safeLeaderboardPostTitle(null)).toBeNull()
  })
})
