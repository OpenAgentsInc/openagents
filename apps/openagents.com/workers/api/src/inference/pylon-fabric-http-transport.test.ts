import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { decidePylonAdmission } from './khala-pylon-admission'
import {
  DEFAULT_OPENAGENTS_NETWORK_SERVING_CAPABILITY_REF,
  DEFAULT_OPENAGENTS_NETWORK_SERVING_LANE_REF,
  makePylonFabricHttpTransport,
  pylonFabricHttpTransportConfigFromEnv,
  pylonGatewayAdmissionFromEnv,
  type PylonFabricFetchLike,
} from './pylon-fabric-http-transport'
import { type PsionicServeRequest } from './psionic-fabric-serve'

const request: PsionicServeRequest = {
  messages: [{ content: 'Respond OK', role: 'user' }],
  model: 'qwen-3p7-plus',
  passthroughParams: { max_tokens: 1, temperature: 0 },
  requireExactGreedyParity: true,
}

describe('pylon fabric HTTP transport', () => {
  test('resolves secret-backed transport config by presence only', () => {
    expect(pylonFabricHttpTransportConfigFromEnv({})).toBeUndefined()
    expect(
      pylonFabricHttpTransportConfigFromEnv({
        OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN: 'token',
        OPENAGENTS_NETWORK_FABRIC_SERVE_URL: ' https://pylon.example/serve ',
      }),
    ).toEqual({
      bearerToken: 'token',
      endpoint: 'https://pylon.example/serve',
    })
  })

  test('posts the Psionic serve request with bearer auth and returns the response body', async () => {
    const calls: Array<Readonly<{ input: string; init: Parameters<PylonFabricFetchLike>[1] }>> =
      []
    const fetchImpl: PylonFabricFetchLike = async (input, init) => {
      calls.push({ input, init })
      return new Response('{"ok":true}', { status: 200 })
    }

    const transport = makePylonFabricHttpTransport({
      bearerToken: 'secret-token',
      endpoint: 'https://pylon.example/serve',
      fetchImpl,
    })
    const outcome = await Effect.runPromise(Effect.result(transport(request)))

    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      expect(outcome.success).toBe('{"ok":true}')
    }
    expect(calls).toHaveLength(1)
    expect(calls[0]!.input).toBe('https://pylon.example/serve')
    expect(calls[0]!.init.headers.authorization).toBe('Bearer secret-token')
    expect(JSON.parse(calls[0]!.init.body)).toMatchObject({
      model: 'qwen-3p7-plus',
      requireExactGreedyParity: true,
    })
  })

  test('classifies overload responses as retryable typed adapter errors', async () => {
    const fetchImpl: PylonFabricFetchLike = async () =>
      new Response('busy', { status: 503 })
    const transport = makePylonFabricHttpTransport({
      bearerToken: 'secret-token',
      endpoint: 'https://pylon.example/serve',
      fetchImpl,
    })
    const outcome = await Effect.runPromise(Effect.result(transport(request)))

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('service_overloaded')
      expect(outcome.failure.retryable).toBe(true)
      expect(outcome.failure.httpStatus).toBe(503)
      expect(outcome.failure.reason).not.toContain('busy')
    }
  })
})

describe('pylon gateway admission from env', () => {
  test('builds an admitted public-safe snapshot when heartbeat and payout refs are present', () => {
    const admission = pylonGatewayAdmissionFromEnv(
      {
        OPENAGENTS_NETWORK_ADMITTED_PYLON_REF:
          'gcloud.gswarm508-clean2-20260325044551-contrib',
        OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT:
          '2026-06-23T21:40:00.000Z',
        OPENAGENTS_NETWORK_PYLON_HEARTBEAT_STATUS: 'ok',
        OPENAGENTS_NETWORK_SPARK_PAYOUT_TARGET_REF:
          'payout.spark.aab6617b16f096dfe02fc6b4',
      },
      Date.parse('2026-06-23T21:40:15.000Z'),
    )
    expect(admission.requiredCapabilityRef).toBe(
      DEFAULT_OPENAGENTS_NETWORK_SERVING_CAPABILITY_REF,
    )
    expect(admission.snapshot.servingLaneRefs).toEqual([
      DEFAULT_OPENAGENTS_NETWORK_SERVING_LANE_REF,
    ])
    expect(decidePylonAdmission(admission).admitted).toBe(true)
  })

  test('fails closed through the admission gate when heartbeat or payout refs are absent', () => {
    const admission = pylonGatewayAdmissionFromEnv(
      {
        OPENAGENTS_NETWORK_ADMITTED_PYLON_REF:
          'gcloud.gswarm508-clean2-20260325044551-contrib',
      },
      Date.parse('2026-06-23T21:40:15.000Z'),
    )
    const decision = decidePylonAdmission(admission)
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(
      'blocker.pylon_admission.no_heartbeat',
    )
    expect(decision.blockerRefs).toContain(
      'blocker.pylon_admission.no_spark_payout_target',
    )
  })
})
