import { describe, expect, test } from 'vitest'

import { base64UrlEncode, jcsBase64Url } from './mpp-canonical'
import {
  type MppChallenge,
  type MppRequestParams,
  buildChallenge,
  buildPaymentReceipt,
  buildPaymentRequiredHeaders,
  buildPaymentRequiredProblem,
  parsePaymentCredential,
  renderChallengeHeader,
  verifyCredential,
} from './mpp-protocol'

const SECRET = 'test-signing-secret'
const REALM = 'openagents.com'
const FUTURE = '2099-01-15T12:05:00.000Z'

// Build a spec-shaped crypto challenge with a valid HMAC id.
const cryptoChallenge = (
  overrides: Partial<Parameters<typeof buildChallenge>[1]> = {},
): Promise<MppChallenge> =>
  buildChallenge(SECRET, {
    amountCents: 100,
    currency: 'usdc',
    expires: FUTURE,
    method: 'base',
    network: 'base',
    opaque: { amount: '100', network: 'base', pi: 'pi_dep_1' },
    paymentIntentId: 'pi_dep_1',
    realm: REALM,
    recipient: '0xdeadbeef',
    request: {
      amount: '100',
      currency: 'usdc',
      network: 'base',
      recipient: '0xdeadbeef',
    },
    ...overrides,
  })

// Compose a wire credential that echoes a challenge + carries a payload.
const credentialFor = (
  challenge: MppChallenge,
  payload: Record<string, unknown>,
): string =>
  base64UrlEncode(
    JSON.stringify({
      challenge: {
        expires: challenge.expires,
        id: challenge.id,
        intent: challenge.intent,
        method: challenge.method,
        opaque: challenge.opaque,
        realm: challenge.realm,
        request: challenge.request,
      },
      payload,
    }),
  )

describe('mpp protocol — 402 challenge wire shape (HMAC-bound)', () => {
  test('renders a WWW-Authenticate Payment header with the spec params', async () => {
    const challenge = await cryptoChallenge()
    const header = renderChallengeHeader(challenge)
    expect(header.startsWith('Payment ')).toBe(true)
    expect(header).toContain(`id="${challenge.id}"`)
    expect(header).toContain('realm="openagents.com"')
    expect(header).toContain('method="base"')
    expect(header).toContain('intent="charge"')
    expect(header).toContain(`request="${challenge.request}"`)
    expect(header).toContain(`expires="${FUTURE}"`)
    expect(header).toContain('opaque=')
    // The id is an HMAC, not a plaintext quote id.
    expect(challenge.id).not.toContain(':crypto')
  })

  test('builds one WWW-Authenticate per challenge + problem body', async () => {
    const crypto = await cryptoChallenge()
    const card = await buildChallenge(SECRET, {
      amountCents: 50,
      currency: 'usd',
      expires: FUTURE,
      method: 'stripe',
      realm: REALM,
      request: {
        amount: '50',
        currency: 'usd',
        methodDetails: { networkId: 'profile_x', paymentMethodTypes: ['card'] },
      },
    })
    const headers = buildPaymentRequiredHeaders([crypto, card])
    const all = headers.get('www-authenticate')
    expect(all).toContain('method="base"')
    expect(all).toContain('method="stripe"')
    expect(headers.get('cache-control')).toBe('no-store')

    const problem = buildPaymentRequiredProblem([crypto, card])
    expect(problem.status).toBe(402)
    expect(problem.type).toBe(
      'https://paymentauth.org/problems/payment-required',
    )
    expect(problem.challenges).toHaveLength(2)
    expect(problem.challenges[0]?.paymentIntentId).toBe('pi_dep_1')
  })

  test('Payment-Receipt is base64url JCS with status success', () => {
    const receipt = buildPaymentReceipt({
      method: 'base',
      reference: 'pi_1',
      timestamp: FUTURE,
    })
    expect(receipt).not.toContain('=')
    expect(receipt).toBe(
      jcsBase64Url({
        method: 'base',
        reference: 'pi_1',
        status: 'success',
        timestamp: FUTURE,
      }),
    )
  })
})

describe('mpp protocol — credential parsing', () => {
  test('undefined for missing / non-Payment scheme', () => {
    expect(parsePaymentCredential(null)).toBeUndefined()
    expect(parsePaymentCredential('Bearer xyz')).toBeUndefined()
  })

  test('undefined for malformed (no challenge/payload)', () => {
    expect(parsePaymentCredential('Payment not-base64')).toBeUndefined()
    expect(
      parsePaymentCredential(`Payment ${base64UrlEncode('{"x":1}')}`),
    ).toBeUndefined()
  })

  test('parses an echoed-challenge + payload credential', async () => {
    const challenge = await cryptoChallenge()
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, { ref: '0xpay' })}`,
    )
    expect(parsed?.scheme).toBe('Payment')
    expect(parsed?.challenge.id).toBe(challenge.id)
    expect(parsed?.challenge.method).toBe('base')
    expect(parsed?.payload.ref).toBe('0xpay')
  })
})

describe('mpp protocol — stateless HMAC verification (FAIL-CLOSED)', () => {
  const expectations = {
    allowedMethods: ['base', 'solana', 'tempo', 'stripe'],
    expectedCurrencyForMethod: (m: string) => (m === 'stripe' ? 'usd' : 'usdc'),
    expectedMinAmountCents: 1,
    nowMs: Date.parse('2026-06-23T12:00:00.000Z'),
    realm: REALM,
  }

  test('accepts a correctly-bound credential and recovers opaque/request', async () => {
    const challenge = await cryptoChallenge()
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, expectations)
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.method).toBe('base')
      expect(result.opaque?.pi).toBe('pi_dep_1')
      expect(result.request.amount).toBe('100')
    }
  })

  test('rejects a tampered challenge (forged id)', async () => {
    const challenge = await cryptoChallenge()
    const tampered: MppChallenge = { ...challenge, id: `${challenge.id}x` }
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(tampered, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, expectations)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid-challenge')
  })

  test('rejects a credential whose request was swapped (binding catches it)', async () => {
    const challenge = await cryptoChallenge()
    // Swap the bound request for a cheaper one but keep the original id.
    const cheaperRequest: MppRequestParams = {
      amount: '1',
      currency: 'usdc',
      network: 'base',
    }
    const tampered: MppChallenge = {
      ...challenge,
      request: jcsBase64Url(cheaperRequest),
    }
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(tampered, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, expectations)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid-challenge')
  })

  test('rejects an expired challenge', async () => {
    const challenge = await cryptoChallenge({
      expires: '2000-01-01T00:00:00.000Z',
    })
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, expectations)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('expired')
  })

  test('rejects a wrong-secret credential (different signer)', async () => {
    const challenge = await buildChallenge('OTHER-secret', {
      amountCents: 100,
      currency: 'usdc',
      expires: FUTURE,
      method: 'base',
      realm: REALM,
      request: { amount: '100', currency: 'usdc' },
    })
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, expectations)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid-challenge')
  })

  test('rejects a method we never offered', async () => {
    const challenge = await cryptoChallenge()
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, {
      ...expectations,
      allowedMethods: ['solana'], // base not offered
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('method-mismatch')
  })

  test('rejects a request whose currency does not match the method', async () => {
    const challenge = await cryptoChallenge()
    const parsed = parsePaymentCredential(
      `Payment ${credentialFor(challenge, {})}`,
    )!
    const result = await verifyCredential(SECRET, parsed, {
      ...expectations,
      expectedCurrencyForMethod: () => 'eur',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('request-mismatch')
  })
})
