import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ExactRoute } from '../http/router'
import {
  DuplicateExactRoutePathError,
  makeExactRouteRegistry,
} from './exact-routes'

type TestEnv = Readonly<{
  marker: string
}>

const okRoute = (path: string): ExactRoute<TestEnv> => ({
  handler: () => Effect.succeed(new Response('ok')),
  path,
})

describe('Exact route registry', () => {
  test('preserves exact route order and handlers', () => {
    const routes = [okRoute('/alpha'), okRoute('/beta')]

    const registry = makeExactRouteRegistry(routes)

    expect(registry.paths).toEqual(['/alpha', '/beta'])
    expect(registry.routes).toBe(routes)
  })

  test('rejects duplicate exact route paths', () => {
    expect(() =>
      makeExactRouteRegistry([okRoute('/same'), okRoute('/same')]),
    ).toThrow(DuplicateExactRoutePathError)
    expect(() =>
      makeExactRouteRegistry([okRoute('/same'), okRoute('/same')]),
    ).toThrow('Duplicate exact route path: /same')
  })
})
