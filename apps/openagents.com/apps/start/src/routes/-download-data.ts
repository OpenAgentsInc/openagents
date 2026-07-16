// Client-safe access to the DIST-10 (#8923) Desktop download resolver.
//
// The resolver contract authority lives in
// `../desktop-download-resolver.server.ts` (Effect Schema, pinned Ed25519
// verification); this file only re-exports its TYPES (erased at build — the
// server module never enters the client bundle) plus a fail-soft fetch
// helper for `/download`, homepage CTAs, and future consumers (#8924).
//
// Every URL in a resolution is derived from the currently promoted signed
// release set. When the resolver reports `unavailable` there is NO URL to
// render — show an honest unavailable state, never a fallback link.

export type {
  DesktopDownloadArtifact,
  DesktopDownloadDetection,
  DesktopDownloadResolution,
  DesktopDownloadTelemetryEvent,
} from '../desktop-download-resolver.server'

import type { DesktopDownloadResolution } from '../desktop-download-resolver.server'

export const DESKTOP_DOWNLOAD_RESOLUTION_URL = '/api/public/desktop-download'
export const DESKTOP_DOWNLOAD_ARTIFACT_URL = '/api/public/desktop-download/artifact'

export const DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA =
  'openagents.desktop.download_resolution.v1'

/**
 * Server-resolved artifact redirect href for a CTA pinned to an explicit
 * target/format. The redirect 302s to the currently promoted verified
 * artifact URL and returns typed JSON unavailability instead of a dead link.
 */
export const desktopDownloadArtifactHref = (
  target:
    | 'darwin-arm64'
    | 'darwin-x64'
    | 'win32-arm64'
    | 'win32-x64'
    | 'linux-arm64'
    | 'linux-x64',
  format: 'dmg' | 'zip' | 'nsis' | 'appimage' | 'deb' | 'rpm',
): string => `${DESKTOP_DOWNLOAD_ARTIFACT_URL}?target=${target}&format=${format}`

export type Loadable<T> =
  | { readonly state: 'loading' }
  | { readonly state: 'ok'; readonly data: T }
  | { readonly state: 'unavailable'; readonly detail: string }

// Fail-soft fetch (same posture as -stats-data.ts / -qa-board-data.ts): any
// network/HTTP/shape error renders an honest unavailable state, never a
// fabricated download.
export const fetchDesktopDownloadResolution = async (
  fetchFn: typeof fetch = fetch,
): Promise<Loadable<DesktopDownloadResolution>> => {
  try {
    const response = await fetchFn(DESKTOP_DOWNLOAD_RESOLUTION_URL, {
      headers: { accept: 'application/json' },
    })
    if (!response.ok) {
      return {
        state: 'unavailable',
        detail: `Download resolver returned HTTP ${response.status}.`,
      }
    }
    const value = (await response.json()) as Partial<DesktopDownloadResolution>
    if (value.schema !== DESKTOP_DOWNLOAD_RESOLUTION_SCHEMA) {
      return {
        state: 'unavailable',
        detail: 'Download resolver returned an unsupported projection.',
      }
    }
    return { state: 'ok', data: value as DesktopDownloadResolution }
  } catch (error) {
    return {
      state: 'unavailable',
      detail:
        error instanceof Error ? error.message : 'Download resolver is unreachable.',
    }
  }
}
