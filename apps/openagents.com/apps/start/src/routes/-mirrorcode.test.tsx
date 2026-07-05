import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { MirrorCodePage } from './-mirrorcode-page'

describe('Start /mirrorcode route', () => {
  test('server-renders the route contract and hero copy', () => {
    const html = renderToStaticMarkup(<MirrorCodePage />)

    expect(html).toContain('data-route="mirrorcode"')
    expect(html).toContain('MirrorCode, powered by Khala')
    expect(html).toContain('public tasks only')
    expect(html).toContain('data-mirrorcode-no-spend-banner=""')
    expect(html).toContain('Live data only / public tasks only')
  })

  test('renders the always-static playground contract', () => {
    const html = renderToStaticMarkup(<MirrorCodePage />)

    expect(html).toContain('data-mirrorcode-playground-panel=""')
    expect(html).toContain('MirrorCode-as-a-Service playground')
    expect(html).toContain('POST /api/gym/mirrorcode/runs')
    expect(html).toContain('GET /api/gym/mirrorcode/runs/{runId}')
    expect(html).toContain('data-mirrorcode-owner-gated-launch=""')
    expect(html).toContain('public visitors can inspect the contract')
  })

  test('renders the honest empty state instead of fabricated run rows', () => {
    const html = renderToStaticMarkup(<MirrorCodePage />)

    expect(html).toContain('data-mirrorcode-live-empty=""')
    expect(html).toContain(
      'No runs yet — machinery shipped, awaiting first Phase-0 run',
    )
    expect(html).toContain('data-mirrorcode-execution-empty=""')
    expect(html).toContain('No execution rows to visualize yet')
    expect(html).toContain('/api/gym/mirrorcode/runs')
  })
})
