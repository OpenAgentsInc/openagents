/**
 * Desktop update-manifest contract (CUT-26, #8706).
 *
 * One typed, fail-closed contract for OpenAgents Desktop release channels,
 * update manifests, version monotonicity, and the ed25519 release-signature
 * verification seam. This is the provable-without-ceremony half of the CUT-26
 * distribution slice: everything here runs against fixture keypairs in tests;
 * the PRODUCTION private key never enters this repo, this process, or any
 * test. Clients pin only the committed PUBLIC release key
 * (`apps/oa-updates/keys/release-pubkey.json`) and reject everything else.
 *
 * Trust boundary: the signature, never the host/TLS. A manifest that fails
 * alg/kid/sha256/ed25519 verification is REJECTED with a typed reason — there
 * is no "trust anyway" path and no free-form error channel that could carry
 * secret material.
 */
import { createHash, createPublicKey, verify as edVerify } from "node:crypto"
import { Exit, Schema } from "effect"

export const UPDATE_CONTRACT_SCHEMA_ID = "openagents.desktop.update_manifest.v1" as const

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/** Release channels. RCs are pre-releases and NEVER take the stable badge. */
export const updateChannels = ["stable", "rc"] as const
export type UpdateChannel = (typeof updateChannels)[number]
export const UpdateChannelSchema = Schema.Literals(updateChannels)

// ---------------------------------------------------------------------------
// Versions — `MAJOR.MINOR.PATCH` with an optional `-rc.N` pre-release tag
// ---------------------------------------------------------------------------

export const RELEASE_VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/

export const ReleaseVersionSchema = Schema.String.check(
  Schema.isMinLength(5),
  Schema.isMaxLength(40),
  Schema.isPattern(RELEASE_VERSION_PATTERN),
)
export type ReleaseVersion = typeof ReleaseVersionSchema.Type

export interface ParsedReleaseVersion {
  readonly major: number
  readonly minor: number
  readonly patch: number
  /** `null` for a stable release; the numeric rc ordinal for a pre-release. */
  readonly rc: number | null
}

export const parseReleaseVersion = (version: string): ParsedReleaseVersion | null => {
  const match = RELEASE_VERSION_PATTERN.exec(version)
  if (match === null) return null
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    rc: match[4] === undefined ? null : Number(match[4]),
  }
}

/**
 * Total order over release versions: numeric triple first, then the semver
 * pre-release rule — `X.Y.Z-rc.N` precedes (is LESS than) `X.Y.Z`, and rc
 * ordinals order numerically. Returns negative/zero/positive.
 */
export const compareReleaseVersions = (a: ParsedReleaseVersion, b: ParsedReleaseVersion): number => {
  if (a.major !== b.major) return a.major - b.major
  if (a.minor !== b.minor) return a.minor - b.minor
  if (a.patch !== b.patch) return a.patch - b.patch
  if (a.rc === null && b.rc === null) return 0
  if (a.rc === null) return 1
  if (b.rc === null) return -1
  return a.rc - b.rc
}

export type MonotonicityVerdict =
  | { readonly admissible: true }
  | {
    readonly admissible: false
    readonly reason: "unparseable_version" | "not_strictly_newer" | "prerelease_on_stable_channel"
  }

/**
 * The ONLY sanctioned forward-update admission rule: the candidate must be
 * STRICTLY newer than the installed version, and the stable channel never
 * admits a pre-release. Downgrades are refused here unconditionally — the
 * rollback state machine is the single sanctioned downgrade path, and it may
 * only return to the retained previous slot.
 */
export const isMonotonicUpgrade = (
  installed: string,
  candidate: string,
  channel: UpdateChannel,
): MonotonicityVerdict => {
  const from = parseReleaseVersion(installed)
  const to = parseReleaseVersion(candidate)
  if (from === null || to === null) return { admissible: false, reason: "unparseable_version" }
  if (channel === "stable" && to.rc !== null) {
    return { admissible: false, reason: "prerelease_on_stable_channel" }
  }
  if (compareReleaseVersions(to, from) <= 0) return { admissible: false, reason: "not_strictly_newer" }
  return { admissible: true }
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

const Sha256HexSchema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/))

/** Bounded artifact file name — never a path (no separators, no traversal). */
const ArtifactNameSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/),
)

/** Public-safe ref charset (mirrors the diagnostics/runtime-gateway pin). */
const PublicRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

