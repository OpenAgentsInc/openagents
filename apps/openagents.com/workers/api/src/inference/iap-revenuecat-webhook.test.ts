import { describe, expect, test } from 'vitest'

import {
  parseRevenueCatWebhookBody,
  verifyRevenueCatWebhookAuth,
} from './iap-revenuecat-webhook'

const fixturePurchase = (overrides: Record<string, unknown> = {}) => ({
  api_version: '1.0',
  event: {
    app_user_id: 'user-1',
    environment: 'SANDBOX',
    id: 'event-1',
    original_transaction_id: 'txn-1',
    product_id: 'credits_999',
    purchased_at_ms: 1751500000000,
    store: 'APP_STORE',
    transaction_id: 'txn-1',
    type: 'NON_RENEWING_PURCHASE',
    ...overrides,
  },
})

describe('verifyRevenueCatWebhookAuth', () => {
  const request = (authorization: string | undefined) =>
    new Request('https://openagents.com/api/webhooks/revenuecat', {
      headers: authorization === undefined ? {} : { authorization },
      method: 'POST',
    })

  test('accepts the raw configured secret in the Authorization header', () => {
    expect(verifyRevenueCatWebhookAuth(request('my-webhook-secret'), 'my-webhook-secret')).toBe(true)
  })

  test('accepts the Bearer-prefixed form too', () => {
    expect(verifyRevenueCatWebhookAuth(request('Bearer my-webhook-secret'), 'my-webhook-secret')).toBe(true)
  })

  test('rejects a wrong secret', () => {
    expect(verifyRevenueCatWebhookAuth(request('wrong-secret'), 'my-webhook-secret')).toBe(false)
  })

  test('rejects a missing header', () => {
    expect(verifyRevenueCatWebhookAuth(request(undefined), 'my-webhook-secret')).toBe(false)
  })

  test('fails closed when no secret is configured (e.g. before #8481 lands one)', () => {
    expect(verifyRevenueCatWebhookAuth(request('anything'), undefined)).toBe(false)
    expect(verifyRevenueCatWebhookAuth(request('anything'), '')).toBe(false)
  })
})

describe('parseRevenueCatWebhookBody', () => {
  test('parses a NON_RENEWING_PURCHASE as kind: purchase', () => {
    const event = parseRevenueCatWebhookBody(fixturePurchase())
    expect(event).toEqual({
      appUserId: 'user-1',
      environment: 'sandbox',
      eventId: 'event-1',
      kind: 'purchase',
      originalTransactionId: 'txn-1',
      productId: 'credits_999',
      rawType: 'NON_RENEWING_PURCHASE',
      store: 'app_store',
      transactionId: 'txn-1',
    })
  })

  test('INITIAL_PURCHASE also parses as kind: purchase', () => {
    const event = parseRevenueCatWebhookBody(fixturePurchase({ type: 'INITIAL_PURCHASE' }))
    expect(event?.kind).toBe('purchase')
  })

  test('REFUND and CANCELLATION both parse as kind: refund', () => {
    expect(parseRevenueCatWebhookBody(fixturePurchase({ type: 'REFUND' }))?.kind).toBe('refund')
    expect(parseRevenueCatWebhookBody(fixturePurchase({ type: 'CANCELLATION' }))?.kind).toBe('refund')
  })

  test('an unrelated lifecycle event (e.g. RENEWAL) parses as kind: ignored, not rejected', () => {
    expect(parseRevenueCatWebhookBody(fixturePurchase({ type: 'RENEWAL' }))?.kind).toBe('ignored')
  })

  test('play_store and unrecognized store values normalize correctly', () => {
    expect(parseRevenueCatWebhookBody(fixturePurchase({ store: 'PLAY_STORE' }))?.store).toBe('play_store')
    expect(parseRevenueCatWebhookBody(fixturePurchase({ store: 'STRIPE' }))?.store).toBe('other')
  })

  test('falls back to transaction_id when original_transaction_id is absent', () => {
    const event = parseRevenueCatWebhookBody(
      fixturePurchase({ original_transaction_id: undefined, transaction_id: 'txn-only' }),
    )
    expect(event?.originalTransactionId).toBe('txn-only')
  })

  test('production environment is recognized', () => {
    expect(parseRevenueCatWebhookBody(fixturePurchase({ environment: 'PRODUCTION' }))?.environment).toBe(
      'production',
    )
  })

  test('returns undefined for a payload missing required fields, never throws', () => {
    expect(parseRevenueCatWebhookBody({ event: { type: 'REFUND' } })).toBeUndefined()
    expect(parseRevenueCatWebhookBody({})).toBeUndefined()
    expect(parseRevenueCatWebhookBody(null)).toBeUndefined()
    expect(parseRevenueCatWebhookBody('not an object')).toBeUndefined()
    expect(parseRevenueCatWebhookBody(42)).toBeUndefined()
  })
})
