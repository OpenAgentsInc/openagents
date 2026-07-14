import { describe, expect, test } from "vite-plus/test"

import {
  computeDpopAccessTokenHash,
  computeDpopJwkThumbprint,
  DEFAULT_DPOP_MAX_AGE_SECONDS,
  DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS,
  generateDpopKeyPair,
  makeInMemoryDpopReplayCache,
  mintDpopProof,
  normalizeDpopHtu,
  verifyAndConsumeDpopProof,
  verifyDpopProof,
  type DpopKeyPair,
} from "./dpop.js"

const NOW = 1_784_000_000
const HTU = "http://127.0.0.1:4310/broker/redeem"

let cachedKeys: Promise<{ client: DpopKeyPair; foreign: DpopKeyPair }> | null = null
const keys = () => {
  cachedKeys ??= (async () => ({
    client: await generateDpopKeyPair(),
    foreign: await generateDpopKeyPair(),
  }))()
  return cachedKeys
}

async function mint(
  overrides: Partial<Parameters<typeof mintDpopProof>[0]> = {},
): Promise<{ proof: string; client: DpopKeyPair; foreign: DpopKeyPair }> {
  const { client, foreign } = await keys()
  const proof = await mintDpopProof({
    privateKey: client.privateKey,
    publicJwk: client.publicJwk,
    htm: "POST",
    htu: HTU,
    nowEpochSeconds: NOW,
    ...overrides,
  })
  return { proof, client, foreign }
}

const textEncoder = new TextEncoder()
const b64u = (value: string) =>
  btoa(value).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "")

