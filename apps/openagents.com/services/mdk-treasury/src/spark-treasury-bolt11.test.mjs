import { describe, expect, test } from 'bun:test'

import { bolt11PaymentHashHex } from './spark-treasury.mjs'

describe('bolt11PaymentHashHex (Spark MPP Lightning rail, EPIC #6049)', () => {
  // BOLT11 spec test vector: the `p` payment-hash field is
  // 0001020304050607080900010203040506070809000102030405060708090102. The Spark
  // `receivePayment` response returns only the bolt11; the MPP 402 challenge
  // needs this hash, so the container decodes it from the minted invoice.
  test('decodes the payment hash from a spec BOLT11 invoice', () => {
    const invoice =
      'lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpuaztrnwngzn3kdzw5hydlzf03qdgm2hdq27cqv3agm2awhz5se903vruatfhq77w3ls4evs3ch9zw97j25emudupq63nyw24cg27h2rspfj9srp'
    expect(bolt11PaymentHashHex(invoice)).toBe(
      '0001020304050607080900010203040506070809000102030405060708090102',
    )
  })

  test('returns null for non-string / malformed input (fail-closed)', () => {
    expect(bolt11PaymentHashHex(undefined)).toBeNull()
    expect(bolt11PaymentHashHex('')).toBeNull()
    expect(bolt11PaymentHashHex('not-an-invoice')).toBeNull()
    expect(bolt11PaymentHashHex('lnbc1notbech32!!!')).toBeNull()
  })
})
