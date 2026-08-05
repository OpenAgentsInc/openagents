import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act } from 'react'
import { type Root, createRoot } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import { KhalaPage, khalaStateFromPublicSnapshot } from './-khala-page'
import { KHALA_TOKENS_SERVED_URL } from './-sales-landing-data'

;(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true

const originalFetch = globalThis.fetch

const stubFetchWith = (handler: (url: string) => Promise<Response>): void => {
  globalThis.fetch = ((input: RequestInfo | URL) =>
    handler(String(input))) as unknown as typeof fetch
}

let container: HTMLDivElement | null = null
let root: Root | null = null

const mountKhalaPage = async (): Promise<HTMLDivElement> => {
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
  await act(async () => {
    root?.render(<KhalaPage />)
  })
  // Flush the fetch -> json -> setState microtask chain.
  await act(async () => {})
  await act(async () => {})
  return container
}

beforeEach(() => {
  container = null
  root = null
})

afterEach(async () => {
  if (root !== null) {
    await act(async () => {
      root?.unmount()
    })
  }
  container?.remove()
  globalThis.fetch = originalFetch
})

describe('Start /khala route', () => {
  test('server-renders the landing content instead of a mount shim', () => {
    const html = renderToStaticMarkup(<KhalaPage />)

    expect(html).toContain('data-route="khala"')
    expect(html).toContain('aria-label="Khala - OpenAgents inference"')
    expect(html).toContain('OpenAgents inference')
    expect(html).toMatch(/<h1[^>]*>Khala<\/h1>/)
    expect(html).toContain(
      'Khala is the OpenAgents inference and work rail: an OpenAI-compatible API for public model access, work receipts, and agent-readable evidence. This public page keeps the usable API basics visible without claiming paid capacity is generally live.',
    )
  })

  test('renders the API basics, the counter copy, and both actions', () => {
    const html = renderToStaticMarkup(<KhalaPage />)

    expect(html).toContain('Model')
    expect(html).toContain('openagents/khala')
    expect(html).toContain('Base URL')
    expect(html).toContain('https://openagents.com/api/v1')
    expect(html).toContain('Free key')
    expect(html).toContain('POST /api/keys/free')
    expect(html).toContain('Tokens Served')
    expect(html).toContain(
      'The live counter is hydrated by the production API on the live app. This route preserves the same live projection for the route-by-route migration.',
    )
    expect(html).toContain('href="/"')
    expect(html).toContain('← OpenAgents')
    expect(html).toContain('href="/docs/openagents"')
    expect(html).toContain('Read the overview')
    expect(html).toContain('href="/khala/chat-sync"')
    expect(html).toContain('Open web chat sync')
  })

  test('server render shows the honest pending placeholder, never a fabricated count', () => {
    const html = renderToStaticMarkup(<KhalaPage />)

    expect(html).toContain('—')
    // No grouped-thousands number may appear before the projection resolves.
    expect(html).not.toMatch(/\d{1,3}(,\d{3})+/)
  })

  test('public tokens-served snapshot hydrates the counter', async () => {
    const requested: string[] = []
    stubFetchWith(async url => {
      requested.push(url)
      return {
        json: async () => ({ tokensServed: 1234567 }),
        ok: true,
        status: 200,
      } as unknown as Response
    })

    const mounted = await mountKhalaPage()

    expect(requested).toContain(KHALA_TOKENS_SERVED_URL)
    expect(mounted.textContent).toContain('1,234,567')
  })

  test('a failed projection fetch keeps the pending placeholder', async () => {
    stubFetchWith(async () => {
      throw new Error('offline')
    })

    const mounted = await mountKhalaPage()

    expect(mounted.textContent).toContain('—')
    expect(mounted.textContent).not.toMatch(/\d{1,3}(,\d{3})+/)
  })

  test('snapshot formatting is fail-soft at the boundary too', () => {
    expect(khalaStateFromPublicSnapshot({ tokensServed: 1234567 })).toEqual({
      tokensServed: '1,234,567',
    })
    expect(khalaStateFromPublicSnapshot(null)).toEqual({ tokensServed: '—' })
  })

  test('the page module carries no Effect Native dependency', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-khala-page.tsx'),
      'utf8',
    )

    expect(source).not.toContain('@effect-native')
    expect(source).not.toContain('lucide-react')
    expect(source).toContain("from './-sales-landing-data'")
  })

  test('the route shell mounts the plain-React khala page', () => {
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/khala/index.tsx'),
      'utf8',
    )

    expect(routeSource).toContain("from '../-khala-page'")
    expect(routeSource).toContain('KhalaPage')
    expect(routeSource).not.toContain('effect-native')
  })
})
