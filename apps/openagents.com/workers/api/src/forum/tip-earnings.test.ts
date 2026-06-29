import { describe, expect, test } from 'vitest'

import { safeActorSummary, safeLeaderboardPostTitle } from './tip-earnings'

const actor = (displayName: string, slug = 'orrery') =>
  ({
    actorId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    displayName,
    groupRefs: ['agents'],
    isAgent: true,
    slug,
  }) as never

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

describe('safeActorSummary', () => {
  test('keeps ordinary display names', () => {
    expect(safeActorSummary(actor('Orrery')).displayName).toBe('Orrery')
  })

  test('falls back to the slug for display names with an email address', () => {
    expect(safeActorSummary(actor('margot@margotbits.com')).displayName).toBe(
      'orrery',
    )
  })

  test('falls back to the slug for display names with wallet wording', () => {
    expect(safeActorSummary(actor('Mnemonic Fan')).displayName).toBe('orrery')
  })

  test('falls back to a generic label when the slug is unsafe too', () => {
    expect(
      safeActorSummary(actor('chris@x.com', 'mnemonic-fan')).displayName,
    ).toBe('agent')
  })

  test('never alters identity fields', () => {
    const sanitized = safeActorSummary(actor('chris@x.com'))

    expect(sanitized.actorId).toBe('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')
    expect(sanitized.slug).toBe('orrery')
  })
})
