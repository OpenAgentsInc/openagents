/**
 * Update-manifest contract oracles (CUT-26, #8706).
 *
 * All signatures here use a FIXTURE ed25519 keypair generated in-process for
 * this test run. The production release private key is owner custody and is
 * never read, loaded, or printed by any test. The only production material
 * referenced is the committed PUBLIC pin, which a drift oracle keeps
 * byte-equal with `apps/oa-updates/keys/release-pubkey.json`.
 */
import { describe, expect, test } from "bun:test"
import { createHash, generateKeyPairSync, sign as edSign } from "node:crypto"
import { readFileSync } from "node:fs"
import path from "node:path"
import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  type UpdateManifest,
  type UpdateSignature,
  UPDATE_CONTRACT_SCHEMA_ID,
  compareReleaseVersions,
  isMonotonicUpgrade,
  parseReleaseVersion,
  verifyArtifactDigest,
  verifySignedUpdateManifest,
} from "../src/update-contract.ts"

// --- fixture keypair (NEVER the production key) ----------------------------

const fixture = generateKeyPairSync("ed25519")
const fixtureJwk = fixture.publicKey.export({ format: "jwk" }) as { x?: string }
const FIXTURE_KID = "fixture-test-key"
const fixturePin: PinnedReleaseKey = {
  alg: "ed25519",
  kid: FIXTURE_KID,
  x: fixtureJwk.x ?? "",
}

const signWithFixture = (payload: Uint8Array, overrides?: Partial<UpdateSignature>): UpdateSignature => ({
  alg: "ed25519",
  kid: FIXTURE_KID,
  sha256: createHash("sha256").update(payload).digest("hex"),
  signature: edSign(null, payload, fixture.privateKey).toString("base64url"),
  ...overrides,
})

const artifactBytes = new TextEncoder().encode("fixture artifact bytes for cut-26")

const validManifest: UpdateManifest = {
  schema: UPDATE_CONTRACT_SCHEMA_ID,
  app: "openagents-desktop",
  channel: "rc",
  version: "0.1.0-rc.1",
  artifactName: "OpenAgents-0.1.0-rc.1-arm64.dmg",
  artifactSha256: createHash("sha256").update(artifactBytes).digest("hex"),
  artifactByteLength: artifactBytes.byteLength,
  releasedAt: "2026-07-12T00:00:00Z",
  notesRef: "release.notes.0.1.0-rc.1",
}
const manifestBytes = new TextEncoder().encode(JSON.stringify(validManifest))

describe("release version order + monotonicity", () => {
  test("parses stable and rc versions, rejects malformed ones", () => {
    expect(parseReleaseVersion("1.2.3")).toEqual({ major: 1, minor: 2, patch: 3, rc: null })
    expect(parseReleaseVersion("0.1.0-rc.7")).toEqual({ major: 0, minor: 1, patch: 0, rc: 7 })
    for (const bad of ["1.2", "01.2.3", "1.2.3-rc", "1.2.3-beta.1", "v1.2.3", "", "1.2.3-rc.01"]) {
      expect(parseReleaseVersion(bad)).toBeNull()
    }
  })

  test("orders rc before its stable release and rc ordinals numerically", () => {
    const order = ["0.0.9", "0.1.0-rc.1", "0.1.0-rc.2", "0.1.0-rc.10", "0.1.0", "0.1.1"]
    for (let index = 1; index < order.length; index += 1) {
      const lower = parseReleaseVersion(order[index - 1]!)!
      const upper = parseReleaseVersion(order[index]!)!
      expect(compareReleaseVersions(lower, upper)).toBeLessThan(0)
      expect(compareReleaseVersions(upper, lower)).toBeGreaterThan(0)
      expect(compareReleaseVersions(upper, upper)).toBe(0)
    }
  })

  test("monotonic admission: strictly newer only, no rc on stable, no downgrades", () => {
    expect(isMonotonicUpgrade("0.1.0", "0.1.1", "stable")).toEqual({ admissible: true })
    expect(isMonotonicUpgrade("0.1.0", "0.2.0-rc.1", "rc")).toEqual({ admissible: true })
    expect(isMonotonicUpgrade("0.1.0-rc.1", "0.1.0", "rc")).toEqual({ admissible: true })
    expect(isMonotonicUpgrade("0.1.0", "0.1.0", "stable")).toEqual({
      admissible: false,
      reason: "not_strictly_newer",
    })
    expect(isMonotonicUpgrade("0.1.1", "0.1.0", "stable")).toEqual({
      admissible: false,
      reason: "not_strictly_newer",
    })
    expect(isMonotonicUpgrade("0.1.0", "0.1.0-rc.9", "rc")).toEqual({
      admissible: false,
      reason: "not_strictly_newer",
    })
    expect(isMonotonicUpgrade("0.1.0", "0.2.0-rc.1", "stable")).toEqual({
      admissible: false,
      reason: "prerelease_on_stable_channel",
    })
    expect(isMonotonicUpgrade("garbage", "0.1.0", "stable")).toEqual({
      admissible: false,
      reason: "unparseable_version",
    })
  })
})

