import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeImageGenerationRoutes } from './image-generation-routes'

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

class EmptyR2Bucket {
  get(key: string): Promise<R2ObjectBody | null> {
    if (key !== 'generated-images/users/2026-06-04/test.png') {
      return Promise.resolve(null)
    }

    return Promise.resolve({
      body: new Blob(['png'], { type: 'image/png' }).stream(),
      httpEtag: '"test"',
      writeHttpMetadata: headers => headers.set('content-type', 'image/png'),
    } as R2ObjectBody)
  }

  put(): Promise<R2Object> {
    return Promise.resolve({ key: 'unused' } as R2Object)
  }
}

const routes = makeImageGenerationRoutes({
  appUrl: () => 'https://openagents.test',
  appendRefreshedSessionCookies: response => response,
  requireBrowserSession: request => {
    if (request.headers.get('authorization') === 'Bearer ok') {
      return Promise.resolve({ user: { userId: 'github:1' } })
    }

    if (request.headers.get('authorization') === 'Bearer nonoperator') {
      return Promise.resolve({ user: { userId: 'github:2' } })
    }

    return Promise.resolve(undefined)
  },
  requireOperatorAccess: (_env, session) =>
    Promise.resolve(session.user.userId === 'github:1'),
})

const env = {
  ARTIFACTS: new EmptyR2Bucket() as unknown as R2Bucket,
  GEMINI_API_KEY: 'test-key',
}

describe('image generation routes', () => {
  test('requires an authenticated session for generation', async () => {
    const effect = routes.routeImageGenerationRequest(
      new Request('https://openagents.com/api/images/generate', {
        body: JSON.stringify({ prompt: 'Create a room' }),
        method: 'POST',
      }),
      env,
      executionContext(),
    )

    expect(effect).not.toBeUndefined()

    const response = await Effect.runPromise(effect!)

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('requires operator access for generation', async () => {
    const effect = routes.routeImageGenerationRequest(
      new Request('https://openagents.com/api/images/generate', {
        body: JSON.stringify({ prompt: 'Create a room' }),
        headers: { authorization: 'Bearer nonoperator' },
        method: 'POST',
      }),
      env,
      executionContext(),
    )

    expect(effect).not.toBeUndefined()

    const response = await Effect.runPromise(effect!)

    expect(response.status).toBe(403)
  })

  test('serves generated image objects through authenticated stable URLs', async () => {
    const effect = routes.routeImageGenerationRequest(
      new Request(
        'https://openagents.com/api/images/generated-images%2Fusers%2F2026-06-04%2Ftest.png',
        {
          headers: { authorization: 'Bearer ok' },
        },
      ),
      env,
      executionContext(),
    )

    expect(effect).not.toBeUndefined()

    const response = await Effect.runPromise(effect!)

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe('image/png')
    expect(response.headers.get('cache-control')).toBe('private, max-age=3600')
  })
})
