import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  ClientsPreviewPage,
  protocolDecisionFixture,
  protocolSessionFixtures,
} from './-clients-preview-page'

describe('Start clients preview route', () => {
  test('server-renders autopilot sessions and the pending decision fixture', () => {
    const html = renderToStaticMarkup(<ClientsPreviewPage />)

    expect(html).toContain('data-route="clients-preview"')
    expect(html).toContain('Clients preview')
    expect(html).toContain('Autopilot control surface')
    expect(html).toContain('Sessions')
    expect(html).toContain('Decision')
    expect(html).toContain('data-autopilot-session-list=""')
    expect(html).toContain(protocolSessionFixtures[0]?.sessionRef)
    expect(html).toContain(protocolSessionFixtures[1]?.sessionRef)
    expect(html).toContain(
      `data-autopilot-decision-id="${protocolDecisionFixture.requestId}"`,
    )
    expect(html).toContain(protocolDecisionFixture.actionRef)
    expect(html).toContain('data-autopilot-decision-action="approve"')
    expect(html).toContain('data-autopilot-decision-action="deny"')
    expect(html).toContain('data-autopilot-decision-action="answer"')
  })
})
