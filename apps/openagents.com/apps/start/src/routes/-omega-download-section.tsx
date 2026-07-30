/**
 * Omega product entry on `/download` (#9280, EP263-07).
 *
 * Omega is a SEPARATE product beside OpenAgents Desktop: this section renders
 * exclusively from the `openagents.omega.download_resolution.v1` projection,
 * which the server derives from the Ed25519-signed Omega download manifest.
 * There is no handwritten artifact URL and no hard-coded version label. Every
 * CTA crosses the `/api/public/omega-download/artifact` redirect, and a
 * verification failure renders an honest unavailable state with zero URLs.
 *
 * The Electron OpenAgents Desktop entry above is never relabeled as Omega,
 * and this section never borrows its release data.
 */
import type { Loadable } from './-download-data'
import {
  omegaDownloadArtifactHref,
  type OmegaDownloadArtifact,
  type OmegaDownloadResolution,
  type OmegaDownloadTarget,
} from './-omega-download-data'

// ---------------------------------------------------------------------------
// Pure projection helpers (exported for tests)
// ---------------------------------------------------------------------------

export const omegaTargetLabel = (target: OmegaDownloadTarget): string => {
  switch (target) {
    case 'darwin-arm64':
      return 'macOS · Apple Silicon'
    case 'darwin-x64':
      return 'macOS · Intel'
    case 'win32-arm64':
      return 'Windows · ARM64'
    case 'win32-x64':
      return 'Windows · x64'
    case 'linux-arm64':
      return 'Linux · ARM64'
    case 'linux-x64':
      return 'Linux · x64'
  }
}

