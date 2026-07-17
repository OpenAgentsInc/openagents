/**
 * DIST-10 (#8923): typed openagents.com server-side Desktop download resolver.
 *
 * One resolver derives every downloadable Desktop artifact URL shown by
 * openagents.com from the currently promoted, signature-verified release
 * feed. There is no handwritten artifact URL anywhere on this path: when the
 * feed cannot be fetched and verified, the resolver fails closed to a typed
 * "unavailable" projection with no URL at all.
 *
 * Trust anchor: the pinned Ed25519 release key (`PRODUCTION_RELEASE_KEY_PIN`)
 * and the LANDED `verifySignedReleaseSet` seam (ReleaseSet v2, #8915). This
 * module deliberately implements NO second verifier; transport (host, TLS,
 * CDN) is never the trust decision — the Ed25519 signature is.
 *
 * ## Feed contract — the REAL, now-landed ReleaseSet v2 feed (#8922)
 *
 * Verified against `apps/oa-updates/src/release-set-feed.ts` (route regex)
 * and `apps/oa-updates/docs/release-set-v2-feed-runbook.md`, not assumed:
 *
 * - Base URL: `OPENAGENTS_DESKTOP_RELEASE_FEED_BASE_URL` env var, default
 *   `https://updates.openagents.com`. Must be https.
 * - Pointer (mutable, 15s bounded cache at the origin):
 *   `GET <base>/desktop/openagents/<channel>/v2/pointer.json` →
 *   `openagents.desktop.release_pointer.v2` — `revision`, `generation`
 *   (== SHA-256 of the exact canonical signed payload bytes),
 *   `previousGeneration`, `payloadSha256` (== `generation`),
 *   `signatureSha256`, `publishedAt`.
 * - Immutable candidate (one-year immutable cache at the origin), fetched by
 *   the exact generation named in the pointer — never the "current" alias:
 *   `GET <base>/desktop/openagents/<channel>/candidates/<generation>/release-set.json`
 *   and `.../release-set.sig.json`. Both responses carry
 *   `x-openagents-release-generation` equal to the requested generation.
 * - Default channel: `OPENAGENTS_DESKTOP_DOWNLOAD_CHANNEL` env var
 *   (`stable` | `rc`), default `rc` while no stable release exists.
 *
 * The bounded v1 darwin-arm64 migration path (used while #8922 did not yet
 * exist) is intentionally REMOVED now that the real v2 authority is live:
 * its unsigned unbounded transport pointer could name any HTTPS host for the
 * redirect target, since the v1 manifest signs only artifact name/hash/length,
 * never the URL. There is exactly one verified authority now; no downgrade
 * surface exists to reason about.
 *
 * ## Integrity binding (fixes the published P0/P1 audit findings)
 *
 * - **Content binding (was: unbound v1 redirect):** the candidate response's
 *   `x-openagents-release-generation` header AND the SHA-256 of its actual
 *   bytes must both equal the pointer's `generation`/`payloadSha256` (and the
 *   signature bytes' hash must equal `signatureSha256`) before the Ed25519
 *   signature is even checked. This guards against combining a payload from
 *   one candidate with a signature from another. The signature check is the
 *   actual root of trust: an attacker who controls the unsigned pointer and
 *   storage layer can set generation/hash labels to match tampered bytes, but
 *   cannot forge a valid Ed25519 signature over those bytes without the
 *   private key.
 * - **Replay/rollback (was: no monotonic tracking):** each resolver instance
 *   remembers the last verified pointer per channel and rejects any new
 *   pointer whose `revision` decreases, whose `publishedAt` moves backward,
 *   whose same-revision content differs (a fork), or whose claimed
 *   `previousGeneration` breaks the observed chain. A never-before-seen
 *   channel trusts its first fully-signature-verified pointer (TOFU), then
 *   enforces monotonicity for the life of the process.
 * - **No partial-404 downgrade:** there is no fallback path left to trigger.
 * - **Cache generation race:** `loadSnapshot` singleflights concurrent
 *   revalidation per channel (one in-flight fetch promise), so two racing
 *   requests can never let a slower stale fetch clobber a faster fresh one.
 * - **Bounded body/error handling:** response bodies are read via a capped
 *   streaming reader (never a bare `arrayBuffer()`), abort past the byte
 *   cap, and fail closed on any stream/decode error instead of throwing.
 *
 * ## Caching / freshness
 *
 * A verified snapshot is cached in-memory per channel for `CACHE_TTL_MS`.
 * Past the TTL the resolver MUST revalidate against the feed; a failed
 * revalidation serves "unavailable", never a stale snapshot, so the cache can
 * never outlive channel-pointer policy. A snapshot is one atomically verified
 * document, so a response can never mix one version's label with another
 * version's artifact. HTTP responses are `no-store` (they vary on client
 * hints/UA and must follow promotion immediately).
 *
 * ## Telemetry
 *
 * First-party, public-safe, schema-validated (`openagents.desktop.
 * download_telemetry.v1` — a fresh event, never a retired Khala/Autopilot
 * event name): server-generated event ref, day bucket, channel, outcome,
 * version, target, format, and a bounded referrer category. No prompts,
 * paths, account identity, credentials, machine IDs, raw user-agent strings,
 * or IP addresses. The default sink is one structured stdout line (Cloud Run
 * logging); tests inject a capture sink.
 */
