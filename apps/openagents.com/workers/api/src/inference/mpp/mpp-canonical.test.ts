import { describe, expect, test } from 'vitest'

import {
  base64UrlDecode,
  base64UrlEncode,
  canonicalJson,
  challengeBindingInput,
  computeChallengeId,
  constantTimeEqual,
  decodeJcsBase64UrlRecord,
  jcsBase64Url,
} from './mpp-canonical'

describe('mpp-canonical — base64url-nopad', () => {
  test('round-trips a UTF-8 string with no padding', () => {
    const value = '{"a":1,"b":"two"}'
    const encoded = base64UrlEncode(value)
    expect(encoded).not.toContain('=')
    expect(encoded).not.toContain('+')
    expect(encoded).not.toContain('/')
    expect(base64UrlDecode(encoded)).toBe(value)
  })

  test('decode returns undefined for invalid input', () => {
    // A character outside the base64url alphabet.
    expect(base64UrlDecode('!!not-valid!!')).toBeUndefined()
  })
})

describe('mpp-canonical — JCS (RFC 8785)', () => {
  test('sorts object keys deterministically regardless of insertion order', () => {
    const a = canonicalJson({ b: '2', a: '1', c: '3' })
    const b = canonicalJson({ c: '3', a: '1', b: '2' })
    expect(a).toBe(b)
    expect(a).toBe('{"a":"1","b":"2","c":"3"}')
  })

  test('omits undefined members and serializes nested objects', () => {
    expect(
      canonicalJson({ z: undefined, a: { y: '2', x: '1' } }),
    ).toBe('{"a":{"x":"1","y":"2"}}')
  })

  test('jcsBase64Url then decode is a stable round-trip', () => {
    const opaque = { pi: 'pi_123', amount: '100', network: 'base' }
    const encoded = jcsBase64Url(opaque)
    const decoded = decodeJcsBase64UrlRecord(encoded)
    expect(decoded).toEqual(opaque)
    // Re-encoding the decoded record yields the SAME base64url (canonical).
    expect(jcsBase64Url(decoded)).toBe(encoded)
  })
})

describe('mpp-canonical — HMAC challenge binding', () => {
  const slots = {
    digest: '',
    expires: '2026-06-23T12:05:00.000Z',
    intent: 'charge',
    method: 'base',
    opaqueB64Url: jcsBase64Url({ pi: 'pi_1' }),
    realm: 'openagents.com',
    requestB64Url: jcsBase64Url({ amount: '100', currency: 'usdc' }),
  }

  test('builds the seven-slot pipe-joined input', () => {
    const input = challengeBindingInput(slots)
    expect(input.split('|')).toHaveLength(7)
    expect(input.startsWith('openagents.com|base|charge|')).toBe(true)
  })

  test('id is deterministic for the same secret + slots', async () => {
    const a = await computeChallengeId('secret-key', slots)
    const b = await computeChallengeId('secret-key', slots)
    expect(a).toBe(b)
    expect(a).not.toContain('=')
  })

  test('id changes when the secret changes (binding is keyed)', async () => {
    const a = await computeChallengeId('secret-A', slots)
    const b = await computeChallengeId('secret-B', slots)
    expect(a).not.toBe(b)
  })

  test('id changes when any slot is tampered', async () => {
    const base = await computeChallengeId('k', slots)
    const tampered = await computeChallengeId('k', {
      ...slots,
      requestB64Url: jcsBase64Url({ amount: '1', currency: 'usdc' }),
    })
    expect(base).not.toBe(tampered)
  })
})

describe('mpp-canonical — constantTimeEqual', () => {
  test('true for equal, false for unequal (incl. different lengths)', () => {
    expect(constantTimeEqual('abc', 'abc')).toBe(true)
    expect(constantTimeEqual('abc', 'abd')).toBe(false)
    expect(constantTimeEqual('abc', 'abcd')).toBe(false)
  })
})
