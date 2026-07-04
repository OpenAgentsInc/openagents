import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { RunPage } from './-run-page'

describe('Start run route', () => {
  test('server-renders the retired Tassadar web-scene pointer', () => {
    const html = renderToStaticMarkup(<RunPage />)

    expect(html).toContain('data-route="tassadar"')
    expect(html).toContain('data-tassadar-scene="retired"')
    expect(html).toContain('Tassadar lives in the Verse')
    expect(html).toContain('Autopilot Desktop Verse')
    expect(html).toContain('href="/api/public/tassadar-run-summary"')
    expect(html).toContain('Public summary API')
    expect(html).toContain('href="/tassadar/replay/first-real-settlement"')
    expect(html).toContain('Proof replay')
    expect(html).not.toContain('data-persistent-scene-overlay="tassadar"')
    expect(html).not.toContain('oa-landing-squares')
  })
})
