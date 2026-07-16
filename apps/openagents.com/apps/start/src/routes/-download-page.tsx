/**
 * DIST-11 (#8924): platform-aware /download page.
 *
 * Every download link on this page is derived from the DIST-10 resolver
 * projection (`openagents.desktop.download_resolution.v1`), which itself is
 * derived from the currently promoted, signature-verified release set. There
 * is no handwritten artifact URL and no hard-coded version label: a platform
 * renders as available exactly when the promoted release set carries its
 * target, and the honest chooser/unavailable states render otherwise.
 *
 * All download CTAs point at the server-side artifact redirect
 * (`/api/public/desktop-download/artifact?...`), which 302s to the verified
 * artifact URL and emits the validated `artifact_redirect` telemetry event —
 * so telemetry fires only when a user actually selects a release-set
 * artifact, never on page render.
 *
 * The page is fully server-renderable: with JavaScript disabled the anchors
 * still download through the redirect, and a resolver/feed failure renders an
 * honest degraded state with zero download URLs.
 */
import { InternalLink } from '@/components/internal-link'
import { DOCS_URL, GITHUB_REPOSITORY_URL } from '@/lib/public-site'

import {
  desktopDownloadArtifactHref,
  type DesktopDownloadArtifact,
  type DesktopDownloadFormat,
  type DesktopDownloadResolution,
  type DesktopDownloadTarget,
  type Loadable,
} from './-download-data'
import { PublicSiteShell } from './-public-site'

// ---------------------------------------------------------------------------
// Pure projection helpers (exported for tests)
// ---------------------------------------------------------------------------

/** Every artifact the current resolution admits as downloadable. */
export const resolutionCatalog = (
  resolution: DesktopDownloadResolution,
): readonly DesktopDownloadArtifact[] => {
  switch (resolution.availability) {
    case 'available':
      return [resolution.selected, ...resolution.alternatives]
    case 'choose_manually':
      return resolution.options
    case 'unavailable':
      return []
  }
}

export const formatByteSize = (byteLength: number): string =>
  byteLength >= 1_000_000_000
    ? `${(byteLength / 1_000_000_000).toFixed(1)} GB`
    : `${Math.max(1, Math.round(byteLength / 1_000_000))} MB`

export const channelLabel = (channel: 'stable' | 'rc'): string =>
  channel === 'stable' ? 'Stable' : 'Release candidate'

export const formatLabel = (format: DesktopDownloadFormat): string => {
  switch (format) {
    case 'dmg':
      return 'DMG'
    case 'zip':
      return 'ZIP'
    case 'nsis':
      return 'Installer (.exe)'
    case 'appimage':
      return 'AppImage'
    case 'deb':
      return 'DEB'
    case 'rpm':
      return 'RPM'
  }
}

const platformOf = (target: DesktopDownloadTarget): 'darwin' | 'win32' | 'linux' =>
  target.startsWith('darwin') ? 'darwin' : target.startsWith('win32') ? 'win32' : 'linux'

export const platformName = (platform: 'darwin' | 'win32' | 'linux'): string =>
  platform === 'darwin' ? 'macOS' : platform === 'win32' ? 'Windows' : 'Linux'

export const architectureLabel = (target: DesktopDownloadTarget): string => {
  switch (target) {
    case 'darwin-arm64':
      return 'Apple Silicon'
    case 'darwin-x64':
      return 'Intel'
    case 'win32-arm64':
    case 'linux-arm64':
      return 'ARM64'
    case 'win32-x64':
    case 'linux-x64':
      return 'x64'
  }
}

export const targetLabel = (target: DesktopDownloadTarget): string =>
  `${platformName(platformOf(target))} · ${architectureLabel(target)}`

/** Human sentence fragment for the detected/selected machine. */
export const targetSentence = (target: DesktopDownloadTarget): string => {
  switch (target) {
    case 'darwin-arm64':
      return 'Apple silicon Macs'
    case 'darwin-x64':
      return 'Intel Macs'
    default:
      return `${platformName(platformOf(target))} on ${architectureLabel(target)}`
  }
}

export const minimumOsLabel = (
  target: DesktopDownloadTarget,
  minimumOs: string,
): string => {
  const platform = platformOf(target)
  if (platform === 'darwin') return `macOS ${minimumOs} or later`
  if (platform === 'win32') return `Windows ${minimumOs} or later`
  return `Linux with ${minimumOs} or later`
}

