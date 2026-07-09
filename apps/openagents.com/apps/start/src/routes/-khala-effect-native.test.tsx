import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  KhalaEffectNativePage,
  initialKhalaLandingState,
  khalaLandingView,
  khalaStateFromPublicSnapshot,
} from './-khala-effect-native-page'

describe('EN-4 /khala Effect Native route', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<KhalaEffectNativePage />)

    expect(html).toContain('data-route="khala"')
    expect(html).toContain('data-khala-effect-native-root=""')
    expect(html).not.toContain('OpenAI-compatible API')
  })

  test('authored content is a typed Effect Native tree', () => {
    const tree = khalaLandingView(initialKhalaLandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'khala-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v25"')
    expect(serialized).toContain('openagents/khala')
    expect(serialized).toContain('https://openagents.com/api/v1')
    expect(serialized).toContain('POST /api/keys/free')
    expect(serialized).toContain('Tokens Served')
    expect(serialized).not.toContain('className')
  })

  test('public tokens-served snapshot hydrates the counter', () => {
    const state = khalaStateFromPublicSnapshot({ tokensServed: 1234567 })

    expect(state.tokensServed).toBe('1,234,567')
  })

  test('missing/failed snapshot keeps the honest pending placeholder', () => {
    const state = khalaStateFromPublicSnapshot(null)

    expect(state.tokensServed).toBe('—')
  })

  test('source boundary uses Effect Native packages instead of direct DOM/JSX content authoring', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-khala-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).not.toContain('lucide-react')
  })
})