import { createHash } from 'node:crypto'

import { Exit, Schema } from 'effect'

import {
  releaseFormats,
  releaseTargetKeys,
  verifySignedReleaseSet,
  type ReleaseFormat,
  type ReleaseSet,
  type ReleaseSetTarget,
  type ReleaseTargetKey,
} from '../../../../../apps/openagents-desktop/src/release-set-contract.ts'
import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  type UpdateChannel,
} from '../../../../../apps/openagents-desktop/src/update-contract.ts'

// ---------------------------------------------------------------------------
// Public paths (admitted through the Cloud Run Start seam in
// `workers/api/src/cloudrun/start-ui.ts`, same pattern as /api/public/qa-board)
// ---------------------------------------------------------------------------

export const DESKTOP_DOWNLOAD_RESOLUTION_PATH = '/api/public/desktop-download'
export const DESKTOP_DOWNLOAD_ARTIFACT_PATH = '/api/public/desktop-download/artifact'

// Feed pointer paths — the documented DIST-09 config contract (see header).
export const releaseSetPointerPath = (channel: UpdateChannel): string =>
  `/desktop/openagents/${channel}/v2/pointer.json`
export const releaseSetCandidatePayloadPath = (
  channel: UpdateChannel,
  generation: string,
): string => `/desktop/openagents/${channel}/candidates/${generation}/release-set.json`
export const releaseSetCandidateSignaturePath = (
  channel: UpdateChannel,
  generation: string,
): string => `/desktop/openagents/${channel}/candidates/${generation}/release-set.sig.json`
export const CACHE_TTL_MS = 60_000
const MAX_PAYLOAD_BYTES = 512 * 1024
const MAX_SIGNATURE_BYTES = 4 * 1024
const MAX_POINTER_BYTES = 4 * 1024

// ---------------------------------------------------------------------------
// Typed response contract (Effect Schema is the authority; the client-safe
// helper in `routes/-download-data.ts` type-imports from here)
// ---------------------------------------------------------------------------

export const DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA_ID =
  'openagents.desktop.download_resolution.v1' as const

const ChannelSchema = Schema.Literals(['stable', 'rc'])
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

export const DesktopDownloadDetectionSchema = Schema.Struct({
  platform: Schema.NullOr(Schema.Literals(['darwin', 'win32', 'linux'])),
  architecture: Schema.NullOr(Schema.Literals(['arm64', 'x64'])),
  method: Schema.Literals(['override', 'client_hints', 'user_agent', 'none']),
})
export type DesktopDownloadDetection = typeof DesktopDownloadDetectionSchema.Type

export const DesktopDownloadArtifactSchema = Schema.Struct({
  target: Schema.Literals(releaseTargetKeys),
  format: Schema.Literals(releaseFormats),
  version: BoundedText,
  channel: ChannelSchema,
  url: HttpsUrl,
  sha256: Sha256Hex,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  minimumOs: BoundedText,
  preferred: Schema.Boolean,
})
export type DesktopDownloadArtifact = typeof DesktopDownloadArtifactSchema.Type

const SourceSchema = Schema.Literal('release_set_v2')

const releaseFields = {
  schema: Schema.Literal(DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  source: SourceSchema,
  channel: ChannelSchema,
  version: BoundedText,
  releasedAt: IsoInstant,
  releaseNotes: Schema.NullOr(BoundedText),
  sourceRevision: Schema.NullOr(Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))),
  detection: DesktopDownloadDetectionSchema,
} as const

const AvailableResolutionSchema = Schema.Struct({
  ...releaseFields,
  availability: Schema.Literal('available'),
  selected: DesktopDownloadArtifactSchema,
  alternatives: Schema.Array(DesktopDownloadArtifactSchema).check(Schema.isMaxLength(24)),
})