describe("DPoP proof mint + verify (ES256 WebCrypto)", () => {
  test("a freshly minted proof verifies and binds the minting key's RFC 7638 thumbprint", async () => {
    const { proof, client } = await mint()
    const result = await verifyDpopProof({ proof, htm: "POST", htu: HTU, nowEpochSeconds: NOW })
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("unreachable")
    expect(result.thumbprint).toBe(client.thumbprint)
    expect(result.thumbprint).toBe(await computeDpopJwkThumbprint(client.publicJwk))
    expect(result.iat).toBe(NOW)
    expect(result.claims.htm).toBe("POST")
    expect(result.claims.htu).toBe(normalizeDpopHtu(HTU) as string)
  })

  test("htm comparison is case-insensitive and htu normalization strips query and fragment", async () => {
    const { proof } = await mint()
    const relaxed = await verifyDpopProof({
      proof,
      htm: "post",
      htu: `${HTU}?cursor=42#section`,
      nowEpochSeconds: NOW,
    })
    expect(relaxed.ok).toBe(true)
  })

  test("adversarial: wrong htm is rejected", async () => {
    const { proof } = await mint()
    const result = await verifyDpopProof({ proof, htm: "DELETE", htu: HTU, nowEpochSeconds: NOW })
    expect(result).toEqual({ ok: false, reason: "htm_mismatch" })
  })

  test("adversarial: wrong htu (different path, port, or unparseable) is rejected", async () => {
    const { proof } = await mint()
    for (const htu of [
      "http://127.0.0.1:4310/broker/revoke",
      "http://127.0.0.1:9999/broker/redeem",
      "https://127.0.0.1:4310/broker/redeem",
      "not a url",
    ]) {
      const result = await verifyDpopProof({ proof, htm: "POST", htu, nowEpochSeconds: NOW })
      expect(result).toEqual({ ok: false, reason: "htu_mismatch" })
    }
  })

  test("adversarial: a proof from a foreign key fails the expected-thumbprint binding", async () => {
    const { proof, foreign } = await mint()
    const result = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW,
      expectedThumbprint: foreign.thumbprint,
    })
    expect(result).toEqual({ ok: false, reason: "thumbprint_mismatch" })
  })

  test("adversarial: a swapped header JWK (foreign key claimed, client key signed) fails the signature", async () => {
    const { proof, foreign } = await mint()
    const [, claimsPart, signaturePart] = proof.split(".") as [string, string, string]
    const forgedHeader = b64u(JSON.stringify({
      typ: "dpop+jwt",
      alg: "ES256",
      jwk: foreign.publicJwk,
    }))
    const forged = `${forgedHeader}.${claimsPart}.${signaturePart}`
    const result = await verifyDpopProof({ proof: forged, htm: "POST", htu: HTU, nowEpochSeconds: NOW })
    expect(result).toEqual({ ok: false, reason: "signature_invalid" })
  })

  test("adversarial: a tampered payload fails the signature", async () => {
    const { proof } = await mint()
    const [headerPart, claimsPart, signaturePart] = proof.split(".") as [string, string, string]
    const claims = JSON.parse(new TextDecoder().decode(
      Uint8Array.from(atob(claimsPart.replaceAll("-", "+").replaceAll("_", "/")), (c) => c.charCodeAt(0)),
    )) as Record<string, unknown>
    claims.htu = normalizeDpopHtu("http://127.0.0.1:4310/broker/revoke")
    const tampered = `${headerPart}.${b64u(JSON.stringify(claims))}.${signaturePart}`
    const result = await verifyDpopProof({
      proof: tampered,
      htm: "POST",
      htu: "http://127.0.0.1:4310/broker/revoke",
      nowEpochSeconds: NOW,
    })
    expect(result).toEqual({ ok: false, reason: "signature_invalid" })
  })

  test("adversarial: stale and future iat are rejected at the exact window bounds", async () => {
    const { proof } = await mint()
    const tooOld = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW + DEFAULT_DPOP_MAX_AGE_SECONDS + 1,
    })
    expect(tooOld).toEqual({ ok: false, reason: "iat_too_old" })
    const atMaxAge = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW + DEFAULT_DPOP_MAX_AGE_SECONDS,
    })
    expect(atMaxAge.ok).toBe(true)
    const tooNew = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW - DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS - 1,
    })
    expect(tooNew).toEqual({ ok: false, reason: "iat_in_future" })
    const withinSkew = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW - DEFAULT_DPOP_MAX_CLOCK_SKEW_SECONDS,
    })
    expect(withinSkew.ok).toBe(true)
  })

  test("adversarial: access-token hash binding rejects a proof for a different token", async () => {
    const { proof } = await mint({ accessToken: "token.for.this.request.0001" })
    const bound = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW,
      expectedAccessToken: "token.for.this.request.0001",
    })
    expect(bound.ok).toBe(true)
    const wrongToken = await verifyDpopProof({
      proof,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW,
      expectedAccessToken: "token.for.another.request.0002",
    })
    expect(wrongToken).toEqual({ ok: false, reason: "access_token_hash_mismatch" })
    const { proof: withoutAth } = await mint()
    const missingAth = await verifyDpopProof({
      proof: withoutAth,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW,
      expectedAccessToken: "token.for.this.request.0001",
    })
    expect(missingAth).toEqual({ ok: false, reason: "access_token_hash_mismatch" })
  })

  test("adversarial: private key material, wrong typ, and wrong alg in the header are rejected", async () => {
    const { proof, client } = await mint()
    const [, claimsPart, signaturePart] = proof.split(".") as [string, string, string]
    const forgeHeader = (header: Record<string, unknown>) =>
      `${b64u(JSON.stringify(header))}.${claimsPart}.${signaturePart}`
    for (const header of [
      { typ: "dpop+jwt", alg: "ES256", jwk: { ...client.publicJwk, d: "c2VjcmV0LXByaXZhdGUta2V5LW1hdGVyaWFs" } },
      { typ: "jwt", alg: "ES256", jwk: client.publicJwk },
      { typ: "dpop+jwt", alg: "RS256", jwk: client.publicJwk },
      { typ: "dpop+jwt", alg: "none", jwk: client.publicJwk },
      { typ: "dpop+jwt", alg: "ES256", jwk: { kty: "RSA", n: "0000", e: "AQAB" } },
    ]) {
      const result = await verifyDpopProof({
        proof: forgeHeader(header),
        htm: "POST",
        htu: HTU,
        nowEpochSeconds: NOW,
      })
      expect(result).toEqual({ ok: false, reason: "header_invalid" })
    }
  })

  test("adversarial: missing, non-compact, and undecodable proofs are rejected without throwing", async () => {
    for (const [proof, reason] of [
      [undefined, "proof_missing"],
      [null, "proof_missing"],
      ["", "proof_missing"],
      ["   ", "proof_missing"],
      ["only.two", "proof_malformed"],
      ["a.b.c.d", "proof_malformed"],
      ["..", "proof_malformed"],
      [`${b64u("{not-json")}.${b64u("{}")}.${b64u("sig")}`, "header_invalid"],
    ] as const) {
      const result = await verifyDpopProof({ proof, htm: "POST", htu: HTU, nowEpochSeconds: NOW })
      expect(result).toEqual({ ok: false, reason })
    }
  })

  test("adversarial: payload missing required claims is rejected", async () => {
    const { proof } = await mint()
    const [headerPart, , signaturePart] = proof.split(".") as [string, string, string]
    const withoutJti = `${headerPart}.${b64u(JSON.stringify({ htm: "POST", htu: normalizeDpopHtu(HTU), iat: NOW }))}.${signaturePart}`
    const result = await verifyDpopProof({ proof: withoutJti, htm: "POST", htu: HTU, nowEpochSeconds: NOW })
    expect(result).toEqual({ ok: false, reason: "payload_invalid" })
  })

  test("adversarial: a replayed jti is rejected by verify-and-consume, per key", async () => {
    const replayCache = makeInMemoryDpopReplayCache()
    const { proof, foreign } = await mint({ jti: "jti-replay-check" })
    const first = await verifyAndConsumeDpopProof({
      proof, htm: "POST", htu: HTU, nowEpochSeconds: NOW, replayCache,
    })
    expect(first.ok).toBe(true)
    const replay = await verifyAndConsumeDpopProof({
      proof, htm: "POST", htu: HTU, nowEpochSeconds: NOW, replayCache,
    })
    expect(replay).toEqual({ ok: false, reason: "jti_replayed" })
    // The same jti under a DIFFERENT key is a distinct proof, not a replay.
    const foreignProof = await mintDpopProof({
      privateKey: foreign.privateKey,
      publicJwk: foreign.publicJwk,
      htm: "POST",
      htu: HTU,
      nowEpochSeconds: NOW,
      jti: "jti-replay-check",
    })
    const foreignResult = await verifyAndConsumeDpopProof({
      proof: foreignProof, htm: "POST", htu: HTU, nowEpochSeconds: NOW, replayCache,
    })
    expect(foreignResult.ok).toBe(true)
  })

  test("replay cache prunes expired entries and enforces its bound", async () => {
    const cache = makeInMemoryDpopReplayCache({ maxEntries: 2 })
    expect(await cache.consume({ thumbprint: "t1", jti: "j1", iat: NOW, expiresAtEpochSeconds: NOW + 10 })).toBe(true)
    expect(await cache.consume({ thumbprint: "t1", jti: "j2", iat: NOW, expiresAtEpochSeconds: NOW + 10 })).toBe(true)
    expect(cache.size()).toBe(2)
    // Bound reached: the oldest entry is evicted, the new entry is accepted.
    expect(await cache.consume({ thumbprint: "t1", jti: "j3", iat: NOW, expiresAtEpochSeconds: NOW + 10 })).toBe(true)
    expect(cache.size()).toBe(2)
    cache.prune(NOW + 11)
    expect(cache.size()).toBe(0)
    expect(await cache.consume({ thumbprint: "t1", jti: "j3", iat: NOW + 12, expiresAtEpochSeconds: NOW + 20 })).toBe(true)
  })

  test("thumbprint and access-token hash are deterministic base64url SHA-256 values", async () => {
    const { client } = await keys()
    const thumbprint = await computeDpopJwkThumbprint(client.publicJwk)
    expect(thumbprint).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(await computeDpopJwkThumbprint(client.publicJwk)).toBe(thumbprint)
    const ath = await computeDpopAccessTokenHash("some.access.token")
    expect(ath).toMatch(/^[A-Za-z0-9_-]{43}$/)
    expect(await computeDpopAccessTokenHash("some.access.token")).toBe(ath)
    expect(await computeDpopAccessTokenHash("another.token")).not.toBe(ath)
  })

  test("RFC 7638 thumbprint matches the canonical known-answer vector construction", async () => {
    // Deterministic cross-check: hash the canonical members JSON with WebCrypto
    // directly and compare against computeDpopJwkThumbprint.
    const { client } = await keys()
    const canonical = `{"crv":"P-256","kty":"EC","x":"${client.publicJwk.x}","y":"${client.publicJwk.y}"}`
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", textEncoder.encode(canonical)))
    let binary = ""
    for (const byte of digest) binary += String.fromCharCode(byte)
    const expected = btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "")
    expect(await computeDpopJwkThumbprint(client.publicJwk)).toBe(expected)
  })
})
