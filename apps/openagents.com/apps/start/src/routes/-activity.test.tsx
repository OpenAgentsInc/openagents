import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ActivityPage } from './-activity-page'

describe('Start activity route', () => {
  test('server-renders the public activity timeline host and panes', () => {
    const html = renderToStaticMarkup(<ActivityPage />)

    expect(html).toContain('data-route="activity"')
    expect(html).toContain('oa-public-activity-timeline')
    expect(html).toContain('data-start-activity-timeline=""')
    expect(html).toContain('Live public activity')
    expect(html).toContain('Read-only public projection')
    expect(html).toContain('data-activity-source-lag=""')
    expect(html).toContain('data-activity-pane="fleet-map"')
    expect(html).toContain('data-activity-pane="active-tasks"')
    expect(html).toContain('data-activity-pane="fleet"')
    expect(html).toContain('data-activity-pane="money"')
    expect(html).toContain('data-activity-pane="forum"')
    expect(html).toContain('data-activity-pane="timeline"')
    expect(html).toContain('data-proof-drawer=""')
    expect(html).toContain('data-activity-filter="settle"')
    expect(html).toContain('data-activity-filter="forum"')
  })
})
