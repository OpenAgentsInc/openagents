import { describe, expect, test } from 'vitest'

const checker = await import('./check-public-projection-freshness.mjs')

describe('public projection freshness deploy gate', () => {
  test('fails a new public projection route without freshness metadata', () => {
    const source = `
      export const route = '/api/public/example'
      export const response = () => noStoreJsonResponse({ status: 'ok' })
    `

    const surfaces = checker.inventorySource('workers/api/src/example.ts', source)
    const result = checker.evaluateProjectionFreshness(surfaces, new Map())

    expect(result.failures).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'workers/api/src/example.ts::route::/api/public/example',
          reason: expect.stringContaining('missing generatedAt/lastRebuiltAt'),
        }),
      ]),
    )
  })

  test('passes a public projection carrying generatedAt and staleness policy', () => {
    const source = `
      const stalenessContract = { maxStalenessSeconds: 30 }
      export const route = '/api/public/example'
      export const response = (nowIso) => noStoreJsonResponse({
        generatedAt: nowIso(),
        staleness: stalenessContract,
        status: 'ok',
      })
    `

    const surfaces = checker.inventorySource('workers/api/src/example.ts', source)
    const result = checker.evaluateProjectionFreshness(surfaces, new Map())

    expect(result.failures).toEqual([])
  })

  test('allows a grandfathered projection only with an issue-ref allowlist entry', () => {
    const source = `
      export const route = '/api/forum/topics/:topicId'
      export const response = () => noStoreJsonResponse({
        topic: { publicProjection: { publicSafe: true } },
      })
    `
    const surfaces = checker.inventorySource('workers/api/src/forum-routes.ts', source)
    const allowlist = new Map([
      [
        'workers/api/src/forum-routes.ts::route::/api/forum/topics/:topicId',
        { id: 'workers/api/src/forum-routes.ts::route::/api/forum/topics/:topicId', issueRef: '#4751' },
      ],
      [
        'workers/api/src/forum-routes.ts::publicProjection::4',
        { id: 'workers/api/src/forum-routes.ts::publicProjection::4', issueRef: '#4751' },
      ],
    ])

    const result = checker.evaluateProjectionFreshness(surfaces, allowlist)

    expect(result.failures).toEqual([])
    expect(result.grandfathered).toHaveLength(2)
  })
})
