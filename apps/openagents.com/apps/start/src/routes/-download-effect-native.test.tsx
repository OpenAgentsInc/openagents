import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_DESKTOP_DMG_URL,
  AUTOPILOT_DESKTOP_RELEASE_URL,
  DOWNLOAD_ONE_CLICK_READY,
  DownloadEffectNativePage,
  PYLON_INSTALL_COMMAND,
  downloadLandingView,
  downloadStatusFromState,
  initialDownloadLandingState,
} from './-download-effect-native-page'

describe('EN-4 /download Effect Native route', () => {
  test('server render is only a thin mount shim, not download-content React', () => {
    const html = renderToStaticMarkup(<DownloadEffectNativePage />)

    expect(html).toContain('data-route="download"')
    expect(html).toContain('data-download-effect-native-root=""')
    expect(html).not.toContain('Signed + notarized .dmg')
    expect(html).not.toContain(AUTOPILOT_DESKTOP_DMG_URL)
    expect(html).not.toContain(PYLON_INSTALL_COMMAND)
  })

  test('authored content is a typed Effect Native tree with platform and CTA copy', () => {
    const tree = downloadLandingView(initialDownloadLandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'download-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v29"')
    expect(serialized).toContain('Download Autopilot for Mac')
    expect(serialized).toContain('Download for Mac (Apple Silicon)')
    expect(serialized).toContain(AUTOPILOT_DESKTOP_DMG_URL)
    expect(serialized).toContain(AUTOPILOT_DESKTOP_RELEASE_URL)
    expect(serialized).toContain('View the release on GitHub')
    expect(serialized).toContain('macOS · Apple Silicon')
    expect(serialized).toContain('macOS · Intel')
    expect(serialized).toContain('Not published yet')
    expect(serialized).toContain('Windows')
    expect(serialized).toContain('Pending the Authenticode signing certificate')
    expect(serialized).toContain('Linux')
    expect(serialized).toContain(PYLON_INSTALL_COMMAND)
    expect(serialized).toContain('Status: auto-onboarding not in this build yet')
    expect(serialized).toContain('download-status:gated')
    expect(serialized).not.toContain('className')
  })

  test('keeps one-click auto-onboarding gated until a fresh signed DMG ships', () => {
    expect(DOWNLOAD_ONE_CLICK_READY).toBe(false)
    expect(downloadStatusFromState(initialDownloadLandingState)).toBe('gated')
    expect(downloadStatusFromState({ oneClickReady: true })).toBe('live')
  })

  test('source boundary uses Effect Native packages instead of launch-ui / React content', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-download-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('@/components/ui/')
    expect(source).not.toContain('launch-ui')
  })
})
