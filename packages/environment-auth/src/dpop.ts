/**
 * DPoP (RFC 9449) proof-of-possession mint + verify for OpenAgents local
 * runtime sockets and Khala Sync device grants (ENV-2, openagents #8780).
 *
 * ES256 (ECDSA P-256 / SHA-256) over WebCrypto only — no native or third-party
 * crypto dependency. Bun, Node >= 20, and browsers all provide
 * `globalThis.crypto.subtle`.
 *
 * Semantics follow RFC 9449:
 * - compact JWS with header `{ typ: "dpop+jwt", alg: "ES256", jwk }` where
 *   `jwk` is the PUBLIC EC key (any private `d` member fails verification);
 * - payload claims `htm` (HTTP method), `htu` (normalized target URI without
 *   query/fragment), `iat` (epoch seconds), `jti` (unique proof id), and an
 *   optional `ath` (base64url SHA-256 of the presented access token);
 * - the verifier binds the proof to the key via the RFC 7638 JWK thumbprint
 *   (SHA-256 over the canonical `{"crv","kty","x","y"}` members);
 * - freshness is a bounded window: `iat` may not be older than
 *   `maxAgeSeconds` nor further in the future than `maxClockSkewSeconds`;
 * - single use is enforced by a `(thumbprint, jti)` replay cache.
 */
import { Schema as S } from "effect"

export const DPOP_JWT_TYP = "dpop+jwt" as const
export const DPOP_JWT_ALG = "ES256" as const
export const DEFAULT_DPOP_MAX_AGE_SECONDS = 300
export const DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS = 5

const BASE64URL_32_BYTES = /^[A-Za-z0-9_-]{43}$/

/** Base64url encoding of a 32-byte value (RFC 7638 thumbprint, `ath`). */
export const DpopSha256Base64Url = S.String.check(
  S.isPattern(BASE64URL_32_BYTES),
)
export type DpopSha256Base64Url = typeof DpopSha256Base64Url.Type

/**
 * Public ES256 JWK carried in a DPoP proof header. `d` is rejected so a
 * client can never leak (and a verifier can never accept) private key
 * material inside a proof header.
 */
export const DpopPublicJwk = S.Struct({
  kty: S.Literal("EC"),
  crv: S.Literal("P-256"),
  x: S.String.check(S.isPattern(BASE64URL_32_BYTES)),
  y: S.String.check(S.isPattern(BASE64URL_32_BYTES)),
  d: S.optionalKey(S.Never),
})
export type DpopPublicJwk = typeof DpopPublicJwk.Type

export const DpopProofClaims = S.Struct({
  htm: S.String.check(S.isMinLength(1)),
  htu: S.String.check(S.isMinLength(1)),
  iat: S.Number.check(S.isInt()),
  jti: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  ath: S.optionalKey(S.String.check(S.isMinLength(1))),
})
export type DpopProofClaims = typeof DpopProofClaims.Type

const DpopProofHeader = S.Struct({
  typ: S.Literal(DPOP_JWT_TYP),
  alg: S.Literal(DPOP_JWT_ALG),
  jwk: DpopPublicJwk,
})

const decodeHeader = S.decodeUnknownOption(S.fromJsonString(DpopProofHeader))
const decodeClaims = S.decodeUnknownOption(S.fromJsonString(DpopProofClaims))

const textEncoder = new TextEncoder()

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = ""
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "")
}

function base64UrlToBytes(value: string): Uint8Array {
  const padded = value.replaceAll("-", "+").replaceAll("_", "/")
    .padEnd(value.length + ((4 - (value.length % 4)) % 4), "=")
  const binary = atob(padded)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes
}

function base64UrlToString(value: string): string {
  return new TextDecoder().decode(base64UrlToBytes(value))
}

async function sha256Base64Url(input: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", textEncoder.encode(input))
  return bytesToBase64Url(new Uint8Array(digest))
}

/**
 * RFC 7638 JWK thumbprint for an EC P-256 key: SHA-256 over the canonical
 * JSON of exactly the required members in lexicographic order.
 */
export async function computeDpopJwkThumbprint(
  jwk: Pick<DpopPublicJwk, "crv" | "kty" | "x" | "y">,
): Promise<string> {
  return sha256Base64Url(
    `{"crv":${JSON.stringify(jwk.crv)},"kty":${JSON.stringify(jwk.kty)},"x":${JSON.stringify(jwk.x)},"y":${JSON.stringify(jwk.y)}}`,
  )
}

/** `ath` claim value: base64url SHA-256 of the presented access token. */
export async function computeDpopAccessTokenHash(accessToken: string): Promise<string> {
  return sha256Base64Url(accessToken)
}

/**
 * RFC 9449 §4.3 `htu` normalization: scheme + authority + path, without
 * query or fragment. Returns null for unparseable URIs so a verifier fails
 * closed instead of comparing garbage.
 */
export function normalizeDpopHtu(url: string): string | null {
  try {
    const parsed = new URL(url)
    return `${parsed.origin}${parsed.pathname}`
  } catch {
    return null
  }
}

