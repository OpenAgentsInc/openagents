/**
 * Omega download identity (#9280, Omega alpha roadmap Wave 3 / EP263-07).
 *
 * Omega is a SEPARATE product from OpenAgents Desktop. This resolver serves
 * the signed Omega alpha download truth beside — never instead of — the
 * DIST-10 Desktop resolver (`desktop-download-resolver.server.ts`). The
 * Electron Desktop artifact is never relabeled as Omega, and Omega artifacts
 * never enter the Desktop ReleaseSet.
 *
 * ## Signed manifest — the single Omega download truth
 *
 * The download surface renders exclusively from ONE Ed25519-signed manifest
 * (`openagents.omega.download_manifest.v1`), signed by the same pinned
 * OpenAgents release/provenance key the Desktop ReleaseSet uses
 * (`PRODUCTION_RELEASE_KEY_PIN`, kid `2dbe811d19f67528`; see
 * `apps/oa-updates/docs/release-signing-runbook.md`). There is no second
 * unsigned download truth: no handwritten artifact URL, no live GitHub API
 * inference, no transport-trust shortcut. If the embedded manifest fails
 * signature or schema verification, the resolver fails closed to a typed
 * `unavailable` projection with no URL at all.
 *
 * The signed payload and its detached signature are checked in as the
 * generated module `omega-release/omega-download-manifest.gen.ts`, produced
 * from the human-edited source manifest
 * `omega-release/omega-download-manifest.json` by
 * `scripts/sign-omega-download-manifest.ts` (which self-verifies through this
 * module's exact verifier before writing). Publishing a new Omega RC is a
 * data change, not a code change: edit the source manifest (version, tag,
 * URL, digest, byte length, notarization truth, limitations), re-run the
 * signing script, commit, deploy.
 *
 * ## Per-cell truth
 *
 * Each published platform cell carries version, channel (alpha/prerelease),
 * platform, architecture, package format, minimum OS, artifact SHA-256
 * digest, and Apple signature/notarization truth. Targets without signed
 * installed evidence appear ONLY in `unavailableTargets` with an explicit
 * statement — a cell is published exactly when the signed manifest carries
 * it. Artifact URLs must live under the manifest's own
 * `<sourceRepository>/releases/download/<releaseTag>/` prefix, so a signed
 * manifest can never point outside its named GitHub release.
 */
import { createHash, createPublicKey, verify as edVerify } from 'node:crypto'

import { Exit, Schema } from 'effect'

import {
  PRODUCTION_RELEASE_KEY_PIN,
  UpdateSignatureSchema,
  type PinnedReleaseKey,
} from '@openagentsinc/release-contract/update-contract'

import {
  omegaDownloadManifestPayload,
  omegaDownloadManifestSignature,
} from './omega-release/omega-download-manifest.gen.ts'

// ---------------------------------------------------------------------------
// Public paths (admitted through the Cloud Run Start seam in
// `workers/api/src/cloudrun/start-ui.ts`, same pattern as the Desktop
// resolver's `/api/public/desktop-download` paths)
// ---------------------------------------------------------------------------

export const OMEGA_DOWNLOAD_RESOLUTION_PATH = '/api/public/omega-download'
export const OMEGA_DOWNLOAD_ARTIFACT_PATH = '/api/public/omega-download/artifact'

// ---------------------------------------------------------------------------
// Signed manifest contract (Effect Schema is the authority)
// ---------------------------------------------------------------------------

export const OMEGA_DOWNLOAD_MANIFEST_SCHEMA_ID =
  'openagents.omega.download_manifest.v1' as const

export const omegaDownloadTargets = [
  'darwin-arm64',
  'darwin-x64',
  'win32-arm64',
  'win32-x64',
  'linux-arm64',
  'linux-x64',
] as const
export type OmegaDownloadTarget = (typeof omegaDownloadTargets)[number]

export const omegaDownloadFormats = ['dmg', 'zip', 'nsis', 'appimage', 'deb', 'rpm'] as const
export type OmegaDownloadFormat = (typeof omegaDownloadFormats)[number]

/** Apple Developer ID / notarization truth for a macOS artifact. */
export const omegaNotarizationStates = ['notarized', 'not_notarized'] as const
export type OmegaNotarizationState = (typeof omegaNotarizationStates)[number]

const BoundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4_096))
const Sha256Hex = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/))
const HttpsUrl = Schema.String.check(
  Schema.isMinLength(9),
  Schema.isMaxLength(2_048),
  Schema.isPattern(/^https:\/\/[^\s]+$/),
)
const IsoInstant = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/),
)
const SourceRevision = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))
/** Omega prerelease version, e.g. `0.2.0-rc26` (dot-free RC suffix). */
const OmegaVersion = Schema.String.check(
  Schema.isMinLength(5),
  Schema.isMaxLength(40),
  Schema.isPattern(/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.]+)?$/),
)
const ReleaseTag = Schema.String.check(
  Schema.isMinLength(2),
  Schema.isMaxLength(64),
  Schema.isPattern(/^v[0-9A-Za-z.-]+$/),
)

export const OmegaAppleSigningSchema = Schema.Struct({
  teamId: Schema.String.check(Schema.isPattern(/^[A-Z0-9]{10}$/)),
  signingIdentity: BoundedText,
  notarization: Schema.Literals(omegaNotarizationStates),
})
export type OmegaAppleSigning = typeof OmegaAppleSigningSchema.Type

export const OmegaDownloadArtifactSchema = Schema.Struct({
  target: Schema.Literals(omegaDownloadTargets),
  format: Schema.Literals(omegaDownloadFormats),
  name: Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(160),
    Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/),
  ),
  url: HttpsUrl,
  sha256: Sha256Hex,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  /** Minimum OS for this cell, e.g. `11.0` on darwin targets. */
  minimumOs: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  /** Present exactly on darwin targets: Developer ID + notarization truth. */
  apple: Schema.optionalKey(OmegaAppleSigningSchema),
})
export type OmegaDownloadArtifact = typeof OmegaDownloadArtifactSchema.Type

export const OmegaUnavailableTargetSchema = Schema.Struct({
  target: Schema.Literals(omegaDownloadTargets),
  statement: BoundedText,
})
export type OmegaUnavailableTarget = typeof OmegaUnavailableTargetSchema.Type

export const OmegaDownloadManifestSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DOWNLOAD_MANIFEST_SCHEMA_ID),
  product: Schema.Literal('omega'),
  productName: Schema.Literal('Omega'),
  channel: Schema.Literal('alpha'),
  version: OmegaVersion,
  releaseTag: ReleaseTag,
  releasedAt: IsoInstant,
  releaseNotes: Schema.NullOr(BoundedText),
  releasePageUrl: HttpsUrl,
  sourceRepository: HttpsUrl,
  sourceRevision: SourceRevision,
  signingPolicy: Schema.Struct({
    alg: Schema.Literal('ed25519'),
    keyId: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  }),
  artifacts: Schema.Array(OmegaDownloadArtifactSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(24),
  ),
  unavailableTargets: Schema.Array(OmegaUnavailableTargetSchema).check(Schema.isMaxLength(8)),
  knownLimitations: Schema.Array(BoundedText).check(Schema.isMaxLength(16)),
})
export type OmegaDownloadManifest = typeof OmegaDownloadManifestSchema.Type

const decodeManifestExit = Schema.decodeUnknownExit(OmegaDownloadManifestSchema)
const decodeSignatureExit = Schema.decodeUnknownExit(UpdateSignatureSchema)

// ---------------------------------------------------------------------------
// Verification — pinned Ed25519, fail closed
// ---------------------------------------------------------------------------

export const omegaManifestVerificationFailures = [
  'malformed_signature_envelope',
  'kid_not_pinned',
  'payload_sha256_mismatch',
  'signature_invalid',
  'manifest_schema_invalid',
  'signing_policy_mismatch',
  'artifact_url_outside_release',
  'target_conflict',
] as const
export type OmegaManifestVerificationFailure =
  (typeof omegaManifestVerificationFailures)[number]

export type OmegaManifestVerificationResult =
  | { readonly ok: true; readonly manifest: OmegaDownloadManifest }
  | { readonly ok: false; readonly reason: OmegaManifestVerificationFailure }

/**
 * Verify the signed Omega download manifest: envelope decode → kid pin →
 * payload sha256 → ed25519 signature → schema decode → signing-policy
 * crosscheck → artifact-URL release binding → published/unavailable target
 * disjointness. Any failure is typed and terminal — never a partial result.
 */
