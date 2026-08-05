/**
 * `/download` — the public download page.
 *
 * The Electron OpenAgents Desktop application and its signed release feed are
 * retired (the `apps/openagents-desktop` package is deleted), so this page no
 * longer advertises it: the entire downloadable surface is Omega, rendered by
 * `OmegaDownloadSection` from the `openagents.omega.download_resolution.v1`
 * projection the server derives from the Ed25519-signed Omega download
 * manifest.
 *
 * There is no handwritten artifact URL and no hard-coded version label on this
 * page. Every CTA crosses the `/api/public/omega-download/artifact` redirect,
 * and a verification failure renders an honest unavailable state with zero
 * download URLs. The page is fully server-renderable: with JavaScript disabled
 * the anchors still download through the redirect.
 */
import type { Loadable } from './-loadable'
import type { OmegaDownloadResolution } from './-omega-download-data'
import { OmegaDownloadSection } from './-omega-download-section'
import { PublicSiteShell } from './-public-site'

// ---------------------------------------------------------------------------
// Route head projection (exported for the /download route + tests)
// ---------------------------------------------------------------------------

export const downloadPageDescription = (
  resolution: Loadable<OmegaDownloadResolution> | undefined,
): string => {
  const base = 'Download Omega, the OpenAgents IDE.'
  if (resolution === undefined || resolution.state !== 'ok') {
    return `${base} Availability is resolved from the signed Omega release manifest.`
  }
  const data = resolution.data
  if (data.availability === 'unavailable') {
    return `${base} Downloads are temporarily unavailable while the signed release manifest is verified.`
  }
  return `${base} Alpha prerelease ${data.version} is available now.`
}

export const downloadPageStructuredData = (
  resolution: Loadable<OmegaDownloadResolution> | undefined,
): string | null => {
  if (resolution === undefined || resolution.state !== 'ok') return null
  const data = resolution.data
  if (data.availability === 'unavailable') return null
  const operatingSystems = [
    ...new Set(
      data.artifacts.map(artifact =>
        artifact.target.startsWith('darwin')
          ? `macOS ${artifact.minimumOs}+`
          : artifact.target.startsWith('win32')
            ? `Windows ${artifact.minimumOs}+`
            : `Linux ${artifact.minimumOs}+`,
      ),
    ),
  ]
  if (operatingSystems.length === 0) return null
  return JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    applicationCategory: 'DeveloperApplication',
    name: 'Omega',
    operatingSystem: operatingSystems.join(', '),
    softwareVersion: data.version,
    url: 'https://openagents.com/download',
  })
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export function DownloadPage({
  omega,
}: {
  /**
   * Omega download entry, resolved from the signed Omega download manifest.
   * `undefined` renders the honest Omega unavailable state, never a fabricated
   * link.
   */
  omega?: Loadable<OmegaDownloadResolution> | undefined
}) {
  return (
    <PublicSiteShell>
      <section aria-labelledby="oa-download-title" className="oa-download-page">
        <div className="oa-container oa-download-shell">
          <header className="oa-download-heading">
            <h1 id="oa-download-title">Download Omega</h1>
            <p>
              Omega is the OpenAgents desktop application. Every link below comes from the
              signed Omega release manifest.
            </p>
          </header>

          <OmegaDownloadSection resolution={omega} />
        </div>
      </section>
    </PublicSiteShell>
  )
}