export const chooseManuallyReasons = [
  'unknown_client',
  'target_unavailable',
  'format_unavailable',
] as const
const ChooseManuallyResolutionSchema = Schema.Struct({
  ...releaseFields,
  availability: Schema.Literal('choose_manually'),
  reason: Schema.Literals(chooseManuallyReasons),
  options: Schema.Array(DesktopDownloadArtifactSchema).check(Schema.isMaxLength(24)),
})

export const unavailableReasons = [
  'feed_unreachable',
  'feed_schema_invalid',
  'release_pointer_invalid',
  'release_pointer_replayed',
  'release_candidate_mismatch',
  'release_set_verification_failed',
] as const
export type DesktopDownloadUnavailableReason = (typeof unavailableReasons)[number]
const UnavailableResolutionSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA_ID),
  availability: Schema.Literal('unavailable'),
  channel: ChannelSchema,
  reason: Schema.Literals(unavailableReasons),
  detection: DesktopDownloadDetectionSchema,
})

export const DesktopDownloadResolutionSchema = Schema.Union([
  AvailableResolutionSchema,
  ChooseManuallyResolutionSchema,
  UnavailableResolutionSchema,
])
export type DesktopDownloadResolution = typeof DesktopDownloadResolutionSchema.Type
export type DesktopDownloadAvailableResolution = typeof AvailableResolutionSchema.Type

const decodeResolutionExit = Schema.decodeUnknownExit(DesktopDownloadResolutionSchema)

// ---------------------------------------------------------------------------
// Telemetry contract — fresh first-party event, never a retired event name
// ---------------------------------------------------------------------------

export const DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID =
  'openagents.desktop.download_telemetry.v1' as const

export const telemetryReferrerCategories = [
  'homepage',
  'download-page',
  'internal',
  'external',
  'none',
] as const
export type TelemetryReferrerCategory = (typeof telemetryReferrerCategories)[number]

export const DesktopDownloadTelemetryEventSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID),
  /** Server-generated event ref — never a client or machine identifier. */
  eventRef: Schema.String.check(
    Schema.isPattern(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/),
  ),
  /** Day-bucketed timestamp only — no precise per-request instants. */
  day: Schema.String.check(Schema.isPattern(/^\d{4}-\d{2}-\d{2}$/)),
  channel: ChannelSchema,
  outcome: Schema.Literals(['available', 'choose_manually', 'unavailable', 'artifact_redirect']),
  version: Schema.NullOr(BoundedText),
  target: Schema.NullOr(Schema.Literals(releaseTargetKeys)),
  format: Schema.NullOr(Schema.Literals(releaseFormats)),
  referrer: Schema.Literals(telemetryReferrerCategories),
})
export type DesktopDownloadTelemetryEvent = typeof DesktopDownloadTelemetryEventSchema.Type

const decodeTelemetryExit = Schema.decodeUnknownExit(DesktopDownloadTelemetryEventSchema)

/** Validate then hand to the sink; an invalid event is dropped, never sent. */
export const emitDesktopDownloadTelemetry = (
  candidate: unknown,
  sink: (event: DesktopDownloadTelemetryEvent) => void,
): boolean => {
  const decoded = decodeTelemetryExit(candidate)
  if (!Exit.isSuccess(decoded)) return false
  sink(decoded.value)
  return true
}

/** Bounded first-party referrer categorization — deterministic field parse. */
export const referrerCategory = (
  refererHeader: string | null,
  requestHost: string,
): TelemetryReferrerCategory => {
  if (refererHeader === null || refererHeader.length === 0 || refererHeader.length > 2_048) {
    return 'none'
  }
  let url: URL
  try {
    url = new URL(refererHeader)
  } catch {
    return 'none'
  }
  if (url.host !== requestHost && url.host !== 'openagents.com') return 'external'
  if (url.pathname === '/') return 'homepage'
  if (url.pathname === '/download') return 'download-page'
  return 'internal'
}

// ---------------------------------------------------------------------------
// Request OS/architecture detection — bounded, deterministic field parsing of
// client hints and user-agent fields (never keyword intent routing)
// ---------------------------------------------------------------------------

type HostPlatform = 'darwin' | 'win32' | 'linux'
type HostArchitecture = 'arm64' | 'x64'

const unquoteHint = (value: string | null): string | null => {
  if (value === null || value.length === 0 || value.length > 64) return null
  const match = /^"([^"]*)"$/.exec(value.trim())
  return match === null ? value.trim() : (match[1] ?? null)
}

const hintPlatform = (headers: Headers): HostPlatform | null => {
  switch (unquoteHint(headers.get('sec-ch-ua-platform'))) {
    case 'macOS':
      return 'darwin'
    case 'Windows':
      return 'win32'
    case 'Linux':
      return 'linux'
    default:
      return null
  }
}

