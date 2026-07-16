import { describe, expect, test } from 'vitest'

import { fetchShareProjection, shareProjectionUrl, userFacingCopy } from './-share-fetch'

const validProjectionJson = {
  schemaVersion: 'openagents.share_projection.v1',
  id: 'share.fixture',
  url: 'https://openagents.com/share/share.fixture',
  audience: { _tag: 'Public' },
  audienceLabel: 'Public link',
  title: 'A shared run',
  subtitle: 'openagents/openagents@main · completed',
  source: { kind: 'agent-run', id: 'run.fixture' },
  status: 'active',
  createdAt: '2026-07-16T00:00:00.000Z',
  updatedAt: '2026-07-16T00:00:00.000Z',
  messages: [],
  files: [],
  artifacts: [],
  approvals: [],
  receipts: [],
  metrics: { eventCount: 0, tokenTotal: 0, toolCallCount: 0 },
}

const fetchStub = (payload: unknown, status = 200): typeof fetch =>
  (async () =>
    new Response(JSON.stringify(payload), {
      status,
      headers: { 'content-type': 'application/json' },
    })) as unknown as typeof fetch

describe('shareProjectionUrl', () => {
  test('builds the share data endpoint for a given shareId', () => {
    expect(shareProjectionUrl('abc-123')).toBe('/api/share/abc-123/v1/data')
  })
})

describe('fetchShareProjection (T14 #8871 client re-validation)', () => {
  test('accepts and decodes a well-formed projection', async () => {
    const result = await fetchShareProjection(
      'share.fixture',
      fetchStub({ projection: validProjectionJson }),
    )

    expect(result.tag).toBe('loaded')
    if (result.tag !== 'loaded') throw new Error('expected a loaded result')
    expect(result.projection.id).toBe('share.fixture')
    expect(result.projection.schemaVersion).toBe('openagents.share_projection.v1')
  })

  test('rejects a payload with the wrong schemaVersion instead of casting it through', async () => {
    const result = await fetchShareProjection(
      'share.fixture',
      fetchStub({
        projection: { ...validProjectionJson, schemaVersion: 'openagents.share_projection.v2' },
      }),
    )

    expect(result.tag).toBe('failed')
    if (result.tag !== 'failed') throw new Error('expected a failed result')
    expect(result.error).toBe('Share response was malformed.')
  })

  test('rejects a projection missing required fields instead of casting it through', async () => {
    const { metrics: _metrics, ...withoutMetrics } = validProjectionJson
    const result = await fetchShareProjection(
      'share.fixture',
      fetchStub({ projection: withoutMetrics }),
    )

    expect(result.tag).toBe('failed')
    if (result.tag !== 'failed') throw new Error('expected a failed result')
    expect(result.error).toBe('Share response was malformed.')
  })

  test('rejects a projection whose message parts do not match any known kind', async () => {
    const result = await fetchShareProjection(
      'share.fixture',
      fetchStub({
        projection: {
          ...validProjectionJson,
          messages: [
            {
              id: 'msg-1',
              author: 'user',
              label: 'chris',
              time: '2026-07-16T00:00:00.000Z',
              parts: [{ kind: 'not-a-real-kind', body: ['hi'] }],
            },
          ],
        },
      }),
    )

    expect(result.tag).toBe('failed')
    if (result.tag !== 'failed') throw new Error('expected a failed result')
    expect(result.error).toBe('Share response was malformed.')
  })

  test('reports the HTTP status and server error for a non-ok response', async () => {
    const result = await fetchShareProjection(
      'share.fixture',
      fetchStub({ error: 'share_forbidden' }, 403),
    )

    expect(result.tag).toBe('failed')
    if (result.tag !== 'failed') throw new Error('expected a failed result')
    expect(result.status).toBe(403)
    expect(result.error).toBe('share_forbidden')
  })
})

describe('userFacingCopy', () => {
  test('rewrites the internal Adjutant codename to Autopilot', () => {
    expect(userFacingCopy('Adjutant is thinking. Try @adjutant help.')).toBe(
      'Autopilot is thinking. Try @autopilot help.',
    )
  })
})
