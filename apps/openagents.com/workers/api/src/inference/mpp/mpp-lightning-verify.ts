// Lightning charge LOCAL preimage verification (EPIC #6049,
// draft-lightning-charge-00 §Verification). PURE crypto; no IO, no node call.
//
// The Lightning settlement proof is the payment PREIMAGE: a 32-byte secret
// whose SHA-256 equals the invoice's paymentHash. The server verifies payment
// WITHOUT contacting the Lightning node by computing
// sha256(hex_to_bytes(preimage)) and comparing it to the stored paymentHash.
//
// SECURITY (spec §"Preimage Confidentiality"): the preimage is a BEARER SECRET.
// This module accepts it, hashes it, and discards it. It MUST NEVER be logged,
// persisted, returned, or placed in an error. The receipt `reference` is the
// paymentHash (public), NEVER the preimage.

// A 32-byte lowercase-hex preimage (64 hex chars).
const PREIMAGE_PATTERN = /^[0-9a-f]{64}$/
const PAYMENT_HASH_PATTERN = /^[0-9a-f]{64}$/

// Decode a 64-char lowercase-hex string to its 32 raw bytes. Returns undefined
// for any non-hex / wrong-length input (fail-closed).
const hexToBytes = (hex: string): Uint8Array | undefined => {
  if (hex.length % 2 !== 0) {
    return undefined
  }
  const bytes = new Uint8Array(hex.length / 2)
  for (let i = 0; i < bytes.length; i += 1) {
    const byte = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)
    if (Number.isNaN(byte)) {
      return undefined
    }
    bytes[i] = byte
  }
  return bytes
}

const bytesToHex = (bytes: Uint8Array): string => {
  let hex = ''
  for (let i = 0; i < bytes.length; i += 1) {
    hex += bytes[i]!.toString(16).padStart(2, '0')
  }
  return hex
}

// Constant-time hex-string comparison (both are fixed 64-char lowercase hex).
const constantTimeHexEqual = (a: string, b: string): boolean => {
  if (a.length !== b.length) {
    return false
  }
  let diff = 0
  for (let i = 0; i < a.length; i += 1) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i)
  }
  return diff === 0
}

export type LightningPreimageResult =
  | Readonly<{ ok: true }>
  // `malformed` = the preimage is absent/not a 64-hex string (spec
  // malformed-credential); `mismatch` = sha256(preimage) != paymentHash (spec
  // invalid-preimage).
  | Readonly<{ ok: false; reason: 'malformed' | 'mismatch' }>

// Verify a Lightning payment preimage against a stored paymentHash, FAIL-CLOSED.
// Computes sha256(hex_to_bytes(preimage)) and constant-time compares to the
// expected paymentHash. The preimage value is never returned or thrown.
export const verifyLightningPreimage = async (
  preimage: unknown,
  expectedPaymentHash: string,
): Promise<LightningPreimageResult> => {
  if (
    typeof preimage !== 'string' ||
    !PREIMAGE_PATTERN.test(preimage) ||
    !PAYMENT_HASH_PATTERN.test(expectedPaymentHash)
  ) {
    return { ok: false, reason: 'malformed' }
  }
  const bytes = hexToBytes(preimage)
  if (bytes === undefined) {
    return { ok: false, reason: 'malformed' }
  }
  const digest = await crypto.subtle.digest('SHA-256', bytes as BufferSource)
  const computed = bytesToHex(new Uint8Array(digest))
  return constantTimeHexEqual(computed, expectedPaymentHash)
    ? { ok: true }
    : { ok: false, reason: 'mismatch' }
}

// Read the `preimage` field from a credential payload as a typed string. Used so
// the route never passes a non-string into the verifier. PURE.
export const readPreimage = (
  payload: Record<string, unknown>,
): string | undefined => {
  const value = payload.preimage
  return typeof value === 'string' ? value : undefined
}

// SHA-256 of arbitrary bytes as lowercase hex — exported only for tests that
// construct a (preimage, paymentHash) pair without re-implementing the hash.
export const sha256Hex = async (bytes: Uint8Array): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', bytes as BufferSource)
  return bytesToHex(new Uint8Array(digest))
}

export const preimageHexToBytes = hexToBytes
