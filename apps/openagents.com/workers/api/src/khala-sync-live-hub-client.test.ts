// LiveHub HTTP client tests (CFG-5, #8520): the KhalaSyncHubNamespaceLike
// adapter that points existing hub consumers at the Cloud Run LiveHub
// service, and the config-driven LiveHub-or-DO resolver.

import { describe, expect, test } from 'vitest'

import {
  makeKhalaSyncLiveHubNamespace,
  resolveKhalaSyncHubNamespace,
} from './khala-sync-live-hub-client'

const capture = () => {
  const requests: Array<Request> = []
  const fetchImpl = async (request: Request) => {
    requests.push(request)
    return Response.json({ ok: true })
  }
  return { requests, fetchImpl }
}

describe('makeKhalaSyncLiveHubNamespace', () => {
  test('rewrites the internal hub URL onto the LiveHub base, preserving path and query', async () => {
    const { requests, fetchImpl } = capture()
    const namespace = makeKhalaSyncLiveHubNamespace({
      baseUrl: 'https://live-hub.example.run.app/',
      token: 'service-token',
      fetchImpl,
    })
    const stub = namespace.get(namespace.idFromName('scope.thread.t1'))

    await stub.fetch(
      new Request(
        'https://khala-sync-hub.openagents.internal/log?scope=scope.thread.t1&cursor=7&limit=100',
      ),
    )

    expect(requests).toHaveLength(1)
    const url = new URL(requests[0]!.url)
    expect(url.origin).toBe('https://live-hub.example.run.app')
    expect(url.pathname).toBe('/log')
    expect(url.searchParams.get('scope')).toBe('scope.thread.t1')
    expect(url.searchParams.get('cursor')).toBe('7')
    expect(url.searchParams.get('limit')).toBe('100')
  })

  test('the shared service bearer ALWAYS replaces any inbound Authorization (a promoted end-user bearer never travels to LiveHub)', async () => {
    const { requests, fetchImpl } = capture()
    const namespace = makeKhalaSyncLiveHubNamespace({
      baseUrl: 'https://live-hub.example.run.app',
      token: 'service-token',
      fetchImpl,
    })
    const stub = namespace.get(namespace.idFromName('scope.thread.t1'))

    await stub.fetch(
      new Request(
        'https://khala-sync-hub.openagents.internal/connect?scope=scope.thread.t1&cursor=0',
        {
          headers: {
            authorization: 'Bearer end-user-token',
            upgrade: 'websocket',
          },
        },
      ),
    )

    expect(requests[0]!.headers.get('authorization')).toBe(
      'Bearer service-token',
    )
    // The WS proxy contract: the Upgrade header survives the rewrite.
    expect(requests[0]!.headers.get('upgrade')).toBe('websocket')
  })

  test('forwards method and body for append/access-changed POSTs', async () => {
    const { requests, fetchImpl } = capture()
    const namespace = makeKhalaSyncLiveHubNamespace({
      baseUrl: 'https://live-hub.example.run.app',
      token: 'service-token',
      fetchImpl,
    })
    const stub = namespace.get(namespace.idFromName('scope.thread.t1'))

    await stub.fetch(
      new Request('https://khala-sync-hub.openagents.internal/append', {
        body: JSON.stringify({ entries: [1], scope: 'scope.thread.t1' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
    )

    expect(requests[0]!.method).toBe('POST')
    expect(await requests[0]!.json()).toEqual({
      entries: [1],
      scope: 'scope.thread.t1',
    })
  })
})

describe('resolveKhalaSyncHubNamespace', () => {
  const doBinding = { idFromName: () => 'do', get: () => ({ fetch: async () => Response.json({}) }) }

  test('prefers the LiveHub adapter when URL and token are BOTH set', () => {
    const namespace = resolveKhalaSyncHubNamespace({
      KHALA_SYNC_LIVE_HUB_URL: 'https://live-hub.example.run.app',
      KHALA_SYNC_LIVE_HUB_TOKEN: 'service-token',
      KHALA_SYNC_HUB: doBinding,
    })
    expect(namespace).toBeDefined()
    // The LiveHub adapter's idFromName is the identity on scopes — the DO
    // fake above returns 'do', so this distinguishes the two.
    expect(namespace!.idFromName('scope.thread.t1')).toBe('scope.thread.t1')
  })

  test('falls back to the DO binding when either half is missing/blank', () => {
    for (const env of [
      { KHALA_SYNC_HUB: doBinding },
      { KHALA_SYNC_LIVE_HUB_URL: 'https://x', KHALA_SYNC_HUB: doBinding },
      { KHALA_SYNC_LIVE_HUB_TOKEN: 't', KHALA_SYNC_HUB: doBinding },
      {
        KHALA_SYNC_LIVE_HUB_URL: '  ',
        KHALA_SYNC_LIVE_HUB_TOKEN: 't',
        KHALA_SYNC_HUB: doBinding,
      },
    ]) {
      const namespace = resolveKhalaSyncHubNamespace(env)
      expect(namespace).toBe(doBinding as never)
    }
  })

  test('undefined when nothing is configured (routes keep their honest hub-unconfigured paths)', () => {
    expect(resolveKhalaSyncHubNamespace({})).toBeUndefined()
  })
})
