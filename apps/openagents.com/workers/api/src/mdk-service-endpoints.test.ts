import { describe, expect, it } from 'vitest'

import {
  MDK_SIDECAR_SERVICE_TOKEN_HEADER,
  makeMdkServiceHttpPathFetch,
  makeMdkSidecarHttpRequestForward,
  mdkServiceHttpBaseUrl,
} from './mdk-service-endpoints'

describe('mdkServiceHttpBaseUrl', () => {
  it('accepts an https URL and strips trailing slashes', () => {
    expect(
      mdkServiceHttpBaseUrl('https://mdk-treasury-abc.a.run.app/'),
    ).toBe('https://mdk-treasury-abc.a.run.app')
    expect(mdkServiceHttpBaseUrl('https://example.com/base///')).toBe(
      'https://example.com/base',
    )
  })

  it('accepts plain http only for loopback hosts', () => {
    expect(mdkServiceHttpBaseUrl('http://localhost:8080')).toBe(
      'http://localhost:8080',
    )
    expect(mdkServiceHttpBaseUrl('http://127.0.0.1:8080')).toBe(
      'http://127.0.0.1:8080',
    )
    expect(mdkServiceHttpBaseUrl('http://example.com')).toBeUndefined()
  })

  it('rejects missing, blank, and malformed values', () => {
    expect(mdkServiceHttpBaseUrl(undefined)).toBeUndefined()
    expect(mdkServiceHttpBaseUrl('')).toBeUndefined()
    expect(mdkServiceHttpBaseUrl('   ')).toBeUndefined()
    expect(mdkServiceHttpBaseUrl('not-a-url')).toBeUndefined()
    expect(mdkServiceHttpBaseUrl('ftp://example.com')).toBeUndefined()
  })
})

describe('makeMdkServiceHttpPathFetch', () => {
  it('sends the path against the base URL with the service token header', async () => {
    const seen: Request[] = []
    const fetchTreasury = makeMdkServiceHttpPathFetch({
      baseUrl: 'https://mdk-treasury-abc.a.run.app',
      fetchImpl: async input => {
        seen.push(input as Request)
        return new Response(JSON.stringify({ ok: true }), { status: 200 })
      },
      serviceToken: 'token-123',
      serviceTokenHeader: 'x-treasury-service-token',
    })

    const response = await fetchTreasury('/spark/balance')

    expect(response.status).toBe(200)
    expect(seen).toHaveLength(1)
    const request = seen[0]!
    expect(request.url).toBe('https://mdk-treasury-abc.a.run.app/spark/balance')
    expect(request.method).toBe('GET')
    expect(request.headers.get('x-treasury-service-token')).toBe('token-123')
    expect(request.headers.get('content-type')).toBe('application/json')
  })

  it('forwards POST bodies and omits the token header when unset', async () => {
    const seen: Request[] = []
    const fetchTreasury = makeMdkServiceHttpPathFetch({
      baseUrl: 'https://mdk-treasury-abc.a.run.app',
      fetchImpl: async input => {
        seen.push(input as Request)
        return new Response('{}', { status: 200 })
      },
      serviceToken: undefined,
      serviceTokenHeader: 'x-treasury-service-token',
    })

    await fetchTreasury('/pay', {
      body: JSON.stringify({ amountSat: 21 }),
      method: 'POST',
    })

    const request = seen[0]!
    expect(request.method).toBe('POST')
    expect(await request.text()).toBe(JSON.stringify({ amountSat: 21 }))
    expect(request.headers.get('x-treasury-service-token')).toBeNull()
  })
})

describe('makeMdkSidecarHttpRequestForward', () => {
  it('forwards path, query, method, body, and adds the sidecar token', async () => {
    const seen: Request[] = []
    const forward = makeMdkSidecarHttpRequestForward({
      baseUrl: 'https://mdk-sidecar-abc.a.run.app',
      fetchImpl: async input => {
        seen.push(input as Request)
        return new Response('{}', { status: 200 })
      },
      serviceToken: 'sidecar-token',
    })

    await forward(
      new Request('https://openagents.com/api/mdk?session=abc', {
        body: JSON.stringify({ kind: 'checkout' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )

    const request = seen[0]!
    expect(request.url).toBe('https://mdk-sidecar-abc.a.run.app/api/mdk?session=abc')
    expect(request.method).toBe('POST')
    expect(request.headers.get(MDK_SIDECAR_SERVICE_TOKEN_HEADER)).toBe(
      'sidecar-token',
    )
    expect(request.headers.get('content-type')).toBe('application/json')
    expect(await request.text()).toBe(JSON.stringify({ kind: 'checkout' }))
  })

  it('forwards GET requests without a body and without the token when unset', async () => {
    const seen: Request[] = []
    const forward = makeMdkSidecarHttpRequestForward({
      baseUrl: 'https://mdk-sidecar-abc.a.run.app',
      fetchImpl: async input => {
        seen.push(input as Request)
        return new Response('{}', { status: 200 })
      },
      serviceToken: undefined,
    })

    await forward(new Request('https://openagents.com/api/mdk'))

    const request = seen[0]!
    expect(request.url).toBe('https://mdk-sidecar-abc.a.run.app/api/mdk')
    expect(request.method).toBe('GET')
    expect(request.body).toBeNull()
    expect(request.headers.get(MDK_SIDECAR_SERVICE_TOKEN_HEADER)).toBeNull()
  })
})
