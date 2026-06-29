import { describe, expect, it } from 'vitest'

import { isLightningAddress, resolveLightningAddressInvoice } from './lnurl-pay'

describe('isLightningAddress', () => {
  it('accepts lud16 addresses and rejects invoices/offers/garbage', () => {
    expect(isLightningAddress('oa9982abc@spark.example.com')).toBe(true)
    expect(isLightningAddress('Name.Tag+1@sub.domain.io')).toBe(true)
    expect(isLightningAddress('lnbc1pxyz...')).toBe(false)
    expect(isLightningAddress('lno1pggx...')).toBe(false)
    expect(isLightningAddress('not-an-address')).toBe(false)
    expect(isLightningAddress('missing@domain')).toBe(false)
  })
})

describe('resolveLightningAddressInvoice', () => {
  const fakeFetch = (
    routes: Record<string, { status?: number; body: unknown }>,
  ): typeof fetch =>
    (async (url: unknown) => {
      const key = String(url)
      const match = Object.keys(routes).find(prefix => key.startsWith(prefix))
      if (match === undefined) return new Response('not found', { status: 404 })
      const { status = 200, body } = routes[match]!
      return new Response(JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json' },
      })
    }) as unknown as typeof fetch

  it('resolves an address + amount to a BOLT11 via the LNURL-pay flow', async () => {
    const fetchFn = fakeFetch({
      'https://spark.example.com/.well-known/lnurlp/oa9982': {
        body: {
          tag: 'payRequest',
          callback: 'https://spark.example.com/lnurlp/oa9982/callback',
          minSendable: 1000,
          maxSendable: 100000000,
        },
      },
      'https://spark.example.com/lnurlp/oa9982/callback': {
        body: { pr: 'lnbc500u1pxyzinvoice' },
      },
    })
    const result = await resolveLightningAddressInvoice(
      'oa9982@spark.example.com',
      50000,
      fetchFn,
    )
    expect(result).toEqual({ ok: true, bolt11: 'lnbc500u1pxyzinvoice' })
  })

  it('rejects when the amount is outside min/maxSendable', async () => {
    const fetchFn = fakeFetch({
      'https://spark.example.com/.well-known/lnurlp/oa9982': {
        body: {
          tag: 'payRequest',
          callback: 'https://spark.example.com/cb',
          minSendable: 100000000,
          maxSendable: 200000000,
        },
      },
    })
    const result = await resolveLightningAddressInvoice(
      'oa9982@spark.example.com',
      50000,
      fetchFn,
    )
    expect(result.ok).toBe(false)
  })

  it('rejects a callback error response (no invoice)', async () => {
    const fetchFn = fakeFetch({
      'https://spark.example.com/.well-known/lnurlp/oa9982': {
        body: {
          tag: 'payRequest',
          callback: 'https://spark.example.com/cb',
          minSendable: 1000,
          maxSendable: 100000000,
        },
      },
      'https://spark.example.com/cb': {
        body: { status: 'ERROR', reason: 'unavailable' },
      },
    })
    const result = await resolveLightningAddressInvoice(
      'oa9982@spark.example.com',
      50000,
      fetchFn,
    )
    expect(result.ok).toBe(false)
  })

  it('rejects a non-address input', async () => {
    const result = await resolveLightningAddressInvoice('lnbc1pxyz', 5, fetch)
    expect(result).toEqual({ ok: false, reason: 'not_a_lightning_address' })
  })
})
