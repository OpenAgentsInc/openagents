/**
 * DIST-11 (#8924) — platform-aware /download page tests.
 *
 * The page renders exclusively from `openagents.desktop.download_resolution.v1`
 * projections (DIST-10 #8923). Fixtures below are hand-built typed
 * resolutions covering: every target/format projection, unknown detection,
 * explicit override, unavailable feeds, partial rollout (a promoted release
 * set carrying only one target), version changes, and the telemetry seam
 * (every CTA goes through the artifact redirect that emits the validated
 * `artifact_redirect` event — never a raw artifact URL).
 */
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  decodeDesktopDownloadResolution,
  desktopDownloadArtifactHref,
  desktopDownloadFormats,
  desktopDownloadTargets,
  fetchDesktopDownloadResolution,
  parseDownloadSearch,
  type DesktopDownloadArtifact,
  type DesktopDownloadFormat,
  type DesktopDownloadResolution,
  type DesktopDownloadTarget,
  type Loadable,
} from './-download-data'
import {
  DownloadPage,
  downloadPageDescription,
  downloadPageStructuredData,
  formatByteSize,
  minimumOsLabel,
  resolutionCatalog,
  targetLabel,
} from './-download-page'

// --- fixtures ----------------------------------------------------------------

const V2_VERSION = '2.4.0-rc.3'
const V1_VERSION = '0.1.0-rc.13'

// Owner amendment 2026-07-20 (#8920, DIST-01): the signed `/download` surface
// carries exactly the four mac+linux targets; `win32-x64` is an optional
// experimental portable outside it (a Windows visitor gets an honest
// detected-unavailable state, exercised by `chooseWindowsUnavailable` below).
const minimumOs: Record<DesktopDownloadTarget, string> = {
  'darwin-arm64': '13.5',
  'darwin-x64': '13.5',
  'linux-arm64': 'glibc 2.35',
  'linux-x64': 'glibc 2.35',
}

const formatsByTarget: Record<DesktopDownloadTarget, readonly DesktopDownloadFormat[]> = {
  'darwin-arm64': ['dmg', 'zip'],
  'darwin-x64': ['dmg', 'zip'],
  'linux-arm64': ['appimage', 'deb', 'rpm'],
  'linux-x64': ['appimage', 'deb', 'rpm'],
}

const preferredFormat: Record<DesktopDownloadTarget, DesktopDownloadFormat> = {
  'darwin-arm64': 'dmg',
  'darwin-x64': 'dmg',
  'linux-arm64': 'appimage',
  'linux-x64': 'appimage',
}

const artifact = (
  target: DesktopDownloadTarget,
  format: DesktopDownloadFormat,
  version: string = V2_VERSION,
  byteLength = 303_959_067,
): DesktopDownloadArtifact => ({
  target,
  format,
  version,
  channel: 'rc',
  url: `https://updates.fixture.test/artifacts/OpenAgents-${version}-rc-${target}.${format}`,
  sha256: 'ab'.repeat(32),
  byteLength,
  minimumOs: minimumOs[target],
  preferred: format === preferredFormat[target],
})

const fullCatalog: readonly DesktopDownloadArtifact[] = desktopDownloadTargets.flatMap(
  target => formatsByTarget[target].map(format => artifact(target, format)),
)

const header = {
  schema: 'openagents.desktop.download_resolution.v1',
  source: 'release_set_v2',
  channel: 'rc',
  version: V2_VERSION,
  releasedAt: '2026-07-16T12:00:00Z',
  releaseNotes: 'Adds Linux targets plus a faster reconnect path.',
  sourceRevision: 'a'.repeat(40),
} as const

const fullAvailable: DesktopDownloadResolution = {
  ...header,
  availability: 'available',
  detection: { platform: 'darwin', architecture: 'arm64', method: 'client_hints' },
  selected: fullCatalog.find(a => a.target === 'darwin-arm64' && a.format === 'dmg')!,
  alternatives: [
    fullCatalog.find(a => a.target === 'darwin-arm64' && a.format === 'zip')!,
    ...fullCatalog.filter(a => a.target !== 'darwin-arm64'),
  ],
}

