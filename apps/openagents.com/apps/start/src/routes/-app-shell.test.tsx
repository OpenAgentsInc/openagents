import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { KhalaInfoPage, TassadarInfoPage } from './-app-shell-routes'

describe('Start app-shell migrated routes', () => {
  test('server-renders the migrated Khala API instructions route', () => {
    const html = renderToStaticMarkup(<KhalaInfoPage />)

    expect(html).toContain('data-route="khala"')
    expect(html).toContain('data-pose="khala"')
    expect(html).toContain('data-khala-instructions=""')
    expect(html).toContain('openagents/khala')
    expect(html).toContain('https://openagents.com/api/v1')
    expect(html).toContain('POST /api/keys/free')
    expect(html).toContain('data-counter="khala-tokens-served"')
    expect(html).toContain('Tokens Served')
    expect(html).toContain('data-khala-back="home"')
    expect(html).not.toContain('data-khala-chat-composer')
  })

  test('server-renders the migrated Tassadar route with agent instructions', () => {
    const html = renderToStaticMarkup(<TassadarInfoPage />)

    expect(html).toContain('data-route="tassadar"')
    expect(html).toContain('data-pose="tassadar"')
    expect(html).toContain('data-tassadar-copy="agent-instructions"')
    expect(html).toContain('Copy Agent Instructions')
    expect(html).toContain(
      'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
    )
    expect(html).toContain('pylon training claim --base-url https://openagents.com')
    expect(html).toContain('data-tassadar-back="home"')
    expect(html).not.toContain('data-tassadar-scene="retired"')
  })
})
