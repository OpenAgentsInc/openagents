import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type MppDiscoveryFlags,
  buildMppDiscoveryDocument,
  renderMppDiscoveryDocument,
} from './mpp-discovery-document'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const get = (method = 'GET'): Request =>
  new Request('https://openagents.com/openapi.json', { method })

// Validate one offer object against the draft-payment-discovery-00 JSON Schema
// for the `offer` def (section "JSON Schema for x-payment-info").
const isValidOffer = (offer: unknown): boolean => {
  if (typeof offer !== 'object' || offer === null) {
    return false
  }
  const o = offer as Record<string, unknown>
  // Required: intent (enum charge|session), method (string), amount (null or
  // non-negative-integer string with no leading zeros). additionalProperties:
  // false beyond intent/method/amount/currency/description.
  const allowed = new Set([
    'intent',
    'method',
    'amount',
    'currency',
    'description',
  ])
  for (const key of Object.keys(o)) {
    if (!allowed.has(key)) {
      return false
    }
  }
  if (o.intent !== 'charge' && o.intent !== 'session') {
    return false
  }
  if (typeof o.method !== 'string') {
    return false
  }
  if (o.amount !== null) {
    if (typeof o.amount !== 'string' || !/^(0|[1-9][0-9]*)$/.test(o.amount)) {
      return false
    }
  }
  if ('currency' in o && typeof o.currency !== 'string') {
    return false
  }
  if ('description' in o && typeof o.description !== 'string') {
    return false
  }
  return true
}

// Validate an `x-payment-info` value against the draft's `oneOf` (single-offer
// shorthand OR { offers: Offer[] } with minItems 1).
const isValidPaymentInfo = (value: unknown): boolean => {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const v = value as Record<string, unknown>
  if ('offers' in v) {
    if (!Array.isArray(v.offers) || v.offers.length < 1) {
      return false
    }
    // additionalProperties: false on the multi-offer form (only `offers`).
    if (Object.keys(v).some(k => k !== 'offers')) {
      return false
    }
    return v.offers.every(isValidOffer)
  }
  return isValidOffer(value)
}

const ARMED: MppDiscoveryFlags = {
  cardRailEnabled: true,
  lightningRailEnabled: false,
  mppEnabled: true,
}
const ARMED_CRYPTO_ONLY: MppDiscoveryFlags = {
  cardRailEnabled: false,
  lightningRailEnabled: false,
  mppEnabled: true,
}
const ARMED_LIGHTNING: MppDiscoveryFlags = {
  cardRailEnabled: true,
  lightningRailEnabled: true,
  mppEnabled: true,
}
const INERT: MppDiscoveryFlags = {
  cardRailEnabled: false,
  lightningRailEnabled: false,
  mppEnabled: false,
}
const INERT_WITH_PROFILE: MppDiscoveryFlags = {
  cardRailEnabled: true,
  lightningRailEnabled: false,
  mppEnabled: false,
}
const INERT_WITH_LIGHTNING_FLAG: MppDiscoveryFlags = {
  cardRailEnabled: false,
  lightningRailEnabled: true,
  mppEnabled: false,
}