const overrideAvailable: DesktopDownloadResolution = {
  ...header,
  availability: 'available',
  detection: { platform: 'darwin', architecture: 'x64', method: 'override' },
  selected: fullCatalog.find(a => a.target === 'darwin-x64' && a.format === 'dmg')!,
  alternatives: fullCatalog.filter(a => !(a.target === 'darwin-x64' && a.format === 'dmg')),
}

// A partial-rollout shape: a promoted v2 release set that currently carries
// exactly one target (the resolver contract only ever reports one source now
// that the bounded v1 darwin-arm64 migration path has been retired — #8923).
const partialV1: DesktopDownloadResolution = {
  schema: 'openagents.desktop.download_resolution.v1',
  source: 'release_set_v2',
  channel: 'rc',
  version: V1_VERSION,
  releasedAt: '2026-07-14T21:05:24.119Z',
  releaseNotes: 'release.notes.0.1.0-rc.13',
  sourceRevision: null,
  availability: 'available',
  detection: { platform: 'darwin', architecture: 'arm64', method: 'user_agent' },
  selected: artifact('darwin-arm64', 'dmg', V1_VERSION),
  alternatives: [],
}

const chooseUnknown: DesktopDownloadResolution = {
  ...header,
  availability: 'choose_manually',
  reason: 'unknown_client',
  detection: { platform: null, architecture: null, method: 'none' },
  options: [artifact('darwin-arm64', 'dmg'), artifact('darwin-arm64', 'zip')],
}

const chooseWindowsUnavailable: DesktopDownloadResolution = {
  schema: 'openagents.desktop.download_resolution.v1',
  source: 'release_set_v2',
  channel: 'rc',
  version: V1_VERSION,
  releasedAt: '2026-07-14T21:05:24.119Z',
  releaseNotes: null,
  sourceRevision: null,
  availability: 'choose_manually',
  reason: 'target_unavailable',
  detection: { platform: 'win32', architecture: 'x64', method: 'user_agent' },
  options: [artifact('darwin-arm64', 'dmg', V1_VERSION)],
}

const unavailableFeed: DesktopDownloadResolution = {
  schema: 'openagents.desktop.download_resolution.v1',
  availability: 'unavailable',
  channel: 'rc',
  reason: 'feed_unreachable',
  detection: { platform: 'darwin', architecture: 'arm64', method: 'client_hints' },
}

const ok = (data: DesktopDownloadResolution): Loadable<DesktopDownloadResolution> => ({
  state: 'ok',
  data,
})

const render = (resolution: Loadable<DesktopDownloadResolution>): string =>
  renderToStaticMarkup(<DownloadPage resolution={resolution} />)

const escapedHref = (
  target: DesktopDownloadTarget,
  format: DesktopDownloadFormat,
): string =>
  `href="${desktopDownloadArtifactHref(target, format, 'rc').replaceAll('&', '&amp;')}"`

// --- page states ---------------------------------------------------------------

