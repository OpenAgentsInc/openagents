import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { StatsPage } from './-stats-page'

describe('Start /stats route (public/anonymous variant)', () => {
  test('server-renders the route contract and hero copy', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('data-route="stats"')
    expect(html).toContain('Network Stats')
    expect(html).toContain(
      'Live public-safe evidence: receipt-backed counters, launch gates, and claim boundaries. No dummy values; missing evidence is marked unavailable.',
    )
  })

  test('keeps the tokens-served counter anchor and honest idle placeholder', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('data-counter="khala-tokens-served"')
    expect(html).toContain('data-counter-display="khala-tokens-served"')
    expect(html).toContain('data-value="—"')
    expect(html).toContain('Tokens Served')
  })

  test('renders the history chart and both mix panels in their honest idle state', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('data-chart="khala-tokens-served-history"')
    expect(html).toContain('Tokens Served / Day')
    expect(html).toContain('Waiting for data…')
    expect(html).toContain('Model Family Mix')
    expect(html).toContain('Waiting for model mix…')
    expect(html).toContain('Channel Mix')
    expect(html).toContain('Waiting for channel mix…')
    expect(html).toContain('Daily')
    expect(html).toContain('Cumulative')
  })

  test('renders the honest idle state for Pylon Stats, Forum Stats, and Accounting Strip instead of fabricated numbers', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('Pylon Stats')
    expect(html).toContain('Forum Stats')
    expect(html).toContain('Accounting Strip')
    expect(html).toContain('Heartbeat freshness unavailable.')
    expect((html.match(/Unavailable/g) ?? []).length).toBeGreaterThan(5)
  })

  test('preserves the Claim Boundary and Endpoint Manifest panels verbatim', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('Claim Boundary')
    expect(html).toContain('Public copy boundaries for money and earning claims.')
    expect(html).toContain('Bounded')
    expect(html).toContain('Endpoint Manifest')
    expect(html).toContain('/.well-known/openagents.json')
    expect(html).toContain('/api/openapi.json')
    expect(html).toContain('/api/public/pylon-stats')
    expect(html).toContain('/api/forum/tip-leaderboards')
    expect(html).toContain('/api/forum/launch-status')
    expect(html).toContain('/api/public/adjutant/activity')
  })

  test('preserves the Nostr Relay Configuration panel copy', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('Nostr Relay Configuration')
    expect(html).toContain('No relay endpoint list is public in the current response.')
  })

  test('keeps the home link', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('href="/"')
  })
})
