import { describe, expect, test } from 'vitest'

import {
  preimageHexToBytes,
  readPreimage,
  sha256Hex,
  verifyLightningPreimage,
} from './mpp-lightning-verify'

// Build a (preimage, paymentHash) pair: paymentHash = sha256(preimage bytes).
const pairFor = async (preimageHex: string): Promise<string> => {
  const bytes = preimageHexToBytes(preimageHex)!
  return sha256Hex(bytes)
}

const PREIMAGE_A =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'
const PREIMAGE_B =
  'ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100'

describe('verifyLightningPreimage (draft-lightning-charge-00 §Verification)', () => {
  test('a correct preimage whose sha256 equals the paymentHash verifies', async () => {
    const paymentHash = await pairFor(PREIMAGE_A)
    expect(await verifyLightningPreimage(PREIMAGE_A, paymentHash)).toEqual({
      ok: true,
    })
  })

  test('a different (wrong) preimage is rejected as a mismatch', async () => {
    const paymentHash = await pairFor(PREIMAGE_A)
    const result = await verifyLightningPreimage(PREIMAGE_B, paymentHash)
    expect(result).toEqual({ ok: false, reason: 'mismatch' })
  })

  test('a non-hex / wrong-length / non-string preimage is malformed', async () => {
    const paymentHash = await pairFor(PREIMAGE_A)
    expect(await verifyLightningPreimage('not-hex', paymentHash)).toEqual({
      ok: false,
      reason: 'malformed',
    })
    // too short (63 chars)
    expect(
      await verifyLightningPreimage('a'.repeat(63), paymentHash),
    ).toEqual({ ok: false, reason: 'malformed' })
    // uppercase hex is not accepted (spec: lowercase hex)
    expect(
      await verifyLightningPreimage(PREIMAGE_A.toUpperCase(), paymentHash),
    ).toEqual({ ok: false, reason: 'malformed' })
    expect(await verifyLightningPreimage(undefined, paymentHash)).toEqual({
      ok: false,
      reason: 'malformed',
    })
    expect(await verifyLightningPreimage(123, paymentHash)).toEqual({
      ok: false,
      reason: 'malformed',
    })
  })

  test('a malformed expected paymentHash never verifies', async () => {
    expect(await verifyLightningPreimage(PREIMAGE_A, 'short')).toEqual({
      ok: false,
      reason: 'malformed',
    })
  })

  test('readPreimage returns the string field or undefined (fail-closed)', () => {
    expect(readPreimage({ preimage: PREIMAGE_A })).toBe(PREIMAGE_A)
    expect(readPreimage({ preimage: 123 })).toBeUndefined()
    expect(readPreimage({})).toBeUndefined()
  })
})