export const UpdateManifestSchema = Schema.Struct({
  schema: Schema.Literal(UPDATE_CONTRACT_SCHEMA_ID),
  app: Schema.Literal("openagents-desktop"),
  channel: UpdateChannelSchema,
  version: ReleaseVersionSchema,
  artifactName: ArtifactNameSchema,
  /** sha256 of the ARTIFACT bytes (the manifest signature covers the manifest bytes). */
  artifactSha256: Sha256HexSchema,
  artifactByteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  releasedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
  ),
  /** Optional public-safe release-notes ref; never a URL with credentials. */
  notesRef: Schema.optional(PublicRefSchema),
})
export type UpdateManifest = typeof UpdateManifestSchema.Type

// ---------------------------------------------------------------------------
// Signature envelope + pinned key
// ---------------------------------------------------------------------------

export const UpdateSignatureSchema = Schema.Struct({
  alg: Schema.Literal("ed25519"),
  kid: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  /** sha256 of the signed payload — REQUIRED here (fail closed on absence). */
  sha256: Sha256HexSchema,
  /** base64url ed25519 signature over the raw payload bytes. */
  signature: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512)),
})
export type UpdateSignature = typeof UpdateSignatureSchema.Type

export const PinnedReleaseKeySchema = Schema.Struct({
  alg: Schema.Literal("ed25519"),
  kid: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  /** JWK OKP public `x` coordinate, base64url. PUBLIC material only. */
  x: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(128)),
})
export type PinnedReleaseKey = typeof PinnedReleaseKeySchema.Type

/**
 * The production pin, mirrored from the committed PUBLIC key file
 * `apps/oa-updates/keys/release-pubkey.json` (kid `2dbe811d19f67528`).
 * A test asserts this constant stays byte-equal with that file so the two
 * can never drift silently. This is public-safe by construction; the private
 * seed lives only in owner custody (Secret Manager + `.secrets`) and is
 * NEVER read by this module or its tests.
 */
export const PRODUCTION_RELEASE_KEY_PIN: PinnedReleaseKey = {
  alg: "ed25519",
  kid: "2dbe811d19f67528",
  x: "P9steasTKRx6gr9QQlbah4kXm17aAh2wLHLAL-Txwak",
}

// ---------------------------------------------------------------------------
// Verification seam — fail closed, typed reasons only
// ---------------------------------------------------------------------------

export const updateVerificationFailures = [
  "malformed_signature_envelope",
  "unexpected_algorithm",
  "kid_not_pinned",
  "payload_sha256_mismatch",
  "signature_invalid",
  "manifest_schema_invalid",
  "manifest_channel_mismatch",
] as const
export type UpdateVerificationFailure = (typeof updateVerificationFailures)[number]

export type UpdateVerificationResult =
  | { readonly ok: true; readonly manifest: UpdateManifest }
  | { readonly ok: false; readonly reason: UpdateVerificationFailure }

// Effect Schema's decoder service type is intentionally erased at this
// perimeter; each call site pins a concrete schema and result type.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const decodeExit = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? (result.value as A) : null
}

/**
 * Verify a signed update manifest against a pinned public key, fail closed.
 *
 * Order matters and every early exit is a REJECTION: envelope decode → alg →
 * kid pin → payload sha256 → ed25519 verify → manifest schema decode →
 * channel cross-check. Only a manifest that survives all seven gates is
 * returned, and the caller still must check `verifyArtifactDigest` and
 * `isMonotonicUpgrade` before staging anything.
 */
export const verifySignedUpdateManifest = (
  manifestBytes: Uint8Array,
  signatureEnvelope: unknown,
  pin: PinnedReleaseKey,
  expectedChannel: UpdateChannel,
): UpdateVerificationResult => {
  const envelope = decodeExit<UpdateSignature>(UpdateSignatureSchema, signatureEnvelope)
  if (envelope === null) return { ok: false, reason: "malformed_signature_envelope" }
  if (envelope.alg !== "ed25519" || pin.alg !== "ed25519") {
    return { ok: false, reason: "unexpected_algorithm" }
  }
  if (envelope.kid !== pin.kid) return { ok: false, reason: "kid_not_pinned" }

  const digest = createHash("sha256").update(manifestBytes).digest("hex")
  if (digest !== envelope.sha256) return { ok: false, reason: "payload_sha256_mismatch" }

  let signatureValid = false
  try {
    const publicKey = createPublicKey({ key: { kty: "OKP", crv: "Ed25519", x: pin.x }, format: "jwk" })
    signatureValid = edVerify(
      null,
      manifestBytes,
      publicKey,
      Buffer.from(envelope.signature, "base64url"),
    )
  } catch {
    signatureValid = false
  }
  if (!signatureValid) return { ok: false, reason: "signature_invalid" }

  let parsed: unknown
  try {
    parsed = JSON.parse(Buffer.from(manifestBytes).toString("utf8"))
  } catch {
    return { ok: false, reason: "manifest_schema_invalid" }
  }
  const manifest = decodeExit<UpdateManifest>(UpdateManifestSchema, parsed)
  if (manifest === null) return { ok: false, reason: "manifest_schema_invalid" }
  if (manifest.channel !== expectedChannel) return { ok: false, reason: "manifest_channel_mismatch" }
  return { ok: true, manifest }
}