describe('/download page — resolver-driven states', () => {
  test('detected supported target renders a primary CTA through the artifact redirect', () => {
    const html = render(ok(fullAvailable))

    expect(html).toContain('Download OpenAgents Desktop')
    expect(html).toContain('Available now for Apple silicon Macs.')
    expect(html).toContain(`Version ${V2_VERSION} · Release candidate`)
    // Primary CTA: server-side redirect (the telemetry emission point),
    // never a raw artifact URL.
    expect(html).toContain('Download for macOS · Apple Silicon')
    expect(html).toContain(escapedHref('darwin-arm64', 'dmg'))
    expect(html).toContain('304 MB')
    expect(html).toContain('macOS 13.5 or later')
    // Same-target format alternative surfaces beside the primary CTA.
    expect(html).toContain('Also available:')
    expect(html).toContain(escapedHref('darwin-arm64', 'zip'))
    // Release notes (the #8927 seam) render from the resolution.
    expect(html).toContain(`What’s new in ${V2_VERSION}`)
    expect(html).toContain('Adds Linux targets plus a faster reconnect path.')
    // Verification guidance with the release-set checksum.
    expect(html).toContain('Verification and support')
    expect(html).toContain('ab'.repeat(32))
  })

  test('every promoted target/format projects an explicit alternative link', () => {
    const html = render(ok(fullAvailable))
    for (const target of desktopDownloadTargets) {
      for (const format of formatsByTarget[target]) {
        expect(html).toContain(escapedHref(target, format))
      }
    }
    // Full rollout: no pending rows remain.
    expect(html).not.toContain('Not yet available')
    expect(html).not.toContain('Coming soon')
    // Platform guidance honestly explains formats once targets exist. Windows
    // is no longer a signed section (#8920), so its per-user guidance is gone.
    expect(html).not.toContain('installs per-user')
    expect(html).toContain('package manager, which then owns updates and rollback')
    expect(html).toContain('Macs with Apple silicon (M1 or later)')
  })

  test('never renders a raw artifact URL — every CTA crosses the redirect telemetry seam', () => {
    for (const resolution of [fullAvailable, partialV1, chooseUnknown, chooseWindowsUnavailable]) {
      const html = render(ok(resolution))
      expect(html).not.toContain('updates.fixture.test')
      expect(html).not.toContain('releases/download/')
      expect(html).toContain('/api/public/desktop-download/artifact?')
    }
  })

  test('partial rollout (a release set promoting only one target) claims exactly one target', () => {
    const html = render(ok(partialV1))

    expect(html).toContain(`Version ${V1_VERSION}`)
    expect(html).toContain(escapedHref('darwin-arm64', 'dmg'))
    // No other target may render as available.
    for (const target of desktopDownloadTargets.filter(t => t !== 'darwin-arm64')) {
      for (const format of formatsByTarget[target]) {
        expect(html).not.toContain(escapedHref(target, format))
      }
    }
    // Three unavailable signed targets remain (darwin-x64, linux-x64,
    // linux-arm64) once darwin-arm64 is promoted. Windows is no longer part of
    // the signed `/download` surface (#8920), so it is not one of them.
    expect(html.match(/Not yet available/g)).toHaveLength(3)
    expect(html.match(/Coming soon/g)).toHaveLength(3)
    expect(html).toContain('The Intel build is not yet available.')
    expect(html).not.toContain('Windows builds are not yet available.')
    expect(html).toContain('Linux builds are not yet available.')
    // Any non-null `releaseNotes` on the (only) `release_set_v2` source
    // renders as-is — the resolver contract is the trust boundary for its
    // content, not this page.
    expect(html).toContain('release.notes.')
    expect(html).toContain('What’s new')
  })

  test('unknown client renders the explicit chooser, not a guessed download', () => {
    const html = render(ok(chooseUnknown))

    expect(html).toContain('We could not detect your platform automatically.')
    expect(html).not.toContain('oa-download-primary-cta')
    expect(html).toContain(escapedHref('darwin-arm64', 'dmg'))
    expect(html).toContain(escapedHref('darwin-arm64', 'zip'))
    expect(html).toContain('All platforms')
  })

  test('detected-but-unpromoted platform states the truth and offers available builds', () => {
    const html = render(ok(chooseWindowsUnavailable))

    expect(html).toContain('OpenAgents Desktop is not yet available for Windows on x64.')
    expect(html).not.toContain('oa-download-primary-cta')
    expect(html).toContain(escapedHref('darwin-arm64', 'dmg'))
  })

  test('explicit override renders the chosen target as such', () => {
    const html = render(ok(overrideAvailable))

    expect(html).toContain('Showing the macOS · Intel build (chosen manually).')
    expect(html).toContain('Download for macOS · Intel')
    expect(html).toContain(escapedHref('darwin-x64', 'dmg'))
  })

  test('unavailable feed fails closed: zero download URLs, honest copy', () => {
    const html = render(ok(unavailableFeed))

    expect(html).toContain('Downloads are temporarily unavailable')
    expect(html).toContain('could not be fetched')
    expect(html).not.toContain('/api/public/desktop-download/artifact?')
    expect(html).not.toContain('Coming soon')
    expect(html).not.toContain(V2_VERSION)
  })

  test('fetch-failure fallback renders the same honest degraded state', () => {
    const html = render({ state: 'unavailable', detail: 'resolver unreachable' })

    expect(html).toContain('Downloads are temporarily unavailable')
    expect(html).not.toContain('/api/public/desktop-download/artifact?')
    // Support guidance still present so the user has a path forward.
    expect(html).toContain('href="/docs"')
    expect(html).toContain('All releases on GitHub')
  })

  test('loading state makes no availability claims', () => {
    const html = render({ state: 'loading' })
    expect(html).toContain('Checking the current release…')
    expect(html).not.toContain('/api/public/desktop-download/artifact?')
  })

  test('a version change re-renders labels from the resolution alone', () => {
    const current = render(ok(partialV1))
    const next = render(ok(fullAvailable))
    expect(current).toContain(V1_VERSION)
    expect(current).not.toContain(V2_VERSION)
    expect(next).toContain(V2_VERSION)
    expect(next).not.toContain(V1_VERSION)
  })
})

