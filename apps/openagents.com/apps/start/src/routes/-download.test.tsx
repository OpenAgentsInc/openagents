import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_DESKTOP_DMG_URL,
  DOWNLOAD_ONE_CLICK_READY,
  DownloadPage,
} from './-download-page'

describe('Start download route', () => {
  test('server-renders the signed DMG link, platform copy, and Pylon alternative', () => {
    const html = renderToStaticMarkup(<DownloadPage />)

    expect(html).toContain('data-route="download"')
    expect(html).toContain('Download Autopilot for Mac')
    expect(html).toContain('data-cta="download-autopilot"')
    expect(html).toContain(AUTOPILOT_DESKTOP_DMG_URL)
    expect(html).toContain('Download for Mac (Apple Silicon)')
    expect(html).toContain('macOS · Apple Silicon')
    expect(html).toContain('macOS · Intel')
    expect(html).toContain('Not published yet')
    expect(html).toContain('Windows')
    expect(html).toContain('Pending the Authenticode signing certificate')
    expect(html).toContain('Linux')
    expect(html).toContain('npx @openagentsinc/pylon')
  })

  test('keeps one-click auto-onboarding gated until a fresh signed DMG ships', () => {
    expect(DOWNLOAD_ONE_CLICK_READY).toBe(false)

    const html = renderToStaticMarkup(<DownloadPage />)

    expect(html).toContain('data-download-status="gated"')
    expect(html).toContain('Status: auto-onboarding not in this build yet')
  })
})
