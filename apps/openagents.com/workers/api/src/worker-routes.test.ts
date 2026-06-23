import { describe, expect, test } from 'vitest'

import { shouldRedirectUnknownDocumentToHome } from './worker-routes'

const requestFor = (pathname: string, init: RequestInit = {}) =>
  new Request(`https://openagents.com${pathname}`, {
    headers: { accept: 'text/html', ...(init.headers ?? {}) },
    method: init.method ?? 'GET',
  })

describe('Worker document route fallback', () => {
  test('keeps the product promises document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/promises'), '/promises'),
    ).toBe(false)
  })

  test('keeps public training run document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/tassadar'), '/tassadar'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/tassadar/replay/first-real-settlement'),
        '/tassadar/replay/first-real-settlement',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor(
          '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
        ),
        '/tassadar/replay/first-real-settlement',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/training/runs'),
        '/training/runs',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/training/runs/run.cs336.a1.demo'),
        '/training/runs/run.cs336.a1.demo',
      ),
    ).toBe(false)
  })

  test('keeps the components gallery document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/components'),
        '/components',
      ),
    ).toBe(false)
  })

  test('keeps Forge and business funnel document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/business'), '/business'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/forge'), '/forge'),
    ).toBe(false)
  })

  test('keeps the Moksha document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/moksha'), '/moksha'),
    ).toBe(false)
  })

  test('keeps the OpenAgents Moksha document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/moksha2'), '/moksha2'),
    ).toBe(false)
  })

  test('keeps the standalone landing document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/landing'), '/landing'),
    ).toBe(false)
  })

  test('keeps the public Khala document route in the app shell when unauthed', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/khala'), '/khala'),
    ).toBe(false)
  })

  test('keeps the Pylon document route in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/pylon'), '/pylon'),
    ).toBe(false)
  })

  test('serves the public legal document routes in the app shell for unauthed visitors', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/terms'), '/terms'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/privacy'), '/privacy'),
    ).toBe(false)
  })

  test('keeps public stats document routes in the app shell', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/stats'), '/stats'),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/stats-old'),
        '/stats-old',
      ),
    ).toBe(false)
  })

  test('keeps forum thread document routes in the app shell for crawlers', () => {
    // `/forum/t/{id}` must reach the social-preview handler, not the
    // unknown-document redirect, so the injected OG/Twitter meta is served.
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/forum/t/55555555-5555-4555-8555-555555555555'),
        '/forum/t/55555555-5555-4555-8555-555555555555',
      ),
    ).toBe(false)
  })

  test('does not redirect the forum OG image route (treated as a file)', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/og/forum/55555555-5555-4555-8555-555555555555.svg'),
        '/og/forum/55555555-5555-4555-8555-555555555555.svg',
      ),
    ).toBe(false)
  })

  test('redirects unknown direct browser document paths to the homepage', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/not-a-real-page'),
        '/not-a-real-page',
      ),
    ).toBe(true)
    expect(
      shouldRedirectUnknownDocumentToHome(requestFor('/live'), '/live'),
    ).toBe(true)
  })

  test('does not redirect API, asset, or file-like requests', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/api/unknown'),
        '/api/unknown',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/assets/missing.js'),
        '/assets/missing.js',
      ),
    ).toBe(false)
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/favicon.ico'),
        '/favicon.ico',
      ),
    ).toBe(false)
  })
})