export const verifySignedOmegaDownloadManifest = (
  payloadText: string,
  signatureValue: unknown,
  pin: PinnedReleaseKey,
): OmegaManifestVerificationResult => {
  const envelopeExit = decodeSignatureExit(signatureValue)
  if (!Exit.isSuccess(envelopeExit)) {
    return { ok: false, reason: 'malformed_signature_envelope' }
  }
  const envelope = envelopeExit.value
  if (envelope.alg !== 'ed25519' || pin.alg !== 'ed25519') {
    return { ok: false, reason: 'malformed_signature_envelope' }
  }
  if (envelope.kid !== pin.kid) return { ok: false, reason: 'kid_not_pinned' }
  const payloadBytes = new TextEncoder().encode(payloadText)
  if (createHash('sha256').update(payloadBytes).digest('hex') !== envelope.sha256) {
    return { ok: false, reason: 'payload_sha256_mismatch' }
  }
  try {
    const publicKey = createPublicKey({
      key: { kty: 'OKP', crv: 'Ed25519', x: pin.x },
      format: 'jwk',
    })
    if (
      !edVerify(null, payloadBytes, publicKey, Buffer.from(envelope.signature, 'base64url'))
    ) {
      return { ok: false, reason: 'signature_invalid' }
    }
  } catch {
    return { ok: false, reason: 'signature_invalid' }
  }
  let raw: unknown
  try {
    raw = JSON.parse(payloadText)
  } catch {
    return { ok: false, reason: 'manifest_schema_invalid' }
  }
  const decoded = decodeManifestExit(raw)
  if (!Exit.isSuccess(decoded)) return { ok: false, reason: 'manifest_schema_invalid' }
  const manifest = decoded.value
  if (manifest.signingPolicy.keyId !== pin.kid) {
    return { ok: false, reason: 'signing_policy_mismatch' }
  }
  const releasePrefix = `${manifest.sourceRepository}/releases/download/${manifest.releaseTag}/`
  if (manifest.artifacts.some(artifact => !artifact.url.startsWith(releasePrefix))) {
    return { ok: false, reason: 'artifact_url_outside_release' }
  }
  const published = new Set(manifest.artifacts.map(artifact => artifact.target))
  if (manifest.unavailableTargets.some(row => published.has(row.target))) {
    return { ok: false, reason: 'target_conflict' }
  }
  return { ok: true, manifest }
}

// ---------------------------------------------------------------------------
// Typed resolution contract (the client-safe mirror lives in
// `routes/-omega-download-data.ts`, same split as the Desktop resolver)
// ---------------------------------------------------------------------------

export const OMEGA_DOWNLOAD_RESOLUTION_SCHEMA_ID =
  'openagents.omega.download_resolution.v1' as const

const AvailableOmegaResolutionSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  product: Schema.Literal('omega'),
  productName: Schema.Literal('Omega'),
  availability: Schema.Literal('available'),
  channel: Schema.Literal('alpha'),
  version: OmegaVersion,
  releaseTag: ReleaseTag,
  releasedAt: IsoInstant,
  releaseNotes: Schema.NullOr(BoundedText),
  releasePageUrl: HttpsUrl,
  sourceRepository: HttpsUrl,
  sourceRevision: SourceRevision,
  artifacts: Schema.Array(OmegaDownloadArtifactSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(24),
  ),
  unavailableTargets: Schema.Array(OmegaUnavailableTargetSchema).check(Schema.isMaxLength(8)),
  knownLimitations: Schema.Array(BoundedText).check(Schema.isMaxLength(16)),
})

const UnavailableOmegaResolutionSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  product: Schema.Literal('omega'),
  availability: Schema.Literal('unavailable'),
  reason: Schema.Literals(omegaManifestVerificationFailures),
})

export const OmegaDownloadResolutionSchema = Schema.Union([
  AvailableOmegaResolutionSchema,
  UnavailableOmegaResolutionSchema,
])
export type OmegaDownloadResolution = typeof OmegaDownloadResolutionSchema.Type
export type OmegaDownloadAvailableResolution = typeof AvailableOmegaResolutionSchema.Type

const decodeResolutionExit = Schema.decodeUnknownExit(OmegaDownloadResolutionSchema)

// ---------------------------------------------------------------------------
// Resolver — embedded signed manifest, verified once per process, fail closed
// ---------------------------------------------------------------------------

export type OmegaDownloadResolver = Readonly<{
  /** Route both public paths; `undefined` for any other path. */
  handle: (request: Request) => Promise<Response | undefined>
  /** Pure resolution for the SSR loader and tests (no Response wrapper). */
  resolve: () => OmegaDownloadResolution
}>

