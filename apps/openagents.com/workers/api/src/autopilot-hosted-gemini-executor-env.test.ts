import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  hostedGeminiExecutorArmed,
  makeHostedGeminiExecuteReadyWork,
  resolveHostedGeminiExecutor,
} from './autopilot-hosted-gemini-executor-env'
import type {
  InferenceProviderAdapter,
  InferenceRequest,
} from './inference/provider-adapter'

// A spy adapter that records every `complete` request and never reaches Vertex.
const spyAdapter = (): {
  adapter: InferenceProviderAdapter
  requests: Array<InferenceRequest>
} => {
  const requests: Array<InferenceRequest> = []
  return {
    adapter: {
      complete: (request: InferenceRequest) => {
        requests.push(request)
        return Effect.succeed({
          content: 'never-persisted',
          finishReason: 'stop' as const,
          servedModel: 'gemini-3.5-flash',
          usage: { completionTokens: 1, promptTokens: 1, totalTokens: 2 },
        })
      },
      id: 'vertex-gemini',
      stream: () => Effect.succeed([]),
    },
    requests,
  }
}

describe('hostedGeminiExecutorArmed', () => {
  test('INERT without the flag even when the secret is present', () => {
    expect(
      hostedGeminiExecutorArmed({ VERTEX_SA_KEY: '{"client_email":"x"}' }),
    ).toBe(false)
  })

  test('INERT with the flag on but no secret', () => {
    expect(
      hostedGeminiExecutorArmed({ HOSTED_GEMINI_EXECUTOR_ENABLED: 'true' }),
    ).toBe(false)
  })

  test('INERT when the secret is blank whitespace', () => {
    expect(
      hostedGeminiExecutorArmed({
        HOSTED_GEMINI_EXECUTOR_ENABLED: 'true',
        VERTEX_SA_KEY: '   ',
      }),
    ).toBe(false)
  })

  test('armed only when BOTH the flag is on AND the secret is present', () => {
    for (const flag of ['1', 'on', 'true', 'yes', 'TRUE', ' On ']) {
      expect(
        hostedGeminiExecutorArmed({
          HOSTED_GEMINI_EXECUTOR_ENABLED: flag,
          VERTEX_SA_KEY: '{"client_email":"x"}',
        }),
      ).toBe(true)
    }
  })

  test('a non-affirmative flag value stays INERT', () => {
    for (const flag of ['', '0', 'off', 'false', 'no', 'enabled?']) {
      expect(
        hostedGeminiExecutorArmed({
          HOSTED_GEMINI_EXECUTOR_ENABLED: flag,
          VERTEX_SA_KEY: '{"client_email":"x"}',
        }),
      ).toBe(false)
    }
  })
})

describe('resolveHostedGeminiExecutor', () => {
  test('returns undefined and never builds an adapter when not armed', () => {
    let built = 0
    const executor = resolveHostedGeminiExecutor(
      { VERTEX_SA_KEY: '{"client_email":"x"}' },
      {
        buildAdapter: () => {
          built += 1
          return spyAdapter().adapter
        },
      },
    )
    expect(executor).toBeUndefined()
    expect(built).toBe(0)
  })

  test('returns a composed executor and builds the adapter once when armed', () => {
    let built = 0
    const executor = resolveHostedGeminiExecutor(
      {
        HOSTED_GEMINI_EXECUTOR_ENABLED: 'true',
        VERTEX_SA_KEY: '{"client_email":"x"}',
      },
      {
        buildAdapter: () => {
          built += 1
          return spyAdapter().adapter
        },
      },
    )
    expect(typeof executor).toBe('function')
    expect(built).toBe(1)
  })
})

describe('makeHostedGeminiExecuteReadyWork', () => {
  const input = {
    nowIso: '2026-06-20T00:00:00.000Z',
    work: {} as never,
  }

  test('resolves undefined and never touches the adapter when not armed', async () => {
    const { adapter, requests } = spyAdapter()
    const executeReadyWork = makeHostedGeminiExecuteReadyWork({
      buildAdapter: () => adapter,
    })
    const result = await executeReadyWork(
      { VERTEX_SA_KEY: '{"client_email":"x"}' },
      input,
    )
    expect(result).toBeUndefined()
    expect(requests).toHaveLength(0)
  })

  // The armed path that drives a real hosted_gemini work projection end-to-end
  // is covered by the route-harness integration test in
  // autopilot-work-routes.test.ts (it needs a full AutopilotWorkOrderProjection
  // built by the route). Here we only assert the env-gating decision.
})
