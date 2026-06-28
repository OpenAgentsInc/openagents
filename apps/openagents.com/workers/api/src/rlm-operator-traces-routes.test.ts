import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OperatorRlmTraceSummary,
  buildOperatorRlmTracesProjection,
  handleOperatorRlmTraces,
} from './rlm-operator-traces-routes'

const run = (effect: Effect.Effect<Response>): Promise<Response> =>
  Effect.runPromise(effect)

const trace: OperatorRlmTraceSummary = {
  agentRef: 'agent.public.rlm_leaf_executor',
  authority: {
    directExecutionAuthority: false,
    payoutAuthority: false,
    publicClaimAuthority: false,
    trainingPromotionAuthority: false,
  },
  blueprintSignatureRefs: [
    'program_signature.frlm_conductor.v1',
    'program_signature.rlm_leaf_executor.v1',
    'program_signature.blueprint_action_submission.evidence_only.v1',
  ],
  createdAt: '2026-06-28T12:00:00.000Z',
  demandKind: 'recursive_language_model',
  demandSource: 'frlm_conductor',
  evidenceRefs: [
    'trace.rlm_trace_1',
    'evidence.rlm_trace.redacted_operator_projection',
    'evidence.blueprint_signature_lookup.safe_projection',
  ],
  schemaVersion: 'ATIF-v1.7',
  stepCount: 9,
  traceRef: 'trace.rlm_trace_1',
  traceUuid: 'rlm_trace_1',
  trajectoryRef: 'trajectory.pylon_rlm:assignment.public.demo:turn_1',
  updatedAt: '2026-06-28T12:01:00.000Z',
  visibility: 'owner_only',
}

describe('operator RLM traces route', () => {
  test('builds an operator-only ref projection with Blueprint governance refs', () => {
    const projection = buildOperatorRlmTracesProjection({
      generatedAt: '2026-06-28T12:05:00.000Z',
      limit: 25,
      ownerUserId: null,
      traces: [trace],
      visibility: null,
    })

    expect(projection.schemaVersion).toBe('openagents.operator.rlm_traces.v1')
    expect(projection.route).toBe('/api/operator/rlm/traces')
    expect(projection.blueprint).toMatchObject({
      conductorRef: 'program_signature.frlm_conductor.v1',
      leafExecutorRef: 'program_signature.rlm_leaf_executor.v1',
      privacyPolicy: 'operator_refs_only',
      programFamily: 'recursive_language_model',
    })
    expect(projection.traces[0]?.authority).toEqual({
      directExecutionAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
      trainingPromotionAuthority: false,
    })
    expect(JSON.stringify(projection)).not.toContain('trajectory_json')
    expect(JSON.stringify(projection)).not.toContain('raw_prompt')
    expect(JSON.stringify(projection)).not.toContain('raw_trace')
  })

  test('requires admin auth', async () => {
    const response = await run(
      handleOperatorRlmTraces(
        new Request('https://openagents.com/api/operator/rlm/traces'),
        {
          requireAdminApiToken: async () => false,
          store: { listTraces: async () => [trace] },
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('returns bounded no-store trace metadata for an admin', async () => {
    const seen: Array<{
      limit: number
      ownerUserId: string | null
      visibility: string | null
    }> = []

    const response = await run(
      handleOperatorRlmTraces(
        new Request(
          'https://openagents.com/api/operator/rlm/traces?limit=999&owner_user_id=user_123&visibility=owner_only',
        ),
        {
          nowIso: () => '2026-06-28T12:05:00.000Z',
          requireAdminApiToken: async () => true,
          store: {
            listTraces: async input => {
              seen.push(input)
              return [trace]
            },
          },
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(seen).toEqual([
      { limit: 100, ownerUserId: 'user_123', visibility: 'owner_only' },
    ])
    const body = (await response.json()) as {
      filters: { limit: number; ownerUserId: string; visibility: string }
      traces: ReadonlyArray<{ traceRef: string; trajectoryRef: string }>
    }
    expect(body.filters).toEqual({
      limit: 100,
      ownerUserId: 'user_123',
      visibility: 'owner_only',
    })
    expect(body.traces[0]).toMatchObject({
      traceRef: 'trace.rlm_trace_1',
      trajectoryRef: 'trajectory.pylon_rlm:assignment.public.demo:turn_1',
    })
  })

  test('rejects non-GET methods', async () => {
    const response = await run(
      handleOperatorRlmTraces(
        new Request('https://openagents.com/api/operator/rlm/traces', {
          method: 'POST',
        }),
        {
          requireAdminApiToken: async () => true,
          store: { listTraces: async () => [trace] },
        },
      ),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('fails closed when the projection contains private-data-shaped material', async () => {
    const response = await run(
      handleOperatorRlmTraces(
        new Request('https://openagents.com/api/operator/rlm/traces'),
        {
          requireAdminApiToken: async () => true,
          store: {
            listTraces: async () => [
              {
                ...trace,
                demandSource: 'sk-abcdef0123456789ABCDEF',
              },
            ],
          },
        },
      ),
    )

    expect(response.status).toBe(500)
    expect((await response.json()) as unknown).toEqual({
      error: 'operator_rlm_traces_unavailable',
    })
  })
})
