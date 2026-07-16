/**
 * Desktop release publish core (CUT-26, #8706).
 *
 * Pure, deterministic building blocks for the scripted publish flow
 * (`scripts/publish-release.ts`): packaged artifact in → signed
 * `openagents.desktop.update_manifest.v1` + updated release descriptor out,
 * with version monotonicity and channel rules enforced by the landed update
 * contract before anything is written.
 *
 * Output shape is EXACTLY what the deployed `apps/oa-updates` serving seam
 * consumes (`openagents-desktop-release.json` descriptor + signed manifest
 * bytes + detached signature envelope, seeded by
 * `apps/oa-updates/src/openagents-desktop-seed.ts` and served at
 * `/desktop/openagents/<channel>/manifest.json` + `manifest.sig.json` +
 * `release.json`). Artifacts are NOT placed in the dist dir — Cloud Run caps
 * responses, so artifact bytes live behind a credential-free HTTPS
 * `artifactUrl` (GCS) and the client verifies the download against the
 * SIGNED sha256/byteLength, never the URL.
 *
 * Key handling: callers pass a `ReleaseSigningKey` VALUE (JWK seed `d` +
 * `kid`). This module never reads the environment, never touches disk, and
 * never logs or embeds key material in errors. Tests use fixture keypairs
 * generated in-process; the production seed reaches the CLI only through
 * the documented env seam (see the script) and is never printed.
 */
import { createHash, createPrivateKey, createPublicKey, sign as edSign } from "node:crypto"
import { Exit, Schema } from "effect"
import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  type UpdateChannel,
  type UpdateManifest,
  type UpdateSignature,
  UPDATE_CONTRACT_SCHEMA_ID,
  UpdateManifestSchema,
  isMonotonicUpgrade,
  parseReleaseVersion,
  verifyArtifactDigest,
  verifySignedUpdateManifest,
} from "./update-contract.ts"

/** Descriptor file name the oa-updates seed reads from its dist dir. */
export const RELEASE_DESCRIPTOR_FILE = "openagents-desktop-release.json" as const

// ---------------------------------------------------------------------------
// Release descriptor — the dist-dir contract shared with oa-updates
// ---------------------------------------------------------------------------

/** Bounded relative file name (mirrors the oa-updates seed's own bound). */
const RelativeFileSchema = Schema.String.check(
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$/),
)

const HttpsUrlSchema = Schema.String.check(
  Schema.isMinLength(9),
  Schema.isMaxLength(2048),
  Schema.isPattern(/^https:\/\//),
)

export const ReleaseDescriptorEntrySchema = Schema.Struct({
  manifestPath: RelativeFileSchema,
  signaturePath: RelativeFileSchema,
  artifactUrl: HttpsUrlSchema,
})
export type ReleaseDescriptorEntry = typeof ReleaseDescriptorEntrySchema.Type

/**
 * Multi-channel descriptor (`releases` latest-per-channel). The original
 * flat single-release shape is still accepted on read for compatibility
 * with the first deployed seed.
 */
export const ReleaseDescriptorSchema = Schema.Struct({
  releases: Schema.Array(ReleaseDescriptorEntrySchema),
})
export type ReleaseDescriptor = typeof ReleaseDescriptorSchema.Type

// Effect Schema's decoder service type is intentionally erased at this
// perimeter; each call site pins a concrete schema and result type.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const decodeExit = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? (result.value as A) : null
}

/** Accepts the flat legacy shape or the `releases` list; null on drift. */
export const decodeReleaseDescriptor = (value: unknown): ReleaseDescriptor | null => {
  const asList = decodeExit<ReleaseDescriptor>(ReleaseDescriptorSchema, value)
  if (asList !== null) return asList
  const asFlat = decodeExit<ReleaseDescriptorEntry>(ReleaseDescriptorEntrySchema, value)
  return asFlat === null ? null : { releases: [asFlat] }
}

export const decodeUpdateManifest = (value: unknown): UpdateManifest | null =>
  decodeExit<UpdateManifest>(UpdateManifestSchema, value)

/** Enforce the oa-updates artifact-URL boundary at publish time too. */
export const assertCredentialFreeHttpsUrl = (value: string): URL => {
  const url = new URL(value)
  if (url.protocol !== "https:" || url.username !== "" || url.password !== "") {
    throw new Error("artifact URL must be credential-free HTTPS")
  }
  return url
}

// ---------------------------------------------------------------------------
// Signing — private key value in, signed release out; nothing else escapes
// ---------------------------------------------------------------------------

/** Private release signing key VALUE. Never logged, never serialized. */
export interface ReleaseSigningKey {
  /** JWK OKP `d` (the ed25519 seed), base64url. SECRET. */
  readonly d: string
  readonly kid: string
}

/** ed25519 PKCS#8 prefix for a raw 32-byte seed (RFC 8410). */
const ED25519_PKCS8_PREFIX = Buffer.from(
  "302e020100300506032b657004220420",
  "hex",
)

