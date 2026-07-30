// Client-safe access to the Omega download resolver (#9280).
//
// The resolver contract authority lives in
// `../omega-download-resolver.server.ts` (Effect Schema, pinned Ed25519
// verification of the embedded signed manifest); this file only re-exports
// its TYPES (erased at build — the server module never enters the client
// bundle) plus fail-soft loading for the `/download` Omega product section.
//
// Every URL in a resolution is derived from the signed Omega download
// manifest. When the resolver reports `unavailable` there is NO URL to
// render — show an honest unavailable state, never a fallback link.

import { createIsomorphicFn } from '@tanstack/react-start'
import { Exit, Schema } from 'effect'

export type {
  OmegaDownloadArtifact,
  OmegaDownloadResolution,
  OmegaUnavailableTarget,
} from '../omega-download-resolver.server'

import type { OmegaDownloadResolution } from '../omega-download-resolver.server'

import type { Loadable } from './-download-data'

export const OMEGA_DOWNLOAD_RESOLUTION_URL = '/api/public/omega-download'
export const OMEGA_DOWNLOAD_ARTIFACT_URL = '/api/public/omega-download/artifact'

export const OMEGA_DOWNLOAD_RESOLUTION_SCHEMA = 'openagents.omega.download_resolution.v1'

// Bounded literal vocab, mirrored client-safe from the Omega manifest
// contract (the contract module is server-only; drift between the two fails
// the lockstep test in `-omega-download.test.tsx`).
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

// ---------------------------------------------------------------------------
// Client-safe strict decoder — mirrors the resolver's
// `OmegaDownloadResolutionSchema` using ONLY the `effect` package and the
// local literal vocab above, never an import of the `.server` module whose
// transitive deps (pinned-key verification, node:crypto) must never enter
// the client bundle.
// ---------------------------------------------------------------------------

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

const ArtifactSchema = Schema.Struct({
  target: Schema.Literals(omegaDownloadTargets),
  format: Schema.Literals(omegaDownloadFormats),
  name: BoundedText,
  url: HttpsUrl,
  sha256: Sha256Hex,
  byteLength: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  minimumOs: BoundedText,
  apple: Schema.optionalKey(
    Schema.Struct({
      teamId: Schema.String.check(Schema.isPattern(/^[A-Z0-9]{10}$/)),
      signingIdentity: BoundedText,
      notarization: Schema.Literals(['notarized', 'not_notarized']),
    }),
  ),
})

const AvailableSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DOWNLOAD_RESOLUTION_SCHEMA),
  product: Schema.Literal('omega'),
  productName: Schema.Literal('Omega'),
  availability: Schema.Literal('available'),
  channel: Schema.Literal('alpha'),
  version: BoundedText,
  releaseTag: BoundedText,
  releasedAt: IsoInstant,
  releaseNotes: Schema.NullOr(BoundedText),
  releasePageUrl: HttpsUrl,
  sourceRepository: HttpsUrl,
  sourceRevision: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/)),
  artifacts: Schema.Array(ArtifactSchema).check(Schema.isMinLength(1), Schema.isMaxLength(24)),
  unavailableTargets: Schema.Array(
    Schema.Struct({
      target: Schema.Literals(omegaDownloadTargets),
      statement: BoundedText,
    }),
  ).check(Schema.isMaxLength(8)),
  knownLimitations: Schema.Array(BoundedText).check(Schema.isMaxLength(16)),
})

const UnavailableSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_DOWNLOAD_RESOLUTION_SCHEMA),
  product: Schema.Literal('omega'),
  availability: Schema.Literal('unavailable'),
  reason: BoundedText,
})

const ClientResolutionSchema = Schema.Union([AvailableSchema, UnavailableSchema])
const decodeResolutionExit = Schema.decodeUnknownExit(ClientResolutionSchema)

/**
 * Strictly decode an unknown JSON value as an `OmegaDownloadResolution`.
 * Any shape violation fails, never silently coerces.
 */
export const decodeOmegaDownloadResolution = (
  value: unknown,
): OmegaDownloadResolution | null => {
  const decoded = decodeResolutionExit(value)
  return Exit.isSuccess(decoded) ? (decoded.value as OmegaDownloadResolution) : null
}

/**
 * Server-resolved artifact redirect href for an Omega CTA pinned to an
 * explicit target/format. The redirect 302s to the URL named by the signed
 * manifest and returns typed JSON unavailability instead of a dead link.
 */
export const omegaDownloadArtifactHref = (
  target: OmegaDownloadTarget,
  format: OmegaDownloadFormat,
): string => `${OMEGA_DOWNLOAD_ARTIFACT_URL}?target=${target}&format=${format}`

// Fail-soft fetch (same posture as -download-data.ts): any network/HTTP/shape
// error renders an honest unavailable state, never a fabricated download.
export const fetchOmegaDownloadResolution = async (
  fetchFn: typeof fetch = fetch,
): Promise<Loadable<OmegaDownloadResolution>> => {
  try {
    const response = await fetchFn(OMEGA_DOWNLOAD_RESOLUTION_URL, {
      headers: { accept: 'application/json' },
    })
    if (!response.ok) {
      return {
        state: 'unavailable',
        detail: `Omega download resolver returned HTTP ${response.status}.`,
      }
    }
    const value: unknown = await response.json()
    const decoded = decodeOmegaDownloadResolution(value)
    if (decoded === null) {
      return {
        state: 'unavailable',
        detail: 'Omega download resolver returned a malformed or unsupported projection.',
      }
    }
    return { state: 'ok', data: decoded }
  } catch (error) {
    return {
      state: 'unavailable',
      detail:
        error instanceof Error ? error.message : 'Omega download resolver is unreachable.',
    }
  }
}

/**
 * Isomorphic Omega resolution loader for the `/download` route.
 *
 * - During SSR the embedded signed manifest is verified in-process (no
 *   network dependency; the no-JS page is the fully resolved page).
 * - On client navigations it fetches the public resolver endpoint.
 *
 * Both paths fail soft: a verification failure renders the honest
 * unavailable state with zero download URLs, never a fake-available page.
 */
export const loadOmegaDownloadResolution = createIsomorphicFn()
  .client(
    (): Promise<Loadable<OmegaDownloadResolution>> => fetchOmegaDownloadResolution(),
  )
  .server(async (): Promise<Loadable<OmegaDownloadResolution>> => {
    try {
      const server = await import('../omega-download-resolver.server')
      return { state: 'ok', data: server.resolveOmegaDownload() }
    } catch (error) {
      return {
        state: 'unavailable',
        detail:
          error instanceof Error
            ? error.message
            : 'Omega download resolver failed during server rendering.',
      }
    }
  })
