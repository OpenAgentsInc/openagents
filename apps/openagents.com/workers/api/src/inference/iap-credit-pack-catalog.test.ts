import { describe, expect, test } from 'vitest'

import { IAP_CREDIT_PACK_CATALOG, iapCreditPackFromSku } from './iap-credit-pack-catalog'

describe('iapCreditPackFromSku', () => {
  test('resolves every catalog SKU to its own server-owned amount', () => {
    for (const pack of IAP_CREDIT_PACK_CATALOG) {
      expect(iapCreditPackFromSku(pack.sku)).toEqual(pack)
    }
  })

  test('an unknown SKU (e.g. a subscription product, or attacker-supplied) resolves to undefined', () => {
    expect(iapCreditPackFromSku('not_a_real_sku')).toBeUndefined()
    expect(iapCreditPackFromSku('credits_999999')).toBeUndefined()
    expect(iapCreditPackFromSku('')).toBeUndefined()
  })

  test('every catalog amount is a positive integer of cents', () => {
    for (const pack of IAP_CREDIT_PACK_CATALOG) {
      expect(Number.isInteger(pack.amountUsdCents)).toBe(true)
      expect(pack.amountUsdCents).toBeGreaterThan(0)
    }
  })
})
