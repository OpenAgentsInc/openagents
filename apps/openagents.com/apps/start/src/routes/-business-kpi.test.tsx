import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { BusinessKpiPage } from './-business-kpi-page'

describe('Start business KPI route', () => {
  test('server-renders baseline, live metrics, evidence, and privacy boundaries', () => {
    const html = renderToStaticMarkup(
      <BusinessKpiPage engagementRef="engagement.public.vertical_pipeline_1" />,
    )

    expect(html).toContain('data-route="business-kpi"')
    expect(html).toContain(
      'data-business-kpi-dashboard="engagement.public.vertical_pipeline_1"',
    )
    expect(html).toContain('Scorekeeper')
    expect(html).toContain('Baseline snapshot and live engagement metrics')
    expect(html).toContain('data-business-kpi-metric="lead_volume"')
    expect(html).toContain('data-business-kpi-metric="conversion"')
    expect(html).toContain('data-business-kpi-metric="aov"')
    expect(html).toContain('data-business-kpi-metric="revenue"')
    expect(html).toContain('data-business-kpi-metric="consult_attach"')
    expect(html).toContain('Baseline')
    expect(html).toContain('Current')
    expect(html).toContain('Delta')
    expect(html).toContain('/api/public/business/funnel-dashboard')
    expect(html).toContain('table:business_funnel_events')
    expect(html).toContain('issue:8105')
    expect(html).toContain('roadmap:BF-7.1')
    expect(html).toContain('Excluded: client name, contact email, phone')
    expect(html).toContain('settlement and payout claims')
    expect(html).not.toContain('customer@example.com')
    expect(html).not.toContain('555-')
  })
})