// --- accessibility / structure ------------------------------------------------

describe('/download page — semantic structure and keyboard access', () => {
  test('semantic headings, labeled sections, and natively focusable controls', () => {
    document.body.innerHTML = render(ok(fullAvailable))

    expect(document.querySelectorAll('h1')).toHaveLength(1)
    expect(document.querySelector('h1')?.textContent).toBe('Download OpenAgents Desktop')

    // Each signed platform section is a labeled region with its own heading.
    // Windows is not a signed `/download` section (#8920).
    for (const platform of ['darwin', 'linux']) {
      const section = document.querySelector(`[aria-labelledby="oa-platform-${platform}"]`)
      expect(section, platform).not.toBeNull()
      expect(document.getElementById(`oa-platform-${platform}`)?.tagName).toBe('H3')
    }

    // The primary CTA is described by its version/format/size metadata.
    const primary = document.querySelector('.oa-download-primary-cta')
    expect(primary?.getAttribute('aria-describedby')).toBe('oa-download-primary-meta')
    expect(document.getElementById('oa-download-primary-meta')).not.toBeNull()

    // Format link groups carry screen-reader labels naming the target.
    const groups = [...document.querySelectorAll('.oa-format-links')]
    expect(groups.length).toBeGreaterThan(0)
    for (const group of groups) {
      expect(group.getAttribute('role')).toBe('group')
      expect(group.getAttribute('aria-label')).toMatch(/downloads$/)
    }

    // Keyboard-only smoke: every interactive element is a native anchor,
    // summary, or button — nothing is a click-handler div, and nothing is
    // removed from the tab order.
    expect(document.querySelectorAll('[onclick]')).toHaveLength(0)
    expect(document.querySelectorAll('[tabindex="-1"]')).toHaveLength(0)
    for (const anchor of document.querySelectorAll('a')) {
      expect(anchor.getAttribute('href'), anchor.outerHTML).toBeTruthy()
    }
    // The checksum disclosure is a native <details>/<summary> pair.
    expect(document.querySelector('details.oa-download-checksum summary')).not.toBeNull()
  })

  test('unavailable state announces itself as a status region', () => {
    document.body.innerHTML = render(ok(unavailableFeed))
    expect(document.querySelector('.oa-download-unavailable')?.getAttribute('role')).toBe('status')
  })
})

// --- head/SEO projections -------------------------------------------------------

