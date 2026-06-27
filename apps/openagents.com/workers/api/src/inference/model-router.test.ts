import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_OVERFLOW_BACKOFF,
  type DispatchFailureTelemetryEvent,
  type DispatchSuccessValidator,
  FIREWORKS_ADAPTER_ID,
  FIREWORKS_STRONG_CODING_ADAPTER_ID,
  HYDRALISK_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
  OPENAGENTS_NETWORK_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
  PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
  PASSTHROUGH_OPENAI_ADAPTER_ID,
  VERTEX_ANTHROPIC_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  classifyModel,
  dispatchWithOverflow,
  dispatchWithOverflowWithMetadata,
  makeBoundedDispatchFailureTelemetry,
  makeKhalaBackedAdapterPlan,
  openModelsByCost,
  selectAdapterPlan,
  selectAdapterPlanForKhalaBacking,
  selectAdapterPlanForKhalaStrongCodingRequest,
  selectAdapterPlanForKhalaToolRequest,
  selectPrimaryAdapterId,
} from './model-router'
import {
  KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH,
  KHALA_BACKING_HYDRALISK_GPT_OSS,
  resolveKhalaBackingModel,
} from './model-serving-policy'
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
// outcomes (one per invocation). `undefined` => default success.
type Scripted = InferenceAdapterError | InferenceResult | undefined
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
    if (outcome instanceof InferenceAdapterError) {
      return Effect.fail(outcome)
    }
    return Effect.succeed(outcome ?? okResult(`served-by-${id}`))
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

const nonEmptyAssistantValidator: DispatchSuccessValidator<
  Readonly<{ id: string; value: InferenceResult }>
> = ({ adapter, value }) =>
  value.value.content.trim() === '' &&
  (value.value.toolCalls === undefined || value.value.toolCalls.length === 0)
    ? {
        _tag: 'failed',
        error: new InferenceAdapterError({
          adapterId: adapter.id,
          kind: 'empty_assistant_content',
          reason: 'adapter returned empty assistant content',
          retryable: true,
        }),
      }
    : { _tag: 'accepted' }

const toolRequiredValidator: DispatchSuccessValidator<
  Readonly<{ id: string; value: InferenceResult }>
