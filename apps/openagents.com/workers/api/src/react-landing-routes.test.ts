import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleReactLandingPage,
  renderReactLandingHtml,
} from './react-landing-routes'

describe('React landing routes', () => {
  test('/demo renders the original Launch UI replica copy', () => {
    const html = renderReactLandingHtml('demo')

    expect(html).toContain('data-react-landing-route="demo"')
    expect(html).toContain('Give your big idea the design it deserves')
    expect(html).toContain('Launch UI v2 is out!')
    expect(html).toContain('Used by 34.7k+ companies and builders')
    expect(html).toContain('/assets/openagents.css')
    expect(html).not.toContain('/assets/index-')
  })

  test('/new renders the OpenAgents four-product adaptation', () => {
    const html = renderReactLandingHtml('new')

    expect(html).toContain('data-react-landing-route="new"')
    expect(html).toContain('The operating system for agents that work')
    expect(html).toContain('Khala Code mobile')
    expect(html).toContain('Khala Code desktop')
    expect(html).toContain('openagents.com')
    expect(html).toContain('Reactor')
    expect(html).toContain('Four products, one receipt spine.')
    expect(html).toContain('/assets/openagents.css')
    expect(html).not.toContain('/assets/index-')
  })

  test('GET returns text/html and HEAD returns headers only', async () => {
    const getResponse = await Effect.runPromise(
      handleReactLandingPage(new Request('https://openagents.com/new'), 'new'),
    )
    expect(getResponse.status).toBe(200)
    expect(getResponse.headers.get('content-type')).toContain('text/html')
    expect(await getResponse.text()).toContain('Khala Code mobile')

    const headResponse = await Effect.runPromise(
      handleReactLandingPage(
        new Request('https://openagents.com/demo', { method: 'HEAD' }),
        'demo',
      ),
    )
    expect(headResponse.status).toBe(200)
    expect(await headResponse.text()).toBe('')
  })

  test('non-document methods are rejected', async () => {
    const response = await Effect.runPromise(
      handleReactLandingPage(
        new Request('https://openagents.com/new', { method: 'POST' }),
        'new',
      ),
    )

    expect(response.status).toBe(405)
  })
})