describe('MPP discovery document (EPIC #6049 — /openapi.json)', () => {
  test('is a valid OpenAPI 3.1 doc with required top-level fields', () => {
    const doc = buildMppDiscoveryDocument(ARMED)
    expect(doc.openapi).toBe('3.1.0')
    const info = doc.info as Record<string, unknown>
    expect(typeof info.title).toBe('string')
    expect((info.title as string).length).toBeGreaterThan(0)
    expect(typeof info.version).toBe('string')
    const paths = doc.paths as Record<string, unknown>
    expect(Object.keys(paths).length).toBeGreaterThanOrEqual(1)
  })

  test('carries x-service-info with categories ["ai"] and the docs links', () => {
    const doc = buildMppDiscoveryDocument(INERT)
    const svc = doc['x-service-info'] as Record<string, unknown>
    expect(svc.categories).toEqual(['ai'])
    const docs = svc.docs as Record<string, unknown>
    expect(docs.homepage).toBe('https://openagents.com')
    expect(docs.llms).toBe('https://openagents.com/llms.txt')
    expect(typeof docs.apiReference).toBe('string')
  })

  test('HONESTY GATE: armed => advertises the paid MPP path with offers + 402', () => {
    const doc = buildMppDiscoveryDocument(ARMED)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    expect(op).toBeDefined()
    // Valid x-payment-info per the draft JSON Schema.
    const paymentInfo = op['x-payment-info']
    expect(isValidPaymentInfo(paymentInfo)).toBe(true)
    // 402 declared (mandatory for payable operations) + 200.
    const responses = op.responses as Record<string, unknown>
    expect(responses['402']).toBeDefined()
    expect(responses['200']).toBeDefined()
    // requestBody JSON schema for the OpenAI chat-completions shape.
    const body = op.requestBody as {
      content: { 'application/json': { schema: Record<string, unknown> } }
    }
    const schema = body.content['application/json'].schema
    expect(schema.required).toEqual(['model', 'messages'])
    const properties = schema.properties as Record<string, Record<string, unknown>>
    expect(properties.model?.examples).toEqual(['openagents/khala'])
  })

  test('HONESTY GATE: armed crypto-only => crypto offers present, NO card offer', () => {
    const doc = buildMppDiscoveryDocument(ARMED_CRYPTO_ONLY)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    const offers = (op['x-payment-info'] as { offers: ReadonlyArray<Record<string, unknown>> }).offers
    expect(offers.length).toBeGreaterThanOrEqual(1)
    expect(offers.every(o => o.currency === 'usdc')).toBe(true)
    expect(offers.some(o => o.method === 'stripe')).toBe(false)
    // crypto networks advertised
    const methods = offers.map(o => o.method)
    expect(methods).toContain('tempo')
    expect(methods).toContain('base')
    expect(methods).toContain('solana')
  })

  test('HONESTY GATE: armed with profile => card offer present alongside crypto', () => {
    const doc = buildMppDiscoveryDocument(ARMED)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    const offers = (op['x-payment-info'] as { offers: ReadonlyArray<Record<string, unknown>> }).offers
    const card = offers.find(o => o.method === 'stripe')
    expect(card).toBeDefined()
    expect(card?.currency).toBe('usd')
    expect(card?.intent).toBe('charge')
    // USD cents amount is a non-negative-integer string >= the SPT floor (50c).
    expect(typeof card?.amount).toBe('string')
    expect(Number(card?.amount)).toBeGreaterThanOrEqual(50)
  })

  test('HONESTY GATE: inert (flag off) => NO paid path advertised at all', () => {
    const doc = buildMppDiscoveryDocument(INERT)
    const paths = doc.paths as Record<string, unknown>
    expect(paths['/mpp/v1/chat/completions']).toBeUndefined()
    // No operation anywhere advertises x-payment-info or a 402 when inert.
    const json = JSON.stringify(doc)
    expect(json).not.toContain('x-payment-info')
    expect(json).not.toContain('"402"')
  })

  test('HONESTY GATE: inert even when a profile id is set => still no paid path', () => {
    const doc = buildMppDiscoveryDocument(INERT_WITH_PROFILE)
    const paths = doc.paths as Record<string, unknown>
    expect(paths['/mpp/v1/chat/completions']).toBeUndefined()
    expect(JSON.stringify(doc)).not.toContain('x-payment-info')
  })

  test('always describes the free keyed /v1/chat/completions surface (no payment offers)', () => {
    const doc = buildMppDiscoveryDocument(INERT)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/v1/chat/completions']?.post as Record<string, unknown>
    expect(op).toBeDefined()
    expect(op['x-payment-info']).toBeUndefined()
    const responses = op.responses as Record<string, unknown>
    expect(responses['402']).toBeUndefined()
  })

  test('crypto offer amount is a USDC base-units integer string (6 decimals)', () => {
    const doc = buildMppDiscoveryDocument(ARMED_CRYPTO_ONLY)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    const offers = (op['x-payment-info'] as { offers: ReadonlyArray<Record<string, unknown>> }).offers
    // 0.01 USDC floor => 10000 base units (6 decimals).
    expect(offers[0]?.amount).toBe('10000')
    expect(/^(0|[1-9][0-9]*)$/.test(offers[0]?.amount as string)).toBe(true)
    expect(offers[0]?.description).toContain('openagents/khala')
  })

  test('serves GET with application/json + public, max-age=300 cache + CORS', async () => {
    const response = await run(renderMppDiscoveryDocument(get('GET'), ARMED))
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('application/json')
    expect(response.headers.get('cache-control')).toBe('public, max-age=300')
    expect(response.headers.get('access-control-allow-origin')).toBe('*')
    const parsed = (await response.json()) as Record<string, unknown>
    expect(parsed.openapi).toBe('3.1.0')
  })

  test('HEAD returns no body, GET returns the doc; non-GET/HEAD is 405', async () => {
    const head = await run(renderMppDiscoveryDocument(get('HEAD'), INERT))
    expect(head.status).toBe(200)
    expect(await head.text()).toBe('')

    const post = await run(renderMppDiscoveryDocument(get('POST'), INERT))
    expect(post.status).toBe(405)
    expect(post.headers.get('allow')).toBe('GET, HEAD')
  })

  test('every offer in an armed doc validates against the draft JSON Schema', () => {
    const doc = buildMppDiscoveryDocument(ARMED)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    expect(isValidPaymentInfo(op['x-payment-info'])).toBe(true)
  })

  test('BITCOIN-FIRST: armed Lightning => Lightning (sat) offer is listed FIRST', () => {
    const doc = buildMppDiscoveryDocument(ARMED_LIGHTNING)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    const offers = (
      op['x-payment-info'] as { offers: ReadonlyArray<Record<string, unknown>> }
    ).offers
    // The FIRST offer is the Lightning charge (Bitcoin-first / preferred).
    expect(offers[0]?.method).toBe('lightning')
    expect(offers[0]?.currency).toBe('sat')
    expect(offers[0]?.intent).toBe('charge')
    // sat amount is a positive-integer string.
    expect(/^[1-9][0-9]*$/.test(offers[0]?.amount as string)).toBe(true)
    // Crypto + card offers still follow.
    const methods = offers.map(o => o.method)
    expect(methods).toContain('tempo')
    expect(methods).toContain('stripe')
    // Every offer still validates against the draft schema.
    expect(isValidPaymentInfo(op['x-payment-info'])).toBe(true)
  })

  test('HONESTY GATE: Lightning flag on but MPP inert => no paid path at all', () => {
    const doc = buildMppDiscoveryDocument(INERT_WITH_LIGHTNING_FLAG)
    const paths = doc.paths as Record<string, unknown>
    expect(paths['/mpp/v1/chat/completions']).toBeUndefined()
    expect(JSON.stringify(doc)).not.toContain('x-payment-info')
    expect(JSON.stringify(doc)).not.toContain('lightning')
  })

  test('HONESTY GATE: Lightning rail OFF => no lightning offer advertised', () => {
    const doc = buildMppDiscoveryDocument(ARMED)
    const paths = doc.paths as Record<string, Record<string, unknown>>
    const op = paths['/mpp/v1/chat/completions']?.post as Record<string, unknown>
    const offers = (
      op['x-payment-info'] as { offers: ReadonlyArray<Record<string, unknown>> }
    ).offers
    expect(offers.some(o => o.method === 'lightning')).toBe(false)
  })
})