> = ({ adapter, request, value }) =>
  request.passthroughParams['tool_choice'] === 'required' &&
  (value.value.toolCalls === undefined || value.value.toolCalls.length === 0)
    ? {
        _tag: 'failed',
        error: new InferenceAdapterError({
          adapterId: adapter.id,
          kind: 'tool_required_no_tool_calls',
          reason: 'adapter returned no tool calls for a tool-required request',
          retryable: true,
        }),
      }
    : { _tag: 'accepted' }

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

  test('routes conversational Khala through Gemini, Fireworks, GLM, then OpenRouter', () => {
    for (const model of [KHALA_MODEL_SLUG, KHALA_MODEL_ID]) {
      expect(classifyModel(model)).toBe('open')
      expect(selectAdapterPlan(model)).toEqual([
        VERTEX_GEMINI_ADAPTER_ID,
        FIREWORKS_ADAPTER_ID,
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
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
        VERTEX_GEMINI_ADAPTER_ID,
        OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
      ])
    }
    expect(
      makeKhalaBackedAdapterPlan(KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH)(
        KHALA_MODEL_ID,
      )[0],
    ).toBe(FIREWORKS_ADAPTER_ID)
    expect(selectAdapterPlan('deepseek-v4-flash')[0]).toBe(FIREWORKS_ADAPTER_ID)
  })

  test('makes fast lanes primary for conversational Khala while keeping GPT-OSS out (#6259)', () => {
    for (const model of [KHALA_MODEL_SLUG, KHALA_MODEL_ID]) {
      const plan = selectAdapterPlanForKhalaBacking(
        model,
        KHALA_BACKING_HYDRALISK_GPT_OSS,
      )
      // Conversational Khala is fast-first. GLM remains in the overflow chain,
      // and GPT-OSS is NOT in this plan.
      expect(plan[0]).toBe(VERTEX_GEMINI_ADAPTER_ID)
      expect(plan).toEqual([
        VERTEX_GEMINI_ADAPTER_ID,
        FIREWORKS_ADAPTER_ID,
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
      ])
      expect(plan).not.toContain(HYDRALISK_GPT_OSS_120B_ADAPTER_ID)
      expect(plan).not.toContain(HYDRALISK_ADAPTER_ID)
    }
    // The committed prod backing value resolves to the fast conversational plan.
    expect(
      makeKhalaBackedAdapterPlan(
        resolveKhalaBackingModel('hydralisk-glm-5.2-reap-504b'),
      )(KHALA_MODEL_ID)[0],
    ).toBe(VERTEX_GEMINI_ADAPTER_ID)
  })

  test('routes tool-bearing Khala requests GLM/self-hosted first while keeping typed fallbacks', () => {
    expect(
      selectAdapterPlanForKhalaToolRequest(
        KHALA_MODEL_ID,
        selectAdapterPlanForKhalaBacking(
          KHALA_MODEL_ID,
          KHALA_BACKING_HYDRALISK_GPT_OSS,
        ),
      ),
    ).toEqual([
      HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      FIREWORKS_ADAPTER_ID,
      VERTEX_GEMINI_ADAPTER_ID,
      OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
    ])
  })

  test('routes internal strong-coding Khala requests to the frontier GLM coding lane first, overflowing to the proven Fireworks backing', () => {
    expect(
      selectAdapterPlanForKhalaStrongCodingRequest(
        KHALA_MODEL_ID,
        selectAdapterPlanForKhalaBacking(
          KHALA_MODEL_ID,
          KHALA_BACKING_HYDRALISK_GPT_OSS,
        ),
      ),
    ).toEqual([
      FIREWORKS_STRONG_CODING_ADAPTER_ID,
      FIREWORKS_ADAPTER_ID,
      VERTEX_GEMINI_ADAPTER_ID,
      OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
    ])
    // The unreliable owned GLM-REAP tool lane (#6310) is excluded from the
    // strong-coding plan on purpose.
    expect(
      selectAdapterPlanForKhalaStrongCodingRequest(KHALA_MODEL_ID),
    ).not.toContain(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID)
  })

  test('leaves non-Khala models untouched for the strong-coding selector', () => {
    const basePlan = selectAdapterPlan('gpt-oss-120b')
    expect(
      selectAdapterPlanForKhalaStrongCodingRequest('gpt-oss-120b', basePlan),
    ).toEqual(basePlan)
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

  test('treats an empty assistant fallback result as a failed lane and continues', async () => {
    const emptyLane = mockAdapter('empty-lane', [
      {
        content: '   ',
        finishReason: 'stop',
        servedModel: 'empty-model',
        usage: { completionTokens: 0, promptTokens: 1, totalTokens: 1 },
      },
    ])
    const healthyLane = mockAdapter('healthy-lane', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(emptyLane.adapter)
    registry.register(healthyLane.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(
        request(KHALA_MODEL_ID),
        completeOp,
        {
          plan: () => ['empty-lane', 'healthy-lane'],
          registry,
          sleep: noSleep,
        },
        nonEmptyAssistantValidator,
      ),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: 'empty_assistant_content',
        primaryAdapterId: 'empty-lane',
        servedAdapterId: 'healthy-lane',
      })
    }
    expect(emptyLane.calls()).toBe(1)
    expect(healthyLane.calls()).toBe(1)
  })

  test('treats an empty assistant result as a retryable lane failure by default', async () => {
    const emptyLane = mockAdapter('empty-default-lane', [
      {
        content: '',
        finishReason: 'stop',
        servedModel: 'empty-default-model',
        usage: { completionTokens: 0, promptTokens: 1, totalTokens: 1 },
      },
    ])
    const healthyLane = mockAdapter('healthy-default-lane', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(emptyLane.adapter)
    registry.register(healthyLane.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: () => ['empty-default-lane', 'healthy-default-lane'],
        registry,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: 'empty_assistant_content',
        primaryAdapterId: 'empty-default-lane',
        servedAdapterId: 'healthy-default-lane',
      })
    }
    expect(emptyLane.calls()).toBe(1)
    expect(healthyLane.calls()).toBe(1)
  })

  test('retries primary empty assistant content after scheduler preemption before overflow', async () => {
    const primary = mockAdapter('primary-glm', [
      {
        content: '',
        finishReason: 'stop',
        servedModel: 'primary-glm-model',
        usage: { completionTokens: 0, promptTokens: 1, totalTokens: 1 },
      },
      {
        content: 'glm primary retry ok',
        finishReason: 'stop',
        servedModel: 'primary-glm-model',
        usage: { completionTokens: 4, promptTokens: 1, totalTokens: 5 },
      },
    ])
    const weakerFallback = mockAdapter('weaker-fallback', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(primary.adapter)
    registry.register(weakerFallback.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(
        request(KHALA_MODEL_ID),
        completeOp,
        {
          plan: () => ['primary-glm', 'weaker-fallback'],
          preemption: {
            demandClass: 'external',
            preempt: () =>
              Effect.succeed({
                evidenceRef: 'scheduler.preemption.internal_stress.retry',
                reason: 'external_reserved_headroom_unavailable',
                targetDemandClass: 'internal_stress' as const,
                targetOutcome: 'preempted_yielded' as const,
              }),
            reason: 'glm_global_internal_stress_active',
            reservedExternalHeadroomAvailable: false,
          },
          registry,
          sleep: noSleep,
        },
        nonEmptyAssistantValidator,
      ),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: null,
        primaryAdapterId: 'primary-glm',
        schedulerPreemption: {
          evidenceRef: 'scheduler.preemption.internal_stress.retry',
          targetDemandClass: 'internal_stress',
          targetOutcome: 'preempted_yielded',
        },
        servedAdapterId: 'primary-glm',
      })
    }
    expect(primary.calls()).toBe(2)
    expect(weakerFallback.calls()).toBe(0)
  })

  test('overflows explicitly when post-preemption primary validation retry is exhausted', async () => {
    const primary = mockAdapter('empty-primary-glm', [
      {
        content: '',
        finishReason: 'stop',
        servedModel: 'primary-glm-model',
        usage: { completionTokens: 0, promptTokens: 1, totalTokens: 1 },
      },
    ])
    const weakerFallback = mockAdapter('weaker-fallback-after-retry', [
      undefined,
    ])
    const registry = new InferenceProviderRegistry()
    registry.register(primary.adapter)
    registry.register(weakerFallback.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(
        request(KHALA_MODEL_ID),
        completeOp,
        {
          plan: () => ['empty-primary-glm', 'weaker-fallback-after-retry'],
          preemption: {
            demandClass: 'external',
            preempt: () =>
              Effect.succeed({
                evidenceRef:
                  'scheduler.preemption.internal_stress.retry_exhausted',
                reason: 'external_reserved_headroom_unavailable',
                targetDemandClass: 'internal_stress' as const,
                targetOutcome: 'preempted_yielded' as const,
              }),
            reason: 'glm_global_internal_stress_active',
            reservedExternalHeadroomAvailable: false,
          },
          registry,
          sleep: noSleep,
        },
        nonEmptyAssistantValidator,
      ),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: 'empty_assistant_content',
        primaryAdapterId: 'empty-primary-glm',
        schedulerPreemption: {
          evidenceRef: 'scheduler.preemption.internal_stress.retry_exhausted',
          targetDemandClass: 'internal_stress',
          targetOutcome: 'preempted_yielded',
        },
        servedAdapterId: 'weaker-fallback-after-retry',
      })
    }
    expect(primary.calls()).toBe(2)
    expect(weakerFallback.calls()).toBe(1)
  })

  test('treats a tool-required response without tool calls as a failed lane', async () => {
    const noToolLane = mockAdapter('no-tool-lane', [
      {
        content: 'I can do that.',
        finishReason: 'stop',
        servedModel: 'no-tool-model',
        usage: { completionTokens: 4, promptTokens: 8, totalTokens: 12 },
      },
    ])
    const toolLane = mockAdapter('tool-lane', [
      {
        content: '',
        finishReason: 'tool_calls',
        servedModel: 'tool-model',
        toolCalls: [
          {
            function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
            id: 'call_bash',
            type: 'function',
          },
        ],
        usage: { completionTokens: 4, promptTokens: 8, totalTokens: 12 },
      },
    ])
    const registry = new InferenceProviderRegistry()
    registry.register(noToolLane.adapter)
    registry.register(toolLane.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(
        {
          ...request(KHALA_MODEL_ID),
          passthroughParams: { tool_choice: 'required' },
        },
        completeOp,
        {
          plan: () => ['no-tool-lane', 'tool-lane'],
          registry,
          sleep: noSleep,
        },
        toolRequiredValidator,
      ),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route.fallbackReason).toBe(
        'tool_required_no_tool_calls',
      )
      expect(result.success.route.servedAdapterId).toBe('tool-lane')
      expect(result.success.value.value.toolCalls).toHaveLength(1)
    }
    expect(noToolLane.calls()).toBe(1)
    expect(toolLane.calls()).toBe(1)
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

  test('retries a retryable GLM lane failure once before overflowing', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [
      new InferenceAdapterError({
        adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        httpStatus: 500,
        kind: 'provider_error',
        reason: 'provider_error: transient GLM replica fault',
        retryable: true,
      }),
      undefined,
    ])
    const fallback = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(fallback.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: () => [
          HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
          VERTEX_GEMINI_ADAPTER_ID,
        ],
        registry,
        retry: { maxRetriesPerLane: 1 },
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route.servedAdapterId).toBe(
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      )
      expect(result.success.route.fallbackReason).toBe(null)
    }
    expect(glm.calls()).toBe(2)
    expect(fallback.calls()).toBe(0)
  })

  test('quarantines an unhealthy GLM lane before dispatch and overflows to a healthy lane', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [undefined])
    const gemini = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(gemini.adapter)
    const events: Array<DispatchFailureTelemetryEvent> = []

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        failureTelemetry: event => {
          events.push(event)
        },
        plan: () => [
          HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
          VERTEX_GEMINI_ADAPTER_ID,
        ],
        registry,
        routingSignals: id =>
          id === HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID
            ? { laneHealth: 'quarantined', providerHealthScore: 0.1 }
            : { laneHealth: 'healthy', providerHealthScore: 0.91 },
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route.primaryAdapterId).toBe(
        VERTEX_GEMINI_ADAPTER_ID,
      )
      expect(result.success.route.servedAdapterId).toBe(VERTEX_GEMINI_ADAPTER_ID)
    }
    expect(glm.calls()).toBe(0)
    expect(gemini.calls()).toBe(1)
    expect(events).toContainEqual({
      adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      classifier: 'provider_error',
      kind: 'quarantined',
      retryable: true,
      stage: 'health_quarantine',
    })
  })

  test('reports a retryable lane-wide breaker failure when every registered lane is quarantined', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [undefined])
    const fallback = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(fallback.adapter)
    const events: Array<DispatchFailureTelemetryEvent> = []

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        failureTelemetry: event => {
          events.push(event)
        },
        plan: () => [
          HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
          VERTEX_GEMINI_ADAPTER_ID,
        ],
        registry,
        routingSignals: id =>
          id === HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID
            ? { laneHealth: 'quarantined' }
            : { laneHealth: 'unhealthy' },
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('lane_quorum_unhealthy')
      expect(result.failure.retryable).toBe(true)
    }
    expect(glm.calls()).toBe(0)
    expect(fallback.calls()).toBe(0)
    expect(events).toEqual([
      {
        adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        classifier: 'provider_error',
        kind: 'quarantined',
        retryable: true,
        stage: 'health_quarantine',
      },
      {
        adapterId: VERTEX_GEMINI_ADAPTER_ID,
        classifier: 'provider_error',
        kind: 'unhealthy',
        retryable: true,
        stage: 'health_quarantine',
      },
    ])
  })

  test('SLO shedding yields internal stress while external demand still serves', async () => {
    const internalLane = mockAdapter('internal-lane', [undefined])
    const externalLane = mockAdapter('external-lane', [undefined])
    const internalRegistry = new InferenceProviderRegistry()
    const externalRegistry = new InferenceProviderRegistry()
    internalRegistry.register(internalLane.adapter)
    externalRegistry.register(externalLane.adapter)

    const internalResult = await runResult(
      dispatchWithOverflow(request(KHALA_MODEL_ID), completeOp, {
        plan: () => ['internal-lane'],
        registry: internalRegistry,
        shedding: {
          demandClass: 'internal_stress',
          slo: { breached: true, reason: 'external_ttft_p90' },
        },
        sleep: noSleep,
      }),
    )
    const externalResult = await runResult(
      dispatchWithOverflow(request(KHALA_MODEL_ID), completeOp, {
        plan: () => ['external-lane'],
        registry: externalRegistry,
        shedding: {
          demandClass: 'external',
          slo: { breached: true, reason: 'external_ttft_p90' },
        },
        sleep: noSleep,
      }),
    )

    expect(internalResult._tag).toBe('Failure')
    if (internalResult._tag === 'Failure') {
      expect(internalResult.failure.kind).toBe('internal_stress_yielded')
      expect(internalResult.failure.httpStatus).toBe(429)
      expect(internalResult.failure.reason).toBe(
        'internal_stress yielded because external SLO is breached: external_ttft_p90',
      )
    }
    expect(externalResult._tag).toBe('Success')
    expect(internalLane.calls()).toBe(0)
    expect(externalLane.calls()).toBe(1)
  })

  test('bounded external hedging uses one different warm lane when primary TTFT breaches P99', async () => {
    const slowPrimary = mockAdapter('slow-primary', [undefined])
    const warmHedge = mockAdapter('warm-hedge', [undefined])
    const coldOverflow = mockAdapter('cold-overflow', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(slowPrimary.adapter)
    registry.register(warmHedge.adapter)
    registry.register(coldOverflow.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        hedging: {
          demandClass: 'external',
          enabled: true,
          ttftP99ThresholdMs: 750,
        },
        plan: () => ['slow-primary', 'warm-hedge', 'cold-overflow'],
        registry,
        routingSignals: id =>
          id === 'slow-primary'
            ? { laneHealth: 'healthy', ttftP99Ms: 1_200, warmState: 'warm' }
            : id === 'warm-hedge'
              ? { laneHealth: 'healthy', ttftP99Ms: 180, warmState: 'warm' }
              : { laneHealth: 'healthy', ttftP99Ms: 200, warmState: 'cold' },
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: 'hedged_ttft_p99_breach',
        primaryAdapterId: 'slow-primary',
        servedAdapterId: 'warm-hedge',
      })
    }
    expect(slowPrimary.calls()).toBe(0)
    expect(warmHedge.calls()).toBe(1)
    expect(coldOverflow.calls()).toBe(0)
  })

  test('failure telemetry records public-safe provider, empty-content, and 429 shapes', async () => {
    const providerFailure = mockAdapter('provider-failure', [
      new InferenceAdapterError({
        adapterId: 'provider-failure',
        httpStatus: 500,
        kind: 'provider_error',
        reason: 'provider_error',
        retryable: true,
      }),
    ])
    const emptyLane = mockAdapter('empty-lane', [
      {
        content: '',
        finishReason: 'stop',
        servedModel: 'empty-model',
        usage: { completionTokens: 0, promptTokens: 1, totalTokens: 1 },
      },
    ])
    const rateLimited = mockAdapter('rate-limited', [
      new InferenceAdapterError({
        adapterId: 'rate-limited',
        httpStatus: 429,
        kind: 'rate_limited',
        reason: 'singleflight saturated',
        retryable: true,
      }),
    ])
    const healthy = mockAdapter('healthy', [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(providerFailure.adapter)
    registry.register(emptyLane.adapter)
    registry.register(rateLimited.adapter)
    registry.register(healthy.adapter)
    const events: Array<DispatchFailureTelemetryEvent> = []

    const result = await runResult(
      dispatchWithOverflowWithMetadata(
        request(KHALA_MODEL_ID),
        completeOp,
        {
          failureTelemetry: event => {
            events.push(event)
          },
          plan: () => [
            'provider-failure',
            'empty-lane',
            'rate-limited',
            'healthy',
          ],
          registry,
          sleep: noSleep,
        },
        nonEmptyAssistantValidator,
      ),
    )

    expect(result._tag).toBe('Success')
    expect(events).toEqual([
      {
        adapterId: 'provider-failure',
        classifier: 'provider_error',
        httpStatus: 500,
        kind: 'provider_error',
        retryable: true,
        stage: 'adapter_error',
      },
      {
        adapterId: 'empty-lane',
        classifier: 'empty_content',
        kind: 'empty_assistant_content',
        retryable: true,
        stage: 'validation_failure',
      },
      {
        adapterId: 'rate-limited',
        classifier: 'rate_limited_429',
        httpStatus: 429,
        kind: 'rate_limited',
        retryable: true,
        stage: 'adapter_error',
      },
      {
        adapterId: 'healthy',
        classifier: 'fallback',
        kind: 'rate_limited',
        retryable: true,
        stage: 'fallback',
      },
    ])
  })

  test('bounded failure telemetry snapshots provider, empty-content, fallback, invalid-tool, and 429 classes', () => {
    let nowMs = 1_000
    const telemetry = makeBoundedDispatchFailureTelemetry({
      maxEvents: 10,
      nowMs: () => nowMs,
      windowMs: 1_000,
    })

    telemetry.record({
      adapterId: 'provider',
      classifier: 'provider_error',
      httpStatus: 500,
      kind: 'provider_error',
      retryable: true,
      stage: 'adapter_error',
    })
    telemetry.record({
      adapterId: 'empty',
      classifier: 'empty_content',
      kind: 'empty_assistant_content',
      retryable: true,
      stage: 'validation_failure',
    })
    telemetry.record({
      adapterId: 'fallback',
      classifier: 'fallback',
      kind: 'provider_error',
      retryable: true,
      stage: 'fallback',
    })
    telemetry.record({
      adapterId: 'tools',
      classifier: 'invalid_tool',
      kind: 'tool_required_no_tool_calls',
      retryable: true,
      stage: 'validation_failure',
    })
    telemetry.record({
      adapterId: 'limited',
      classifier: 'rate_limited_429',
      httpStatus: 429,
      kind: 'rate_limited',
      retryable: true,
      stage: 'adapter_error',
    })

    expect(telemetry.snapshot().counts).toEqual({
      empty_content: 1,
      fallback: 1,
      invalid_tool: 1,
      provider_error: 1,
      rate_limited_429: 1,
    })

    nowMs = 2_001
    expect(telemetry.snapshot().counts).toEqual({
      empty_content: 0,
      fallback: 0,
      invalid_tool: 0,
      provider_error: 0,
      rate_limited_429: 0,
    })
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

  test('GLM lane quorum unhealthy overflows without exposing private route data', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [
      new InferenceAdapterError({
        adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        adapterRouteMetadata: {
          glmSaturationPolicy: 'queue_then_overflow',
          queueWaitMs: 0,
          replicaBusyReason: 'lane_quorum_unhealthy',
          replicaFallbackReason: 'lane_quorum_unhealthy',
          replicaHealthScore: 0,
        },
        httpStatus: 503,
        kind: 'lane_quorum_unhealthy',
        reason: 'hydralisk GLM lane quorum unhealthy',
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
          queueWaitMs: 0,
          replicaBusyReason: 'lane_quorum_unhealthy',
          replicaFallbackReason: 'lane_quorum_unhealthy',
          replicaHealthScore: 0,
        },
        fallbackReason: 'lane_quorum_unhealthy',
        primaryAdapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        servedAdapterId: VERTEX_GEMINI_ADAPTER_ID,
      })
    }
    expect(glm.calls()).toBe(1)
    expect(gemini.calls()).toBe(1)
  })

  test('tool/agent Khala Spot-death/5xx skips GPT-OSS and falls through to Fireworks when GLM dies (#6259)', async () => {
    // Tool/agent Khala is GLM/self-hosted first, but a retryable Hydralisk death
    // must still overflow to an armed tool-capable fallback and never hit GPT-OSS.
    const glmDead = new InferenceAdapterError({
      adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      kind: 'transport_error',
      reason: 'retryable: hydralisk transport error (spot host preempted)',
      retryable: true,
    })
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [glmDead])
    const gptOss120b = mockAdapter(HYDRALISK_GPT_OSS_120B_ADAPTER_ID, [
      err(HYDRALISK_GPT_OSS_120B_ADAPTER_ID, true, 503),
    ])
    const gptOss20b = mockAdapter(HYDRALISK_ADAPTER_ID, [undefined])
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(gptOss120b.adapter)
    registry.register(gptOss20b.adapter)
    registry.register(fireworks.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: model =>
          selectAdapterPlanForKhalaToolRequest(
            model,
            selectAdapterPlanForKhalaBacking(
              model,
              KHALA_BACKING_HYDRALISK_GPT_OSS,
            ),
          ),
        registry,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route.primaryAdapterId).toBe(
        HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      )
      expect(result.success.route.servedAdapterId).toBe(FIREWORKS_ADAPTER_ID)
      expect(result.success.value.value.servedModel).toBe(
        `served-by-${FIREWORKS_ADAPTER_ID}`,
      )
    }
    expect(glm.calls()).toBe(1)
    expect(gptOss120b.calls()).toBe(0)
    expect(gptOss20b.calls()).toBe(0)
    expect(fireworks.calls()).toBe(1)
  })

  test('conversational Khala starts on Gemini before GLM/OpenRouter and keeps GPT-OSS out of the main thread', async () => {
    const glmDead = new InferenceAdapterError({
      adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      kind: 'transport_error',
      reason: 'retryable: hydralisk transport error (spot host preempted)',
      retryable: true,
    })
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [glmDead])
    const openRouter = mockAdapter(OPENROUTER_KHALA_FALLBACK_ADAPTER_ID, [
      {
        content: 'OpenRouter GLM fallback answer',
        finishReason: 'stop',
        servedModel: 'openrouter/glm-class',
        usage: { completionTokens: 6, promptTokens: 4, totalTokens: 10 },
      },
    ])
    const gemini = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [
      {
        content: 'Gemini warm overflow answer',
        finishReason: 'stop',
        servedModel: 'gemini-3.5-flash',
        usage: { completionTokens: 5, promptTokens: 4, totalTokens: 9 },
      },
    ])
    const gptOss120b = mockAdapter(HYDRALISK_GPT_OSS_120B_ADAPTER_ID, [
      undefined,
    ])
    const gptOss20b = mockAdapter(HYDRALISK_ADAPTER_ID, [undefined])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(openRouter.adapter)
    registry.register(gptOss120b.adapter)
    registry.register(gptOss20b.adapter)
    registry.register(gemini.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: model =>
          selectAdapterPlanForKhalaBacking(
            model,
            KHALA_BACKING_HYDRALISK_GPT_OSS,
          ),
        registry,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route).toMatchObject({
        fallbackReason: null,
        primaryAdapterId: VERTEX_GEMINI_ADAPTER_ID,
        servedAdapterId: VERTEX_GEMINI_ADAPTER_ID,
      })
      expect(result.success.value.value.content).toBe(
        'Gemini warm overflow answer',
      )
      expect(result.success.value.value.servedModel).toBe(
        'gemini-3.5-flash',
      )
    }
    expect(glm.calls()).toBe(0)
    expect(openRouter.calls()).toBe(0)
    expect(gptOss120b.calls()).toBe(0)
    expect(gptOss20b.calls()).toBe(0)
    expect(gemini.calls()).toBe(1)
  })

  test('conversational Khala falls through Gemini and Fireworks before trying GLM/OpenRouter', async () => {
    const glm = mockAdapter(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, [
      err(HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID, true, 503),
    ])
    const gemini = mockAdapter(VERTEX_GEMINI_ADAPTER_ID, [
      err(VERTEX_GEMINI_ADAPTER_ID, true, 503),
    ])
    const fireworks = mockAdapter(FIREWORKS_ADAPTER_ID, [
      err(FIREWORKS_ADAPTER_ID, true, 503),
    ])
    const openRouter = mockAdapter(OPENROUTER_KHALA_FALLBACK_ADAPTER_ID, [
      undefined,
    ])
    const registry = new InferenceProviderRegistry()
    registry.register(glm.adapter)
    registry.register(openRouter.adapter)
    registry.register(gemini.adapter)
    registry.register(fireworks.adapter)

    const result = await runResult(
      dispatchWithOverflowWithMetadata(request(KHALA_MODEL_ID), completeOp, {
        plan: model =>
          selectAdapterPlanForKhalaBacking(
            model,
            KHALA_BACKING_HYDRALISK_GPT_OSS,
          ),
        registry,
        sleep: noSleep,
      }),
    )

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.route.primaryAdapterId).toBe(
        VERTEX_GEMINI_ADAPTER_ID,
      )
      expect(result.success.route.servedAdapterId).toBe(
        OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
      )
      expect(result.success.value.value.servedModel).toBe(
        `served-by-${OPENROUTER_KHALA_FALLBACK_ADAPTER_ID}`,
      )
    }
    expect(glm.calls()).toBe(1)
    expect(gemini.calls()).toBe(1)
    expect(openRouter.calls()).toBe(1)
    expect(fireworks.calls()).toBe(1)
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
