import { describe, expect, test } from 'vitest'

import {
  networkFromBolt11,
  validateLightningInvoice,
} from './mpp-lightning-invoice'

const HASH =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'
const MAINNET_INVOICE = `lnbc100n1p${'a'.repeat(40)}`
const REGTEST_INVOICE = `lnbcrt100n1p${'a'.repeat(40)}`
const SIGNET_INVOICE = `lntbs100n1p${'a'.repeat(40)}`

describe('networkFromBolt11', () => {
  test('reads the network from the BOLT11 human-readable prefix', () => {
    expect(networkFromBolt11(MAINNET_INVOICE)).toBe('mainnet')
    expect(networkFromBolt11(REGTEST_INVOICE)).toBe('regtest')
    expect(networkFromBolt11(SIGNET_INVOICE)).toBe('signet')
    expect(networkFromBolt11('notaninvoice')).toBeUndefined()
  })
})

describe('validateLightningInvoice (surface validation, fail-closed)', () => {
  test('accepts a well-formed mainnet invoice + 64-hex paymentHash', () => {
    const invoice = validateLightningInvoice({
      bolt11: MAINNET_INVOICE,
      paymentHash: HASH,
    })
    expect(invoice).toBeDefined()
    expect(invoice?.network).toBe('mainnet')
    expect(invoice?.paymentHash).toBe(HASH)
    expect(invoice?.bolt11).toBe(MAINNET_INVOICE)
  })

  test('normalizes invoice expiry to ISO when present', () => {
    const invoice = validateLightningInvoice({
      bolt11: MAINNET_INVOICE,
      invoiceExpiresAt: '2099-01-15T12:05:00Z',
      paymentHash: HASH,
    })
    expect(invoice?.invoiceExpiresAt).toBe('2099-01-15T12:05:00.000Z')
  })

  test('rejects a non-bolt11 string', () => {
    expect(
      validateLightningInvoice({ bolt11: 'http://evil', paymentHash: HASH }),
    ).toBeUndefined()
    expect(
      validateLightningInvoice({ bolt11: 123, paymentHash: HASH }),
    ).toBeUndefined()
  })

  test('rejects a malformed / wrong-length paymentHash', () => {
    expect(
      validateLightningInvoice({ bolt11: MAINNET_INVOICE, paymentHash: 'short' }),
    ).toBeUndefined()
    expect(
      validateLightningInvoice({ bolt11: MAINNET_INVOICE, paymentHash: 999 }),
    ).toBeUndefined()
  })

  test('lowercases an uppercase paymentHash so it always stores lowercase hex', () => {
    const invoice = validateLightningInvoice({
      bolt11: MAINNET_INVOICE,
      paymentHash: HASH.toUpperCase(),
    })
    expect(invoice?.paymentHash).toBe(HASH)
  })
})
