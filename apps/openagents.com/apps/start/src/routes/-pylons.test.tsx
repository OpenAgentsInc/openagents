import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { PylonsPage } from './-pylons-page'

describe('Start /pylons route', () => {
  test('server-renders the route contract and install CTA', () => {
    const html = renderToStaticMarkup(<PylonsPage />)

    expect(html).toContain('data-route="pylon"')
    expect(html).toContain('data-cta="install-pylon"')
    expect(html).toContain('Run a Pylon node')
    expect(html).toContain('Paste this to your coding agent')
    expect(html).toContain('npx @openagentsinc/pylon')
    expect(html).toContain('data-cta="download-autopilot-link"')
    expect(html).toContain('Or download the Mac app')
    expect(html).toContain('href="/download"')
  })

  test('renders the honest pre-fetch idle state for the live stats overlay', () => {
    const html = renderToStaticMarkup(<PylonsPage />)

    expect(html).toContain('data-pylon-scene="stats-overlay"')
    expect(html).toContain('data-stat-value="online"')
    expect(html).toContain('data-stat-value="working"')
    expect(html).toContain('data-stat-value="sats24h"')
    expect(html).toContain('data-stat-value="training"')
    expect(html).toContain('pylons online')
    expect(html).toContain('work-ready now')
    expect(html).toContain('sats settled · 24h')
    expect(html).toContain('training contributors')
    // Snapshot is null before the client effect resolves, so every counter
    // shows the loading placeholder rather than a fabricated number.
    const loadingCount = (html.match(/…/g) ?? []).length
    expect(loadingCount).toBe(4)
  })

  test('renders an empty bezier network graph before the first live poll', () => {
    const html = renderToStaticMarkup(<PylonsPage />)

    expect(html).toContain('data-pylon-scene="bezier-network"')
    // No online pylons yet (null snapshot) -> zero edges/nodes, not fabricated.
    expect(html).not.toContain('<path')
    expect(html).not.toContain('<circle')
  })

  test('renders the reachable copy-instructions control (post-launch state)', () => {
    const html = renderToStaticMarkup(<PylonsPage />)

    expect(html).toContain('data-cta="copy-agent-instructions"')
    expect(html).toContain('Copy Agent Instructions')
  })

  test('renders a design-consistent ambient placeholder, not a fabricated 3D scene', () => {
    const html = renderToStaticMarkup(<PylonsPage />)

    expect(html).toContain('data-pylon-scene="ambient-placeholder"')
  })
})