export const createOmegaDownloadResolver = (input?: Readonly<{
  payloadText?: string
  signature?: unknown
  pin?: PinnedReleaseKey
}>): OmegaDownloadResolver => {
  const payloadText = input?.payloadText ?? omegaDownloadManifestPayload
  const signature = input?.signature ?? omegaDownloadManifestSignature
  const pin = input?.pin ?? PRODUCTION_RELEASE_KEY_PIN

  let cached: OmegaDownloadResolution | undefined

  const resolve = (): OmegaDownloadResolution => {
    if (cached !== undefined) return cached
    const verified = verifySignedOmegaDownloadManifest(payloadText, signature, pin)
    if (!verified.ok) {
      cached = {
        schema: OMEGA_DOWNLOAD_RESOLUTION_SCHEMA_ID,
        product: 'omega',
        availability: 'unavailable',
        reason: verified.reason,
      }
      return cached
    }
    const manifest = verified.manifest
    cached = {
      schema: OMEGA_DOWNLOAD_RESOLUTION_SCHEMA_ID,
      product: 'omega',
      productName: 'Omega',
      availability: 'available',
      channel: 'alpha',
      version: manifest.version,
      releaseTag: manifest.releaseTag,
      releasedAt: manifest.releasedAt,
      releaseNotes: manifest.releaseNotes,
      releasePageUrl: manifest.releasePageUrl,
      sourceRepository: manifest.sourceRepository,
      sourceRevision: manifest.sourceRevision,
      artifacts: manifest.artifacts,
      unavailableTargets: manifest.unavailableTargets,
      knownLimitations: manifest.knownLimitations,
    }
    return cached
  }

  const handleResolution = (): Response => {
    const resolution = resolve()
    // Self-check the outgoing contract; a non-conforming projection is a bug
    // and MUST NOT be served as truth.
    if (!Exit.isSuccess(decodeResolutionExit(resolution))) {
      return Response.json(
        { error: 'resolution_contract_violation' },
        { status: 500, headers: { 'cache-control': 'no-store' } },
      )
    }
    return Response.json(resolution, { headers: { 'cache-control': 'no-store' } })
  }

  const handleArtifactRedirect = (url: URL): Response => {
    const target = url.searchParams.get('target')
    const format = url.searchParams.get('format')
    if (
      target === null ||
      format === null ||
      !(omegaDownloadTargets as readonly string[]).includes(target) ||
      !(omegaDownloadFormats as readonly string[]).includes(format)
    ) {
      return Response.json(
        { error: 'invalid_query', required: ['target', 'format'] },
        { status: 400, headers: { 'cache-control': 'no-store' } },
      )
    }
    const resolution = resolve()
    if (resolution.availability !== 'available') {
      return Response.json(
        { error: 'artifact_unavailable', reason: resolution.reason },
        { status: 503, headers: { 'cache-control': 'no-store' } },
      )
    }
    const artifact = resolution.artifacts.find(
      candidate => candidate.target === target && candidate.format === format,
    )
    if (artifact === undefined) {
      return Response.json(
        { error: 'artifact_unavailable', reason: 'target_or_format_not_in_manifest' },
        { status: 404, headers: { 'cache-control': 'no-store' } },
      )
    }
    return new Response(null, {
      status: 302,
      headers: { location: artifact.url, 'cache-control': 'no-store' },
    })
  }

  const handle = async (request: Request): Promise<Response | undefined> => {
    const url = new URL(request.url)
    if (
      url.pathname !== OMEGA_DOWNLOAD_RESOLUTION_PATH &&
      url.pathname !== OMEGA_DOWNLOAD_ARTIFACT_PATH
    ) {
      return undefined
    }
    if (request.method !== 'GET') {
      return Response.json(
        { error: 'method_not_allowed' },
        { status: 405, headers: { allow: 'GET' } },
      )
    }
    return url.pathname === OMEGA_DOWNLOAD_RESOLUTION_PATH
      ? handleResolution()
      : handleArtifactRedirect(url)
  }

  return { handle, resolve }
}

// ---------------------------------------------------------------------------
// Default server wiring (used by `server.ts` and the SSR /download loader)
// ---------------------------------------------------------------------------

let defaultResolver: OmegaDownloadResolver | undefined

export const routeOmegaDownloadRequest = (
  request: Request,
): Promise<Response | undefined> => {
  defaultResolver ??= createOmegaDownloadResolver()
  return defaultResolver.handle(request)
}

/** Direct server-side resolution for the SSR `/download` page loader. */
export const resolveOmegaDownload = (): OmegaDownloadResolution => {
  defaultResolver ??= createOmegaDownloadResolver()
  return defaultResolver.resolve()
}