const privateKeyFromSeed = (d: string) => {
  const seed = Buffer.from(d, "base64url")
  if (seed.byteLength !== 32) {
    throw new Error("release signing seed must be 32 bytes (invalid JWK d)")
  }
  return createPrivateKey({
    key: Buffer.concat([ED25519_PKCS8_PREFIX, seed]),
    format: "der",
    type: "pkcs8",
  })
}

/** Derive the PUBLIC pin for a signing key (safe to print/compare). */
export const deriveReleaseKeyPin = (key: ReleaseSigningKey): PinnedReleaseKey => {
  const publicJwk = createPublicKey(privateKeyFromSeed(key.d)).export({
    format: "jwk",
  }) as { x?: string }
  if (typeof publicJwk.x !== "string" || publicJwk.x.length === 0) {
    throw new Error("failed to derive public key from release signing seed")
  }
  return { alg: "ed25519", kid: key.kid, x: publicJwk.x }
}

/**
 * A key claiming the PRODUCTION kid must actually be the production key.
 * This blocks a wrong/rotated/fixture seed from silently signing under the
 * production key id — clients pin kid AND key, so such a manifest would be
 * rejected in the field; refuse it at publish time instead.
 */
export const assertProductionKidIntegrity = (pin: PinnedReleaseKey): void => {
  if (pin.kid === PRODUCTION_RELEASE_KEY_PIN.kid && pin.x !== PRODUCTION_RELEASE_KEY_PIN.x) {
    throw new Error(
      `signing key claims production kid ${pin.kid} but derives a different public key; refusing to publish`,
    )
  }
}

export interface SignedManifestResult {
  readonly payloadBytes: Uint8Array
  readonly envelope: UpdateSignature
  readonly pin: PinnedReleaseKey
}

/**
 * Sign already-canonical release-selection bytes with the existing custody
 * seam. This is shared by v1 and ReleaseSet v2; it deliberately accepts bytes
 * rather than an object so the caller, not JSON.stringify insertion order,
 * owns the canonical payload. The returned public pin is checked before any
 * publisher can expose an envelope claiming the production key id.
 */
export const signReleasePayload = (
  payloadBytes: Uint8Array,
  key: ReleaseSigningKey,
): SignedManifestResult => {
  const pin = deriveReleaseKeyPin(key)
  assertProductionKidIntegrity(pin)
  const envelope: UpdateSignature = {
    alg: "ed25519",
    kid: key.kid,
    sha256: createHash("sha256").update(payloadBytes).digest("hex"),
    signature: edSign(null, payloadBytes, privateKeyFromSeed(key.d)).toString("base64url"),
  }
  return { payloadBytes, envelope, pin }
}

/**
 * Sign a manifest and SELF-VERIFY the result through the exact client seam
 * (`verifySignedUpdateManifest`) before returning — a publish that a client
 * would reject can never be produced.
 */
export const signUpdateManifest = (
  manifest: UpdateManifest,
  key: ReleaseSigningKey,
): SignedManifestResult => {
  const payloadBytes = new TextEncoder().encode(JSON.stringify(manifest))
  const { envelope, pin } = signReleasePayload(payloadBytes, key)
  const verified = verifySignedUpdateManifest(payloadBytes, envelope, pin, manifest.channel)
  if (!verified.ok) {
    throw new Error(`self-verification of the signed manifest failed: ${verified.reason}`)
  }
  return { payloadBytes, envelope, pin }
}

// ---------------------------------------------------------------------------
// Manifest construction
// ---------------------------------------------------------------------------

export interface BuildManifestInput {
  readonly version: string
  readonly channel: UpdateChannel
  readonly artifactName: string
  readonly artifactBytes: Uint8Array
  readonly releasedAt: string
  readonly notesRef?: string
}

/** Build and schema-validate the manifest for one packaged artifact. */
export const buildUpdateManifestForArtifact = (
  input: BuildManifestInput,
): UpdateManifest => {
  const candidate = {
    schema: UPDATE_CONTRACT_SCHEMA_ID,
    app: "openagents-desktop",
    channel: input.channel,
    version: input.version,
    artifactName: input.artifactName,
    artifactSha256: createHash("sha256").update(input.artifactBytes).digest("hex"),
    artifactByteLength: input.artifactBytes.byteLength,
    releasedAt: input.releasedAt,
    ...(input.notesRef === undefined ? {} : { notesRef: input.notesRef }),
  }
  const manifest = decodeUpdateManifest(candidate)
  if (manifest === null) {
    throw new Error(
      "manifest inputs are not admissible (version/artifactName/releasedAt/notesRef bounds)",
    )
  }
  return manifest
}

export const artifactExtension = (artifactName: string): ".dmg" | ".zip" | null => {
  if (artifactName.toLowerCase().endsWith(".dmg")) return ".dmg"
  if (artifactName.toLowerCase().endsWith(".zip")) return ".zip"
  return null
}