// ---------------------------------------------------------------------------
// Platform matrix structure — the six release-set targets, always rendered,
// with availability admitted exclusively by the resolver catalog.
// ---------------------------------------------------------------------------

type PlatformSection = Readonly<{
  platform: 'darwin' | 'win32' | 'linux'
  mark: string
  targets: readonly DesktopDownloadTarget[]
}>

export const PLATFORM_SECTIONS: readonly PlatformSection[] = [
  { platform: 'darwin', mark: 'M', targets: ['darwin-arm64', 'darwin-x64'] },
  { platform: 'win32', mark: 'W', targets: ['win32-x64', 'win32-arm64'] },
  { platform: 'linux', mark: 'L', targets: ['linux-x64', 'linux-arm64'] },
]

const artifactsForTarget = (
  catalog: readonly DesktopDownloadArtifact[],
  target: DesktopDownloadTarget,
): readonly DesktopDownloadArtifact[] => {
  const rows = catalog.filter(artifact => artifact.target === target)
  return [...rows.filter(row => row.preferred), ...rows.filter(row => !row.preferred)]
}

const platformGuidance = (
  platform: 'darwin' | 'win32' | 'linux',
  catalog: readonly DesktopDownloadArtifact[],
): string => {
  const available = catalog.some(artifact => platformOf(artifact.target) === platform)
  if (platform === 'darwin') {
    const intelAvailable = catalog.some(artifact => artifact.target === 'darwin-x64')
    const base =
      'Macs with Apple silicon (M1 or later) use the Apple Silicon build; Intel Macs need the Intel build.'
    const formats =
      ' DMG is the standard installer; ZIP is the same signed app as a plain archive.'
    if (!available) return `${base} Neither build is available yet.`
    return intelAvailable
      ? base + formats
      : `${base}${formats} The Intel build is not yet available.`
  }
  if (platform === 'win32') {
    return available
      ? 'One installer (.exe) per architecture. It installs per-user and does not require administrator rights.'
      : 'Windows builds are not yet available. They will appear here when a release carries them.'
  }
  return available
    ? 'AppImage runs on most distributions and updates through the app itself. DEB and RPM install through your package manager, which then owns updates and rollback.'
    : 'Linux builds are not yet available. They will appear here when a release carries them.'
}

// ---------------------------------------------------------------------------
// Page states
// ---------------------------------------------------------------------------

const UNAVAILABLE_HEADLINE =
  'Downloads are temporarily unavailable while we verify the current release. Please try again shortly.'

const headline = (resolution: DesktopDownloadResolution): string => {
  if (resolution.availability === 'available') {
    return resolution.detection.method === 'override'
      ? `Showing the ${targetLabel(resolution.selected.target)} build (chosen manually).`
      : `Available now for ${targetSentence(resolution.selected.target)}.`
  }
  if (resolution.availability === 'choose_manually') {
    switch (resolution.reason) {
      case 'unknown_client':
        return 'We could not detect your platform automatically. Pick the build that matches your machine.'
      case 'target_unavailable':
        return resolution.detection.platform === null
          ? 'The requested build is not part of the current release. These builds are available today.'
          : `OpenAgents Desktop is not yet available for ${platformName(resolution.detection.platform)}${
              resolution.detection.architecture === null
                ? ''
                : ` on ${resolution.detection.architecture === 'arm64' ? 'ARM64' : 'x64'}`
            }. These builds are available today.`
      case 'format_unavailable':
        return 'The requested format is not part of the current release. Pick an available build below.'
    }
  }
  return UNAVAILABLE_HEADLINE
}

function ReleaseSummaryLine({
  resolution,
}: {
  resolution: Extract<DesktopDownloadResolution, { version: string }>
}) {
  return (
    <p className="oa-download-release-line">
      Version {resolution.version} · {channelLabel(resolution.channel)}
    </p>
  )
}

