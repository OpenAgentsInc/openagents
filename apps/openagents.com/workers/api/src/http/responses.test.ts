import { describe, expect, test } from 'vitest'

import {
  decorateJsonHttpResultHeaders,
  materializeHttpResult,
  methodNotAllowedResult,
  noStoreJsonResult,
  responseInitForHttpResult,
  streamHttpResult,
} from './responses'

describe('typed HTTP results', () => {
  test('materializes JSON status, body, and required headers at the edge', async () => {
    const response = materializeHttpResult(
      noStoreJsonResult(
        { ok: true },
        { status: 202, headers: { 'x-route-ref': 'route.test.v1' } },
      ),
    )

    expect(response.status).toBe(202)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('content-type')).toBe('application/json')
    expect(response.headers.get('x-route-ref')).toBe('route.test.v1')
    expect(await response.json()).toEqual({ ok: true })
  })

  test('preserves Allow and repeated refreshed-session cookies', () => {
    const result = decorateJsonHttpResultHeaders(
      methodNotAllowedResult(['GET', 'POST']),
      headers => {
        headers.append('set-cookie', 'oa_access=refreshed; Secure; HttpOnly')
        headers.append('set-cookie', 'oa_refresh=refreshed; Secure; HttpOnly')
      },
    )
    const response = materializeHttpResult(result)

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET, POST')
    expect(response.headers.getSetCookie()).toEqual([
      'oa_access=refreshed; Secure; HttpOnly',
      'oa_refresh=refreshed; Secure; HttpOnly',
    ])
  })

  test('materializes an upstream stream without buffering it in the route', async () => {
    const response = materializeHttpResult(
      streamHttpResult(new Blob(['data: accepted\n\n']).stream(), {
        status: 206,
        statusText: 'Partial Content',
        headers: {
          'cache-control': 'no-store',
          'content-type': 'text/event-stream',
        },
      }),
    )

    expect(response.status).toBe(206)
    expect(response.statusText).toBe('Partial Content')
    expect(response.headers.get('content-type')).toBe('text/event-stream')
    expect(await response.text()).toBe('data: accepted\n\n')
  })

  test('preserves the complete Workers ResponseInit at the legacy constructor boundary', () => {
    const workerInit: ResponseInit = {
      cf: { cacheEverything: true, cacheTtl: 60 },
      encodeBody: 'manual',
      headers: { 'x-worker-init': 'preserved' },
      status: 207,
      statusText: 'Multi-Status',
      webSocket: null,
    }
    const result = noStoreJsonResult({ ok: true }, workerInit)
    const constructorInit = responseInitForHttpResult(result, workerInit)
    const headers = new Headers(constructorInit.headers)

    expect(constructorInit.cf).toEqual({
      cacheEverything: true,
      cacheTtl: 60,
    })
    expect(constructorInit.encodeBody).toBe('manual')
    expect(Object.hasOwn(constructorInit, 'webSocket')).toBe(true)
    expect(constructorInit.webSocket).toBeNull()
    expect(constructorInit.status).toBe(207)
    expect(constructorInit.statusText).toBe('Multi-Status')
    expect(headers.get('cache-control')).toBe('no-store')
    expect(headers.get('content-type')).toBe('application/json')
    expect(headers.get('x-worker-init')).toBe('preserved')
  })
})
