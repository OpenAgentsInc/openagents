import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { LandingPage } from './index'

describe('Start landing route', () => {
  test('server-renders the ported landing copy', () => {
    const html = renderToStaticMarkup(<LandingPage />)

    expect(html).toContain('OpenAgents')
    expect(html).toContain('What is Khala?')
    expect(html).toContain('Join the Tassadar training run')
    expect(html).toContain('data-route="landing"')
    expect(html).not.toContain('id="root"')
  })
})