function PrimaryCta({
  resolution,
}: {
  resolution: Extract<DesktopDownloadResolution, { availability: 'available' }>
}) {
  const selected = resolution.selected
  const sameTargetAlternatives = resolution.alternatives.filter(
    artifact => artifact.target === selected.target,
  )
  return (
    <div className="oa-download-primary">
      <a
        aria-describedby="oa-download-primary-meta"
        className="oa-button oa-button-primary oa-download-primary-cta"
        href={desktopDownloadArtifactHref(selected.target, selected.format, selected.channel)}
        rel="noreferrer"
      >
        Download for {targetLabel(selected.target)}
      </a>
      <p className="oa-download-primary-meta" id="oa-download-primary-meta">
        {selected.version} · {channelLabel(selected.channel)} · {formatLabel(selected.format)} ·{' '}
        {formatByteSize(selected.byteLength)} · {minimumOsLabel(selected.target, selected.minimumOs)}
      </p>
      {sameTargetAlternatives.length > 0 ? (
        <p className="oa-download-primary-alts">
          Also available:{' '}
          {sameTargetAlternatives.map((artifact, index) => (
            <span key={artifact.format}>
              {index > 0 ? ' · ' : null}
              <a
                href={desktopDownloadArtifactHref(artifact.target, artifact.format, artifact.channel)}
                rel="noreferrer"
              >
                {formatLabel(artifact.format)} ({formatByteSize(artifact.byteLength)})
              </a>
            </span>
          ))}
        </p>
      ) : null}
    </div>
  )
}

function PlatformMatrix({
  resolution,
}: {
  resolution: Extract<DesktopDownloadResolution, { version: string }>
}) {
  const catalog = resolutionCatalog(resolution)
  return (
    <div className="oa-download-matrix">
      <h2 className="oa-download-matrix-title" id="oa-download-all-platforms">
        All platforms
      </h2>
      {PLATFORM_SECTIONS.map(section => (
        <section
          aria-labelledby={`oa-platform-${section.platform}`}
          className="oa-platform-section"
          key={section.platform}
        >
          <h3 className="oa-platform-title" id={`oa-platform-${section.platform}`}>
            {platformName(section.platform)}
          </h3>
          <p className="oa-platform-guidance">{platformGuidance(section.platform, catalog)}</p>
          <div className="oa-platform-list">
            {section.targets.map(target => {
              const artifacts = artifactsForTarget(catalog, target)
              if (artifacts.length === 0) {
                return (
                  <div className="oa-platform-row oa-platform-row-pending" key={target}>
                    <div className="oa-platform-name">
                      <span aria-hidden="true" className="oa-platform-mark">
                        {section.mark}
                      </span>
                      <div>
                        <strong>{platformName(section.platform)}</strong>
                        <span>{architectureLabel(target)}</span>
                      </div>
                    </div>
                    <span className="oa-platform-release">Not yet available</span>
                    <span
                      aria-label={`${targetLabel(target)} coming soon`}
                      className="oa-platform-action"
                    >
                      Coming soon
                    </span>
                  </div>
                )
              }
              const first = artifacts[0] as DesktopDownloadArtifact
              return (
                <div className="oa-platform-row" key={target}>
                  <div className="oa-platform-name">
                    <span aria-hidden="true" className="oa-platform-mark">
                      {section.mark}
                    </span>
                    <div>
                      <strong>{platformName(section.platform)}</strong>
                      <span>{architectureLabel(target)}</span>
                    </div>
                  </div>
                  <span className="oa-platform-release">
                    {first.version} · {minimumOsLabel(target, first.minimumOs)}
                  </span>
                  <div
                    aria-label={`${targetLabel(target)} downloads`}
                    className="oa-format-links"
                    role="group"
                  >
                    {artifacts.map(artifact => (
                      <a
                        className="oa-platform-action"
                        href={desktopDownloadArtifactHref(
                          artifact.target,
                          artifact.format,
                          artifact.channel,
                        )}
                        key={artifact.format}
                        rel="noreferrer"
                      >
                        {formatLabel(artifact.format)} · {formatByteSize(artifact.byteLength)}
                      </a>
                    ))}
                  </div>
                </div>
              )
            })}
          </div>
        </section>
      ))}
    </div>
  )
}

function ReleaseNotes({
  resolution,
}: {
  resolution: Extract<DesktopDownloadResolution, { version: string }>
}) {
  // The bounded v1 migration feed carries an internal notes ref, not human
  // release-notes text — never expose raw internal feed details on the page.
  if (resolution.releaseNotes === null || resolution.source !== 'release_set_v2') return null
  return (
    <section aria-labelledby="oa-download-notes-title" className="oa-download-section">
      <h2 id="oa-download-notes-title">What’s new in {resolution.version}</h2>
      <p>{resolution.releaseNotes}</p>
      <p>
        <InternalLink href="/changelog" preload="intent">
          Full changelog <span aria-hidden="true">→</span>
        </InternalLink>
      </p>
    </section>
  )
}