export type DpopKeyPair = {
  readonly privateKey: CryptoKey
  readonly publicJwk: DpopPublicJwk
  readonly thumbprint: string
}

/**
 * Generate a fresh non-extractable ES256 client key pair. The private key
 * never leaves WebCrypto; only the public JWK and its thumbprint are
 * exportable.
 */
export async function generateDpopKeyPair(): Promise<DpopKeyPair> {
  const pair = await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["sign", "verify"],
  )
  const exported = await crypto.subtle.exportKey("jwk", pair.publicKey)
  if (exported.kty !== "EC" || exported.crv !== "P-256" || !exported.x || !exported.y) {
    throw new Error("WebCrypto exported an unexpected DPoP public JWK shape")
  }
  const publicJwk: DpopPublicJwk = {
    kty: "EC",
    crv: "P-256",
    x: exported.x,
    y: exported.y,
  }
  return {
    privateKey: pair.privateKey,
    publicJwk,
    thumbprint: await computeDpopJwkThumbprint(publicJwk),
  }
}

export type MintDpopProofInput = {
  readonly privateKey: CryptoKey
  readonly publicJwk: DpopPublicJwk
  readonly htm: string
  readonly htu: string
  readonly nowEpochSeconds: number
  readonly jti?: string
  readonly accessToken?: string
}

/** Mint one single-use DPoP proof (compact JWS) for one HTTP request. */
export async function mintDpopProof(input: MintDpopProofInput): Promise<string> {
  const htu = normalizeDpopHtu(input.htu)
  if (htu === null) {
    throw new Error("cannot mint a DPoP proof for an unparseable htu")
  }
  const header = {
    typ: DPOP_JWT_TYP,
    alg: DPOP_JWT_ALG,
    jwk: {
      kty: input.publicJwk.kty,
      crv: input.publicJwk.crv,
      x: input.publicJwk.x,
      y: input.publicJwk.y,
    },
  }
  const claims: Record<string, unknown> = {
    htm: input.htm.toUpperCase(),
    htu,
    iat: Math.floor(input.nowEpochSeconds),
    jti: input.jti ?? crypto.randomUUID(),
  }
  if (input.accessToken !== undefined) {
    claims.ath = await computeDpopAccessTokenHash(input.accessToken)
  }
  const signingInput = `${bytesToBase64Url(textEncoder.encode(JSON.stringify(header)))}.${
    bytesToBase64Url(textEncoder.encode(JSON.stringify(claims)))
  }`
  const signature = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    input.privateKey,
    textEncoder.encode(signingInput),
  )
  return `${signingInput}.${bytesToBase64Url(new Uint8Array(signature))}`
}

export type DpopProofRejectionReason =
  | "proof_missing"
  | "proof_malformed"
  | "header_invalid"
  | "payload_invalid"
  | "thumbprint_mismatch"
  | "htm_mismatch"
  | "htu_mismatch"
  | "access_token_hash_mismatch"
  | "iat_too_old"
  | "iat_in_future"
  | "signature_invalid"
  | "jti_replayed"

export type DpopProofVerification =
  | {
      readonly ok: true
      readonly thumbprint: string
      readonly jti: string
      readonly iat: number
      readonly claims: DpopProofClaims
    }
  | {
      readonly ok: false
      readonly reason: DpopProofRejectionReason
    }

export type VerifyDpopProofInput = {
  readonly proof: string | null | undefined
  /** Actual HTTP method of the received request — never client-supplied. */
  readonly htm: string
  /** Actual target URI of the received request — never client-supplied. */
  readonly htu: string
  readonly nowEpochSeconds: number
  readonly expectedThumbprint?: string
  readonly expectedAccessToken?: string
  readonly maxAgeSeconds?: number
  readonly maxClockSkewSeconds?: number
}

/**
 * Verify one DPoP proof against the request the server actually received.
 * Pure verification: replay defense is layered on by
 * `verifyAndConsumeDpopProof`.
 */
