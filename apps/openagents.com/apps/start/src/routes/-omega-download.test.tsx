/**
 * #9280 — Omega product entry on `/download`.
 *
 * The section renders exclusively from `openagents.omega.download_resolution.v1`
 * projections. Fixtures are hand-built typed resolutions covering: the
 * available alpha entry (per-cell version/channel/format/minimum-OS/digest/
 * notarization truth, explicit unavailable targets, known limitations, alpha
 * positioning), the honest unavailable state, and the strict client decoder.
 */
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import type { Loadable } from './-download-data'
import {
  decodeOmegaDownloadResolution,
  omegaDownloadArtifactHref,
  type OmegaDownloadResolution,
} from './-omega-download-data'
import {
  OmegaDownloadSection,
  omegaByteSize,
  omegaSignatureTruth,
  omegaTargetLabel,
} from './-omega-download-section'

const availableResolution = {
  schema: 'openagents.omega.download_resolution.v1',
  product: 'omega',
  productName: 'Omega',
  availability: 'available',
  channel: 'alpha',
  version: '0.2.0-rc26',
  releaseTag: 'v0.2.0-rc26',
  releasedAt: '2026-07-29T20:57:30Z',
  releaseNotes: 'Fixes Sarah voice session reconnects.',
  releasePageUrl: 'https://github.com/OpenAgentsInc/omega/releases/tag/v0.2.0-rc26',
  sourceRepository: 'https://github.com/OpenAgentsInc/omega',
  sourceRevision: '7142e3404ae59af22a7a975615935e807c3e1288',
  artifacts: [
    {
      target: 'darwin-arm64',
      format: 'dmg',
      name: 'Omega-v0.2.0-rc26-macos-arm64.dmg',
      url: 'https://github.com/OpenAgentsInc/omega/releases/download/v0.2.0-rc26/Omega-v0.2.0-rc26-macos-arm64.dmg',
      sha256: 'e39cd7b822fc143f03e1d54d68a36ec0fe150bc33334d2bb50573342de3fcee3',
      byteLength: 193720537,
      minimumOs: '11.0',
      apple: {
        teamId: 'HQWSG26L43',
        signingIdentity: 'Developer ID Application: OpenAgents, Inc. (HQWSG26L43)',
        notarization: 'not_notarized',
      },
    },
  ],
  unavailableTargets: [
    { target: 'darwin-x64', statement: 'No Omega build is published for Intel Macs in this alpha.' },
    { target: 'win32-x64', statement: 'No Omega build is published for Windows in this alpha.' },
    { target: 'linux-x64', statement: 'No Omega build is published for Linux in this alpha.' },
  ],
  knownLimitations: [
    'Alpha prerelease: Omega has not passed every release-readiness gate.',
    'Published for Apple silicon Macs only.',
  ],
} as const

const asResolution = (value: unknown): OmegaDownloadResolution => {
  const decoded = decodeOmegaDownloadResolution(value)
  if (decoded === null) throw new Error('fixture failed strict decode')
  return decoded
}

const loaded = (value: unknown): Loadable<OmegaDownloadResolution> => ({
  state: 'ok',
  data: asResolution(value),
})

const render = (resolution: Loadable<OmegaDownloadResolution> | undefined): string =>
  renderToStaticMarkup(<OmegaDownloadSection resolution={resolution} />)