describe("signed manifest verification (fixture keypair)", () => {
  test("accepts a correctly signed, schema-valid manifest on the expected channel", () => {
    const result = verifySignedUpdateManifest(manifestBytes, signWithFixture(manifestBytes), fixturePin, "rc")
    expect(result).toEqual({ ok: true, manifest: validManifest })
  })

  test("fails closed on a malformed signature envelope", () => {
    for (const envelope of [null, {}, { alg: "ed25519" }, "sig", 42]) {
      expect(verifySignedUpdateManifest(manifestBytes, envelope, fixturePin, "rc")).toEqual({
        ok: false,
        reason: "malformed_signature_envelope",
      })
    }
  })

  test("rejects an unpinned kid", () => {
    const envelope = signWithFixture(manifestBytes, { kid: "some-other-key" })
    expect(verifySignedUpdateManifest(manifestBytes, envelope, fixturePin, "rc")).toEqual({
      ok: false,
      reason: "kid_not_pinned",
    })
  })

  test("rejects a payload sha256 mismatch before touching the signature", () => {
    const envelope = signWithFixture(manifestBytes, { sha256: "0".repeat(64) })
    expect(verifySignedUpdateManifest(manifestBytes, envelope, fixturePin, "rc")).toEqual({
      ok: false,
      reason: "payload_sha256_mismatch",
    })
  })

  test("rejects a bit-flipped payload (signature over different bytes)", () => {
    const tampered = new Uint8Array(manifestBytes)
    tampered[10]! ^= 0b0000_0001
    const envelope = signWithFixture(manifestBytes, {
      sha256: createHash("sha256").update(tampered).digest("hex"),
    })
    expect(verifySignedUpdateManifest(tampered, envelope, fixturePin, "rc")).toEqual({
      ok: false,
      reason: "signature_invalid",
    })
  })

  test("rejects a signature from a different (attacker) key", () => {
    const attacker = generateKeyPairSync("ed25519")
    const envelope: UpdateSignature = {
      alg: "ed25519",
      kid: FIXTURE_KID, // claims to be the pinned key
      sha256: createHash("sha256").update(manifestBytes).digest("hex"),
      signature: edSign(null, manifestBytes, attacker.privateKey).toString("base64url"),
    }
    expect(verifySignedUpdateManifest(manifestBytes, envelope, fixturePin, "rc")).toEqual({
      ok: false,
      reason: "signature_invalid",
    })
  })

  test("rejects a correctly signed but schema-invalid manifest", () => {
    const invalid = new TextEncoder().encode(
      JSON.stringify({ ...validManifest, artifactName: "../../etc/evil.dmg" }),
    )
    expect(verifySignedUpdateManifest(invalid, signWithFixture(invalid), fixturePin, "rc")).toEqual({
      ok: false,
      reason: "manifest_schema_invalid",
    })
    const notJson = new TextEncoder().encode("not json at all")
    expect(verifySignedUpdateManifest(notJson, signWithFixture(notJson), fixturePin, "rc")).toEqual({
      ok: false,
      reason: "manifest_schema_invalid",
    })
  })

  test("rejects a validly signed manifest for the WRONG channel", () => {
    expect(verifySignedUpdateManifest(manifestBytes, signWithFixture(manifestBytes), fixturePin, "stable"))
      .toEqual({ ok: false, reason: "manifest_channel_mismatch" })
  })
})

describe("artifact digest admission", () => {
  test("accepts exact bytes, rejects wrong bytes and wrong length", () => {
    expect(verifyArtifactDigest(validManifest, artifactBytes)).toBe(true)
    const flipped = new Uint8Array(artifactBytes)
    flipped[0]! ^= 0xff
    expect(verifyArtifactDigest(validManifest, flipped)).toBe(false)
    expect(verifyArtifactDigest(validManifest, artifactBytes.slice(0, 8))).toBe(false)
  })
})

describe("production pin drift oracle", () => {
  test("the embedded pin equals the committed public release key file", () => {
    // PUBLIC key material only — committed, client-pinned, safe to read.
    const pubkeyPath = path.resolve(import.meta.dirname, "../../oa-updates/keys/release-pubkey.json")
    const committed = JSON.parse(readFileSync(pubkeyPath, "utf8")) as {
      alg: string
      kid: string
      x: string
    }
    expect(PRODUCTION_RELEASE_KEY_PIN.alg).toBe(committed.alg as "ed25519")
    expect(PRODUCTION_RELEASE_KEY_PIN.kid).toBe(committed.kid)
    expect(PRODUCTION_RELEASE_KEY_PIN.x).toBe(committed.x)
  })
})
