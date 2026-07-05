import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { CodePage } from './-code-page'

describe('Start /code route', () => {
  test('server-renders the Khala Code landing surface with the scene and headline', () => {
    const html = renderToStaticMarkup(<CodePage />)

    expect(html).toContain('data-route="code"')
    expect(html).toContain('data-pose="khala"')
    expect(html).toContain('Khala Code')
    expect(html).toContain('Code, on your own capacity')
    expect(html).toContain('model: openagents/khala')
    expect(html).toContain(
      'A coding agent that reads your repo, makes the edit, runs the',
    )
  })

  test('renders the representative user/assistant chat turns with tool, diff, and code anatomy', () => {
    const html = renderToStaticMarkup(<CodePage />)

    expect(html).toContain('data-chat-turn="user"')
    expect(html).toContain('data-chat-turn="assistant"')
    expect(html).toContain(
      'Refactor',
    )
    expect(html).toContain('greet')
    expect(html).toContain('data-ai-tool="read_file"')
    expect(html).toContain('data-ai-diff=""')
    expect(html).toContain('data-ai-code-block="src/greet.ts"')
    expect(html).toContain('bun test')
    expect(html).toContain('6 passed')
    expect(html).toContain('data-ai-tool="cargo test"')
  })

  test('keeps the composer decorative and public, with no live send action', () => {
    const html = renderToStaticMarkup(<CodePage />)

    expect(html).toContain('data-chat-composer="khala-code"')
    expect(html).toContain('Ask Khala to change your code')
  })
})