const hintArchitecture = (headers: Headers): HostArchitecture | null => {
  const arch = unquoteHint(headers.get('sec-ch-ua-arch'))
  if (arch === 'arm') return 'arm64'
  if (arch === 'x86') {
    // 32-bit x86 is not a supported target; only 64-bit maps to x64.
    return unquoteHint(headers.get('sec-ch-ua-bitness')) === '64' ? 'x64' : null
  }
  return null
}

const uaPlatform = (ua: string): HostPlatform | null => {
  if (/Android|iPhone|iPad|iPod|CrOS/.test(ua)) return null
  if (/Windows NT \d/.test(ua)) return 'win32'
  if (/Macintosh|Mac OS X/.test(ua)) return 'darwin'
  if (/\bLinux\b/.test(ua)) return 'linux'
  return null
}

const uaArchitecture = (ua: string, platform: HostPlatform): HostArchitecture | null => {
  if (/\b(?:ARM64|aarch64|arm64)\b/i.test(ua)) return 'arm64'
  // Deliberate: `Intel Mac OS X` is NOT mapped to darwin-x64 — Safari and
  // Chrome report Intel on Apple-silicon Macs, so the token proves nothing.
  // Undetectable architecture yields an honest choose-manually projection.
  if (platform === 'darwin') return null
  if (/\b(?:Win64|WOW64|x64|x86_64|amd64)\b/i.test(ua)) return 'x64'
  return null
}

export const detectDesktopClient = (headers: Headers): DesktopDownloadDetection => {
  const hintedPlatform = hintPlatform(headers)
  if (hintedPlatform !== null) {
    return {
      platform: hintedPlatform,
      architecture: hintArchitecture(headers),
      method: 'client_hints',
    }
  }
  const ua = headers.get('user-agent')
  if (ua !== null && ua.length > 0 && ua.length <= 1_024) {
    const platform = uaPlatform(ua)
    if (platform !== null) {
      return { platform, architecture: uaArchitecture(ua, platform), method: 'user_agent' }
    }
  }
  return { platform: null, architecture: null, method: 'none' }
}

// ---------------------------------------------------------------------------
// Query overrides — explicit target/format/channel selection
// ---------------------------------------------------------------------------

const QueryOverridesSchema = Schema.Struct({
  channel: Schema.optionalKey(ChannelSchema),
  target: Schema.optionalKey(Schema.Literals(releaseTargetKeys)),
  format: Schema.optionalKey(Schema.Literals(releaseFormats)),
})
type QueryOverrides = typeof QueryOverridesSchema.Type
/** Explicit channel/target/format selection — shared with the /download page. */
export type DesktopDownloadOverrides = QueryOverrides
const decodeQueryExit = Schema.decodeUnknownExit(QueryOverridesSchema)

