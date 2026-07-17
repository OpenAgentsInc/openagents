// Client-safe access to the DIST-10 (#8923) Desktop download resolver.
//
// The resolver contract authority lives in
// `../desktop-download-resolver.server.ts` (Effect Schema, pinned Ed25519
// verification); this file only re-exports its TYPES (erased at build — the
// server module never enters the client bundle) plus fail-soft loading for
// `/download`, homepage CTAs, and other consumers (DIST-11 #8924).
//
// Every URL in a resolution is derived from the currently promoted signed
// release set. When the resolver reports `unavailable` there is NO URL to
// render — show an honest unavailable state, never a fallback link.

import { Exit, Schema } from 'effect'

export type {
  DesktopDownloadArtifact,
  DesktopDownloadDetection,
  DesktopDownloadOverrides,
  DesktopDownloadResolution,
  DesktopDownloadTelemetryEvent,
} from '../desktop-download-resolver.server'

import type {
  DesktopDownloadOverrides,
  DesktopDownloadResolution,
} from '../desktop-download-resolver.server'

export const DESKTOP_DOWNLOAD_RESOLUTION_URL = '/api/public/desktop-download'
export const DESKTOP_DOWNLOAD_ARTIFACT_URL = '/api/public/desktop-download/artifact'

export const DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA =
  'openagents.desktop.download_resolution.v1'

// Bounded literal vocab, mirrored client-safe from the release-set contract
// (the contract module is server-only; these lists are asserted against the
// typed contract below so drift fails typecheck, not at runtime).
export const desktopDownloadTargets = [
  'darwin-arm64',
  'darwin-x64',
  'win32-x64',
  'linux-arm64',
  'linux-x64',
] as const
export type DesktopDownloadTarget = (typeof desktopDownloadTargets)[number]

export const desktopDownloadFormats = [
  'dmg',
  'zip',
  'nsis',
  'appimage',
  'deb',
  'rpm',
] as const
export type DesktopDownloadFormat = (typeof desktopDownloadFormats)[number]

export const desktopDownloadChannels = ['stable', 'rc'] as const
export type DesktopDownloadChannel = (typeof desktopDownloadChannels)[number]

// Compile-time drift guards: the local literal vocab must stay assignable to
// the resolver's schema-derived override contract in both directions.
type _TargetsCover = DesktopDownloadTarget extends NonNullable<DesktopDownloadOverrides['target']>
  ? NonNullable<DesktopDownloadOverrides['target']> extends DesktopDownloadTarget
    ? true
    : never
  : never
type _FormatsCover = DesktopDownloadFormat extends NonNullable<DesktopDownloadOverrides['format']>
  ? NonNullable<DesktopDownloadOverrides['format']> extends DesktopDownloadFormat
    ? true
    : never
  : never
const _targetsCover: _TargetsCover = true
const _formatsCover: _FormatsCover = true
void _targetsCover
void _formatsCover

// ---------------------------------------------------------------------------
// Client-safe strict decoder (closes the DIST-11 #8924 independent-review
// finding: client navigations previously validated only the `schema` string,
// then cast the rest of the response — a same-schema but malformed payload
// would have misrendered instead of degrading honestly). This mirrors the
// resolver's `DesktopDownloadResolutionSchema` shape using ONLY the `effect`
// package and the local literal vocab above — never an import of
// `../desktop-download-resolver.server`, whose transitive deps (pinned-key
// verification, feed fetch/cache) must never enter the client bundle. The
// `format vocabulary stays in lockstep` test in `-download.test.tsx` and the
// resolver's own self-check (`decodeResolutionExit` in the `.server.ts` file)
// keep the two schemas from silently drifting apart.
const ChannelSchema = Schema.Literals(desktopDownloadChannels)
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

const DetectionSchema = Schema.Struct({
  platform: Schema.NullOr(Schema.Literals(['darwin', 'win32', 'linux'])),
  architecture: Schema.NullOr(Schema.Literals(['arm64', 'x64'])),
  method: Schema.Literals(['override', 'client_hints', 'user_agent', 'none']),
})

const ArtifactSchema = Schema.Struct({
  target: Schema.Literals(desktopDownloadTargets),
  format: Schema.Literals(desktopDownloadFormats),
  version: BoundedText,
  channel: ChannelSchema,
  url: HttpsUrl,
  sha256: Sha256Hex,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  minimumOs: BoundedText,
  preferred: Schema.Boolean,
})

const sharedFields = {
  schema: Schema.Literal(DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA),
  source: Schema.Literal('release_set_v2'),
  channel: ChannelSchema,
  version: BoundedText,
  releasedAt: IsoInstant,
  releaseNotes: Schema.NullOr(BoundedText),
  sourceRevision: Schema.NullOr(Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/))),
  detection: DetectionSchema,
} as const

const AvailableSchema = Schema.Struct({
  ...sharedFields,
  availability: Schema.Literal('available'),
  selected: ArtifactSchema,
  alternatives: Schema.Array(ArtifactSchema).check(Schema.isMaxLength(24)),
})
const ChooseManuallySchema = Schema.Struct({
  ...sharedFields,
  availability: Schema.Literal('choose_manually'),
  reason: Schema.Literals(['unknown_client', 'target_unavailable', 'format_unavailable']),
  options: Schema.Array(ArtifactSchema).check(Schema.isMaxLength(24)),
})
const UnavailableSchema = Schema.Struct({
  schema: Schema.Literal(DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA),
  availability: Schema.Literal('unavailable'),
  channel: ChannelSchema,
  reason: Schema.Literals([
    'feed_unreachable',
    'feed_schema_invalid',
    'release_pointer_invalid',
    'release_pointer_replayed',
    'release_candidate_mismatch',
    'release_set_verification_failed',
  ]),
  detection: DetectionSchema,
})

