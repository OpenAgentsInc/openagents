import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { publicAgentDisplayName, PublicAgentPage } from './-public-agent-page'

describe('Start /agents/$agentRef and /adjutant routes', () => {
  test('server-renders the honest pre-fetch shell for adjutant', () => {
    const html = renderToStaticMarkup(<PublicAgentPage agentRef="adjutant" />)

    expect(html).toContain('data-route="public-agent"')
    expect(html).toContain('data-agent="adjutant"')
    expect(html).toContain('Public agent')
    expect(html).toContain('Autopilot')
    expect(html).toContain('Loading public goal.')
  })

  test('server-renders the honest pre-fetch shell for an arbitrary agent ref', () => {
    const html = renderToStaticMarkup(
      <PublicAgentPage agentRef="some-other-agent" />,
    )

    expect(html).toContain('data-agent="some-other-agent"')
    expect(html).toContain('some-other-agent')
    expect(html).toContain('Loading public goal.')
  })

  test('displayName mapping matches the Foldkit original', () => {
    expect(publicAgentDisplayName('artanis')).toBe('Artanis')
    expect(publicAgentDisplayName('adjutant')).toBe('Autopilot')
    expect(publicAgentDisplayName('some-other-agent')).toBe(
      'some-other-agent',
    )
  })
})
