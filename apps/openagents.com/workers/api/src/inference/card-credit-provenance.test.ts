import { describe, expect, test } from 'vitest'

import {
  CARD_CREDIT_GRANT_CONTEXT_PREFIX,
  cardCreditGrantContextRef,
  parseCardCreditGrantContextRef,
} from './card-credit-provenance'

describe('cardCreditGrantContextRef / parseCardCreditGrantContextRef', () => {
  test('round-trips a Stripe checkout session id through the grant context_ref', () => {
    const contextRef = cardCreditGrantContextRef('cs_test_123')
    expect(contextRef).toBe(
      `${CARD_CREDIT_GRANT_CONTEXT_PREFIX}cs_test_123`,
    )
    expect(parseCardCreditGrantContextRef(contextRef ?? '')).toBe('cs_test_123')
  })

  test('returns undefined for a blank or delimiter-bearing session id', () => {
    expect(cardCreditGrantContextRef('   ')).toBeUndefined()
    // A colon would make the round-trip ambiguous, so it is refused.
    expect(cardCreditGrantContextRef('cs:bad')).toBeUndefined()
  })

  test('parses the legacy generic grant context_ref to undefined (no card origin)', () => {
    // The pre-provenance bridge format carries the user, not a session.
    expect(
      parseCardCreditGrantContextRef('inference:usd-credit:user_42'),
    ).toBeUndefined()
  })

  test('parses an unrelated or malformed context_ref to undefined', () => {
    expect(parseCardCreditGrantContextRef('')).toBeUndefined()
    expect(parseCardCreditGrantContextRef('receipt.something.else')).toBeUndefined()
    // Card prefix present but no trailing session id.
    expect(
      parseCardCreditGrantContextRef(CARD_CREDIT_GRANT_CONTEXT_PREFIX),
    ).toBeUndefined()
  })
})
