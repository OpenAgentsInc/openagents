import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { SplashPage } from './-splash-page'

describe('Desktop splash', () => {
  test('server-renders the live workroom reconstruction and its accessible controls', () => {
    const html = renderToStaticMarkup(<SplashPage />)

    expect(html).toContain('data-route="splash"')
    expect(html).toContain('OpenAgents Desktop live product preview')
    expect(html).toContain('Live product splash')
    expect(html).toContain('Build a new /splash page from the real Desktop workroom.')
    expect(html).toContain('Review changes')
    expect(html).toContain('Message Codex')
    expect(html).not.toContain('<img')
  })
})
