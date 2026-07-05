import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ArtanisConsolePage } from './-artanis-console-page'

describe('Start /artanis route', () => {
  test('server-renders the console masthead and campaign objective', () => {
    const html = renderToStaticMarkup(<ArtanisConsolePage />)

    expect(html).toContain('data-route="artanis"')
    expect(html).toContain('data-agent="artanis"')
    expect(html).toContain('ARTANIS console')
    expect(html).toContain('LIVE')
    expect(html).toContain('No public goal')
    expect(html).toContain('Active slots loading')
    expect(html).toContain('no active public run')
    expect(html).toContain('Campaign objective')
    expect(html).toContain(
      'Release the next version of Pylon, connect it deeply to Omega',
    )
    expect(html).toContain('Awaiting the first public durable Artanis goal.')
  })

  test('renders the always-static fleet map, task board, and virtual merge queue', () => {
    const html = renderToStaticMarkup(<ArtanisConsolePage />)

    expect(html).toContain('data-component="artanis-fleet-map-task-board"')
    expect(html).toContain('Pylons, slots, active tasks')
    expect(html).toContain('data-fleet-map-slot="empty"')
    expect(html).toContain('no public heartbeat')
    expect(html).toContain('Active Task Board')
    expect(html).toContain('Ready')
    expect(html).toContain('Claimed')
    expect(html).toContain('Verifying')
    expect(html).toContain('Resolved')
    expect(html).toContain('No public rows in this lane.')
    expect(html).toContain('data-component="artanis-virtual-merge-queue"')
    expect(html).toContain('Projected branch base for parallel agents')
    expect(html).toContain('Actual head')
    expect(html).toContain('Virtual head')
    expect(html).toContain('Next branch base')
    expect(html).toContain('Conflict lane')
    expect(html).toContain('24 accepted / 0 conflicts')
  })

  test('renders the static fleet-onboarding panel with the connect commands', () => {
    const html = renderToStaticMarkup(<ArtanisConsolePage />)

    expect(html).toContain('Have Codex or Claude? Join the fleet.')
    expect(html).toContain('npm install -g @openagentsinc/khala')
    expect(html).toContain('khala fleet connect')
    expect(html).toContain('khala fleet status')
    expect(html).toContain('href="/docs/connect-codex-fleet"')
  })

  test('renders the honest pre-fetch empty states for the live-model panels', () => {
    const html = renderToStaticMarkup(<ArtanisConsolePage />)

    expect(html).toContain('Token pace unavailable.')
    expect(html).toContain('What the fleet is doing now')
    expect(html).toContain('Loading live fleet activity.')
    expect(html).toContain('Omega Pylon stats')
    expect(html).toContain('Feed loading')
    expect(html).toContain('Loading recent Pylon presence.')
    expect(html).not.toContain('accountRef')
    expect(html).not.toContain('raw prompt')
    expect(html).not.toContain('/Users/')
  })
})