const ClientResolutionSchema = Schema.Union([AvailableSchema, ChooseManuallySchema, UnavailableSchema])
const decodeResolutionExit = Schema.decodeUnknownExit(ClientResolutionSchema)

/**
 * Strictly decode an unknown JSON value as a `DesktopDownloadResolution`.
 * Exported for direct testing; `fetchDesktopDownloadResolution` is the normal
 * consumer. Any shape violation — wrong enum value, missing field, malformed
 * hash/URL/version string, an over-length alternatives array — fails, never
 * silently coerces.
 */
export const decodeDesktopDownloadResolution = (
  value: unknown,
): DesktopDownloadResolution | null => {
  const decoded = decodeResolutionExit(value)
  return Exit.isSuccess(decoded) ? (decoded.value as DesktopDownloadResolution) : null
}

/** Validated `/download` search params — explicit target/format/channel overrides. */
export type DownloadSearch = Readonly<{
  target?: DesktopDownloadTarget
  format?: DesktopDownloadFormat
  channel?: DesktopDownloadChannel
}>

const pick = <T extends string>(
  value: unknown,
  allowed: readonly T[],
): T | undefined =>
  typeof value === 'string' && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : undefined

/**
 * Bounded, deterministic parse of the `/download` search params. Unknown keys
 * and invalid values are dropped (never an error page): a mangled shared link
 * still renders the honest detected/chooser state.
 */
export const parseDownloadSearch = (
  search: Record<string, unknown>,
): DownloadSearch => {
  const target = pick(search['target'], desktopDownloadTargets)
  const format = pick(search['format'], desktopDownloadFormats)
  const channel = pick(search['channel'], desktopDownloadChannels)
  return {
    ...(target === undefined ? {} : { target }),
    ...(format === undefined ? {} : { format }),
    ...(channel === undefined ? {} : { channel }),
  }
}

const searchQuery = (overrides: DownloadSearch): string => {
  const params = new URLSearchParams()
  if (overrides.target !== undefined) params.set('target', overrides.target)
  if (overrides.format !== undefined) params.set('format', overrides.format)
  if (overrides.channel !== undefined) params.set('channel', overrides.channel)
  const query = params.toString()
  return query === '' ? '' : `?${query}`
}

/**
 * Server-resolved artifact redirect href for a CTA pinned to an explicit
 * target/format. The redirect 302s to the currently promoted verified
 * artifact URL — emitting the validated `artifact_redirect` telemetry event
 * exactly when the user selects a real release-set artifact — and returns
 * typed JSON unavailability instead of a dead link.
 */
export const desktopDownloadArtifactHref = (
  target: DesktopDownloadTarget,
  format: DesktopDownloadFormat,
  channel?: DesktopDownloadChannel,
): string =>
  `${DESKTOP_DOWNLOAD_ARTIFACT_URL}?target=${target}&format=${format}${
    channel === undefined ? '' : `&channel=${channel}`
  }`

export type Loadable<T> =
  | { readonly state: 'loading' }
  | { readonly state: 'ok'; readonly data: T }
  | { readonly state: 'unavailable'; readonly detail: string }

// Fail-soft fetch (same posture as -stats-data.ts / -qa-board-data.ts): any
// network/HTTP/shape error renders an honest unavailable state, never a
// fabricated download.
export const fetchDesktopDownloadResolution = async (
  overrides: DownloadSearch = {},
  fetchFn: typeof fetch = fetch,
): Promise<Loadable<DesktopDownloadResolution>> => {
  try {
    const response = await fetchFn(
      `${DESKTOP_DOWNLOAD_RESOLUTION_URL}${searchQuery(overrides)}`,
      { headers: { accept: 'application/json' } },
    )
    if (!response.ok) {
      return {
        state: 'unavailable',
        detail: `Download resolver returned HTTP ${response.status}.`,
      }
    }
    const value: unknown = await response.json()
    const decoded = decodeDesktopDownloadResolution(value)
    if (decoded === null) {
      return {
        state: 'unavailable',
        detail: 'Download resolver returned a malformed or unsupported projection.',
      }
    }
    return { state: 'ok', data: decoded }
  } catch (error) {
    return {
      state: 'unavailable',
      detail:
        error instanceof Error ? error.message : 'Download resolver is unreachable.',
    }
  }
}

/**
 * Isomorphic `/download` route loader body.
 *
 * - During SSR the resolver runs in-process against the incoming request's
 *   headers (client-hint/user-agent platform detection works without any
 *   client JavaScript — the no-JS page is the fully resolved page).
 * - On client navigations it fetches the public resolver endpoint, which
 *   performs the same detection from the browser's own request headers.
 *
 * Both paths fail soft: a resolver/feed failure renders the honest
 * unavailable state with zero download URLs, never a fake-available page.
 */
export const loadDesktopDownloadResolution = async (
  overrides: DownloadSearch = {},
): Promise<Loadable<DesktopDownloadResolution>> => {
  if (import.meta.env.SSR) {
    try {
      const [{ getStartRequestContext }, server] = await Promise.all([
        import('@openagentsinc/effect-start'),
        import('../desktop-download-resolver.server'),
      ])
      const headers =
        getStartRequestContext()?.request.headers ?? new Headers()
      return {
        state: 'ok',
        data: await server.resolveDesktopDownloadForRequest(headers, overrides),
      }
    } catch (error) {
      return {
        state: 'unavailable',
        detail:
          error instanceof Error
            ? error.message
            : 'Download resolver failed during server rendering.',
      }
    }
  }
  return fetchDesktopDownloadResolution(overrides)
}
