import { describe, expect, test } from 'vitest'

import {
  type MppChallenge,
  buildPaymentRequiredHeaders,
  buildPaymentRequiredProblem,
  parsePaymentCredential,
  renderChallengeHeader,
} from './mpp-protocol'

const cryptoChallenge: MppChallenge = {
  amountCents: 1,
  currency: 'usdc',
  id: 'chal_abc:crypto',
  intent: 'charge',
  method: 'base',
  network: 'base',
  paymentIntentId: 'pi_123',
  recipient: '0xdeadbeef',
}

describe('mpp protocol — 402 challenge wire shape', () => {
  test('renders a WWW-Authenticate Payment header with the load-bearing params', () => {
    const header = renderChallengeHeader(cryptoChallenge)
    expect(header.startsWith('Payment ')).toBe(true)
    expect(header).toContain('id="chal_abc:crypto"')
    expect(header).toContain('method="base"')
    expect(header).toContain('intent="charge"')
    expect(header).toContain('amount="1"')
    expect(header).toContain('currency="usdc"')
    expect(header).toContain('network="base"')
    expect(header).toContain('recipient="0xdeadbeef"')
    expect(header).toContain('payment_intent="pi_123"')
  })

  test('builds one WWW-Authenticate header per challenge', () => {
    const headers = buildPaymentRequiredHeaders([
      cryptoChallenge,
      {
        amountCents: 50,
        currency: 'usd',
        id: 'chal_abc:card',
        intent: 'charge',
        method: 'stripe',
      },
    ])
    const all = headers.get('www-authenticate')
    expect(all).toContain('method="base"')
    expect(all).toContain('method="stripe"')
    expect(headers.get('content-type')).toBe('application/problem+json')
    expect(headers.get('cache-control')).toBe('no-store')
  })

  test('builds the problem+json body with the paymentauth.org type', () => {
    const problem = buildPaymentRequiredProblem([cryptoChallenge])
    expect(problem.status).toBe(402)
    expect(problem.type).toBe(
      'https://paymentauth.org/problems/payment-required',
    )
    expect(problem.title).toBe('Payment Required')
    expect(problem.challengeId).toBe('chal_abc:crypto')
    expect(problem.challenges).toHaveLength(1)
    expect(problem.challenges[0]?.paymentIntentId).toBe('pi_123')
  })
})

describe('mpp protocol — credential parsing', () => {
  test('undefined for a missing Authorization header', () => {
    expect(parsePaymentCredential(null)).toBeUndefined()
  })

  test('undefined for a non-Payment scheme (so the endpoint re-challenges)', () => {
    expect(parsePaymentCredential('Bearer xyz')).toBeUndefined()
  })

  test('parses the quoted-param Payment credential and extracts the payment intent', () => {
    const parsed = parsePaymentCredential(
      'Payment id="chal_abc:crypto", method="base", payment_intent="pi_123"',
    )
    expect(parsed?.scheme).toBe('Payment')
    expect(parsed?.challengeId).toBe('chal_abc:crypto')
    expect(parsed?.method).toBe('base')
    expect(parsed?.paymentIntentId).toBe('pi_123')
  })

  test('parses a base64url JSON Payment token and extracts the payment intent', () => {
    const token = btoa(
      JSON.stringify({
        challengeId: 'chal_zzz',
        method: 'tempo',
        payload: { payment_intent: 'pi_999' },
      }),
    )
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
    const parsed = parsePaymentCredential(`Payment ${token}`)
    expect(parsed?.paymentIntentId).toBe('pi_999')
    expect(parsed?.challengeId).toBe('chal_zzz')
  })

  test('retains the raw value when a token does not decode (sidecar verify path)', () => {
    const parsed = parsePaymentCredential('Payment not-base64-or-params')
    expect(parsed?.scheme).toBe('Payment')
    expect(parsed?.paymentIntentId).toBeUndefined()
    expect(parsed?.raw).toBe('not-base64-or-params')
  })
})