export async function verifyDpopProof(
  input: VerifyDpopProofInput,
): Promise<DpopProofVerification> {
  if (typeof input.proof !== "string" || input.proof.trim() === "") {
    return { ok: false, reason: "proof_missing" }
  }
  const parts = input.proof.split(".")
  if (parts.length !== 3 || parts.some((part) => part === "")) {
    return { ok: false, reason: "proof_malformed" }
  }
  const [headerPart, claimsPart, signaturePart] = parts as [string, string, string]

  let headerJson: string
  let claimsJson: string
  let signature: Uint8Array
  try {
    headerJson = base64UrlToString(headerPart)
    claimsJson = base64UrlToString(claimsPart)
    signature = base64UrlToBytes(signaturePart)
  } catch {
    return { ok: false, reason: "proof_malformed" }
  }

  const header = decodeHeader(headerJson)
  if (header._tag === "None") {
    return { ok: false, reason: "header_invalid" }
  }
  const claims = decodeClaims(claimsJson)
  if (claims._tag === "None") {
    return { ok: false, reason: "payload_invalid" }
  }

  const thumbprint = await computeDpopJwkThumbprint(header.value.jwk)
  if (input.expectedThumbprint !== undefined && thumbprint !== input.expectedThumbprint) {
    return { ok: false, reason: "thumbprint_mismatch" }
  }
  if (claims.value.htm.toUpperCase() !== input.htm.toUpperCase()) {
    return { ok: false, reason: "htm_mismatch" }
  }
  const expectedHtu = normalizeDpopHtu(input.htu)
  if (expectedHtu === null || claims.value.htu !== expectedHtu) {
    return { ok: false, reason: "htu_mismatch" }
  }
  if (input.expectedAccessToken !== undefined) {
    const expectedAth = await computeDpopAccessTokenHash(input.expectedAccessToken)
    if (claims.value.ath !== expectedAth) {
      return { ok: false, reason: "access_token_hash_mismatch" }
    }
  }

  const maxAgeSeconds = input.maxAgeSeconds ?? DEFAULT_DPOP_MAX_AGE_SECONDS
  const maxClockSkewSeconds =
    input.maxClockSkewSeconds ?? DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS
  if (claims.value.iat > input.nowEpochSeconds + maxClockSkewSeconds) {
    return { ok: false, reason: "iat_in_future" }
  }
  if (input.nowEpochSeconds - claims.value.iat > maxAgeSeconds) {
    return { ok: false, reason: "iat_too_old" }
  }

  let verified: boolean
  try {
    const publicKey = await crypto.subtle.importKey(
      "jwk",
      {
        kty: header.value.jwk.kty,
        crv: header.value.jwk.crv,
        x: header.value.jwk.x,
        y: header.value.jwk.y,
        ext: true,
      },
      { name: "ECDSA", namedCurve: "P-256" },
      false,
      ["verify"],
    )
    verified = await crypto.subtle.verify(
      { name: "ECDSA", hash: "SHA-256" },
      publicKey,
      signature as BufferSource,
      textEncoder.encode(`${headerPart}.${claimsPart}`),
    )
  } catch {
    return { ok: false, reason: "signature_invalid" }
  }
  if (!verified) {
    return { ok: false, reason: "signature_invalid" }
  }
  return {
    ok: true,
    thumbprint,
    jti: claims.value.jti,
    iat: claims.value.iat,
    claims: claims.value,
  }
}

export type DpopReplayCache = {
  /**
   * Record `(thumbprint, jti)` as consumed. Returns false when the pair was
   * already consumed (a replay) — callers must then reject the proof.
   */
  readonly consume: (input: {
    readonly thumbprint: string
    readonly jti: string
    readonly iat: number
    readonly expiresAtEpochSeconds: number
  }) => Promise<boolean>
}

export type InMemoryDpopReplayCache = DpopReplayCache & {
  readonly prune: (nowEpochSeconds: number) => void
  readonly size: () => number
}

const DEFAULT_REPLAY_CACHE_MAX_ENTRIES = 10_000

/**
 * Bounded in-memory `(thumbprint, jti)` replay window. Entries expire with
 * the proof freshness window; when the bound is hit the oldest entries are
 * evicted first, so the cache can never grow without limit.
 */
export function makeInMemoryDpopReplayCache(
  options: { readonly maxEntries?: number } = {},
): InMemoryDpopReplayCache {
  const maxEntries = Math.max(1, options.maxEntries ?? DEFAULT_REPLAY_CACHE_MAX_ENTRIES)
  const entries = new Map<string, number>()
  const prune = (nowEpochSeconds: number) => {
    for (const [key, expiresAt] of entries) {
      if (expiresAt <= nowEpochSeconds) entries.delete(key)
    }
  }
  return {
    consume: async (input) => {
      prune(input.iat)
      const key = `${input.thumbprint}:${input.jti}`
      if (entries.has(key)) return false
      while (entries.size >= maxEntries) {
        const oldest = entries.keys().next()
        if (oldest.done) break
        entries.delete(oldest.value)
      }
      entries.set(key, input.expiresAtEpochSeconds)
      return true
    },
    prune,
    size: () => entries.size,
  }
}

/**
 * Verify one proof and consume its `jti` in one step — the shape server
 * seams should use. A valid signature whose `(thumbprint, jti)` was already
 * seen is rejected as `jti_replayed`.
 */
export async function verifyAndConsumeDpopProof(
  input: VerifyDpopProofInput & { readonly replayCache: DpopReplayCache },
): Promise<DpopProofVerification> {
  const result = await verifyDpopProof(input)
  if (!result.ok) return result
  const maxAgeSeconds = input.maxAgeSeconds ?? DEFAULT_DPOP_MAX_AGE_SECONDS
  const maxClockSkewSeconds =
    input.maxClockSkewSeconds ?? DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS
  const consumed = await input.replayCache.consume({
    thumbprint: result.thumbprint,
    jti: result.jti,
    iat: result.iat,
    expiresAtEpochSeconds: result.iat + maxAgeSeconds + maxClockSkewSeconds,
  })
  if (!consumed) {
    return { ok: false, reason: "jti_replayed" }
  }
  return result
}