export const omegaFormatLabel = (format: OmegaDownloadArtifact['format']): string => {
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

export const omegaByteSize = (byteLength: number): string =>
  byteLength >= 1_000_000_000
    ? `${(byteLength / 1_000_000_000).toFixed(1)} GB`
    : `${Math.max(1, Math.round(byteLength / 1_000_000))} MB`

export const omegaMinimumOsLabel = (artifact: OmegaDownloadArtifact): string => {
  if (artifact.target.startsWith('darwin')) return `macOS ${artifact.minimumOs} or later`
  if (artifact.target.startsWith('win32')) return `Windows ${artifact.minimumOs} or later`
  return `Linux with ${artifact.minimumOs} or later`
}

/** Signature/notarization truth line for one artifact cell — never inferred. */
export const omegaSignatureTruth = (artifact: OmegaDownloadArtifact): string => {
  if (artifact.apple === undefined) return 'Signature truth is recorded in the signed manifest.'
  const base = `Developer ID signed (Team ${artifact.apple.teamId})`
  return artifact.apple.notarization === 'notarized'
    ? `${base} and notarized by Apple.`
    : `${base}, not yet notarized — macOS Gatekeeper will warn when you first open it.`
}

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

function OmegaAlphaPositioning() {
  return (
    <>
      <p>
        Omega is the Zed-based OpenAgents IDE, published here as an early alpha for
        experienced developers and coding agent power users. It is a separate product from
        OpenAgents Desktop above: a different application with its own version line, package,
        and update path. Downloading Omega does not change OpenAgents Desktop.
      </p>
      <ul className="oa-omega-alpha-warnings">
        <li>
          <strong>Prerelease warning:</strong> this is an alpha prerelease. It has not passed
          every release-readiness gate and can break, change, or stop working without notice.
        </li>
        <li>
          <strong>Prerequisites:</strong> a machine listed as available below, plus your own
          accounts and credentials for any external coding agents you connect — each agent
          keeps its own login and configuration.
        </li>
        <li>
          <strong>Support boundary:</strong> alpha support happens in the tester channels
          inside Omega, not through OpenAgents Desktop support paths.
        </li>
        <li>
          <strong>Data risk:</strong> alpha builds can lose or corrupt data. Do not point
          Omega at projects you have not backed up.
        </li>
      </ul>
    </>
  )
}

function OmegaArtifactRow({
  artifact,
  version,
}: {
  artifact: OmegaDownloadArtifact
  version: string
}) {
  const [platformName = '', architecture = ''] = omegaTargetLabel(artifact.target).split(' · ')
  return (
    <div className="oa-platform-row" data-omega-target={artifact.target}>
      <div className="oa-platform-name">
        <span aria-hidden="true" className="oa-platform-mark">
          Ω
        </span>
        <div>
          <strong>{platformName}</strong>
          <span>{architecture}</span>
        </div>
      </div>
      <span className="oa-platform-release">
        {version} · Alpha prerelease · {omegaMinimumOsLabel(artifact)}
      </span>
      <div
        aria-label={`Omega ${omegaTargetLabel(artifact.target)} downloads`}
        className="oa-format-links"
        role="group"
      >
        <a
          className="oa-platform-action"
          href={omegaDownloadArtifactHref(artifact.target, artifact.format)}
          rel="noreferrer"
        >
          {omegaFormatLabel(artifact.format)} · {omegaByteSize(artifact.byteLength)}
        </a>
      </div>
    </div>
  )
}

function OmegaArtifactDetail({ artifact }: { artifact: OmegaDownloadArtifact }) {
  return (
    <div className="oa-omega-artifact-detail">
      <p>{omegaSignatureTruth(artifact)}</p>
      <details className="oa-download-checksum">
        <summary>
          Verify {omegaTargetLabel(artifact.target)} {omegaFormatLabel(artifact.format)}{' '}
          (SHA-256)
        </summary>
        <p>
          <code>{artifact.sha256}</code>
        </p>
      </details>
    </div>
  )
}

export function OmegaDownloadSection({
  resolution,
}: {
  resolution: Loadable<OmegaDownloadResolution> | undefined
}) {
  const resolved =
    resolution !== undefined && resolution.state === 'ok' ? resolution.data : null
  const available = resolved !== null && resolved.availability === 'available' ? resolved : null
  return (
    <section
      aria-labelledby="oa-omega-download-title"
      className="oa-download-section oa-omega-download"
      data-product="omega"
    >
      <h2 id="oa-omega-download-title">
        Omega <span className="oa-omega-channel-badge">Alpha prerelease</span>
      </h2>
      <OmegaAlphaPositioning />
      {available === null ? (
        <div className="oa-download-unavailable" role="status">
          <p>
            {resolution === undefined || resolution.state === 'loading'
              ? 'Checking the current Omega alpha release…'
              : 'No Omega download links are shown right now because the signed Omega release manifest could not be verified. Please try again shortly.'}
          </p>
        </div>
      ) : (
        <>
          <p className="oa-download-release-line">
            Omega {available.version} · Alpha prerelease · released{' '}
            {available.releasedAt.slice(0, 10)}
          </p>
          <div className="oa-platform-list">
            {available.artifacts.map(artifact => (
              <OmegaArtifactRow
                artifact={artifact}
                key={`${artifact.target}-${artifact.format}`}
                version={available.version}
              />
            ))}
            {available.unavailableTargets.map(row => (
              <div
                className="oa-platform-row oa-platform-row-pending"
                data-omega-target={row.target}
                key={row.target}
              >
                <div className="oa-platform-name">
                  <span aria-hidden="true" className="oa-platform-mark">
                    Ω
                  </span>
                  <div>
                    <strong>{omegaTargetLabel(row.target).split(' · ')[0]}</strong>
                    <span>{omegaTargetLabel(row.target).split(' · ')[1]}</span>
                  </div>
                </div>
                <span className="oa-platform-release">{row.statement}</span>
                <span
                  aria-label={`Omega for ${omegaTargetLabel(row.target)} is not available`}
                  className="oa-platform-action"
                >
                  Not available
                </span>
              </div>
            ))}
          </div>
          {available.artifacts.map(artifact => (
            <OmegaArtifactDetail
              artifact={artifact}
              key={`${artifact.target}-${artifact.format}-detail`}
            />
          ))}
          {available.knownLimitations.length > 0 ? (
            <div className="oa-omega-limitations">
              <h3>Known limitations</h3>
              <ul>
                {available.knownLimitations.map(limitation => (
                  <li key={limitation}>{limitation}</li>
                ))}
              </ul>
            </div>
          ) : null}
          {available.releaseNotes === null ? null : (
            <p className="oa-omega-release-notes">
              What’s new in {available.version}: {available.releaseNotes}
            </p>
          )}
          <div className="oa-download-support-links">
            <a href={available.releasePageUrl} rel="noreferrer" target="_blank">
              This release on GitHub <span aria-hidden="true">↗</span>
            </a>
            <a
              href={`${available.sourceRepository}/releases`}
              rel="noreferrer"
              target="_blank"
            >
              All Omega prereleases <span aria-hidden="true">↗</span>
            </a>
          </div>
        </>
      )}
    </section>
  )
}