function VerificationAndSupport({
  resolution,
}: {
  resolution: DesktopDownloadResolution | null
}) {
  const selected =
    resolution !== null && resolution.availability === 'available'
      ? resolution.selected
      : null
  return (
    <section aria-labelledby="oa-download-verify-title" className="oa-download-section">
      <h2 id="oa-download-verify-title">Verification and support</h2>
      <p>
        Every download link on this page resolves through the signed release set — the same
        verified feed the installed app updates from. If a platform is not listed as available,
        no build for it has been promoted yet, and this page will not pretend otherwise.
      </p>
      {selected === null ? null : (
        <details className="oa-download-checksum">
          <summary>Verify your download (SHA-256)</summary>
          <p>
            {formatLabel(selected.format)} checksum: <code>{selected.sha256}</code>
          </p>
        </details>
      )}
      <div className="oa-download-support-links">
        <InternalLink href={DOCS_URL} preload="intent">
          Installation help <span aria-hidden="true">→</span>
        </InternalLink>
        <a href={`${GITHUB_REPOSITORY_URL}/releases`} rel="noreferrer" target="_blank">
          All releases on GitHub <span aria-hidden="true">↗</span>
        </a>
      </div>
    </section>
  )
}

function UnavailableBody() {
  return (
    <div className="oa-download-unavailable" role="status">
      <p>
        No download links are shown right now because the current release could not be fetched
        and verified. Nothing is wrong with your machine — please try again in a few minutes.
      </p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Route head projection (exported for the /download route + tests)
// ---------------------------------------------------------------------------

export const downloadPageDescription = (
  resolution: Loadable<DesktopDownloadResolution> | undefined,
): string => {
  const base = 'Download OpenAgents Desktop.'
  if (resolution === undefined || resolution.state !== 'ok') {
    return `${base} Platform availability is resolved from the current signed release.`
  }
  const data = resolution.data
  if (data.availability === 'unavailable') {
    return `${base} Downloads are temporarily unavailable while the current release is verified.`
  }
  const catalog = resolutionCatalog(data)
  if (catalog.length === 0) {
    return `${base} Downloads are temporarily unavailable while the current release is verified.`
  }
  const targets = [...new Set(catalog.map(artifact => artifact.target))]
  const platforms = targets.map(targetSentence).join(', ')
  return `${base} Version ${data.version} is available for ${platforms}.`
}

export const downloadPageStructuredData = (
  resolution: Loadable<DesktopDownloadResolution> | undefined,
): string | null => {
  if (resolution === undefined || resolution.state !== 'ok') return null
  const data = resolution.data
  if (data.availability === 'unavailable') return null
  const catalog = resolutionCatalog(data)
  if (catalog.length === 0) return null
  const operatingSystems = [
    ...new Set(
      catalog.map(artifact =>
        minimumOsLabel(artifact.target, artifact.minimumOs).replace(' or later', '+'),
      ),
    ),
  ]
  return JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    applicationCategory: 'DeveloperApplication',
    name: 'OpenAgents Desktop',
    operatingSystem: operatingSystems.join(', '),
    softwareVersion: data.version,
    url: 'https://openagents.com/download',
  })
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export function DownloadPage({
  resolution,
}: {
  resolution: Loadable<DesktopDownloadResolution>
}) {
  const resolved = resolution.state === 'ok' ? resolution.data : null
  return (
    <PublicSiteShell>
      <section aria-labelledby="oa-download-title" className="oa-download-page">
        <div className="oa-container oa-download-shell">
          <header className="oa-download-heading">
            <h1 id="oa-download-title">Download OpenAgents Desktop</h1>
            <p>
              {resolution.state === 'loading'
                ? 'Checking the current release…'
                : resolution.state === 'unavailable'
                  ? 'Downloads are temporarily unavailable while we verify the current release. Please try again shortly.'
                  : headline(resolution.data)}
            </p>
            {resolved !== null && resolved.availability !== 'unavailable' ? (
              <ReleaseSummaryLine resolution={resolved} />
            ) : null}
          </header>

          {resolved !== null && resolved.availability === 'available' ? (
            <PrimaryCta resolution={resolved} />
          ) : null}

          {resolved !== null && resolved.availability !== 'unavailable' ? (
            <PlatformMatrix resolution={resolved} />
          ) : null}

          {resolution.state === 'unavailable' ||
          (resolved !== null && resolved.availability === 'unavailable') ? (
            <UnavailableBody />
          ) : null}

          {resolved !== null && resolved.availability !== 'unavailable' ? (
            <ReleaseNotes resolution={resolved} />
          ) : null}

          <VerificationAndSupport resolution={resolved} />
        </div>
      </section>
    </PublicSiteShell>
  )
}