describe('/download head projections', () => {
  test('description never claims unpromoted platforms', () => {
    const partial = downloadPageDescription(ok(partialV1))
    expect(partial).toContain(V1_VERSION)
    expect(partial).toContain('Apple silicon Macs')
    expect(partial).not.toContain('Windows')
    expect(partial).not.toContain('Linux')

    const full = downloadPageDescription(ok(fullAvailable))
    // Windows is outside the signed `/download` catalog (#8920), so the
    // description never advertises it; Linux remains a signed platform.
    expect(full).not.toContain('Windows')
    expect(full).toContain('Linux')

    expect(downloadPageDescription(ok(unavailableFeed))).toContain('temporarily unavailable')
    expect(downloadPageDescription({ state: 'unavailable', detail: 'x' })).toContain(
      'resolved from the current signed release',
    )
    expect(downloadPageDescription(undefined)).toContain('signed release')
  })

  test('structured data mirrors the promoted catalog only', () => {
    expect(downloadPageStructuredData(ok(unavailableFeed))).toBeNull()
    expect(downloadPageStructuredData({ state: 'loading' })).toBeNull()

    const partial = JSON.parse(downloadPageStructuredData(ok(partialV1)) ?? 'null') as {
      softwareVersion: string
      operatingSystem: string
    }
    expect(partial.softwareVersion).toBe(V1_VERSION)
    expect(partial.operatingSystem).toBe('macOS 13.5+')

    const full = JSON.parse(downloadPageStructuredData(ok(fullAvailable)) ?? 'null') as {
      operatingSystem: string
    }
    expect(full.operatingSystem).not.toContain('Windows')
    expect(full.operatingSystem).toContain('macOS')
    expect(full.operatingSystem).toContain('Linux')
  })
})

// --- pure helpers ----------------------------------------------------------------

