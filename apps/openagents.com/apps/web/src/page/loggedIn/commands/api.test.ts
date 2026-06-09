import { Effect, Schema as S } from 'effect'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { errorMessageFromUnknown, requestBlob, requestJson } from './api'

const TestResponse = S.Struct({
  message: S.String,
  ok: S.Boolean,
})

const jsonResponse = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    ...init,
  })

describe('logged-in API command helpers', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('decodes successful JSON responses with the provided schema', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse({ message: 'ready', ok: true }),
    )

    const response = await Effect.runPromise(
      requestJson({
        init: { credentials: 'include' },
        name: 'test.requestJson.success',
        request: '/api/test',
        schema: TestResponse,
      }),
    )

    expect(response).toEqual({ message: 'ready', ok: true })
    expect(globalThis.fetch).toHaveBeenCalledWith('/api/test', {
      credentials: 'include',
    })
  })

  test('preserves API error messages from failed JSON responses', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse({ error: 'Connect a ChatGPT account.' }, { status: 401 }),
    )

    let caught: unknown

    try {
      await Effect.runPromise(
        requestJson({
          name: 'test.requestJson.failure',
          request: '/api/test',
          schema: TestResponse,
        }),
      )
    } catch (error) {
      caught = error
    }

    expect(errorMessageFromUnknown(caught)).toBe('Connect a ChatGPT account.')
  })

  test('downloads successful blob responses', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(new Blob(['hello'], { type: 'text/plain' })),
    )

    const blob = await Effect.runPromise(
      requestBlob({
        init: { headers: { accept: '*/*' } },
        name: 'test.requestBlob.success',
        request: '/api/thread-files/file-id/download',
      }),
    )

    expect(await blob.text()).toBe('hello')
  })
})
