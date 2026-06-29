import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { routeExact, type ExactRoute } from './router'

describe('routeExact', () => {
  const env = {}
  const ctx = {} as ExecutionContext

  const route = (path: string, name: string): ExactRoute<typeof env> => ({
    handler: () => Effect.succeed(new Response(name)),
    path,
  })

  test('matches literal routes', async () => {
    const response = await Effect.runPromise(
      routeExact(
        [route('/v1/models', 'models')],
        '/v1/models',
        new Request('https://openagents.com/v1/models'),
        env,
        ctx,
      ) ?? Effect.succeed(new Response('missing', { status: 404 })),
    )

    expect(response.status).toBe(200)
    expect(await response.text()).toBe('models')
  })

  test('matches colon path segments without matching extra segments', async () => {
    const routes = [
      route('/v1/inference/batches/:jobId/results', 'results'),
      route('/v1/inference/batches/:jobId', 'status'),
    ]

    const results = await Effect.runPromise(
      routeExact(
        routes,
        '/v1/inference/batches/batch_123/results',
        new Request(
          'https://openagents.com/v1/inference/batches/batch_123/results',
        ),
        env,
        ctx,
      ) ?? Effect.succeed(new Response('missing', { status: 404 })),
    )
    const status = await Effect.runPromise(
      routeExact(
        routes,
        '/v1/inference/batches/batch_123',
        new Request('https://openagents.com/v1/inference/batches/batch_123'),
        env,
        ctx,
      ) ?? Effect.succeed(new Response('missing', { status: 404 })),
    )
    const missing = routeExact(
      routes,
      '/v1/inference/batches/batch_123/results/extra',
      new Request(
        'https://openagents.com/v1/inference/batches/batch_123/results/extra',
      ),
      env,
      ctx,
    )

    expect(await results.text()).toBe('results')
    expect(await status.text()).toBe('status')
    expect(missing).toBeUndefined()
  })
})