/**
 * The downloaded artifact is admissible only when its bytes hash to exactly
 * the manifest's `artifactSha256` AND its length matches. Both checks fail
 * closed; there is no partial acceptance.
 */
export const verifyArtifactDigest = (
  manifest: UpdateManifest,
  artifactBytes: Uint8Array,
): boolean => {
  if (artifactBytes.byteLength !== manifest.artifactByteLength) return false
  return createHash("sha256").update(artifactBytes).digest("hex") === manifest.artifactSha256
}

// ---------------------------------------------------------------------------
// Post-update first-launch receipt (DMG-1, #8786)
//
// From the 2026-07-13 ChatGPT incident
// (docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md): an
// updater swapped a working app for one the machine refused to exec and
// never noticed. Applying an update is therefore NOT success — the first
// demonstrated launch of the new build is. The freshly launched app writes
// this typed marker; the update host keeps the previous version staged
// until the marker appears within a bounded window, else it rolls back
// automatically with a diagnostic (`update-rollback.ts` models the states).
// ---------------------------------------------------------------------------

export const LAUNCH_RECEIPT_SCHEMA_ID = "openagents.desktop.launch_receipt.v1" as const

const IsoInstantSchema = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
)

export const LaunchReceiptSchema = Schema.Struct({
  schema: Schema.Literal(LAUNCH_RECEIPT_SCHEMA_ID),
  app: Schema.Literal("openagents-desktop"),
  /** The version that demonstrably reached first launch — must equal the applied candidate. */
  version: ReleaseVersionSchema,
  launchedAt: IsoInstantSchema,
})
export type LaunchReceipt = typeof LaunchReceiptSchema.Type

/**
 * Bounded receipt window: the previous release stays staged for rollback
 * until the new build writes its first-launch receipt. No receipt within
 * this window → automatic rollback. Ten minutes comfortably covers a slow
 * relaunch while still bounding how long a dead update can sit undetected.
 */
export const LAUNCH_RECEIPT_WINDOW_MS = 10 * 60 * 1000

export const createLaunchReceipt = (version: ReleaseVersion, launchedAt: string): LaunchReceipt => ({
  schema: LAUNCH_RECEIPT_SCHEMA_ID,
  app: "openagents-desktop",
  version,
  launchedAt,
})

export const launchReceiptProblems = [
  "receipt_missing",
  "receipt_invalid",
  "receipt_version_mismatch",
] as const
export type LaunchReceiptProblem = (typeof launchReceiptProblems)[number]

export type LaunchReceiptEvaluation =
  | { readonly outcome: "confirmed" }
  | {
    readonly outcome: "awaiting"
    readonly problem: LaunchReceiptProblem
    readonly remainingMs: number
  }
  | { readonly outcome: "rollback_required"; readonly problem: LaunchReceiptProblem }

/**
 * Deterministic, clock-free receipt evaluation (the host injects both
 * instants). Fail closed: only a schema-valid receipt whose version equals
 * the applied candidate confirms the update. Anything else — absent marker,
 * undecodable marker, stale marker from the previous build — is a problem
 * that becomes `rollback_required` once the bounded window has elapsed.
 * A LATE receipt (arriving after the window) never resurrects the update;
 * by then the state machine has already left `awaiting_launch_receipt` and
 * refuses the event.
 */
export const evaluateLaunchReceipt = (input: {
  /** Parsed marker document as read from disk, or `null` when absent. */
  readonly receipt: unknown | null
  readonly expectedVersion: string
  readonly appliedAtMs: number
  readonly nowMs: number
  readonly windowMs?: number
}): LaunchReceiptEvaluation => {
  const windowMs = input.windowMs ?? LAUNCH_RECEIPT_WINDOW_MS
  const problem: LaunchReceiptProblem | null = (() => {
    if (input.receipt === null) return "receipt_missing"
    const decoded = decodeExit<LaunchReceipt>(LaunchReceiptSchema, input.receipt)
    if (decoded === null) return "receipt_invalid"
    if (decoded.version !== input.expectedVersion) return "receipt_version_mismatch"
    return null
  })()
  if (problem === null) return { outcome: "confirmed" }
  const elapsedMs = input.nowMs - input.appliedAtMs
  if (elapsedMs < windowMs) return { outcome: "awaiting", problem, remainingMs: windowMs - elapsedMs }
  return { outcome: "rollback_required", problem }
}