describe('/download helpers', () => {
  test('resolutionCatalog projects each availability shape', () => {
    expect(resolutionCatalog(fullAvailable)).toHaveLength(fullCatalog.length)
    expect(resolutionCatalog(chooseUnknown)).toHaveLength(2)
    expect(resolutionCatalog(unavailableFeed)).toHaveLength(0)
  })

  test('formatByteSize and labels', () => {
    expect(formatByteSize(303_959_067)).toBe('304 MB')
    expect(formatByteSize(1_500_000_000)).toBe('1.5 GB')
    expect(formatByteSize(950_000)).toBe('1 MB')
    expect(targetLabel('darwin-x64')).toBe('macOS · Intel')
    expect(minimumOsLabel('linux-x64', 'glibc 2.35')).toBe('Linux with glibc 2.35 or later')
  })

  test('parseDownloadSearch keeps only bounded valid values', () => {
    expect(parseDownloadSearch({})).toEqual({})
    expect(
      parseDownloadSearch({ target: 'darwin-arm64', format: 'zip', channel: 'rc' }),
    ).toEqual({ target: 'darwin-arm64', format: 'zip', channel: 'rc' })
    expect(
      parseDownloadSearch({
        target: 'amiga-68k',
        format: 'tar.lz4',
        channel: 'nightly',
        junk: '1',
      }),
    ).toEqual({})
    expect(parseDownloadSearch({ target: 42, format: null })).toEqual({})
  })

  test('artifact hrefs pin target, format, and channel to the rendered release', () => {
    expect(desktopDownloadArtifactHref('darwin-arm64', 'dmg', 'rc')).toBe(
      '/api/public/desktop-download/artifact?target=darwin-arm64&format=dmg&channel=rc',
    )
    expect(desktopDownloadArtifactHref('linux-x64', 'rpm')).toBe(
      '/api/public/desktop-download/artifact?target=linux-x64&format=rpm',
    )
  })

  test('fetchDesktopDownloadResolution fails soft and forwards overrides', async () => {
    const seen: string[] = []
    const okFetch = ((input: RequestInfo | URL) => {
      seen.push(String(input))
      return Promise.resolve(Response.json(unavailableFeed))
    }) as typeof fetch

    const loaded = await fetchDesktopDownloadResolution(
      { target: 'linux-x64', format: 'deb', channel: 'rc' },
      okFetch,
    )
    expect(loaded.state).toBe('ok')
    expect(seen[0]).toBe(
      '/api/public/desktop-download?target=linux-x64&format=deb&channel=rc',
    )

    const httpError = await fetchDesktopDownloadResolution(
      {},
      (() => Promise.resolve(new Response('nope', { status: 503 }))) as typeof fetch,
    )
    expect(httpError).toEqual({
      state: 'unavailable',
      detail: 'Download resolver returned HTTP 503.',
    })

    const badSchema = await fetchDesktopDownloadResolution(
      {},
      (() => Promise.resolve(Response.json({ schema: 'other.v9' }))) as typeof fetch,
    )
    expect(badSchema.state).toBe('unavailable')

    // A malformed payload that keeps the CORRECT schema id (the DIST-11
    // #8924 independent-review finding: a shallow `schema` string check
    // alone would have cast this straight through instead of degrading).
    const malformedSameSchema = await fetchDesktopDownloadResolution(
      {},
      (() =>
        Promise.resolve(
          Response.json({
            schema: 'openagents.desktop.download_resolution.v1',
            source: 'release_set_v2',
            availability: 'available',
            channel: 'rc',
            version: V2_VERSION,
            releasedAt: header.releasedAt,
            releaseNotes: null,
            sourceRevision: null,
            detection: { platform: 'darwin', architecture: 'arm64', method: 'client_hints' },
            // `selected` is missing entirely — a same-schema, wrong-shape payload.
            alternatives: [],
          }),
        )) as typeof fetch,
    )
    expect(malformedSameSchema.state).toBe('unavailable')
    if (malformedSameSchema.state === 'unavailable') {
      expect(malformedSameSchema.detail).toContain('malformed')
    }

    const network = await fetchDesktopDownloadResolution(
      {},
      (() => Promise.reject(new Error('offline'))) as typeof fetch,
    )
    expect(network).toEqual({ state: 'unavailable', detail: 'offline' })
  })

  test('decodeDesktopDownloadResolution strictly validates every resolution shape', () => {
    // Every hand-built fixture used across this suite must decode losslessly
    // through the client-safe schema — proof the two schemas (server + this
    // client mirror) have not drifted apart.
    for (const resolution of [
      fullAvailable,
      overrideAvailable,
      partialV1,
      chooseUnknown,
      chooseWindowsUnavailable,
      unavailableFeed,
    ]) {
      expect(decodeDesktopDownloadResolution(resolution)).toEqual(resolution)
    }

    // Wrong enum value.
    expect(
      decodeDesktopDownloadResolution({ ...unavailableFeed, reason: 'not_a_real_reason' }),
    ).toBeNull()
    const macDmg = artifact('darwin-arm64', 'dmg')
    // Malformed sha256 (not 64 lowercase hex chars).
    expect(
      decodeDesktopDownloadResolution({
        ...fullAvailable,
        selected: { ...macDmg, sha256: 'not-a-hash' },
      }),
    ).toBeNull()
    // byteLength must be a positive integer.
    expect(
      decodeDesktopDownloadResolution({
        ...fullAvailable,
        selected: { ...macDmg, byteLength: -1 },
      }),
    ).toBeNull()
    // Completely unrelated JSON.
    expect(decodeDesktopDownloadResolution({ hello: 'world' })).toBeNull()
    expect(decodeDesktopDownloadResolution(null)).toBeNull()
    expect(decodeDesktopDownloadResolution('a string')).toBeNull()
  })

  test('format vocabulary stays in lockstep with the release-set contract', () => {
    // The format vocabulary stays the full six-format release vocabulary
    // (nsis remains for the drift guard), but `win32-x64` is no longer a signed
    // download target (#8920).
    expect(desktopDownloadFormats).toEqual(['dmg', 'zip', 'nsis', 'appimage', 'deb', 'rpm'])
    expect(desktopDownloadTargets).toEqual([
      'darwin-arm64',
      'darwin-x64',
      'linux-arm64',
      'linux-x64',
    ])
  })
})
