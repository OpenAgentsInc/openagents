import { readFileSync } from 'node:fs'
import path from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { DesktopLandingPage, HoldingPage, InstallPage } from './-public-site'

describe('TanStack Start public site', () => {
  test('server-renders the complete Desktop MVP landing', () => {
    const html = renderToStaticMarkup(<DesktopLandingPage />)

    expect(html).toContain('A serious place')
    expect(html).toContain('Conversation first.')
    expect(html).toContain('Stable thread identity')
    expect(html).toContain('Repository review never mutates the tree')
    expect(html).toContain('The important boundaries, plainly.')
    expect(html).toContain('The work should survive the window.')
    expect(html).toContain('0.1.0-rc.12')
    expect(html).toContain('href="/docs"')
    expect(html).not.toContain('data-astro')
    expect(html).not.toContain('Launch UI')
  })

  test('preserves the public holding page content and background seam', () => {
    const html = renderToStaticMarkup(<HoldingPage />)
    const css = readFileSync(
      path.resolve(import.meta.dirname, '../public-site.css'),
      'utf8',
    )

    expect(html).toContain('<h1>OpenAgents</h1>')
    expect(html).toContain('<p>be right back</p>')
    expect(css).toContain("url('/holding-bg.jpg')")
    expect(css).toContain('--oa-void: #05070d')
  })

  test('links the exact published Mac release candidate', () => {
    const html = renderToStaticMarkup(<InstallPage />)

    expect(html).toContain('OpenAgents Desktop')
    expect(html).toContain('Download for Mac')
    expect(html).toContain('0.1.0-rc.12')
    expect(html).toContain('OpenAgents-0.1.0-rc.12-arm64.dmg')
    expect(html).toContain('href="/docs"')
  })
})
