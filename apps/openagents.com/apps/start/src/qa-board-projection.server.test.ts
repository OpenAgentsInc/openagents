import { describe, expect, test } from 'vitest'

import {
  buildQaBoardProjection,
  routeQaBoardRequest,
} from './qa-board-projection.server'

const githubFetch = ((input: RequestInfo | URL) => {
  const url = String(input)
  if (!url.endsWith('/issues/8912'))
    throw new Error(`unexpected issue lookup: ${url}`)
  return Promise.resolve(Response.json({ state: 'open' }))
}) as typeof fetch

describe('QA board server projection', () => {
  test('normalizes the latest durable QA-1 and QA-2 artifacts with live issue state', async () => {
    const result = await buildQaBoardProjection(
      githubFetch,
      '2026-07-16T16:30:00.000Z',
    )
    expect(result.schema).toBe('openagents.qa.board.v1')
    expect(result.sources).toEqual({
      issues: 'ok',
      observer: 'ok',
      swarm: 'ok',
    })
    expect(result.observer?.checks).toHaveLength(7)
    expect(result.observer?.summary.pass).toBe(7)
    expect(result.swarm?.lanes).toHaveLength(6)
    expect(result.swarm?.runRef).toBe('qa.six-lane.20260716T150054760Z')
    expect(result.findings).toContainEqual(
      expect.objectContaining({
        issueNumber: 8912,
        issueState: 'open',
        severity: 'high',
        surface: 'public product-promise registry',
      }),
    )
  })

  test('marks the issue ledger unavailable and preserves the confirmed evidence when GitHub fails', async () => {
    const result = await buildQaBoardProjection(
      (async () => new Response(null, { status: 503 })) as typeof fetch,
    )
    expect(result.sources.issues).toBe('unavailable')
    expect(result.findings).toContainEqual(
      expect.objectContaining({
        issueNumber: 8912,
        issueState: 'unavailable',
      }),
    )
  })

  test('serves only GET on the public-safe same-origin endpoint', async () => {
    const response = await routeQaBoardRequest(
      new Request('https://openagents.com/api/public/qa-board'),
      githubFetch,
    )
    expect(response?.status).toBe(200)
    expect(response?.headers.get('cache-control')).toContain('max-age=30')
    if (response === undefined) throw new Error('expected QA board response')
    expect(((await response.json()) as { schema: string }).schema).toBe(
      'openagents.qa.board.v1',
    )

    const rejected = await routeQaBoardRequest(
      new Request('https://openagents.com/api/public/qa-board', {
        method: 'POST',
      }),
      githubFetch,
    )
    expect(rejected?.status).toBe(405)
    await expect(
      routeQaBoardRequest(
        new Request('https://openagents.com/stats'),
        githubFetch,
      ),
    ).resolves.toBeUndefined()
  })
})
