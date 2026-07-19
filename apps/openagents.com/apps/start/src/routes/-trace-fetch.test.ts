import { describe, expect, test, vi } from 'vitest'

import { fetchTraceProjection } from './-trace-fetch'

const apiPayload = {
  trace: {
    uuid: '448644bd-f2ce-4ad4-bfad-e4e898ed12ef',
    schemaVersion: 'ATIF-v1.7',
    trajectoryId: 'trajectory.fixture',
    visibility: 'public',
    agentRef: 'agent:fixture',
    stepCount: 1,
    trajectory: {
      schema_version: 'ATIF-v1.7',
      trajectory_id: 'trajectory.fixture',
      agent: { name: 'qa-runner', version: '1.0.0' },
      steps: [{ step_id: 1, source: 'user', message: 'Verify it.' }],
    },
    blobRefs: [],
    createdAt: '2026-07-18T12:00:00.000Z',
    dataMarket: {
      trainingConsent: false,
      uploadSource: 'agent',
      reward: { eligible: false, amountSats: null, status: 'tbd' },
    },
    authority: {
      acceptedWorkAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
    },
  },
}

describe('fetchTraceProjection', () => {
  test('decodes the canonical ATIF trace response at the browser boundary', async () => {
    const fetchFn = vi.fn(async () => new Response(JSON.stringify(apiPayload), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }))

    const result = await fetchTraceProjection('448644bd-f2ce-4ad4-bfad-e4e898ed12ef', undefined, fetchFn)
    expect(result.tag).toBe('loaded')
    if (result.tag === 'loaded') {
      expect(result.projection.trajectory.schema_version).toBe('ATIF-v1.7')
      expect(result.projection.trajectory.steps).toHaveLength(1)
    }
    expect(fetchFn).toHaveBeenCalledWith(
      '/api/traces/448644bd-f2ce-4ad4-bfad-e4e898ed12ef',
      expect.objectContaining({ cache: 'no-store', credentials: 'include' }),
    )
  })

  test('fails closed when the trajectory drifts from the Effect schema', async () => {
    const malformed = structuredClone(apiPayload)
    malformed.trace.trajectory.schema_version = 'ATIF-v2'
    const result = await fetchTraceProjection('trace.fixture', undefined, async () =>
      new Response(JSON.stringify(malformed), { status: 200 }))
    expect(result).toEqual({ tag: 'failed', status: 0, error: 'Trace response was malformed.' })
  })

  test('does not reveal more than the API error for a private or missing trace', async () => {
    const result = await fetchTraceProjection('trace.private', undefined, async () =>
      new Response(JSON.stringify({ error: 'trace_not_found' }), { status: 404 }))
    expect(result).toEqual({ tag: 'failed', status: 404, error: 'trace_not_found' })
  })
})
