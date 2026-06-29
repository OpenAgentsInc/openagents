// MPP / Payment Auth canonicalization + HMAC primitives (EPIC #6049, defect B).
// Worker-native (WebCrypto). PURE crypto/encoding; no IO, no Stripe, no env.
//
// The Payment Auth core spec (draft-httpauth-payment-00) makes STATELESS
// challenge binding possible by deriving the challenge `id` as
//
//   id = base64url(HMAC-SHA256(server_secret, "|".join([
//          realm, method, intent, request_b64url,
//          expires||"", digest||"", opaque_b64url||"" ])))
//
// where `request` and `opaque` are JSON serialized with JSON Canonicalization
// Scheme (JCS, RFC 8785) before base64url-nopad encoding. Because the HMAC input
// uses the base64url-encoded request/opaque AS THEY APPEAR ON THE WIRE, the
// server can recompute the `id` from the echoed challenge in the retry credential
// and verify the binding without storing any per-challenge state (Section 5.1.3
// "Recommended: HMAC-SHA256 Binding").
//
// This module owns: RFC 8785 JCS serialization, base64url-nopad encode/decode,
// the WebCrypto HMAC-SHA256 over the canonical input, and a constant-time
// comparison for the recomputed id.

import { parseJsonUnknown } from '../../json-boundary'

// Typed error for a programmer-misuse canonicalization input (a non-finite
// number or unsupported value type). A typed error keeps the zero-debt
// architecture boundary (no generic `throw new Error`) while still failing loud
// on a true coding mistake rather than silently mis-canonicalizing an id.
export class CanonicalJsonError extends Error {
  override readonly name = 'CanonicalJsonError'
}

// ---- base64url (RFC 4648 §5, no padding) ----

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder()

// Encode raw bytes as base64url without padding.
export const base64UrlEncodeBytes = (bytes: Uint8Array): string => {
  let binary = ''
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]!)
  }
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

// Encode a UTF-8 string as base64url-nopad.
export const base64UrlEncode = (value: string): string =>
  base64UrlEncodeBytes(textEncoder.encode(value))

// Decode a base64url-nopad string back to a UTF-8 string. Returns undefined for
// invalid input (so callers fail-closed rather than throwing through verify).
export const base64UrlDecode = (value: string): string | undefined => {
  try {
    const padded = value
      .replaceAll('-', '+')
      .replaceAll('_', '/')
      .padEnd(Math.ceil(value.length / 4) * 4, '=')
    const binary = atob(padded)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i += 1) {
      bytes[i] = binary.charCodeAt(i)
    }
    return textDecoder.decode(bytes)
  } catch {
    return undefined
  }
}

// ---- JCS / RFC 8785 canonical JSON ----

// Serialize a JSON value per the JSON Canonicalization Scheme (RFC 8785):
//   - object members sorted by key (UTF-16 code-unit order, which matches
//     JavaScript's default Array.prototype.sort on the ASCII key set we use);
//   - no insignificant whitespace;
//   - strings serialized with JSON.stringify (the JS string escaping matches
//     the JCS rules for the ASCII payloads MPP uses: amounts, ids, networks).
//   - numbers serialized via the ES Number-to-string algorithm (JSON.stringify),
//     which RFC 8785 defers to for the integer/short-decimal range we emit.
// We restrict inputs to JSON-safe values (no NaN/Infinity, no bigint); a
// non-finite number throws so a programming error never produces a silently
// mis-canonicalized id.
export const canonicalJson = (value: unknown): string => {
  if (value === null) {
    return 'null'
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false'
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new CanonicalJsonError('canonicalJson: non-finite number')
    }
    return JSON.stringify(value)
  }
  if (typeof value === 'string') {
    return JSON.stringify(value)
  }
  if (Array.isArray(value)) {
    return `[${value.map(item => canonicalJson(item)).join(',')}]`
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).filter(
      ([, v]) => v !== undefined,
    )
    entries.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    const members = entries.map(
      ([k, v]) => `${JSON.stringify(k)}:${canonicalJson(v)}`,
    )
    return `{${members.join(',')}}`
  }
  throw new CanonicalJsonError(
    `canonicalJson: unsupported value type ${typeof value}`,
  )
}

// Serialize a JSON value with JCS, then base64url-nopad encode it. This is the
// exact transform the spec applies to the `request` and `opaque` challenge
// parameters before they enter the HMAC input and the WWW-Authenticate header.
export const jcsBase64Url = (value: unknown): string =>
  base64UrlEncode(canonicalJson(value))

// Decode a base64url-nopad JCS parameter back to a JSON record. Returns
// undefined on any decode/parse failure (fail-closed).
export const decodeJcsBase64UrlRecord = (
  value: string,
): Record<string, unknown> | undefined => {
  const json = base64UrlDecode(value)
  if (json === undefined) {
    return undefined
  }
  try {
    const parsed = parseJsonUnknown(json)
    if (parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>
    }
    return undefined
  } catch {
    return undefined
  }
}

// ---- HMAC-SHA256 challenge id ----

// The seven fixed positional HMAC slots (core spec §5.1.3). Required fields
// supply their string value; optional fields are the empty string when absent.
export type ChallengeBindingSlots = Readonly<{
  realm: string
  method: string
  intent: string
  // The base64url-nopad of the JCS-serialized `request` JSON.
  requestB64Url: string
  // RFC 3339 expiry; '' when absent.
  expires: string
  // RFC 9530 content digest; '' when absent.
  digest: string
  // The base64url-nopad of the JCS-serialized `opaque` JSON; '' when absent.
  opaqueB64Url: string
}>

// Build the canonical HMAC input string: the seven slots joined with '|'. Every
// slot is always present; absent optional fields appear as empty segments.
export const challengeBindingInput = (slots: ChallengeBindingSlots): string =>
  [
    slots.realm,
    slots.method,
    slots.intent,
    slots.requestB64Url,
    slots.expires,
    slots.digest,
    slots.opaqueB64Url,
  ].join('|')

// Compute the challenge `id` = base64url(HMAC-SHA256(secret, input)). Worker
// WebCrypto; the secret is the server-held signing secret (never logged).
export const computeChallengeId = async (
  serverSecret: string,
  slots: ChallengeBindingSlots,
): Promise<string> => {
  const key = await crypto.subtle.importKey(
    'raw',
    textEncoder.encode(serverSecret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const signature = await crypto.subtle.sign(
    'HMAC',
    key,
    textEncoder.encode(challengeBindingInput(slots)),
  )
  return base64UrlEncodeBytes(new Uint8Array(signature))
}

// Constant-time string comparison for the recomputed vs presented id. Compares
// over the max length so the timing does not leak the length of the match.
export const constantTimeEqual = (a: string, b: string): boolean => {
  const aBytes = textEncoder.encode(a)
  const bBytes = textEncoder.encode(b)
  const length = Math.max(aBytes.length, bBytes.length)
  let diff = aBytes.length ^ bBytes.length
  for (let i = 0; i < length; i += 1) {
    diff |= (aBytes[i] ?? 0) ^ (bBytes[i] ?? 0)
  }
  return diff === 0
}
