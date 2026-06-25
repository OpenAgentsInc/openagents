import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_OVERFLOW_BACKOFF,
  FIREWORKS_ADAPTER_ID,
  HYDRALISK_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
  OPENAGENTS_NETWORK_ADAPTER_ID,
  PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
  PASSTHROUGH_OPENAI_ADAPTER_ID,
  VERTEX_ANTHROPIC_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  classifyModel,
  dispatchWithOverflow,
  dispatchWithOverflowWithMetadata,
  makeKhalaBackedAdapterPlan,
  openModelsByCost,
  selectAdapterPlan,
  selectAdapterPlanForKhalaBacking,
  selectPrimaryAdapterId,
} from './model-router'
import { KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH } from './model-serving-policy'
import { openAgentsNetworkAdapter } from './openagents-network-adapter'
import {
  AUTOPILOT_CONCIERGE_MODEL_ID,
  HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_CODE_MODEL_ID,
  KHALA_MINI_MODEL_ID,
  KHALA_MODEL_ID,
  KHALA_MODEL_SLUG,
  KHALA_PYLON_MINI_MODEL_ID,
} from './pricing'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
  type InferenceRequest,
  type InferenceResult,
} from './provider-adapter'

// --- test plumbing -------------------------------------------------------

const runResult = <A>(effect: Effect.Effect<A, InferenceAdapterError>) =>
  Effect.runPromise(Effect.result(effect))

const request = (model: string): InferenceRequest => ({
  messages: [{ content: 'hi', role: 'user' }],
  model,
  passthroughParams: {},
  stream: false,
})

const okResult = (servedModel: string): InferenceResult => ({
  content: 'ok',
  finishReason: 'stop',
  servedModel,
  usage: { completionTokens: 1, promptTokens: 1, totalTokens: 2 },
})

// A mock adapter that records calls and returns a scripted sequence of
// outcomes (one per invocation). `error: undefined` => success.
type Scripted = InferenceAdapterError | undefined
const mockAdapter = (
  id: string,
  script: ReadonlyArray<Scripted>,
): { adapter: InferenceProviderAdapter; calls: () => number } => {
  let n = 0
  const next = (): Scripted => {
    const outcome = script[Math.min(n, script.length - 1)]
    n += 1
    return outcome
  }
  const run = (): Effect.Effect<InferenceResult, InferenceAdapterError> => {
    const outcome = next()
    return outcome === undefined
      ? Effect.succeed(okResult(`served-by-${id}`))
      : Effect.fail(outcome)
  }
  return {
    adapter: {
      complete: () => run(),
      id,
      stream: () => run().pipe(Effect.map(() => [])),
    },
    calls: () => n,
  }
}

const err = (
  adapterId: string,
  retryable: boolean,
  httpStatus?: number,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId,
    httpStatus,
    kind: retryable ? 'rate_limited' : 'request_rejected',
    reason: `${adapterId} ${retryable ? 'retryable' : 'fatal'}`,
    retryable,
  })

// No-wait sleep so overflow backoff never delays a test.
const noSleep = () => Effect.void

const completeOp = (adapter: InferenceProviderAdapter, req: InferenceRequest) =>
  adapter.complete(req).pipe(Effect.map(value => ({ id: adapter.id, value })))

// ==========================================================================
// 1. Model -> lane classification + selection
// ==========================================================================

