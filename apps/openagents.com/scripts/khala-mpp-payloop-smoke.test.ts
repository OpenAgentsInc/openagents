import { describe, expect, test } from 'vitest'

const smoke = await import('./khala-mpp-payloop-smoke.mjs')

// Helper: build a base64url-nopad JCS-ish encoding the way the worker does, so
// the decode helpers are exercised against real wire shapes. No network here.
const b64url = obj => smoke.base64UrlEncode(JSON.stringify(obj))

describe('Khala MPP pay-loop smoke helpers (pure, no network)', () => {
  test('redact scrubs agent, bearer, payment, and Stripe secret-shaped values', () => {
    const cleaned = smoke.redact({
      agent: 'oa_agent_super_secret',
      bearer: 'Bearer oa_agent_another_secret',
      payment: 'Authorization: Payment abc.def.ghi',
      credential: 'Payment eyJjaGFsbGVuZ2UiOnsiaWQiOiJ4N1RnMnBMcVI5bUt2In19',
      stripe: 'sk_test_hidden_value_12345',
    })
    expect(cleaned).not.toContain('oa_agent_super_secret')
    expect(cleaned).not.toContain('oa_agent_another_secret')
    expect(cleaned).not.toContain('abc.def.ghi')
    expect(cleaned).not.toContain('sk_test_hidden_value_12345')
    expect(cleaned).not.toContain('eyJjaGFsbGVuZ2UiOnsiaWQiOiJ4N1RnMnBMcVI5bUt2In19')
    expect(cleaned).toContain('[REDACTED]')
  })

  test('base64url round-trips and decodes a JCS record', () => {
    const encoded = b64url({ amount: '1', network: 'base', pi: 'pi_123' })
    expect(smoke.base64UrlDecode(encoded)).toBe(
      '{"amount":"1","network":"base","pi":"pi_123"}',
    )
    expect(smoke.decodeJcsBase64UrlRecord(encoded)).toEqual({
      amount: '1',
      network: 'base',
      pi: 'pi_123',
    })
    expect(smoke.decodeJcsBase64UrlRecord('not!valid!b64')).toBeUndefined()
  })

  test('decodeOpaquePaymentIntentId recovers the bound PaymentIntent id', () => {
    const opaque = b64url({
      amount: '1',
      model: 'openagents/khala-mini',
      network: 'base',
      pi: 'pi_3Test123',
    })
    expect(smoke.decodeOpaquePaymentIntentId(opaque)).toBe('pi_3Test123')
    // Missing pi => undefined.
    expect(
      smoke.decodeOpaquePaymentIntentId(b64url({ amount: '1' })),
    ).toBeUndefined()
    expect(smoke.decodeOpaquePaymentIntentId('')).toBeUndefined()
    expect(smoke.decodeOpaquePaymentIntentId(undefined)).toBeUndefined()
  })

  test('parsePaymentChallengeHeader parses spec WWW-Authenticate params', () => {
    const params = smoke.parsePaymentChallengeHeader(
      'Payment id="abc123", realm="openagents.com", method="base", intent="charge", request="eyJ9", expires="2026-01-01T00:00:00Z", opaque="eyJ9"',
    )
    expect(params).toMatchObject({
      id: 'abc123',
      realm: 'openagents.com',
      method: 'base',
      intent: 'charge',
    })
    // Non-Payment scheme => undefined.
    expect(smoke.parsePaymentChallengeHeader('Bearer xyz')).toBeUndefined()
    expect(smoke.parsePaymentChallengeHeader(undefined)).toBeUndefined()
  })

  test('recoverCryptoChallenge picks the crypto challenge + recovers PI + recipient', () => {
    const opaque = b64url({
      amount: '1',
      network: 'base',
      pi: 'pi_3Recover',
    })
    const request = b64url({
      amount: '1',
      currency: 'usdc',
      network: 'base',
      recipient: '0xDeposITaddr',
    })
    const header = `Payment id="cid", realm="openagents.com", method="base", intent="charge", request="${request}", expires="2026-01-01T00:00:00Z", opaque="${opaque}", Payment id="sid", realm="openagents.com", method="stripe", intent="charge", request="${request}", expires="2026-01-01T00:00:00Z"`
    const result = {
      status: 402,
      headers: new Headers({ 'www-authenticate': header }),
      body: {
        challenges: [
          {
            id: 'cid',
            method: 'base',
            recipient: '0xDeposITaddr',
            paymentIntentId: 'pi_3Recover',
          },
        ],
      },
    }
    const crypto = smoke.recoverCryptoChallenge(result, ['base', 'solana', 'tempo'])
    expect(crypto?.network).toBe('base')
    expect(crypto?.paymentIntentId).toBe('pi_3Recover')
    expect(crypto?.recipient).toBe('0xDeposITaddr')
    // The echoed challenge carries exactly the fields the server re-binds over.
    expect(crypto?.challenge).toMatchObject({
      id: 'cid',
      realm: 'openagents.com',
      method: 'base',
      intent: 'charge',
      request,
      expires: '2026-01-01T00:00:00Z',
      opaque,
    })
  })

  test('buildPaymentCredential echoes the challenge and is decodable per spec', () => {
    const challenge = {
      id: 'cid',
      realm: 'openagents.com',
      method: 'base',
      intent: 'charge',
      request: b64url({ amount: '1', currency: 'usdc' }),
      expires: '2026-01-01T00:00:00Z',
      opaque: b64url({ pi: 'pi_3Echo' }),
    }
    const credential = smoke.buildPaymentCredential({
      challenge,
      payload: { network: 'base', transaction_hash: '0xtestsuccess' },
    })
    // Decodes back to { challenge, payload } with the challenge echoed verbatim.
    const decoded = smoke.decodeJcsBase64UrlRecord(credential)
    expect(decoded?.challenge).toEqual(challenge)
    expect(decoded?.payload).toEqual({
      network: 'base',
      transaction_hash: '0xtestsuccess',
    })
    // source is optional and omitted when not provided.
    expect('source' in (decoded ?? {})).toBe(false)
  })

  test('mppCreditGrantReceiptRef derives the dereferenceable grant ref', () => {
    expect(smoke.mppCreditGrantReceiptRef('pi_3Grant')).toBe(
      'receipt.inference.usd_credit_grant.mpp:pi_3Grant',
    )
  })

  test('parseArgs honors flags + env defaults and rejects unknown args', () => {
    const options = smoke.parseArgs([
      '--base-url',
      'https://staging.example',
      '--stripe-test-key',
      'sk_test_abc',
      '--model',
      'openagents/khala-mini',
      '--settle-timeout-ms',
      '30000',
      '--json',
    ])
    expect(options.baseUrl).toBe('https://staging.example')
    expect(options.stripeTestKey).toBe('sk_test_abc')
    expect(options.model).toBe('openagents/khala-mini')
    expect(options.settleTimeoutMs).toBe(30000)
    expect(options.json).toBe(true)
    expect(() => smoke.parseArgs(['--wat'])).toThrowError(/Unknown argument/)
  })

  test('buildSummary is complete only without failures or skips; ok ignores skips', () => {
    expect(
      smoke.buildSummary([
        { name: 'a', status: 'PASS', details: {} },
        { name: 'b', status: 'SKIP', details: {} },
      ]).complete,
    ).toBe(false)
    expect(
      smoke.buildSummary([{ name: 'a', status: 'PASS', details: {} }]).complete,
    ).toBe(true)
    expect(
      smoke.buildSummary([{ name: 'a', status: 'FAIL', details: {} }]).failed,
    ).toBe(1)
  })
})