const parseQueryOverrides = (url: URL): QueryOverrides | null => {
  const raw: Record<string, string> = {}
  for (const key of ['channel', 'target', 'format'] as const) {
    const value = url.searchParams.get(key)
    if (value !== null) raw[key] = value
  }
  const decoded = decodeQueryExit(raw)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

// ---------------------------------------------------------------------------
// Verified feed snapshot (fetch + pinned verification + bounded cache)
// ---------------------------------------------------------------------------

type VerifiedSnapshot = {
  readonly source: 'release_set_v2'
  readonly releaseSet: ReleaseSet
  readonly pointer: ReleaseSetPointer
}

type ReleaseSetPointer = Readonly<{
  schema: 'openagents.desktop.release_pointer.v2'
  channel: UpdateChannel
  revision: number
  generation: string
  previousGeneration: string | null
  payloadSha256: string
  signatureSha256: string
  publishedAt: string
}>

type SnapshotResult =
  | { readonly ok: true; readonly snapshot: VerifiedSnapshot }
  | { readonly ok: false; readonly reason: DesktopDownloadUnavailableReason }

const boundedBody = async (response: Response, maximum: number): Promise<Uint8Array | null> => {
  const contentLength = response.headers.get('content-length')
  if (contentLength !== null) {
    if (!/^\d+$/.test(contentLength)) return null
    const declared = Number(contentLength)
    if (!Number.isSafeInteger(declared) || declared > maximum) return null
  }
  if (response.body === null) return new Uint8Array()
  const reader = response.body.getReader()
  const chunks: Array<Uint8Array> = []
  let total = 0
  try {
    while (true) {
      const result = await reader.read()
      if (result.done) break
      total += result.value.byteLength
      if (total > maximum) {
        await reader.cancel('body_size_invalid').catch(() => undefined)
        return null
      }
      chunks.push(result.value)
    }
  } catch {
    await reader.cancel('body_read_failed').catch(() => undefined)
    return null
  }
  const bytes = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  return bytes
}

const parseJson = (bytes: Uint8Array): unknown | undefined => {
  try {
    return JSON.parse(new TextDecoder().decode(bytes)) as unknown
  } catch {
    return undefined
  }
}

const sha256 = (bytes: Uint8Array): string =>
  createHash('sha256').update(bytes).digest('hex')

const sha256Pattern = /^[0-9a-f]{64}$/
const instantPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/

const decodeReleaseSetPointer = (
  candidate: unknown,
  channel: UpdateChannel,
  now: number,
): ReleaseSetPointer | null => {
  if (typeof candidate !== 'object' || candidate === null || Array.isArray(candidate)) return null
  const row = candidate as Record<string, unknown>
  if (
    Object.keys(row).toSorted().join(',') !==
      ['channel', 'generation', 'payloadSha256', 'previousGeneration', 'publishedAt', 'revision', 'schema', 'signatureSha256'].toSorted().join(',') ||
    row['schema'] !== 'openagents.desktop.release_pointer.v2' ||
    row['channel'] !== channel ||
    !Number.isSafeInteger(row['revision']) ||
    Number(row['revision']) <= 0 ||
    typeof row['generation'] !== 'string' ||
    !sha256Pattern.test(row['generation']) ||
    (row['previousGeneration'] !== null &&
      (typeof row['previousGeneration'] !== 'string' ||
        !sha256Pattern.test(row['previousGeneration']))) ||
    row['previousGeneration'] === row['generation'] ||
    typeof row['payloadSha256'] !== 'string' ||
    !sha256Pattern.test(row['payloadSha256']) ||
    row['payloadSha256'] !== row['generation'] ||
    typeof row['signatureSha256'] !== 'string' ||
    !sha256Pattern.test(row['signatureSha256']) ||
    typeof row['publishedAt'] !== 'string' ||
    !instantPattern.test(row['publishedAt'])
  ) return null
  const publishedAt = Date.parse(row['publishedAt'])
  if (!Number.isFinite(publishedAt) || publishedAt > now + 5 * 60_000) return null
  return row as ReleaseSetPointer
}

const samePointer = (left: ReleaseSetPointer, right: ReleaseSetPointer): boolean =>
  left.revision === right.revision &&
  left.generation === right.generation &&
  left.previousGeneration === right.previousGeneration &&
  left.payloadSha256 === right.payloadSha256 &&
  left.signatureSha256 === right.signatureSha256 &&
  left.publishedAt === right.publishedAt

export type DesktopDownloadResolverConfig = Readonly<{
  baseUrl: string
  defaultChannel: UpdateChannel
  pin: PinnedReleaseKey
  cacheTtlMs: number
}>

const envConfig = (): DesktopDownloadResolverConfig => {
  const baseCandidate = process.env['OPENAGENTS_DESKTOP_RELEASE_FEED_BASE_URL']
  const baseUrl =
    baseCandidate !== undefined && /^https:\/\/[^\s/]+$/.test(baseCandidate)
      ? baseCandidate
      : 'https://updates.openagents.com'
  const channelCandidate = process.env['OPENAGENTS_DESKTOP_DOWNLOAD_CHANNEL']
  const defaultChannel: UpdateChannel =
    channelCandidate === 'stable' || channelCandidate === 'rc' ? channelCandidate : 'rc'
  return { baseUrl, defaultChannel, pin: PRODUCTION_RELEASE_KEY_PIN, cacheTtlMs: CACHE_TTL_MS }
}

export type DesktopDownloadResolver = Readonly<{
  /** Route both resolver paths; `undefined` for any other path. */
  handle: (request: Request) => Promise<Response | undefined>
  /** Pure-ish resolution for direct consumers/tests (no Response wrapper). */
  resolve: (channel: UpdateChannel, headers: Headers, overrides?: QueryOverrides) =>
    Promise<DesktopDownloadResolution>
}>

export const createDesktopDownloadResolver = (input?: Readonly<{
  config?: Partial<DesktopDownloadResolverConfig>
  fetchFn?: typeof fetch
  nowMs?: () => number
  telemetrySink?: (event: DesktopDownloadTelemetryEvent) => void
}>): DesktopDownloadResolver => {
  const config: DesktopDownloadResolverConfig = { ...envConfig(), ...input?.config }
  const fetchFn = input?.fetchFn ?? fetch
  const nowMs = input?.nowMs ?? Date.now
  const sink =
    input?.telemetrySink ??
    ((event: DesktopDownloadTelemetryEvent): void => {
      console.log(JSON.stringify(event))
    })

  const cache = new Map<UpdateChannel, { fetchedAtMs: number; snapshot: VerifiedSnapshot }>()
  const lastPointers = new Map<UpdateChannel, ReleaseSetPointer>()
  const inFlight = new Map<UpdateChannel, Promise<SnapshotResult>>()

  const fetchSnapshot = async (channel: UpdateChannel): Promise<SnapshotResult> => {
    let pointerResponse: Response
    try {
      pointerResponse = await fetchFn(`${config.baseUrl}${releaseSetPointerPath(channel)}`)
    } catch {
      return { ok: false, reason: 'feed_unreachable' }
    }
    if (!pointerResponse.ok) return { ok: false, reason: 'feed_unreachable' }
    const pointerBytes = await boundedBody(pointerResponse, MAX_POINTER_BYTES)
    if (pointerBytes === null) return { ok: false, reason: 'feed_schema_invalid' }
    const pointer = decodeReleaseSetPointer(parseJson(pointerBytes), channel, nowMs())
    if (pointer === null) return { ok: false, reason: 'release_pointer_invalid' }

    const previous = lastPointers.get(channel)
    if (
      previous !== undefined &&
      (pointer.revision < previous.revision ||
        (pointer.revision === previous.revision && !samePointer(pointer, previous)) ||
        Date.parse(pointer.publishedAt) < Date.parse(previous.publishedAt) ||
        (pointer.revision === previous.revision + 1 &&
          pointer.previousGeneration !== previous.generation))
    ) return { ok: false, reason: 'release_pointer_replayed' }

    let payloadResponse: Response
    let signatureResponse: Response
    try {
      ;[payloadResponse, signatureResponse] = await Promise.all([
        fetchFn(`${config.baseUrl}${releaseSetCandidatePayloadPath(channel, pointer.generation)}`),
        fetchFn(`${config.baseUrl}${releaseSetCandidateSignaturePath(channel, pointer.generation)}`),
      ])
    } catch {
      return { ok: false, reason: 'feed_unreachable' }
    }
    if (!payloadResponse.ok || !signatureResponse.ok) {
      return { ok: false, reason: 'feed_unreachable' }
    }
    if (
      payloadResponse.headers.get('x-openagents-release-generation') !== pointer.generation ||
      signatureResponse.headers.get('x-openagents-release-generation') !== pointer.generation
    ) return { ok: false, reason: 'release_candidate_mismatch' }
    const [payloadBytes, signatureBytes] = await Promise.all([
      boundedBody(payloadResponse, MAX_PAYLOAD_BYTES),
      boundedBody(signatureResponse, MAX_SIGNATURE_BYTES),
    ])
    if (payloadBytes === null || signatureBytes === null) {
      return { ok: false, reason: 'feed_schema_invalid' }
    }
    if (
      sha256(payloadBytes) !== pointer.payloadSha256 ||
      sha256(signatureBytes) !== pointer.signatureSha256
    ) return { ok: false, reason: 'release_candidate_mismatch' }
    const signature = parseJson(signatureBytes)
    if (signature === undefined) return { ok: false, reason: 'feed_schema_invalid' }
    const verified = verifySignedReleaseSet(payloadBytes, signature, config.pin, channel)
    if (!verified.ok) return { ok: false, reason: 'release_set_verification_failed' }
    lastPointers.set(channel, pointer)
    return {
      ok: true,
      snapshot: { source: 'release_set_v2', releaseSet: verified.releaseSet, pointer },
    }
  }

  const loadSnapshot = async (channel: UpdateChannel): Promise<SnapshotResult> => {
    const cached = cache.get(channel)
    if (cached !== undefined && nowMs() - cached.fetchedAtMs < config.cacheTtlMs) {
      return { ok: true, snapshot: cached.snapshot }
    }
    const pending = inFlight.get(channel)
    if (pending !== undefined) return pending
    const refresh = fetchSnapshot(channel).then(result => {
      if (result.ok) {
        cache.set(channel, { fetchedAtMs: nowMs(), snapshot: result.snapshot })
      } else {
        cache.delete(channel)
      }
      return result
    }).finally(() => {
      if (inFlight.get(channel) === refresh) inFlight.delete(channel)
    })
    inFlight.set(channel, refresh)
    return refresh
  }

  // -- projection --------------------------------------------------------

  const projectV2Target = (
    releaseSet: ReleaseSet,
    row: ReleaseSetTarget,
  ): readonly DesktopDownloadArtifact[] =>
    row.artifacts.map(artifact => ({
      target: artifact.target,
      format: artifact.format,
      version: artifact.version,
      channel: releaseSet.channel,
      url: artifact.url,
      sha256: artifact.sha256,
      byteLength: artifact.byteLength,
      minimumOs: row.minimumOs,
      preferred: artifact.format === row.preferredFormat,
    }))

  const catalog = (snapshot: VerifiedSnapshot): readonly DesktopDownloadArtifact[] =>
    snapshot.releaseSet.targets.flatMap(row => projectV2Target(snapshot.releaseSet, row))

  const releaseHeader = (snapshot: VerifiedSnapshot) => ({
    schema: DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA_ID,
    source: snapshot.source,
    channel: snapshot.releaseSet.channel,
    version: snapshot.releaseSet.version,
    releasedAt: snapshot.releaseSet.publishedAt,
    releaseNotes: snapshot.releaseSet.releaseNotes.summary,
    sourceRevision: snapshot.releaseSet.sourceRevision,
  })

  const buildResolution = (
    snapshot: VerifiedSnapshot,
    detection: DesktopDownloadDetection,
    overrides: QueryOverrides,
  ): DesktopDownloadResolution => {
    const header = releaseHeader(snapshot)
    const options = catalog(snapshot)
    const detectedTarget =
      detection.platform !== null && detection.architecture !== null
        ? `${detection.platform}-${detection.architecture}`
        : null
    const target: ReleaseTargetKey | null =
      overrides.target ??
      (detectedTarget !== null &&
      (releaseTargetKeys as readonly string[]).includes(detectedTarget)
        ? (detectedTarget as ReleaseTargetKey)
        : null)
    const effectiveDetection: DesktopDownloadDetection =
      overrides.target !== undefined
        ? {
            platform: overrides.target.split('-')[0] as HostPlatform,
            architecture: overrides.target.split('-')[1] as HostArchitecture,
            method: 'override',
          }
        : detection

    if (target === null) {
      return {
        ...header,
        availability: 'choose_manually',
        reason: detectedTarget === null ? 'unknown_client' : 'target_unavailable',
        detection: effectiveDetection,
        options,
      }
    }
    const targetArtifacts = options.filter(artifact => artifact.target === target)
    if (targetArtifacts.length === 0) {
      return {
        ...header,
        availability: 'choose_manually',
        reason: 'target_unavailable',
        detection: effectiveDetection,
        options,
      }
    }
    const selected =
      overrides.format !== undefined
        ? targetArtifacts.find(artifact => artifact.format === overrides.format)
        : targetArtifacts.find(artifact => artifact.preferred)
    if (selected === undefined) {
      return {
        ...header,
        availability: 'choose_manually',
        reason: 'format_unavailable',
        detection: effectiveDetection,
        options,
      }
    }
    // Same-target alternatives first, then the other targets' preferred
    // artifacts — explicit alternatives per the DIST-01 §15 contract.
    const alternatives = [
      ...targetArtifacts.filter(artifact => artifact !== selected),
      ...options.filter(artifact => artifact.target !== target && artifact.preferred),
    ]
    return {
      ...header,
      availability: 'available',
      detection: effectiveDetection,
      selected,
      alternatives,
    }
  }

  const resolve = async (
    channel: UpdateChannel,
    headers: Headers,
    overrides: QueryOverrides = {},
  ): Promise<DesktopDownloadResolution> => {
    const detection = detectDesktopClient(headers)
    const loaded = await loadSnapshot(channel)
    if (!loaded.ok) {
      return {
        schema: DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA_ID,
        availability: 'unavailable',
        channel,
        reason: loaded.reason,
        detection,
      }
    }
    return buildResolution(loaded.snapshot, detection, overrides)
  }

  const emit = (
    request: Request,
    channel: UpdateChannel,
    outcome: DesktopDownloadTelemetryEvent['outcome'],
    fields: Readonly<{
      version?: string | null
      target?: ReleaseTargetKey | null
      format?: ReleaseFormat | null
    }>,
  ): void => {
    emitDesktopDownloadTelemetry(
      {
        schema: DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID,
        eventRef: crypto.randomUUID(),
        day: new Date(nowMs()).toISOString().slice(0, 10),
        channel,
        outcome,
        version: fields.version ?? null,
        target: fields.target ?? null,
        format: fields.format ?? null,
        referrer: referrerCategory(request.headers.get('referer'), new URL(request.url).host),
      },
      sink,
    )
  }

  const responseHeaders = {
    'cache-control': 'no-store',
    'accept-ch': 'Sec-CH-UA-Platform, Sec-CH-UA-Arch, Sec-CH-UA-Bitness',
    vary: 'Sec-CH-UA-Platform, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, User-Agent',
  } as const

  const handleResolution = async (request: Request, url: URL): Promise<Response> => {
    const overrides = parseQueryOverrides(url)
    if (overrides === null) {
      return Response.json(
        { error: 'invalid_query' },
        { status: 400, headers: { 'cache-control': 'no-store' } },
      )
    }
    const channel = overrides.channel ?? config.defaultChannel
    const resolution = await resolve(channel, request.headers, overrides)
    // Self-check the outgoing contract; a non-conforming projection is a bug
    // and MUST NOT be served as truth.
    if (!Exit.isSuccess(decodeResolutionExit(resolution))) {
      return Response.json(
        { error: 'resolution_contract_violation' },
        { status: 500, headers: { 'cache-control': 'no-store' } },
      )
    }
    emit(request, channel, resolution.availability, {
      version: resolution.availability === 'unavailable' ? null : resolution.version,
      target: resolution.availability === 'available' ? resolution.selected.target : null,
      format: resolution.availability === 'available' ? resolution.selected.format : null,
    })
    return Response.json(resolution, { headers: responseHeaders })
  }

  const handleArtifactRedirect = async (request: Request, url: URL): Promise<Response> => {
    const overrides = parseQueryOverrides(url)
    if (overrides === null || overrides.target === undefined || overrides.format === undefined) {
      return Response.json(
        { error: 'invalid_query', required: ['target', 'format'] },
        { status: 400, headers: { 'cache-control': 'no-store' } },
      )
    }
    const channel = overrides.channel ?? config.defaultChannel
    const loaded = await loadSnapshot(channel)
    if (!loaded.ok) {
      return Response.json(
        { error: 'artifact_unavailable', reason: loaded.reason },
        { status: 503, headers: { 'cache-control': 'no-store' } },
      )
    }
    const artifact = catalog(loaded.snapshot).find(
      candidate => candidate.target === overrides.target && candidate.format === overrides.format,
    )
    if (artifact === undefined) {
      return Response.json(
        { error: 'artifact_unavailable', reason: 'target_or_format_not_in_release_set' },
        { status: 404, headers: { 'cache-control': 'no-store' } },
      )
    }
    emit(request, channel, 'artifact_redirect', {
      version: artifact.version,
      target: artifact.target,
      format: artifact.format,
    })
    return new Response(null, {
      status: 302,
      headers: { location: artifact.url, 'cache-control': 'no-store' },
    })
  }

  const handle = async (request: Request): Promise<Response | undefined> => {
    const url = new URL(request.url)
    if (
      url.pathname !== DESKTOP_DOWNLOAD_RESOLUTION_PATH &&
      url.pathname !== DESKTOP_DOWNLOAD_ARTIFACT_PATH
    ) {
      return undefined
    }
    if (request.method !== 'GET') {
      return Response.json(
        { error: 'method_not_allowed' },
        { status: 405, headers: { allow: 'GET' } },
      )
    }
    return url.pathname === DESKTOP_DOWNLOAD_RESOLUTION_PATH
      ? handleResolution(request, url)
      : handleArtifactRedirect(request, url)
  }

  return { handle, resolve }
}

// ---------------------------------------------------------------------------
// Default server wiring (used by `server.ts`)
// ---------------------------------------------------------------------------

let defaultResolver: DesktopDownloadResolver | undefined

export const routeDesktopDownloadRequest = (
  request: Request,
): Promise<Response | undefined> => {
  defaultResolver ??= createDesktopDownloadResolver()
  return defaultResolver.handle(request)
}

/**
 * DIST-11 (#8924): direct server-side resolution for the SSR `/download`
 * page loader. Shares the default resolver instance (and its verified
 * snapshot cache) with the public API routes. Deliberately emits NO
 * telemetry — rendering the page is not a download selection; the validated
 * `artifact_redirect` event fires only when the user follows a rendered CTA
 * through `DESKTOP_DOWNLOAD_ARTIFACT_PATH`.
 */
export const resolveDesktopDownloadForRequest = (
  headers: Headers,
  overrides: DesktopDownloadOverrides = {},
): Promise<DesktopDownloadResolution> => {
  defaultResolver ??= createDesktopDownloadResolver()
  return defaultResolver.resolve(
    overrides.channel ?? envConfig().defaultChannel,
    headers,
    overrides,
  )
}