describe('OmegaDownloadSection (available)', () => {
  const html = render(loaded(availableResolution))

  test('renders Omega as a distinct product entry with the alpha channel', () => {
    expect(html).toContain('data-product="omega"')
    expect(html).toContain('Omega')
    expect(html).toContain('Alpha prerelease')
    expect(html).toContain('0.2.0-rc26')
    // Never relabels the Electron Desktop artifact: the section names no
    // OpenAgents Desktop version or artifact.
    expect(html).not.toContain('OpenAgents Desktop 0.1.0')
  })

  test('shows the per-cell truth: platform, arch, format, minimum OS, size', () => {
    expect(html).toContain('data-omega-target="darwin-arm64"')
    expect(html).toContain('Apple Silicon')
    expect(html).toContain('DMG')
    expect(html).toContain('macOS 11.0 or later')
    expect(html).toContain(omegaByteSize(193720537))
  })

  test('publishes the exact digest and the honest notarization truth', () => {
    expect(html).toContain(
      'e39cd7b822fc143f03e1d54d68a36ec0fe150bc33334d2bb50573342de3fcee3',
    )
    expect(html).toContain('Developer ID signed (Team HQWSG26L43)')
    expect(html).toContain('not yet notarized')
  })

  test('every CTA crosses the server artifact redirect — no raw artifact URL', () => {
    // React HTML-escapes `&` in attribute values.
    expect(html).toContain(
      `href="${omegaDownloadArtifactHref('darwin-arm64', 'dmg').replace('&', '&amp;')}"`,
    )
    expect(html).not.toContain('href="https://github.com/OpenAgentsInc/omega/releases/download/')
  })

  test('the per-cell row carries the release version', () => {
    expect(html).toContain('0.2.0-rc26 · Alpha prerelease · macOS 11.0 or later')
  })

  test('lists explicitly unavailable targets with their statements', () => {
    expect(html).toContain('No Omega build is published for Intel Macs in this alpha.')
    expect(html).toContain('No Omega build is published for Windows in this alpha.')
    expect(html).toContain('No Omega build is published for Linux in this alpha.')
    expect(html).toContain('Not available')
  })

  test('carries the alpha positioning beside the download', () => {
    expect(html).toContain('experienced developers and coding agent power users')
    expect(html).toContain('Prerelease warning:')
    expect(html).toContain('Prerequisites:')
    expect(html).toContain('Support boundary:')
    expect(html).toContain('tester channels')
    expect(html).toContain('Data risk:')
    expect(html).toContain('separate product from OpenAgents Desktop')
  })

  test('renders known limitations and the GitHub release links', () => {
    expect(html).toContain('Known limitations')
    expect(html).toContain('Alpha prerelease: Omega has not passed every release-readiness gate.')
    expect(html).toContain(
      'href="https://github.com/OpenAgentsInc/omega/releases/tag/v0.2.0-rc26"',
    )
    expect(html).toContain('href="https://github.com/OpenAgentsInc/omega/releases"')
  })
})

describe('OmegaDownloadSection (degraded)', () => {
  test('notarized artifact renders the notarized truth', () => {
    const html = render(
      loaded({
        ...availableResolution,
        artifacts: [
          {
            ...availableResolution.artifacts[0],
            apple: { ...availableResolution.artifacts[0].apple, notarization: 'notarized' },
          },
        ],
      }),
    )
    expect(html).toContain('notarized by Apple')
    expect(html).not.toContain('not yet notarized')
  })

  test('unavailable resolution renders zero download URLs', () => {
    const html = render(
      loaded({
        schema: 'openagents.omega.download_resolution.v1',
        product: 'omega',
        availability: 'unavailable',
        reason: 'signature_invalid',
      }),
    )
    expect(html).toContain('could not be verified')
    expect(html).not.toContain('/api/public/omega-download/artifact')
    expect(html).not.toContain('github.com/OpenAgentsInc/omega/releases/download')
    // The alpha positioning still renders — the product entry never vanishes
    // into an implied Desktop relabel.
    expect(html).toContain('experienced developers and coding agent power users')
  })

  test('missing loader data renders the honest checking state', () => {
    const html = render(undefined)
    expect(html).toContain('Checking the current Omega alpha release')
    expect(html).not.toContain('/api/public/omega-download/artifact')
  })
})

describe('strict client decoder', () => {
  test('rejects a malformed digest', () => {
    expect(
      decodeOmegaDownloadResolution({
        ...availableResolution,
        artifacts: [{ ...availableResolution.artifacts[0], sha256: 'nope' }],
      }),
    ).toBeNull()
  })

  test('rejects an http (non-https) artifact URL', () => {
    expect(
      decodeOmegaDownloadResolution({
        ...availableResolution,
        artifacts: [
          { ...availableResolution.artifacts[0], url: 'http://example.com/omega.dmg' },
        ],
      }),
    ).toBeNull()
  })
})

describe('labels', () => {
  test('target labels cover the full vocab', () => {
    expect(omegaTargetLabel('darwin-arm64')).toBe('macOS · Apple Silicon')
    expect(omegaTargetLabel('win32-x64')).toBe('Windows · x64')
    expect(omegaTargetLabel('linux-arm64')).toBe('Linux · ARM64')
  })

  test('signature truth line has no apple claim for non-darwin artifacts', () => {
    expect(
      omegaSignatureTruth({
        target: 'linux-x64',
        format: 'appimage',
        name: 'Omega-fixture.AppImage',
        url: 'https://github.com/OpenAgentsInc/omega/releases/download/v0/Omega-fixture.AppImage',
        sha256: availableResolution.artifacts[0].sha256,
        byteLength: 1,
        minimumOs: 'glibc 2.35',
      }),
    ).not.toContain('Developer ID')
  })
})