describe('model classification', () => {
  test('routes Claude-family ids to the Vertex lane', () => {
    for (const model of [
      'claude-opus-4-8',
      'claude-sonnet-4-6',
      'claude-haiku-4-5',
      'opus',
      'sonnet',
      'haiku',
      'anthropic/claude-opus-4-8',
      'vertex/claude-sonnet-4-6',
    ]) {
      expect(classifyModel(model)).toBe('claude')
      expect(selectPrimaryAdapterId(model)).toBe(VERTEX_ANTHROPIC_ADAPTER_ID)
    }
  })

  test('routes the open set to the Fireworks lane', () => {
    for (const model of [
      'deepseek-v4-pro',
      'kimi-k2p6',
      'glm-5p2',
      'qwen-3p7-plus',
      'minimax',
      'gpt-oss-120b',
      'nemotron-3-ultra',
      'fireworks/deepseek-v4-flash',
    ]) {
      expect(classifyModel(model)).toBe('open')
      expect(selectPrimaryAdapterId(model)).toBe(FIREWORKS_ADAPTER_ID)
    }
  })

  test('routes the single Khala model through the Hydralisk mix, then degrades to Vertex Gemini on full outage', () => {
    for (const model of [KHALA_MODEL_SLUG, KHALA_MODEL_ID]) {
      expect(classifyModel(model)).toBe('open')
      expect(selectAdapterPlan(model)).toEqual([
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
        HYDRALISK_ADAPTER_ID,
        VERTEX_GEMINI_ADAPTER_ID,
      ])
    }
  })

  test('can route the single Khala model through Fireworks DeepSeek V4 Flash by operator backing policy', () => {
    for (const model of [KHALA_MODEL_SLUG, KHALA_MODEL_ID]) {
      expect(
        selectAdapterPlanForKhalaBacking(
          model,
          KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH,
        ),
      ).toEqual([
        FIREWORKS_ADAPTER_ID,
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
        HYDRALISK_ADAPTER_ID,
        VERTEX_GEMINI_ADAPTER_ID,
      ])
    }
    expect(
      makeKhalaBackedAdapterPlan(KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH)(
        KHALA_MODEL_ID,
      )[0],
    ).toBe(FIREWORKS_ADAPTER_ID)
    expect(selectAdapterPlan('deepseek-v4-flash')[0]).toBe(FIREWORKS_ADAPTER_ID)
  })

  test('does not treat old Khala split ids as the public Khala route', () => {
    expect(classifyModel(KHALA_MINI_MODEL_ID)).toBe('unknown')
    expect(selectAdapterPlan(KHALA_MINI_MODEL_ID)).toEqual([
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })

  test('routes the Autopilot Concierge virtual model to its priced backing lane', () => {
    expect(classifyModel(AUTOPILOT_CONCIERGE_MODEL_ID)).toBe('gemini')
    expect(selectAdapterPlan(AUTOPILOT_CONCIERGE_MODEL_ID)).toEqual([
      VERTEX_GEMINI_ADAPTER_ID,
    ])
  })

  test('routes the Khala code virtual model to the open coding lane', () => {
    expect(classifyModel(KHALA_CODE_MODEL_ID)).toBe('open')
    expect(selectAdapterPlan(KHALA_CODE_MODEL_ID)).toEqual([
      FIREWORKS_ADAPTER_ID,
      OPENAGENTS_NETWORK_ADAPTER_ID,
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })

  test('routes the OpenAI GPT-OSS 20B model id only to the Hydralisk lane', () => {
    expect(classifyModel(HYDRALISK_GPT_OSS_20B_MODEL_ID)).toBe('open')
    expect(selectAdapterPlan(HYDRALISK_GPT_OSS_20B_MODEL_ID)).toEqual([
      HYDRALISK_ADAPTER_ID,
    ])
  })

  test('routes the Hydralisk GLM-5.2 REAP internal model id only to its private G4 lane', () => {
    expect(selectAdapterPlan(HYDRALISK_GLM_52_REAP_504B_MODEL_ID)).toEqual([
      HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
    ])
  })

  test('routes the OpenAI GPT-OSS 120B model id only to its high-memory Hydralisk lane', () => {
    expect(classifyModel(HYDRALISK_GPT_OSS_120B_MODEL_ID)).toBe('open')
    expect(selectAdapterPlan(HYDRALISK_GPT_OSS_120B_MODEL_ID)).toEqual([
      HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
    ])
  })

  test('routes the Khala Pylon canary alias only to the serving fabric lane', () => {
    expect(classifyModel(KHALA_PYLON_MINI_MODEL_ID)).toBe('open')
    expect(selectAdapterPlan(KHALA_PYLON_MINI_MODEL_ID)).toEqual([
      OPENAGENTS_NETWORK_ADAPTER_ID,
    ])
  })

  test('keeps direct gpt-oss-20b Fireworks-first on day zero', () => {
    expect(selectAdapterPlan('gpt-oss-20b')).toEqual([
      FIREWORKS_ADAPTER_ID,
      OPENAGENTS_NETWORK_ADAPTER_ID,
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })

  test('routes unknown models to passthrough only', () => {
    expect(classifyModel('some-random-model')).toBe('unknown')
    expect(selectAdapterPlan('some-random-model')).toEqual([
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })
})

describe('lane plan ordering (cheapest viable first, then overflow)', () => {
  test('claude: Vertex first, then passthrough overflow', () => {
    expect(selectAdapterPlan('claude-opus-4-8')).toEqual([
      VERTEX_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })

  test('open: Fireworks first, then OpenAgents serving fabric, then passthrough overflow', () => {
    // #5483: the serving-fabric lane (`openagents-network`) is inserted into the
    // open-class plan AHEAD of partner passthrough (our own compute is preferred).
    expect(selectAdapterPlan('kimi-k2p6')).toEqual([
      FIREWORKS_ADAPTER_ID,
      OPENAGENTS_NETWORK_ADAPTER_ID,
      PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
      PASSTHROUGH_OPENAI_ADAPTER_ID,
    ])
  })

  test('the inert network lane is SKIPPED at dispatch when no adapter is registered (#5483)', async () => {
    // The lane id is in the plan, but with no `openagents-network` adapter
    // registered, dispatchWithOverflow filters it out and falls through to the
    // next viable lane — a real selectable insert point, never a faked serve.
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      new InferenceAdapterError({
        adapterId: FIREWORKS_ADAPTER_ID,
        reason: 'retryable: rate limited',
        retryable: true,
      }),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)
    // Note: NO openagents-network adapter registered.

    const outcome = await runResult(
      dispatchWithOverflow(
        request('kimi-k2p6'),
        (adapter, req) => adapter.complete(req),
        { plan: selectAdapterPlan, registry, sleep: () => Effect.void },
      ),
    )
    expect(outcome._tag).toBe('Success')
    // Fireworks (retryable fail) then straight to passthrough — the network lane
    // contributed no dispatch attempt because it is unregistered.
    expect(fireworks.calls()).toBe(1)
    expect(passthrough.calls()).toBe(1)
  })

  test('a REGISTERED inert network adapter typed-fails non-retryably (#5483 honest-scope)', async () => {
    // When the inert adapter IS registered (e.g. a future probe), it honestly
    // typed-fails `network_dispatch_unavailable` rather than faking a serve.
    const outcome = await runResult(
      openAgentsNetworkAdapter.complete(request('kimi-k2p6')),
    )
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('network_dispatch_unavailable')
      expect(outcome.failure.retryable).toBe(false)
    }
  })

  test('open models are ordered cheapest-first by blended cost', () => {
    // gpt-oss-20b is the cheapest priced open model; deepseek-v4-pro the dearest.
    expect(openModelsByCost[0]).toBe('gpt-oss-20b')
    expect(openModelsByCost.at(-1)).toBe('deepseek-v4-pro')
    // Monotonic non-decreasing ordering is preserved end-to-end.
    expect(openModelsByCost.length).toBeGreaterThan(2)
  })
})

// ==========================================================================
// 2. Dispatch with overflow
// ==========================================================================

describe('dispatchWithOverflow', () => {
  test('serves from the primary (cheapest) lane when it succeeds', async () => {
    const vertex = mockAdapter(VERTEX_ANTHROPIC_ADAPTER_ID, [undefined])
    const passthrough = mockAdapter(PASSTHROUGH_ANTHROPIC_ADAPTER_ID, [
      undefined,
    ])
    const registry = new InferenceProviderRegistry()
    registry.register(vertex.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('claude-opus-4-8'), completeOp, {
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.id).toBe(VERTEX_ANTHROPIC_ADAPTER_ID)
    }
    // Overflow lane was never touched.
    expect(passthrough.calls()).toBe(0)
  })

  test('429 on the primary lane overflows to the next viable lane', async () => {
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      err(FIREWORKS_ADAPTER_ID, true, 429),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('kimi-k2p6'), completeOp, {
        // Plan: Fireworks then the OpenAI passthrough (Anthropic passthrough
        // absent from the registry, so it is skipped).
        plan: () => [FIREWORKS_ADAPTER_ID, PASSTHROUGH_OPENAI_ADAPTER_ID],
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.id).toBe(PASSTHROUGH_OPENAI_ADAPTER_ID)
    }
    expect(fireworks.calls()).toBe(1)
    expect(passthrough.calls()).toBe(1)
  })

  test('metadata reports the served lane, fallback reason, region, and health score', async () => {
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      new InferenceAdapterError({
        adapterId: FIREWORKS_ADAPTER_ID,
        httpStatus: 429,
        kind: 'rate_limited',
        reason: 'rate limited',
        retryable: true,
      }),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request('kimi-k2p6'), completeOp, {
        plan: () => [FIREWORKS_ADAPTER_ID, PASSTHROUGH_OPENAI_ADAPTER_ID],
        registry,
        routingSignals: id =>
          id === PASSTHROUGH_OPENAI_ADAPTER_ID
            ? { providerHealthScore: 0.82, region: 'us-east-1' }
            : undefined,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toEqual({
        fallbackReason: 'rate_limited',
        primaryAdapterId: FIREWORKS_ADAPTER_ID,
        providerHealthScore: 0.82,
        region: 'us-east-1',
        servedAdapterId: PASSTHROUGH_OPENAI_ADAPTER_ID,
      })
    }
  })

  test('503 overflows the same way (service overloaded)', async () => {
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      err(FIREWORKS_ADAPTER_ID, true, 503),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('kimi-k2p6'), completeOp, {
        plan: () => [FIREWORKS_ADAPTER_ID, PASSTHROUGH_OPENAI_ADAPTER_ID],
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Success')
  })

  test('GLM saturation overflows with public-safe queue and busy metadata', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [
      new InferenceAdapterError({
        adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        adapterRouteMetadata: {
          glmSaturationPolicy: 'queue_then_overflow',
          queueWaitMs: 125,
          replicaBusyReason: 'inflight_full',
          replicaFallbackReason: 'inflight_full',
        },
        httpStatus: 429,
        kind: 'glm_pool_saturated',
        reason: 'pool saturated',
        retryable: true,
      }),
    ])
    const gemini = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(gemini.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: () => [
          HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
          VERTEX_GEMINI_ADAPTER_ID,
        ],
        registry,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toEqual({
        fallbackAdapterRouteMetadata: {
          glmSaturationPolicy: 'queue_then_overflow',
          queueWaitMs: 125,
          replicaBusyReason: 'inflight_full',
          replicaFallbackReason: 'inflight_full',
        },
        fallbackReason: 'glm_pool_saturated',
        primaryAdapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        servedAdapterId: VERTEX_GEMINI_ADAPTER_ID,
      })
    }
    expect(glm.calls()).toBe(1)
    expect(gemini.calls()).toBe(1)
  })

  test('a non-retryable failure surfaces immediately without overflow', async () => {
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      err(FIREWORKS_ADAPTER_ID, false, 400),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('kimi-k2p6'), completeOp, {
        plan: () => [FIREWORKS_ADAPTER_ID, PASSTHROUGH_OPENAI_ADAPTER_ID],
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.adapterId).toBe(FIREWORKS_ADAPTER_ID)
      expect(result.failure.retryable).toBe(false)
    }
    // Overflow lane never reached.
    expect(passthrough.calls()).toBe(0)
  })

  test('surfaces the last retryable error when every viable lane fails', async () => {
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      err(FIREWORKS_ADAPTER_ID, true, 429),
    ])
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [
      err(PASSTHROUGH_OPENAI_ADAPTER_ID, true, 503),
    ])
    const registry = new InferenceProviderRegistry()
    registry.register(fireworks.adapter)
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('kimi-k2p6'), completeOp, {
        plan: () => [FIREWORKS_ADAPTER_ID, PASSTHROUGH_OPENAI_ADAPTER_ID],
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      // The LAST lane's error surfaces.
      expect(result.failure.adapterId).toBe(PASSTHROUGH_OPENAI_ADAPTER_ID)
    }
  })

  test('skips planned lanes that are not registered (absent partner secret)', async () => {
    // Plan names the Anthropic passthrough first, but only the OpenAI one is
    // registered (e.g. ANTHROPIC_API_KEY absent) — the dispatcher serves OpenAI.
    const passthrough = mockAdapter(PASSTHROUGH_OPENAI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(passthrough.adapter)

    const result = await runResult(
      dispatchWithOverflow(request('unknown-model'), completeOp, {
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.id).toBe(PASSTHROUGH_OPENAI_ADAPTER_ID)
    }
  })

  test('fails with a router configuration_error when no lane is registered', async () => {
    const registry = new InferenceProviderRegistry()
    const result = await runResult(
      dispatchWithOverflow(request('claude-opus-4-8'), completeOp, {
        registry,
        sleep: noSleep,
      }),
    )
    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.adapterId).toBe('router')
      expect(result.failure.kind).toBe('configuration_error')
      expect(result.failure.retryable).toBe(false)
    }
  })

  test('applies bounded exponential backoff before each overflow attempt', async () => {
    // Two retryable failures then a success forces two backoff sleeps; assert
    // the injected delays follow base × 2^n capped at maxDelayMs.
    const a = mockAdapter('lane-a', [err('lane-a', true, 429)])
    const b = mockAdapter('lane-b', [err('lane-b', true, 503)])
    const c = mockAdapter('lane-c', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(a.adapter)
    registry.register(b.adapter)
    registry.register(c.adapter)

    const delays: Array<number> = []
    const result = await runResult(
      dispatchWithOverflow(request('x'), completeOp, {
        backoff: { baseDelayMs: 10, maxDelayMs: 15 },
        plan: () => ['lane-a', 'lane-b', 'lane-c'],
        registry,
        sleep: ms =>
          Effect.sync(() => {
            delays.push(ms)
          }),
      }),
    )
    expect(result._tag).toBe('Success')
    // First overflow: 10 × 2^0 = 10; second overflow: 10 × 2^1 = 20 capped at 15.
    expect(delays).toEqual([10, 15])
  })

  test('DEFAULT_OVERFLOW_BACKOFF is bounded and sane', () => {
    expect(DEFAULT_OVERFLOW_BACKOFF.baseDelayMs).toBeGreaterThan(0)
    expect(DEFAULT_OVERFLOW_BACKOFF.maxDelayMs).toBeGreaterThanOrEqual(
      DEFAULT_OVERFLOW_BACKOFF.baseDelayMs,
    )
  })
})