// ---------------------------------------------------------------------------
// Publish plan — the fail-closed admission gate for one new release
// ---------------------------------------------------------------------------

export const publishRefusalReasons = [
  "unparseable_version",
  "prerelease_on_stable_channel",
  "not_strictly_newer",
  "existing_manifest_invalid",
] as const
export type PublishRefusalReason = (typeof publishRefusalReasons)[number]

export type PublishPlanVerdict =
  | { readonly ok: true; readonly installedHead: string | null }
  | { readonly ok: false; readonly reason: PublishRefusalReason }

/**
 * Decide whether `version` may be published to `channel` given the parsed
 * JSON of the channel's CURRENT published manifest (or `null` for a first
 * release). Fail closed on everything: an existing manifest that no longer
 * decodes or names another channel, and any candidate that is not STRICTLY
 * newer under the landed monotonicity rule (which also refuses pre-releases
 * on the stable channel and all downgrades and duplicates unconditionally).
 */
export const planDesktopReleasePublish = (input: {
  readonly existingManifest: unknown | null
  readonly channel: UpdateChannel
  readonly version: string
}): PublishPlanVerdict => {
  const candidate = parseReleaseVersion(input.version)
  if (candidate === null) return { ok: false, reason: "unparseable_version" }
  if (input.channel === "stable" && candidate.rc !== null) {
    return { ok: false, reason: "prerelease_on_stable_channel" }
  }

  if (input.existingManifest === null) return { ok: true, installedHead: null }

  const existing = decodeUpdateManifest(input.existingManifest)
  if (existing === null || existing.channel !== input.channel) {
    return { ok: false, reason: "existing_manifest_invalid" }
  }
  const verdict = isMonotonicUpgrade(existing.version, input.version, input.channel)
  if (!verdict.admissible) {
    return {
      ok: false,
      reason: verdict.reason === "unparseable_version" ? "existing_manifest_invalid" : verdict.reason,
    }
  }
  return { ok: true, installedHead: existing.version }
}

// ---------------------------------------------------------------------------
// Full publish computation (no I/O)
// ---------------------------------------------------------------------------

export interface ComputedDesktopReleasePublish {
  readonly manifest: UpdateManifest
  readonly payloadBytes: Uint8Array
  readonly envelope: UpdateSignature
  readonly pin: PinnedReleaseKey
  /** Versioned file names — history stays in the dist dir, never clobbered. */
  readonly manifestFileName: string
  readonly signatureFileName: string
  readonly descriptorEntry: ReleaseDescriptorEntry
}

/**
 * Full fail-closed publish computation: plan → manifest → artifact digest
 * cross-check → sign → self-verify → production-kid integrity → descriptor
 * entry. Returns everything the CLI needs to write.
 */
export const computeDesktopReleasePublish = (input: {
  readonly existingManifest: unknown | null
  readonly channel: UpdateChannel
  readonly version: string
  readonly artifactName: string
  readonly artifactBytes: Uint8Array
  readonly artifactUrl: string
  readonly releasedAt: string
  readonly notesRef?: string
  readonly key: ReleaseSigningKey
}): ComputedDesktopReleasePublish => {
  const plan = planDesktopReleasePublish({
    existingManifest: input.existingManifest,
    channel: input.channel,
    version: input.version,
  })
  if (!plan.ok) {
    throw new Error(`publish refused: ${plan.reason}`)
  }
  assertCredentialFreeHttpsUrl(input.artifactUrl)
  const manifest = buildUpdateManifestForArtifact({
    version: input.version,
    channel: input.channel,
    artifactName: input.artifactName,
    artifactBytes: input.artifactBytes,
    releasedAt: input.releasedAt,
    ...(input.notesRef === undefined ? {} : { notesRef: input.notesRef }),
  })
  if (!verifyArtifactDigest(manifest, input.artifactBytes)) {
    throw new Error("artifact digest cross-check failed (internal error)")
  }
  const signed = signUpdateManifest(manifest, input.key)
  assertProductionKidIntegrity(signed.pin)

  const manifestFileName = `manifest-${input.channel}-${input.version}.json`
  const signatureFileName = `manifest-${input.channel}-${input.version}.sig.json`
  const descriptorEntry = decodeExit<ReleaseDescriptorEntry>(ReleaseDescriptorEntrySchema, {
    manifestPath: manifestFileName,
    signaturePath: signatureFileName,
    artifactUrl: input.artifactUrl,
  })
  if (descriptorEntry === null) {
    throw new Error("descriptor entry is not admissible (file name/url bounds)")
  }

  return {
    manifest,
    payloadBytes: signed.payloadBytes,
    envelope: signed.envelope,
    pin: signed.pin,
    manifestFileName,
    signatureFileName,
    descriptorEntry,
  }
}
