import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { LandingPage } from './index'

describe('Start landing route', () => {
  test('server-renders the phase-1 Launch UI replica structure', () => {
    const html = renderToStaticMarkup(<LandingPage />)

    expect(html).toContain('data-route="landing"')
    expect(html).toContain('data-launch-ui-replica="blue-minimal"')
    expect(html).toContain('Launch UI v2 is out!')
    expect(html).toContain('Give your big idea the design it deserves')
    expect(html).toContain('Professionally designed blocks and templates')
    expect(html).toContain('Used by 34.7k+ companies and builders')
    expect(html).toContain('Built with industry-standard tools')
    expect(html).toContain("Everything you need. Nothing you don&#x27;t.")
    expect(html).toContain('Build your dream landing page, today.')
    expect(html).toContain('Questions and Answers')
    expect(html).toContain('Start building')
    expect(html).not.toContain('ModeToggle')
    expect(html).not.toContain('id="root"')
  })
})
