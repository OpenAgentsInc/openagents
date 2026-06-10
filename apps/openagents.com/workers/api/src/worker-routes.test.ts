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

  test('redirects unknown direct browser document paths to the homepage', () => {
    expect(
      shouldRedirectUnknownDocumentToHome(
        requestFor('/not-a-real-page'),
        '/not-a-real-page',
      ),
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
