import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { isKnownStartDocumentPath } from '../route-table'
import { WorkPage } from './-work-page'

describe('/work sales landing page', () => {
  test('the route table admits /work', () => {
    expect(isKnownStartDocumentPath('/work')).toBe(true)
    expect(isKnownStartDocumentPath('/work/')).toBe(true)
    expect(isKnownStartDocumentPath('/work/anything')).toBe(false)
  })

  test('server-renders the AI-employees landing page', () => {
    const html = renderToStaticMarkup(<WorkPage />)

    expect(html).toContain('data-route="work"')
    expect(html).toContain('AI employees that work.')

    // Every CTA drives into the Sarah sales agent (owner direction
    // 2026-07-28), and the page discloses that Sarah is an AI up front.
    expect(html).toContain('https://sarah.openagents.com')
    expect(html).toContain('Talk to Sarah')
    expect(html).toContain('Our sales rep is an AI.')

    // Sarah's public employee card: stated authority ceiling and limits.
    expect(html).toContain('Closes deals up to $10,000 with a real payment link')
    expect(html).toContain('Cannot invent case studies, metrics, or guarantees')

    // Pricing reuses the public /business rate card verbatim (single source,
    // -funnel-data.ts businessPackages); no new numbers are authored here.
    expect(html).toContain('Quick Win')
    expect(html).toContain('$1,000-$5,000 fixed')
    expect(html).toContain('On Autopilot Retainer')
    expect(html).toContain('$2,000-$10,000 / month')
  })
})
