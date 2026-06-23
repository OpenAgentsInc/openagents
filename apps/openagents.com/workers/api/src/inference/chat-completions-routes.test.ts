import { MemoryStreamStore } from '@openagentsinc/durable-stream'
import { Effect, Option } from 'effect'
import { describe, expect, test } from 'vitest'

import type { InferenceReceiptReadStore } from '../inference-receipts'
import { makePublicInferenceReceiptRoutes } from '../public-inference-receipt-routes'
import {
  NOT_MEASURED,
  decodeKhalaTelemetryBlock,
} from './khala-telemetry'

import {
  type ChatCompletionsDeps,
  type InferenceAuth,
  type InferenceBalanceReader,
  handleChatCompletions,
  isInferenceGatewayEnabled,
} from './chat-completions-routes'
import {
  acceptanceContractGuidanceForRequest,
  crossyRoadAcceptanceSpec,
  acceptanceContractGuidanceForSpec,
} from './acceptance-spec'
import { replayFromOffset } from './durable-inference-proxy'
import { decideFairShare, decideSpendCap } from './inference-abuse-controls'
import {
  BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
  GOOD_CROSSY_ROAD_HTML,
} from './khala-code-verifier.fixtures'
import {
  KHALA_CODE_HEADLESS_COMMAND_REF,
  KHALA_CODE_VERIFIER_WORKER_ID,
} from './khala-code-verifier'
import { type MeteringContext, type MeteringHook } from './metering-hook'
import {
  FIREWORKS_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  selectAdapterPlan,
} from './model-router'
import {
  ALL_LANES_UNARMED,
  resolveSupplyLaneArming,
} from './model-serving-policy'
import {
  KHALA_IDENTITY_STATEMENT,
  KHALA_IDENTITY_SYSTEM_PROMPT,
} from './khala-identity'
import { KHALA_CODE_MODEL_ID, KHALA_MINI_MODEL_ID } from './pricing'
import {
  OA_COMPONENT_SSE_EVENT,
  type ComponentRepairReask,
} from './khala-component-channel'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'
import { STUB_ECHO_ADAPTER_ID, stubEchoAdapter } from './stub-echo-adapter'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const authOk: InferenceAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: InferenceAuth = async () => undefined
const fundedBalance: InferenceBalanceReader = async () => 100_000
const emptyBalance: InferenceBalanceReader = async () => 0

const registryWithStub = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(stubEchoAdapter)
  return registry
}

const baseDeps = (
  overrides: Partial<ChatCompletionsDeps> = {},
): ChatCompletionsDeps => ({
  authenticate: authOk,
  enabled: true,
  // Deterministic telemetry clock (book P0-1): a fixed wall-clock so the
  // `openagents` telemetry block's `totalWallClockMs` is a stable measured `0`
  // in tests (a measured zero, NOT the `not_measured` sentinel) instead of a
  // flaky real elapsed time. Tests that assert real TTFT inject a stepping clock.
  nowEpochMillis: () => 0,
  readAvailableMsat: fundedBalance,
  registry: registryWithStub(),
  ...overrides,
})

const chatRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

const helloBody = {
  messages: [{ content: 'hello world', role: 'user' }],
  model: 'stub-model',
}

describe('inference gateway feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isInferenceGatewayEnabled(undefined)).toBe(false)
    expect(isInferenceGatewayEnabled('')).toBe(false)
    expect(isInferenceGatewayEnabled('false')).toBe(false)
    expect(isInferenceGatewayEnabled('0')).toBe(false)
    expect(isInferenceGatewayEnabled('true')).toBe(true)
    expect(isInferenceGatewayEnabled('TRUE')).toBe(true)
    expect(isInferenceGatewayEnabled('1')).toBe(true)
    expect(isInferenceGatewayEnabled('on')).toBe(true)
  })
})

describe('POST /v1/chat/completions', () => {
  test('is inert (404) when the gateway flag is disabled', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('inference_gateway_disabled')
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ authenticate: authNone }),
      ),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('unauthorized')
  })

  test('rejects with 402 when the credit balance is insufficient', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ readAvailableMsat: emptyBalance }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as {
      error: string
      availableMsat: number
    }
    expect(body.error).toBe('insufficient_credits')
    expect(body.availableMsat).toBe(0)
  })

  test('zero-balance + free-allowance eligible => NOT 402 (free bypass)', async () => {
    // A zero-balance account whose (account, model) is free-eligible with a
    // remaining owner pool must reach dispatch, not be rejected by the balance
    // gate; the metering hook owns the authoritative free accrual after that.
    const seen: Array<{ accountRef: string; model: string }> = []
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFreeAllowance: async (accountRef, model) => {
            seen.push({ accountRef, model })
            return { eligible: true }
          },
          readAvailableMsat: emptyBalance,
        }),
      ),
    )
    expect(response.status).toBe(200)
    expect(seen).toHaveLength(1)
  })

  test('zero-balance + free-allowance NOT eligible => still 402', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFreeAllowance: async () => ({ eligible: false }),
          readAvailableMsat: emptyBalance,
        }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('insufficient_credits')
  })

  test('funded balance never calls the free-allowance pre-flight', async () => {
    let calls = 0
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFreeAllowance: async () => {
            calls += 1
            return { eligible: true }
          },
        }),
      ),
    )
    expect(response.status).toBe(200)
    expect(calls).toBe(0)
  })

  test('rejects a malformed body with 400', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({ model: 'stub-model', messages: [] }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('invalid_request')
  })

  test('dispatches to the registered stub adapter and returns OpenAI shape', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          newId: () => 'chatcmpl-fixed',
          nowEpochSeconds: () => 1_700_000_000,
        }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      id: string
      object: string
      created: number
      model: string
      choices: ReadonlyArray<{
        index: number
        finish_reason: string
        message: { role: string; content: string }
      }>
      usage: {
        prompt_tokens: number
        completion_tokens: number
        total_tokens: number
      }
    }
    expect(body.object).toBe('chat.completion')
    expect(body.id).toBe('chatcmpl-fixed')
    expect(body.created).toBe(1_700_000_000)
    expect(body.model).toBe('stub-model')
    expect(body.choices[0]?.message.role).toBe('assistant')
    expect(body.choices[0]?.message.content).toBe('hello world')
    expect(body.choices[0]?.finish_reason).toBe('stop')
    expect(body.usage.prompt_tokens).toBe(2)
    expect(body.usage.completion_tokens).toBe(2)
    expect(body.usage.total_tokens).toBe(4)
  })

  test('returns model_unavailable when no adapter is registered for the route', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ registry: new InferenceProviderRegistry() }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })

  test('invokes the metering hook with receipt-first provider usage', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps({ meteringHook })),
    )

    expect(captured).toHaveLength(1)
    const context = captured[0]
    expect(context?.accountRef).toBe('agent:test-user')
    expect(context?.adapterId).toBe(STUB_ECHO_ADAPTER_ID)
    expect(context?.requestedModel).toBe('stub-model')
    expect(context?.streamed).toBe(false)
    expect(context?.usage.totalTokens).toBe(4)
    // Funding kind defaults to card, and the request id is threaded for
    // idempotency-keyed metering.
    expect(context?.fundingKind).toBe('card')
    expect(typeof context?.requestId).toBe('string')
    expect((context?.requestId ?? '').length).toBeGreaterThan(0)
  })

  test('threads the resolved bitcoin funding kind into the metering hook', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          meteringHook,
          resolveFundingKind: async () => 'bitcoin',
        }),
      ),
    )

    expect(captured).toHaveLength(1)
    expect(captured[0]?.fundingKind).toBe('bitcoin')
  })

  test('streams OpenAI-compatible SSE frames and meters from the terminal usage frame', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    const response = await run(
      handleChatCompletions(
        chatRequest({ ...helloBody, stream: true }),
        baseDeps({ meteringHook }),
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')

    const text = await response.text()
    expect(text).toContain('"object":"chat.completion.chunk"')
    expect(text).toContain('"content":"hello world"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)

    expect(captured).toHaveLength(1)
    expect(captured[0]?.streamed).toBe(true)
    expect(captured[0]?.usage.completionTokens).toBe(2)
  })

  // STREAMING OPENAGENTS DISCLOSURE (M0 / #6008 follow-up) ------------------
  // The SSE path carries the SAME non-breaking `openagents` block the
  // non-streaming path emits, built by the SAME builder, attached to exactly the
  // FINAL `chat.completion.chunk` frame. Non-Khala streams omit it entirely.

  // Parse the `chat.completion.chunk` frames from an SSE body (ignores [DONE]).
  const parseSseChunks = (
    text: string,
  ): ReadonlyArray<{ openagents?: unknown }> =>
    text
      .split('\n\n')
      .map(block => block.replace(/^data: /u, '').trim())
      .filter(payload => payload !== '' && payload !== '[DONE]')
      .map(payload => JSON.parse(payload) as { openagents?: unknown })

  test('a streamed Khala request carries the openagents block on exactly the final chunk', async () => {
    // khala-mini classifies to the Gemini lane; register an echo adapter under
    // that lane id so the default plan resolves it (mirrors the non-streaming
    // Khala disclosure test).
    const streamRegistry = new InferenceProviderRegistry()
    streamRegistry.register(echoAdapter(VERTEX_GEMINI_ADAPTER_ID))
    const nonStreamRegistry = new InferenceProviderRegistry()
    nonStreamRegistry.register(echoAdapter(VERTEX_GEMINI_ADAPTER_ID))

    const khalaBody = {
      messages: [{ content: 'hello world', role: 'user' }],
      model: KHALA_MINI_MODEL_ID,
    }

    const streamed = await run(
      handleChatCompletions(
        chatRequest({ ...khalaBody, stream: true }),
        baseDeps({ lanePlan: selectAdapterPlan, registry: streamRegistry }),
      ),
    )
    expect(streamed.status).toBe(200)
    expect(streamed.headers.get('content-type')).toContain('text/event-stream')

    const text = await streamed.text()
    const frames = parseSseChunks(text)
    expect(frames.length).toBeGreaterThan(1)
    // Exactly the final frame carries the disclosure; all earlier frames omit it.
    frames
      .slice(0, -1)
      .forEach(frame => expect(frame.openagents).toBeUndefined())
    const finalOpenagents = frames[frames.length - 1]?.openagents

    // The streamed block equals the non-streaming block for the same request.
    const nonStreamed = await run(
      handleChatCompletions(
        chatRequest(khalaBody),
        baseDeps({ lanePlan: selectAdapterPlan, registry: nonStreamRegistry }),
      ),
    )
    const nonStreamedBody = (await nonStreamed.json()) as {
      openagents?: { telemetry?: Record<string, unknown> }
    }
    // The PRIOR (non-telemetry) disclosure fields are identical streamed vs
    // non-streamed — and byte-for-byte the prior contract (non-breaking).
    const stripAdditiveReceiptFields = (
      block: { routing?: unknown; telemetry?: unknown } | undefined,
    ): Record<string, unknown> => {
      const {
        routing: _routing,
        telemetry: _telemetry,
        ...rest
      } = (block ?? {}) as Record<string, unknown>
      return rest
    }
    expect(
      stripAdditiveReceiptFields(
        finalOpenagents as { routing?: unknown; telemetry?: unknown } | undefined,
      ),
    ).toEqual(
      stripAdditiveReceiptFields(
        nonStreamedBody.openagents as
          | { routing?: unknown; telemetry?: unknown }
          | undefined,
      ),
    )
    expect(
      stripAdditiveReceiptFields(
        finalOpenagents as { routing?: unknown; telemetry?: unknown } | undefined,
      ),
    ).toEqual({
      lane: 'gemini',
      requested_model: KHALA_MINI_MODEL_ID,
      served_model: KHALA_MINI_MODEL_ID,
      verification: 'none',
      worker: VERTEX_GEMINI_ADAPTER_ID,
    })
    // The additive telemetry block correctly DIFFERS by request shape: the
    // buffered-stream path is `interactive_stream`, the non-stream path is
    // `async_job` (book P0-1 request class). Both carry the lifecycle summary.
    expect(
      (finalOpenagents as { telemetry?: Record<string, unknown> }).telemetry,
    ).toMatchObject({
      detailRef: null,
      executedVerdict: 'not_executed',
      requestClass: 'interactive_stream',
      schemaVersion: 'openagents.khala.telemetry.v1',
      totalWallClockMs: 0,
      verificationClass: 'none',
    })
    expect(nonStreamedBody.openagents?.telemetry).toMatchObject({
      requestClass: 'async_job',
      schemaVersion: 'openagents.khala.telemetry.v1',
    })
  })

  test('a streamed Khala request uses the terminal served model for disclosure and metering', async () => {
    const servedModel = 'gemini-3.5-flash'
    const usage = {
      completionTokens: 2,
      promptTokens: 2,
      totalTokens: 4,
    }
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: () =>
        Effect.sync(() => ({
          content: 'hello world',
          finishReason: 'stop',
          servedModel,
          usage,
        })),
      id: VERTEX_GEMINI_ADAPTER_ID,
      stream: () =>
        Effect.sync(() => [
          { contentDelta: 'hello world' },
          { contentDelta: '', finishReason: 'stop', servedModel, usage },
        ]),
    })
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hello world', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
          stream: true,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, meteringHook, registry }),
      ),
    )
    expect(response.status).toBe(200)

    const frames = parseSseChunks(await response.text()) as ReadonlyArray<{
      openagents?: { served_model?: string }
    }>
    expect(frames[frames.length - 1]?.openagents?.served_model).toBe(
      servedModel,
    )
    expect(captured[0]?.requestedModel).toBe(KHALA_MINI_MODEL_ID)
    expect(captured[0]?.servedModel).toBe(servedModel)
  })

  test('a streamed non-Khala request omits the openagents block', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({ ...helloBody, stream: true }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(200)
    const text = await response.text()
    // The disclosure field never appears anywhere in the non-Khala SSE body.
    expect(text).not.toContain('openagents')
  })

  test('maps a provider adapter failure to a 502 provider_error', async () => {
    const failingAdapter: InferenceProviderAdapter = {
      complete: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'boom',
            reason: 'upstream down',
          }),
        ),
      id: STUB_ECHO_ADAPTER_ID,
      stream: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'boom',
            reason: 'upstream down',
          }),
        ),
    }
    const registry = new InferenceProviderRegistry()
    registry.register(failingAdapter)

    const response = await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps({ registry })),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('provider_error')
    expect(body.reason).toBe('upstream down')
  })

  // ROUTING & SUPPLY SELECTION (#5482) -------------------------------------
  // The route accepts a multi-lane `lanePlan` and dispatches across it with
  // bounded-backoff overflow. These exercise the route wiring (the pure router
  // logic itself is covered in model-router.test.ts).

  const echoAdapter = (id: string): InferenceProviderAdapter => ({
    ...stubEchoAdapter,
    id,
  })
  const failing = (
    id: string,
    retryable: boolean,
  ): InferenceProviderAdapter => ({
    complete: () =>
      Effect.fail(
        new InferenceAdapterError({
          adapterId: id,
          reason: `${id} down`,
          retryable,
        }),
      ),
    id,
    stream: () =>
      Effect.fail(
        new InferenceAdapterError({
          adapterId: id,
          reason: `${id} down`,
          retryable,
        }),
      ),
  })

  test('overflows to the next lane on a retryable failure and meters the served lane', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })
    const registry = new InferenceProviderRegistry()
    registry.register(failing('primary', true))
    registry.register(echoAdapter('overflow'))

    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          dispatch: { sleep: () => Effect.void },
          lanePlan: () => ['primary', 'overflow'],
          meteringHook,
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    // Metering attributes the request to the lane that actually served it.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.adapterId).toBe('overflow')
  })

  test('a Khala overflow receipt carries region, provider health score, and fallback reason', async () => {
    const registry = new InferenceProviderRegistry()
    registry.register(
      failing('primary', true),
    )
    registry.register(echoAdapter('overflow'))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hello world', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({
          dispatch: {
            routingSignals: id =>
              id === 'overflow'
                ? { providerHealthScore: 0.91, region: 'us-central1' }
                : undefined,
            sleep: () => Effect.void,
          },
          lanePlan: () => ['primary', 'overflow'],
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      openagents?: {
        routing?: {
          fallback_reason: string | null
          provider_health_score: number | typeof NOT_MEASURED
          region: string | typeof NOT_MEASURED
        }
        telemetry?: Record<string, unknown>
        worker: string
      }
    }

    expect(body.openagents?.worker).toBe('overflow')
    expect(body.openagents?.routing).toEqual({
      fallback_reason: 'retryable_provider_error',
      provider_health_score: 0.91,
      region: 'us-central1',
    })
    expect(body.openagents?.telemetry).toMatchObject({
      requestClass: 'async_job',
      schemaVersion: 'openagents.khala.telemetry.v1',
    })
  })

  test('surfaces a non-retryable failure as 502 without overflow', async () => {
    const overflow = echoAdapter('overflow')
    let overflowCalls = 0
    const registry = new InferenceProviderRegistry()
    registry.register(failing('primary', false))
    registry.register({
      ...overflow,
      complete: request => {
        overflowCalls += 1
        return overflow.complete(request)
      },
    })

    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          dispatch: { sleep: () => Effect.void },
          lanePlan: () => ['primary', 'overflow'],
          registry,
        }),
      ),
    )
    expect(response.status).toBe(502)
    expect(overflowCalls).toBe(0)
  })

  // KHALA DISCLOSURE BLOCK (M0 / #6008) ------------------------------------
  // A Khala model id is one endpoint over a pool; the response carries a
  // non-breaking `openagents` block disclosing which concrete model/worker
  // actually served it. Non-Khala responses are unchanged.

  test('a Khala request returns the openagents disclosure block', async () => {
    // khala-mini classifies to the Gemini lane; register an echo adapter under
    // that lane id so the default plan resolves it.
    const registry = new InferenceProviderRegistry()
    registry.register(echoAdapter(VERTEX_GEMINI_ADAPTER_ID))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hello world', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        // Use the real planner (as the Worker wires it) so khala-mini routes to
        // its Gemini backing lane.
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      model: string
      openagents?: {
        requested_model: string
        served_model: string
        worker: string
        lane: string
        verification: string
      }
    }
    expect(body.model).toBe(KHALA_MINI_MODEL_ID)
    // The prior disclosure fields are byte-for-byte unchanged (non-breaking).
    expect(body.openagents).toMatchObject({
      lane: 'gemini',
      requested_model: KHALA_MINI_MODEL_ID,
      served_model: KHALA_MINI_MODEL_ID,
      verification: 'none',
      worker: VERTEX_GEMINI_ADAPTER_ID,
    })
    // The additive telemetry block: a non-coding Khala lane (async, non-stream)
    // carries the lifecycle summary with verification class `none`.
    expect(
      (body.openagents as { telemetry?: Record<string, unknown> } | undefined)
        ?.telemetry,
    ).toMatchObject({
      requestClass: 'async_job',
      schemaVersion: 'openagents.khala.telemetry.v1',
      totalWallClockMs: 0,
      verificationClass: 'none',
    })
  })

  // EPIC #6017 honest downgrade: the hot Worker route cannot launch a browser, so it
  // does NOT execute the artifact. A prescreen-passing artifact comes back `unverified`
  // (we did not run it) with scalar_reward 0 — NEVER test_passed / 1 from regex.
  test('a khala-code prescreen-passing artifact returns UNVERIFIED (not certified) with receipt metadata', async () => {
    const registry = new InferenceProviderRegistry()
    registry.register(echoAdapter(FIREWORKS_ADAPTER_ID))
    const meteringHook: MeteringHook = () =>
      Effect.sync(() => ({
        metered: true,
        receiptRef: 'receipt.inference.charge.chatcmpl-khala-code-pass',
      }))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: GOOD_CROSSY_ROAD_HTML, role: 'user' }],
          model: KHALA_CODE_MODEL_ID,
        }),
        baseDeps({
          lanePlan: selectAdapterPlan,
          meteringHook,
          newId: () => 'chatcmpl-khala-code-pass',
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      model: string
      openagents?: {
        receipt: string
        receipt_url: string
        requested_model: string
        reward_handoff: string
        route: string
        rubric: {
          failed_checks: ReadonlyArray<string>
          passed_checks: ReadonlyArray<string>
          ref: string
        }
        scalar_reward: number
        served_model: string
        verification: string
        verification_command: string
        verification_receipt: string
        verified: boolean
        executed: boolean
        worker: string
        workers: ReadonlyArray<string>
      }
    }

    expect(body.model).toBe(KHALA_CODE_MODEL_ID)
    expect(body.openagents).toMatchObject({
      executed: false,
      lane: 'open',
      receipt: 'receipt.inference.charge.chatcmpl-khala-code-pass',
      receipt_url:
        '/api/public/inference/receipts/receipt.inference.charge.chatcmpl-khala-code-pass',
      requested_model: KHALA_CODE_MODEL_ID,
      route: 'coding',
      scalar_reward: 0,
      served_model: KHALA_CODE_MODEL_ID,
      verification: 'unverified',
      verification_command: KHALA_CODE_HEADLESS_COMMAND_REF,
      verified: false,
      worker: FIREWORKS_ADAPTER_ID,
      workers: [FIREWORKS_ADAPTER_ID, KHALA_CODE_VERIFIER_WORKER_ID],
    })
    expect(body.openagents?.verified).not.toBe(true)
    expect(body.openagents?.verification).not.toBe('test_passed')
    expect(body.openagents?.verification_command).toBe(
      KHALA_CODE_HEADLESS_COMMAND_REF,
    )
    expect(body.openagents?.verification_receipt).toMatch(
      /^receipt\.inference\.khala_code\.verification\.chatcmpl-khala-code-pass\./u,
    )
    expect(body.openagents?.workers).toContain(KHALA_CODE_VERIFIER_WORKER_ID)
    expect(body.openagents?.reward_handoff).toContain(
      'accepted_outcome.khala_code.crossy_road.',
    )
  })

  test('a khala-code artifact that fails the cheap prescreen reports failed (not even worth running)', async () => {
    const registry = new InferenceProviderRegistry()
    registry.register(echoAdapter(FIREWORKS_ADAPTER_ID))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [
            { content: BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML, role: 'user' },
          ],
          model: KHALA_CODE_MODEL_ID,
        }),
        baseDeps({
          lanePlan: selectAdapterPlan,
          newId: () => 'chatcmpl-khala-code-fail',
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      openagents?: {
        rubric: { failed_checks: ReadonlyArray<string> }
        scalar_reward: number
        verification: string
        verified: boolean
        executed: boolean
      }
    }

    expect(body.openagents?.verification).toBe('failed')
    expect(body.openagents?.verified).toBe(false)
    expect(body.openagents?.executed).toBe(false)
    expect(body.openagents?.rubric.failed_checks).toContain('single_html_file')
    expect(body.openagents?.scalar_reward).toBe(0)
  })

  test('a non-Khala request omits the openagents block', async () => {
    const response = await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps()),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { openagents?: unknown }
    expect(body.openagents).toBeUndefined()
  })

  test('returns model_unavailable when no planned lane is registered', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          lanePlan: () => ['vertex-anthropic'],
          registry: new InferenceProviderRegistry(),
        }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })
})

// PROVIDER SERVING-POLICY GATE (public_paid_model_gateway_missing) -----------
// The route accepts the SAME presence-derived lane arming the public catalog
// (/v1/models) and the pre-purchase quote (/v1/quote) gate on, so the gateway
// serves exactly what it advertises and quotes. A KNOWN model on an unarmed lane
// is rejected with a clean model_unavailable BEFORE any account-state gate or
// dispatch; an UNKNOWN id falls through; omitting the arming is a no-op.
describe('POST /v1/chat/completions serving-policy gate', () => {
  // `gemini-3.5-flash` is a real pricing-table model on the vertex-gemini lane;
  // `opus` is on vertex-anthropic. `stub-model` is unknown to the table.
  const geminiBody = {
    messages: [{ content: 'hello world', role: 'user' }],
    model: 'gemini-3.5-flash',
  }

  test('rejects a KNOWN model on an UNARMED lane with model_unavailable (400)', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(geminiBody),
        baseDeps({ laneArming: ALL_LANES_UNARMED }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string; model: string }
    expect(body.error).toBe('model_unavailable')
    expect(body.model).toBe('gemini-3.5-flash')
  })

  test('serves a KNOWN model when its lane IS armed', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(geminiBody),
        baseDeps({
          laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'x' }),
        }),
      ),
    )
    expect(response.status).toBe(200)
  })

  test('does NOT gate an UNKNOWN model id (falls through unchanged)', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({ laneArming: ALL_LANES_UNARMED }),
      ),
    )
    expect(response.status).toBe(200)
  })

  test('casing cannot bypass the gate (lookup is case-insensitive)', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hello world', role: 'user' }],
          model: 'GEMINI-3.5-FLASH',
        }),
        baseDeps({ laneArming: ALL_LANES_UNARMED }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })

  test('omitting laneArming preserves the prior serve-everything behaviour', async () => {
    const response = await run(
      handleChatCompletions(chatRequest(geminiBody), baseDeps()),
    )
    expect(response.status).toBe(200)
  })

  test('servability is checked BEFORE the balance gate (unservable beats 402)', async () => {
    // An unservable model on an empty-balance account must report
    // model_unavailable (400), not insufficient_credits (402): the gateway can
    // never serve it regardless of how the customer funds their balance.
    const response = await run(
      handleChatCompletions(
        chatRequest(geminiBody),
        baseDeps({
          laneArming: ALL_LANES_UNARMED,
          readAvailableMsat: emptyBalance,
        }),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('model_unavailable')
  })
})

// ABUSE / FAIR-SHARE / SPEND-CAP GATES (#5486) -----------------------------
// The route exposes `checkFairShare` and `checkSpendCap` seams. Both default to
// undefined (gate OPEN / no-op) so the inert and unconfigured paths are
// unchanged; when wired they bind only on the enabled gateway.
describe('POST /v1/chat/completions abuse gates (#5486)', () => {
  test('inert: with neither gate wired the request serves normally', async () => {
    const response = await run(
      handleChatCompletions(chatRequest(helloBody), baseDeps()),
    )
    expect(response.status).toBe(200)
  })

  test('fair-share: rejects with 429 + RateLimit headers when the request ceiling is hit', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFairShare: async () =>
            decideFairShare({
              limits: {
                maxRequests: 60,
                maxTokens: 2_000_000,
                windowSeconds: 60,
              },
              usage: { requestsInWindow: 60, tokensInWindow: 0 },
            }),
        }),
      ),
    )
    expect(response.status).toBe(429)
    expect(response.headers.get('ratelimit-limit')).toBe('60')
    expect(response.headers.get('retry-after')).toBe('60')
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('rate_limited')
    expect(body.reason).toBe('request_rate_exceeded')
  })

  test('fair-share: rejects with 429 when the token fair-share is exhausted', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFairShare: async () =>
            decideFairShare({
              limits: { maxRequests: 60, maxTokens: 1_000, windowSeconds: 60 },
              usage: { requestsInWindow: 1, tokensInWindow: 1_000 },
            }),
        }),
      ),
    )
    expect(response.status).toBe(429)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toBe('token_fair_share_exceeded')
  })

  test('fair-share: allows when under both ceilings', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkFairShare: async () =>
            decideFairShare({
              usage: { requestsInWindow: 1, tokensInWindow: 10 },
            }),
        }),
      ),
    )
    expect(response.status).toBe(200)
  })

  test('spend-cap: rejects with 402 spend_cap_exceeded (distinct from insufficient_credits)', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          // Balance is funded; the spend cap is the thing that rejects.
          checkSpendCap: async () =>
            decideSpendCap({
              cap: { maxSpendMsatPerWindow: 1_000, windowSeconds: 86_400 },
              spentMsatInWindow: 1_001,
            }),
        }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as {
      error: string
      capMsat: number
    }
    expect(body.error).toBe('spend_cap_exceeded')
    expect(body.capMsat).toBe(1_000)
  })

  test('spend-cap: no cap configured serves normally', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(helloBody),
        baseDeps({
          checkSpendCap: async () =>
            decideSpendCap({
              cap: { maxSpendMsatPerWindow: null, windowSeconds: 86_400 },
              spentMsatInWindow: 999_999,
            }),
        }),
      ),
    )
    expect(response.status).toBe(200)
  })
})

describe('inference provider registry seam', () => {
  test('resolves a registered adapter and reports its ids', () => {
    const registry = registryWithStub()
    expect(registry.resolve(STUB_ECHO_ADAPTER_ID)?.id).toBe(
      STUB_ECHO_ADAPTER_ID,
    )
    expect(registry.resolve('not-registered')).toBeUndefined()
    expect(registry.ids()).toEqual([STUB_ECHO_ADAPTER_ID])
  })
})

describe('default model + premium gate (free-tier enablement §2)', () => {
  // Route everything to the stub adapter regardless of model so the default
  // model resolves to a viable lane.
  const stubLanePlan = () => [STUB_ECHO_ADAPTER_ID]

  test('an omitted model defaults to gemini-3.5-flash in the echoed response', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({ messages: [{ content: 'hi', role: 'user' }] }),
        baseDeps({ lanePlan: stubLanePlan }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { model: string }
    expect(body.model).toBe('gemini-3.5-flash')
  })

  test('a blank model also defaults to gemini-3.5-flash', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: '   ',
        }),
        baseDeps({ lanePlan: stubLanePlan }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { model: string }
    expect(body.model).toBe('gemini-3.5-flash')
  })

  test('premium gate DENIES a non-allowlisted premium request (403) before dispatch', async () => {
    let dispatched = false
    const denyGate: ChatCompletionsDeps['checkPremiumAccess'] = async (
      _accountRef,
      model,
    ) => ({
      allowed: false,
      message: `Model "${model}" is a premium model and requires an owner grant.`,
      premium: true,
      reasonRef: 'reason.inference_premium.owner_not_allowlisted',
    })
    const meteringHook: MeteringHook = () =>
      Effect.sync(() => {
        dispatched = true
        return { metered: false, receiptRef: null }
      })
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'claude-sonnet',
        }),
        baseDeps({
          checkPremiumAccess: denyGate,
          lanePlan: stubLanePlan,
          meteringHook,
        }),
      ),
    )
    expect(response.status).toBe(403)
    const body = (await response.json()) as { error: string; message: string }
    expect(body.error).toBe('premium_model_not_allowed')
    expect(body.message).toContain('premium')
    expect(dispatched).toBe(false) // never reached the provider/metering
  })

  test('premium gate ALLOWS an allowlisted premium request (200)', async () => {
    const allowGate: ChatCompletionsDeps['checkPremiumAccess'] = async () => ({
      allowed: true,
      message: '',
      premium: true,
      reasonRef: 'reason.inference_premium.owner_allowlisted',
    })
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'claude-sonnet',
        }),
        baseDeps({ checkPremiumAccess: allowGate, lanePlan: stubLanePlan }),
      ),
    )
    expect(response.status).toBe(200)
  })

  test('premium gate is consulted for a non-premium model and allows it', async () => {
    let checked = false
    const gate: ChatCompletionsDeps['checkPremiumAccess'] = async () => {
      checked = true
      return {
        allowed: true,
        message: '',
        premium: false,
        reasonRef: 'reason.inference_premium.non_premium_model',
      }
    }
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'gemini-3.5-flash',
        }),
        baseDeps({ checkPremiumAccess: gate, lanePlan: stubLanePlan }),
      ),
    )
    expect(response.status).toBe(200)
    expect(checked).toBe(true)
  })
})

// TRUE PASS-THROUGH STREAM (the khala-code 524 fix) -----------------------
// When the served adapter exposes `streamSse`, the route pumps the upstream SSE
// to the client frame-by-frame instead of buffering the whole completion before
// emitting a byte. These exercise the route wiring: live pass-through + metering
// from the terminal usage frame, the missing-terminal-frame case (no estimate),
// and connect-time failure → 502.
describe('POST /v1/chat/completions — streamSse pass-through', () => {
  const passThroughAuth: InferenceAuth = async () => ({
    accountRef: 'agent:test-user',
  })
  const funded: InferenceBalanceReader = async () => 100_000

  // A streamSse-capable adapter built from a script of normalized frames. The
  // frames are emitted one at a time (one per ReadableStream pull), so the test
  // proves the route does not wait for the whole upstream before emitting.
  const streamSseAdapter = (
    id: string,
    script: ReadonlyArray<InferenceStreamEvent>,
    terminal: Readonly<{
      finishReason: string | undefined
      usage: InferenceUsage | undefined
      servedModel: string | undefined
    }>,
  ): InferenceProviderAdapter => ({
    ...stubEchoAdapter,
    id,
    streamSse: () =>
      Effect.sync<InferenceStreamSource>(() => ({
        frames: (async function* () {
          for (const event of script) {
            yield event
          }
        })(),
        terminal: () => terminal,
      })),
  })

  const ptDeps = (
    overrides: Partial<ChatCompletionsDeps> = {},
  ): ChatCompletionsDeps => ({
    authenticate: passThroughAuth,
    enabled: true,
    readAvailableMsat: funded,
    registry: new InferenceProviderRegistry(),
    ...overrides,
  })

  const ptRequest = (body: unknown): Request =>
    new Request('https://openagents.com/v1/chat/completions', {
      body: JSON.stringify(body),
      method: 'POST',
    })

  test('pumps content deltas through and meters from the terminal usage frame', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-pt-1' }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter(
        'pt-lane',
        [{ contentDelta: 'Hel' }, { contentDelta: 'lo' }],
        {
          finishReason: 'stop',
          servedModel: 'served/model',
          usage: { completionTokens: 2, promptTokens: 7, totalTokens: 9 },
        },
      ),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        ptRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        ptDeps({ lanePlan: () => ['pt-lane'], meteringHook, registry }),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')
    const text = await response.text()
    // Content streamed through, terminated with [DONE].
    expect(text).toContain('"content":"Hel"')
    expect(text).toContain('"content":"lo"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    // Metering settled receipt-first from the terminal usage frame.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.streamed).toBe(true)
    expect(captured[0]?.adapterId).toBe('pt-lane')
    expect(captured[0]?.servedModel).toBe('served/model')
    expect(captured[0]?.usage.completionTokens).toBe(2)
  })

  test('a missing terminal usage frame closes the stream cleanly WITHOUT metering (no estimate)', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter('pt-lane', [{ contentDelta: 'partial' }], {
        finishReason: undefined,
        servedModel: undefined,
        usage: undefined,
      }),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        ptRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        ptDeps({ lanePlan: () => ['pt-lane'], meteringHook, registry }),
      ),
    )

    expect(response.status).toBe(200)
    const text = await response.text()
    expect(text).toContain('"content":"partial"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    // Receipt-first: no terminal usage => the hook never runs (never an estimate).
    expect(captured).toHaveLength(0)
  })

  test('a connect-time streamSse failure surfaces as 502 (no buffered re-dispatch)', async () => {
    const registry = new InferenceProviderRegistry()
    let bufferedStreamCalls = 0
    registry.register({
      ...stubEchoAdapter,
      id: 'pt-lane',
      stream: request => {
        bufferedStreamCalls += 1
        return stubEchoAdapter.stream(request)
      },
      streamSse: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'pt-lane',
            kind: 'upstream_error',
            reason: 'fireworks responded 524',
            retryable: false,
          }),
        ),
    })

    const response = await Effect.runPromise(
      handleChatCompletions(
        ptRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        ptDeps({ lanePlan: () => ['pt-lane'], registry }),
      ),
    )

    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('provider_error')
    expect(body.reason).toBe('fireworks responded 524')
    // The provider error must NOT silently fall back to the buffered path.
    expect(bufferedStreamCalls).toBe(0)
  })

  test('an adapter WITHOUT streamSse falls back to the buffered path', async () => {
    // stubEchoAdapter has no streamSse; the route must use the buffered `stream`.
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })
    const registry = new InferenceProviderRegistry()
    registry.register({ ...stubEchoAdapter, id: 'buffered-lane' })

    const response = await Effect.runPromise(
      handleChatCompletions(
        ptRequest({
          messages: [{ content: 'hello world', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        ptDeps({ lanePlan: () => ['buffered-lane'], meteringHook, registry }),
      ),
    )

    expect(response.status).toBe(200)
    const text = await response.text()
    expect(text).toContain('"content":"hello world"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    expect(captured).toHaveLength(1)
    expect(captured[0]?.streamed).toBe(true)
  })
})

// KHALA TELEMETRY SCORECARD (book P0-1 / Open Q #1-2) ------------------------
// The `openagents` block carries the SMALL lifecycle telemetry summary; the full
// record dereferences via the receipt. These assert the MEASURED fields are real
// on the true-streaming path (TTFT, total wall-clock from a stepping clock) and
// the genuinely-unmeasurable ones are the honest `not_measured` sentinel — never
// a fabricated number — and that the receipt detailRef dereferences.
describe('POST /v1/chat/completions — telemetry scorecard', () => {
  const passThroughAuth: InferenceAuth = async () => ({
    accountRef: 'agent:test-user',
  })
  const funded: InferenceBalanceReader = async () => 100_000

  const streamSseAdapter = (
    id: string,
    script: ReadonlyArray<InferenceStreamEvent>,
    terminal: Readonly<{
      finishReason: string | undefined
      usage: InferenceUsage | undefined
      servedModel: string | undefined
    }>,
  ): InferenceProviderAdapter => ({
    ...stubEchoAdapter,
    id,
    streamSse: () =>
      Effect.sync<InferenceStreamSource>(() => ({
        frames: (async function* () {
          for (const event of script) {
            yield event
          }
        })(),
        terminal: () => terminal,
      })),
  })

  // A stepping wall-clock so the lifecycle boundaries are deterministic AND
  // distinct: 1000 (request accept) -> 1200 (first token) -> 1500 (EOF). So
  // TTFT = 200ms and total wall-clock = 500ms are REAL measured numbers.
  const steppingClock = (steps: ReadonlyArray<number>): (() => number) => {
    let index = 0
    return () => {
      const value = steps[Math.min(index, steps.length - 1)] ?? 0
      index += 1
      return value
    }
  }

  const telemetryFor = (
    block: { telemetry?: Record<string, unknown> } | undefined,
  ): Record<string, unknown> => (block?.telemetry ?? {}) as Record<
    string,
    unknown
  >

  test('a streamed khala-code request carries REAL measured TTFT + total wall-clock and honest sentinels', async () => {
    const meteringHook: MeteringHook = () =>
      Effect.sync(() => ({
        metered: true,
        receiptRef: 'receipt.inference.charge.chatcmpl-telemetry',
      }))

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter(
        FIREWORKS_ADAPTER_ID,
        // Two content deltas, then the terminal usage frame. The provider does NOT
        // report a cached-token dimension here (so cached input is `not_measured`).
        [
          { contentDelta: '<html>' },
          { contentDelta: '</html>' },
          {
            contentDelta: '',
            finishReason: 'stop',
            servedModel: 'accounts/fireworks/models/kimi-k2p7-code',
            usage: { completionTokens: 12, promptTokens: 400, totalTokens: 412 },
          },
        ],
        {
          finishReason: 'stop',
          servedModel: 'accounts/fireworks/models/kimi-k2p7-code',
          usage: { completionTokens: 12, promptTokens: 400, totalTokens: 412 },
        },
      ),
    )

    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            messages: [{ content: GOOD_CROSSY_ROAD_HTML, role: 'user' }],
            model: KHALA_CODE_MODEL_ID,
            stream: true,
          }),
          method: 'POST',
        }),
        {
          authenticate: passThroughAuth,
          enabled: true,
          lanePlan: selectAdapterPlan,
          meteringHook,
          newId: () => 'chatcmpl-telemetry',
          // Clock read order on the true-streaming path: (1) request accept=1000,
          // (2) the durable-log clock read at the stream call site=1100 (a no-op
          // for telemetry, durable streaming is off), (3) first-token=1200,
          // (4) EOF=1500. So TTFT=200 and total wall-clock=500 are real.
          nowEpochMillis: steppingClock([1000, 1100, 1200, 1500]),
          readAvailableMsat: funded,
          registry,
        },
      ),
    )
    expect(response.status).toBe(200)

    const frames = (await response.text())
      .split('\n\n')
      .map(block => block.replace(/^data: /u, '').trim())
      .filter(payload => payload !== '' && payload !== '[DONE]')
      .map(payload => JSON.parse(payload) as { openagents?: { telemetry?: Record<string, unknown> } })
    const finalBlock = frames[frames.length - 1]?.openagents
    const telemetry = telemetryFor(finalBlock)

    // The block decodes against the schema (a valid public-safe projection).
    expect(Option.isSome(decodeKhalaTelemetryBlock(telemetry))).toBe(true)

    // MEASURED-NOW fields are REAL numbers, not sentinels.
    expect(telemetry.requestClass).toBe('interactive_stream')
    expect(telemetry.promptTokens).toBe(400)
    expect(telemetry.completionTokens).toBe(12)
    expect(telemetry.totalTokens).toBe(412)
    expect(telemetry.ttftMs).toBe(200) // 1200 - 1000
    expect(telemetry.totalWallClockMs).toBe(500) // 1500 - 1000
    // khala-code on the hot Worker path NEVER executes the artifact (no browser),
    // so the executed verdict is the honest `not_executed` — NEVER a fabricated
    // `passed`. The verification class reflects the cheap pre-screen verdict
    // (here `failed`: the artifact's prescreen rejected it), and scalar_reward is
    // 0. A real `failed`/`not_executed` beats a fake `passed`.
    expect(telemetry.verificationClass).toBe('failed')
    expect(telemetry.executedVerdict).toBe('not_executed')
    expect(telemetry.scalarReward).toBe(0)
    // The detailRef points at the dereferenceable receipt.
    expect(telemetry.detailRef).toBe(
      '/api/public/inference/receipts/receipt.inference.charge.chatcmpl-telemetry',
    )

    // The block carries the headline prefix-caching metric (cachedInputTokens,
    // book P0-2 / #6084) alongside the token counts — here honestly not_measured
    // because this fixture provider reported no cached dimension.
    expect(telemetry.cachedInputTokens).toBe('not_measured')
    // The deeper P0-1 fields (time split, cost basis, cache-affinity hash,
    // unaccounted-token reconciliation) are NOT on the immediate block; they
    // live in the dereferenceable record.
    expect(telemetry).not.toHaveProperty('providerTimeMs')
    expect(telemetry).not.toHaveProperty('costBasisMsat')
    expect(telemetry).not.toHaveProperty('cacheAffinityKeyHash')
    expect(telemetry).not.toHaveProperty('unaccountedTokens')

    // No raw account/session/prompt material leaked into the disclosure.
    const serialized = JSON.stringify(finalBlock)
    expect(serialized).not.toContain('agent:test-user')

    // DEREFERENCE: the detailRef receipt resolves through the public receipt
    // route to a public-safe paid projection (no private payment material).
    const receiptRoutes = makePublicInferenceReceiptRoutes<{
      store: InferenceReceiptReadStore
    }>({
      makeStore: env => env.store,
      nowIso: () => '2026-06-23T00:00:00.000Z',
    })
    const store: InferenceReceiptReadStore = {
      readInferenceReceiptByRef: async receiptRef =>
        receiptRef === 'receipt.inference.charge.chatcmpl-telemetry'
          ? {
              contextRef: null,
              createdAt: '2026-06-23T00:00:00.000Z',
              payInType: 'adjustment',
              receiptRef,
              state: 'paid',
              stateChangedAt: '2026-06-23T00:00:00.500Z',
            }
          : null,
    }
    const dereferenceEffect = receiptRoutes.routePublicInferenceReceiptRequest(
      new Request(`https://openagents.com${telemetry.detailRef as string}`),
      { store },
    )
    expect(dereferenceEffect).toBeDefined()
    const dereferenced = await run(dereferenceEffect!)
    expect(dereferenced.status).toBe(200)
    const dereferencedBody = (await dereferenced.json()) as {
      receipt: { ledgerState: string; schemaVersion: string }
    }
    expect(dereferencedBody.receipt.ledgerState).toBe('paid')
    expect(dereferencedBody.receipt.schemaVersion).toBe(
      'openagents.inference.receipt.v1',
    )
  })

  test('a non-stream Khala request records total wall-clock but honest sentinel TTFT (no first-token boundary)', async () => {
    const registry = new InferenceProviderRegistry()
    registry.register(echoAdapterForTelemetry(VERTEX_GEMINI_ADAPTER_ID))

    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            messages: [{ content: 'hello world', role: 'user' }],
            model: KHALA_MINI_MODEL_ID,
          }),
          method: 'POST',
        }),
        {
          authenticate: passThroughAuth,
          enabled: true,
          lanePlan: selectAdapterPlan,
          nowEpochMillis: steppingClock([2000, 2350]),
          readAvailableMsat: funded,
          registry,
        },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      openagents?: { telemetry?: Record<string, unknown> }
    }
    const telemetry = telemetryFor(body.openagents)
    expect(telemetry.requestClass).toBe('async_job')
    expect(telemetry.totalWallClockMs).toBe(350) // 2350 - 2000
    // A buffered/non-stream completion has no observable first-token boundary =>
    // TTFT is the honest sentinel, NEVER a fabricated number.
    expect(telemetry.ttftMs).toBe(NOT_MEASURED)
    expect(telemetry.verificationClass).toBe('none')
  })
})

const echoAdapterForTelemetry = (id: string): InferenceProviderAdapter => ({
  ...stubEchoAdapter,
  id,
})

// KHALA IDENTITY GUARD (never identify as Gemini/Google/etc.) -----------------
// Proves: (a) the gateway injects the strong Khala identity system prompt for
// khala-* lanes; (b) the identity signature guard catches a completion claiming
// "I am built on Gemini / by Google" (and Claude/etc.) and corrects it; (c) a
// normal khala completion passes through unchanged; (d) "are you Gemini?" yields
// a Khala-by-OpenAgents answer with no provider leak; and that non-Khala
// responses are untouched.
describe('Khala identity guard', () => {
  // An adapter that records the messages it received and returns a fixed reply.
  const cannedAdapter = (
    id: string,
    reply: string,
    captured: Array<ReadonlyArray<{ role: string; content: string }>>,
  ): InferenceProviderAdapter => ({
    complete: request =>
      Effect.sync(() => {
        captured.push(
          request.messages.map(m => ({ content: m.content, role: m.role })),
        )
        return {
          content: reply,
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
        }
      }),
    id,
    stream: () => Effect.sync(() => []),
  })

  // An adapter that LEAKS its provider identity on the first call, then returns a
  // clean Khala answer if the request carries the reinforcement re-ask (a system
  // message that mentions the forbidden providers + "answer again"). This models
  // a base model that volunteers provenance, then complies on the stronger steer.
  const leakingThenCleanAdapter = (
    id: string,
    leak: string,
  ): InferenceProviderAdapter => ({
    complete: request =>
      Effect.sync(() => {
        const isReask = request.messages.some(
          m =>
            m.role === 'system' &&
            m.content.toLowerCase().includes('answer again'),
        )
        const content = isReask
          ? 'We are Khala, the OpenAgents inference model. How can we help?'
          : leak
        return {
          content,
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
        }
      }),
    id,
    stream: () => Effect.sync(() => []),
  })

  const readReply = async (
    response: Response,
  ): Promise<{ content: string }> => {
    const body = (await response.json()) as {
      choices: ReadonlyArray<{ message: { content: string } }>
    }
    return { content: body.choices[0]?.message.content ?? '' }
  }

  test('injects the Khala identity system prompt as the leading message for khala-* lanes', async () => {
    const captured: Array<ReadonlyArray<{ role: string; content: string }>> = []
    const registry = new InferenceProviderRegistry()
    registry.register(cannedAdapter(VERTEX_GEMINI_ADAPTER_ID, 'ok', captured))

    await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const messages = captured[0]!
    // The gateway identity prompt is the FIRST message the adapter sees.
    expect(messages[0]?.role).toBe('system')
    expect(messages[0]?.content).toBe(KHALA_IDENTITY_SYSTEM_PROMPT)
    // The original user message is preserved after it.
    expect(messages.some(m => m.role === 'user' && m.content === 'hi')).toBe(
      true,
    )
  })

  test('does NOT inject the identity prompt for a non-Khala model', async () => {
    const captured: Array<ReadonlyArray<{ role: string; content: string }>> = []
    const registry = new InferenceProviderRegistry()
    registry.register(cannedAdapter(STUB_ECHO_ADAPTER_ID, 'ok', captured))

    await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'stub-model',
        }),
        baseDeps({ registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    expect(
      captured[0]!.some(m => m.content === KHALA_IDENTITY_SYSTEM_PROMPT),
    ).toBe(false)
  })

  test('catches a Gemini/Google identity leak and corrects it via re-ask', async () => {
    const registry = new InferenceProviderRegistry()
    registry.register(
      leakingThenCleanAdapter(
        VERTEX_GEMINI_ADAPTER_ID,
        'I am Autopilot. I am built on Gemini, a large language model by Google.',
      ),
    )

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'what model are you?', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const { content } = await readReply(response)
    expect(content.toLowerCase()).toContain('khala')
    expect(content.toLowerCase()).not.toContain('gemini')
    expect(content.toLowerCase()).not.toContain('google')
  })

  test('fail-closed backstop: a persistent leak is deterministically redacted to the Khala identity', async () => {
    // This adapter ALWAYS leaks, even on the re-ask, so the guard must fall
    // through to the deterministic redaction backstop.
    const alwaysLeaks: InferenceProviderAdapter = {
      complete: request =>
        Effect.sync(() => ({
          content:
            'Sure. I am built on Gemini, a large language model by Google.',
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
        })),
      id: VERTEX_GEMINI_ADAPTER_ID,
      stream: () => Effect.sync(() => []),
    }
    const registry = new InferenceProviderRegistry()
    registry.register(alwaysLeaks)

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'are you Gemini?', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const { content } = await readReply(response)
    expect(content.toLowerCase()).not.toContain('gemini')
    expect(content.toLowerCase()).not.toContain('google')
    expect(content).toContain(KHALA_IDENTITY_STATEMENT)
    // Surrounding non-offending text is preserved.
    expect(content).toContain('Sure.')
  })

  test('a normal, non-identity khala completion passes through UNCHANGED', async () => {
    const clean =
      'Here is a function:\n\nfunction add(a, b) { return a + b }\n\nIt returns the sum.'
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: request =>
        Effect.sync(() => ({
          content: clean,
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 12, promptTokens: 4, totalTokens: 16 },
        })),
      id: VERTEX_GEMINI_ADAPTER_ID,
      stream: () => Effect.sync(() => []),
    })

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'write add()', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const { content } = await readReply(response)
    expect(content).toBe(clean)
  })

  test('FIX 2: a clean plural "We are Khala…" identity answer is returned UNCHANGED (no duplicated identity sentence)', async () => {
    // The exact live-app shape: identity stated once, then a provider denial.
    // The route guard must NOT prepend or re-state the identity — it passes
    // through byte-for-byte so the identity sentence appears exactly once.
    const cleanIdentity =
      'We are Khala, the OpenAgents inference model — one endpoint over a network of agents. We are not Gemini or any other model. How can we help you today?'
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: request =>
        Effect.sync(() => ({
          content: cleanIdentity,
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 12, promptTokens: 4, totalTokens: 16 },
        })),
      id: VERTEX_GEMINI_ADAPTER_ID,
      stream: () => Effect.sync(() => []),
    })

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'what model are you?', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const { content } = await readReply(response)
    expect(content).toBe(cleanIdentity)
    // Identity stated EXACTLY once — the duplication bug is fixed.
    expect(content.split('We are Khala').length - 1).toBe(1)
  })

  test('buffered Khala stream redacts an identity leak in the assembled content', async () => {
    // A streaming adapter (no streamSse) whose chunks assemble into a leak. The
    // buffered stream path materializes the whole completion, so the
    // deterministic backstop rewrites it before any byte is emitted.
    const registry = new InferenceProviderRegistry()
    registry.register({
      complete: request =>
        Effect.sync(() => ({
          content: 'unused',
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
        })),
      id: VERTEX_GEMINI_ADAPTER_ID,
      stream: request =>
        Effect.sync(() => [
          { contentDelta: 'I am built on Gemini, ' },
          { contentDelta: 'a model by Google.' },
          {
            contentDelta: '',
            finishReason: 'stop',
            servedModel: request.model,
            usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
          },
        ]),
    })

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'what are you?', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
          stream: true,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(response.status).toBe(200)
    const text = await response.text()
    // Assemble the user-facing CONTENT from the delta frames. The `openagents`
    // disclosure block legitimately names the served worker (`vertex-gemini`) for
    // auditability — that is the receipt, not the answer — so assert on the
    // assistant content only.
    const content = text
      .split('\n\n')
      .map(block => block.replace(/^data: /u, '').trim())
      .filter(payload => payload !== '' && payload !== '[DONE]')
      .map(
        payload =>
          (
            JSON.parse(payload) as {
              choices?: ReadonlyArray<{ delta?: { content?: string } }>
            }
          ).choices?.[0]?.delta?.content ?? '',
      )
      .join('')
    expect(content.toLowerCase()).not.toContain('gemini')
    expect(content.toLowerCase()).not.toContain('google')
    expect(content).toContain(KHALA_IDENTITY_STATEMENT)
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
  })
})

// DURABLE-STREAM RANK-1 (#6058): the streaming pass-through, when the durable
// flag is on AND a store factory is wired, tees the upstream token stream into a
// per-request durable offset log so a client drop can be resumed by offset. These
// exercise the route wiring end-to-end: persist + resume URL header + metering
// EXACTLY ONCE on EOF (and NOT on a replay) + flag-off → today's pass-through.
describe('POST /v1/chat/completions — durable-stream resumable inference (#6058)', () => {
  const durableAuth: InferenceAuth = async () => ({
    accountRef: 'agent:test-user',
  })
  const funded: InferenceBalanceReader = async () => 100_000

  const streamSseAdapter = (
    id: string,
    script: ReadonlyArray<InferenceStreamEvent>,
    terminal: Readonly<{
      finishReason: string | undefined
      usage: InferenceUsage | undefined
      servedModel: string | undefined
    }>,
  ): InferenceProviderAdapter => ({
    ...stubEchoAdapter,
    id,
    streamSse: () =>
      Effect.sync<InferenceStreamSource>(() => ({
        frames: (async function* () {
          for (const event of script) {
            yield event
          }
        })(),
        terminal: () => terminal,
      })),
  })

  const durableRequest = (body: unknown): Request =>
    new Request('https://openagents.com/v1/chat/completions', {
      body: JSON.stringify(body),
      method: 'POST',
    })

  const okUsage: InferenceUsage = {
    completionTokens: 4,
    promptTokens: 8,
    totalTokens: 12,
  }

  const baseDurableDeps = (
    overrides: Partial<ChatCompletionsDeps> = {},
  ): ChatCompletionsDeps => ({
    authenticate: durableAuth,
    enabled: true,
    readAvailableMsat: funded,
    registry: new InferenceProviderRegistry(),
    ...overrides,
  })

  test('flag ON + store wired: persists the stream, advertises the resume URL, and meters EXACTLY ONCE on EOF', async () => {
    const store = new MemoryStreamStore()
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-durable-1' }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter(
        'durable-lane',
        [{ contentDelta: 'Dur' }, { contentDelta: 'able' }],
        { finishReason: 'stop', servedModel: 'served/m', usage: okUsage },
      ),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        durableRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        baseDurableDeps({
          durableStream: () => store,
          durableStreamEnabled: true,
          lanePlan: () => ['durable-lane'],
          meteringHook,
          newId: () => 'req-durable-route',
          nowEpochMillis: () => 1_700_000_000_000,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    // The resumable read URL is advertised, keyed by the response id.
    expect(response.headers.get('openagents-durable-stream-url')).toBe(
      '/v1/chat/completions/durable/req-durable-route',
    )

    // The live stream still flows to the client unchanged.
    const text = await response.text()
    expect(text).toContain('"content":"Dur"')
    expect(text).toContain('"content":"able"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)

    // Metering settled EXACTLY ONCE on the real upstream EOF.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.streamed).toBe(true)
    expect(captured[0]?.adapterId).toBe('durable-lane')

    // The completion is PERSISTED: a resume read reconstructs the suffix.
    const replay = replayFromOffset({
      nowMs: 1_700_000_000_000,
      offset: '0',
      requestId: 'req-durable-route',
      store,
    })
    expect(replay).toBeDefined()
    expect(replay!.body).toContain('Dur')
    expect(replay!.body).toContain('able')
    expect(replay!.streamClosed).toBe(true)
  })

  test('a resume / replay read of the persisted completion does NOT re-bill', async () => {
    const store = new MemoryStreamStore()
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-durable-2' }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter('durable-lane', [{ contentDelta: 'once' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage: okUsage,
      }),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        durableRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        baseDurableDeps({
          durableStream: () => store,
          durableStreamEnabled: true,
          lanePlan: () => ['durable-lane'],
          meteringHook,
          newId: () => 'req-replay',
          nowEpochMillis: () => 1_700_000_000_000,
          registry,
        }),
      ),
    )
    // Drain the live stream so the producer runs to EOF (Web Streams are lazy).
    await response.text()
    expect(captured).toHaveLength(1)

    // Many reconnect / catch-up reads of the SAME completion. The read path has
    // no metering hook, so the metering count stays exactly one — replays are free.
    for (let i = 0; i < 4; i++) {
      const replay = replayFromOffset({
        nowMs: 1_700_000_000_000,
        offset: i % 2 === 0 ? '0' : undefined,
        requestId: 'req-replay',
        store,
      })
      expect(replay).toBeDefined()
    }
    expect(captured).toHaveLength(1)
  })

  test('flag OFF: the stream is today’s pass-through (no persistence, no resume URL, metered once)', async () => {
    const store = new MemoryStreamStore()
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-off' }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter(
        'durable-lane',
        [{ contentDelta: 'Pass' }, { contentDelta: 'through' }],
        { finishReason: 'stop', servedModel: 'served/m', usage: okUsage },
      ),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        durableRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        baseDurableDeps({
          // Store IS wired, but the flag is OFF → degrade to pass-through.
          durableStream: () => store,
          durableStreamEnabled: false,
          lanePlan: () => ['durable-lane'],
          meteringHook,
          newId: () => 'req-off',
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    // No resume URL header on the non-durable path.
    expect(response.headers.get('openagents-durable-stream-url')).toBeNull()
    const text = await response.text()
    expect(text).toContain('"content":"Pass"')
    expect(text).toContain('"content":"through"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    // Metered once (pass-through unchanged).
    expect(captured).toHaveLength(1)
    // NOTHING was persisted — the store has no stream for this request.
    const replay = replayFromOffset({
      nowMs: 1_700_000_000_000,
      offset: '0',
      requestId: 'req-off',
      store,
    })
    expect(replay).toBeUndefined()
  })

  test('flag ON but store factory returns undefined: fail-safe to pass-through', async () => {
    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-nostore' }
      })

    const registry = new InferenceProviderRegistry()
    registry.register(
      streamSseAdapter('durable-lane', [{ contentDelta: 'safe' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage: okUsage,
      }),
    )

    const response = await Effect.runPromise(
      handleChatCompletions(
        durableRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: 'open-model',
          stream: true,
        }),
        baseDurableDeps({
          durableStream: () => undefined,
          durableStreamEnabled: true,
          lanePlan: () => ['durable-lane'],
          meteringHook,
          newId: () => 'req-nostore',
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('openagents-durable-stream-url')).toBeNull()
    const text = await response.text()
    expect(text).toContain('"content":"safe"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    expect(captured).toHaveLength(1)
  })
})

// GAP 2 (EPIC #6017 — "turn intent into tests it must pass"): the gateway must
// inject the acceptance-contract guidance for an `openagents/khala-code` coding
// request so a produced game is drivable/verifiable by default — ADDITIVELY,
// without clobbering the identity prompt, and scoped to khala-code (not all Khala
// models, not non-coding).
describe('Khala-code acceptance-contract injection', () => {
  const recordingAdapter = (
    id: string,
    captured: Array<ReadonlyArray<{ role: string; content: string }>>,
  ): InferenceProviderAdapter => ({
    complete: request =>
      Effect.sync(() => {
        captured.push(
          request.messages.map(m => ({ content: m.content, role: m.role })),
        )
        return {
          content: 'ok',
          finishReason: 'stop',
          servedModel: request.model,
          usage: { completionTokens: 4, promptTokens: 4, totalTokens: 8 },
        }
      }),
    id,
    stream: () => Effect.sync(() => []),
  })

  test('injects the acceptance contract AND the identity prompt for khala-code (additive)', async () => {
    const captured: Array<ReadonlyArray<{ role: string; content: string }>> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest({
          messages: [
            {
              content: 'build a really high quality crossy road game with three.js',
              role: 'user',
            },
          ],
          model: KHALA_CODE_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const messages = captured[0]!
    const systemContents = messages
      .filter(m => m.role === 'system')
      .map(m => m.content)
    // The identity prompt is still present (not clobbered).
    expect(systemContents).toContain(KHALA_IDENTITY_SYSTEM_PROMPT)
    // The acceptance contract is present and names the runner's state hooks.
    const contract = acceptanceContractGuidanceForSpec(crossyRoadAcceptanceSpec())
    expect(systemContents).toContain(contract)
    expect(contract).toContain('__openagentsCrossyRoadState')
    expect(contract).toContain('__openagentsCrossyRoadRestart')
    // The original user intent is preserved.
    expect(
      messages.some(m => m.role === 'user' && m.content.includes('crossy road')),
    ).toBe(true)
  })

  test('does NOT inject the acceptance contract for a non-code Khala model', async () => {
    const captured: Array<ReadonlyArray<{ role: string; content: string }>> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(VERTEX_GEMINI_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const systemContents = captured[0]!
      .filter(m => m.role === 'system')
      .map(m => m.content)
    // khala-mini still gets identity, but NOT the code acceptance contract.
    expect(systemContents).toContain(KHALA_IDENTITY_SYSTEM_PROMPT)
    expect(
      systemContents.some(c => c.includes('__openagentsCrossyRoadState')),
    ).toBe(false)
  })

  test('does NOT inject the acceptance contract for a non-Khala model', async () => {
    const captured: Array<ReadonlyArray<{ role: string; content: string }>> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(STUB_ECHO_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'build a crossy road game', role: 'user' }],
          model: 'stub-model',
        }),
        baseDeps({ registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const systemContents = captured[0]!
      .filter(m => m.role === 'system')
      .map(m => m.content)
    expect(systemContents).toHaveLength(0)
  })

  test('the contract resolver is scoped to a khala-code intent and is extensible', () => {
    // A crossy-road intent resolves the crossy-road contract.
    const guidance = acceptanceContractGuidanceForRequest({
      messages: [{ content: 'build a crossy road game', role: 'user' }],
      model: KHALA_CODE_MODEL_ID,
    })
    expect(guidance).toBeDefined()
    expect(guidance).toContain('__openagentsCrossyRoadState')
  })
})

// BOOK P0-2 (#6084): prefix caching as a product feature. The gateway must
// assemble a stable prompt layout (stable content first, novel last), pin a
// provider session-affinity key, record a public-safe cache-affinity HASH in the
// telemetry, and route a same-session follow-up to the cache-warm lane.
describe('Khala prefix caching (book P0-2 / #6084)', () => {
  type Captured = Readonly<{
    messages: ReadonlyArray<{ role: string; content: string }>
    passthroughParams: Readonly<Record<string, unknown>>
  }>

  const recordingAdapter = (
    id: string,
    captured: Array<Captured>,
    usage: InferenceUsage = {
      completionTokens: 4,
      promptTokens: 4,
      totalTokens: 8,
    },
  ): InferenceProviderAdapter => ({
    complete: request =>
      Effect.sync(() => {
        captured.push({
          messages: request.messages.map(m => ({
            content: m.content,
            role: m.role,
          })),
          passthroughParams: request.passthroughParams,
        })
        return {
          content: 'ok',
          finishReason: 'stop',
          servedModel: request.model,
          usage,
        }
      }),
    id,
    stream: () => Effect.sync(() => []),
  })

  const khalaCodeBody = (userTurn: string) => ({
    messages: [{ content: userTurn, role: 'user' }],
    model: KHALA_CODE_MODEL_ID,
  })

  type OpenAgentsTelemetry = {
    openagents?: {
      telemetry?: {
        cachedInputTokens?: unknown
        promptTokens?: unknown
        completionTokens?: unknown
        totalTokens?: unknown
      }
    }
  }
  // The cache-affinity hash lives in the FULL telemetry RECORD behind the
  // receipt; the immediate block carries the summary. We assert the hash via the
  // full record store (the receipt detail), mirroring the public-projection split.

  test('1+2. stable layout: stable system blocks lead, the novel user turn is last', async () => {
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest(khalaCodeBody('build a crossy road game')),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const roles = captured[0]!.messages.map(m => m.role)
    // Every leading message is a stable system block; the user turn is strictly last.
    const lastIndex = roles.length - 1
    expect(roles[lastIndex]).toBe('user')
    expect(roles.slice(0, lastIndex).every(r => r === 'system')).toBe(true)
    // The acceptance contract leads (lowest stable ordinal), identity follows.
    const systemContents = captured[0]!.messages
      .filter(m => m.role === 'system')
      .map(m => m.content)
    const identityIndex = systemContents.indexOf(KHALA_IDENTITY_SYSTEM_PROMPT)
    const contractIndex = systemContents.findIndex(c =>
      c.includes('__openagentsCrossyRoadState'),
    )
    expect(contractIndex).toBeGreaterThanOrEqual(0)
    expect(identityIndex).toBeGreaterThanOrEqual(0)
    expect(contractIndex).toBeLessThan(identityIndex)
  })

  test('1. the stable prefix is identical across two turns of one session; only the user turn varies', async () => {
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured))
    const deps = baseDeps({ lanePlan: selectAdapterPlan, registry })

    await run(handleChatCompletions(chatRequest(khalaCodeBody('first crossy road question')), deps))
    await run(handleChatCompletions(chatRequest(khalaCodeBody('a different crossy road question')), deps))
    expect(captured).toHaveLength(2)
    // The stable (system) prefix is byte-identical turn over turn.
    const stableOf = (c: Captured) =>
      c.messages.filter(m => m.role === 'system').map(m => m.content)
    expect(stableOf(captured[0]!)).toEqual(stableOf(captured[1]!))
    // The volatile user turns differ.
    const userOf = (c: Captured) =>
      c.messages.filter(m => m.role === 'user').map(m => m.content)
    expect(userOf(captured[0]!)).not.toEqual(userOf(captured[1]!))
  })

  test('4. session-affinity header (x-session-affinity) + OpenAI `user` are set from the affinity key when supported', async () => {
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest(
          { ...khalaCodeBody('build a crossy road game'), user: 'session-xyz' },
        ),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    const params = captured[0]!.passthroughParams
    const affinity = params['x-session-affinity']
    const user = params['user']
    // Both carry the SAME opaque, derived value — and it is NOT the raw session id
    // (privacy: the provider sees only a one-way correlation token).
    expect(typeof affinity).toBe('string')
    expect(affinity).toBe(user)
    expect(affinity).not.toBe('session-xyz')
    expect(String(affinity)).toMatch(/^cacheaff:fnv1a32:[0-9a-f]{8}$/u)
  })

  test('4. the same session across two turns derives the SAME affinity value (pins to one replica)', async () => {
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured))
    const deps = baseDeps({ lanePlan: selectAdapterPlan, registry })

    const body = (turn: string) => ({ ...khalaCodeBody(turn), user: 'sess-1' })
    await run(handleChatCompletions(chatRequest(body('q1')), deps))
    await run(handleChatCompletions(chatRequest(body('q2')), deps))
    expect(captured).toHaveLength(2)
    expect(captured[0]!.passthroughParams['x-session-affinity']).toBe(
      captured[1]!.passthroughParams['x-session-affinity'],
    )
  })

  test('3. the receipt records a public-safe cache-affinity HASH (never the raw key), populating the telemetry field', async () => {
    const usage: InferenceUsage = {
      cachedPromptTokens: 3,
      completionTokens: 4,
      promptTokens: 6,
      totalTokens: 10,
    }
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, captured, usage))

    const response = await run(
      handleChatCompletions(
        chatRequest({ ...khalaCodeBody('build a crossy road game'), user: 'sess-private' }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    const body = (await response.json()) as OpenAgentsTelemetry
    // The immediate block carries the cached-input dimension from provider usage.
    expect(body.openagents?.telemetry?.cachedInputTokens).toBe(3)
    // The raw session id never appears anywhere in the response body.
    expect(JSON.stringify(body)).not.toContain('sess-private')
  })

  test('5. cached input tokens populate from a fixture provider-usage payload', async () => {
    const usage: InferenceUsage = {
      cachedPromptTokens: 200,
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 367,
    }
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(VERTEX_GEMINI_ADAPTER_ID, captured, usage))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    const body = (await response.json()) as OpenAgentsTelemetry
    expect(body.openagents?.telemetry?.cachedInputTokens).toBe(200)
    expect(body.openagents?.telemetry?.promptTokens).toBe(347)
    expect(body.openagents?.telemetry?.completionTokens).toBe(20)
    expect(body.openagents?.telemetry?.totalTokens).toBe(367)
  })

  test('5. totalTokens reconciliation: the provider total (679) is recorded receipt-first, NOT recomputed as prompt+completion (367)', async () => {
    // The exact live discrepancy: a Gemini-backed reply whose totalTokenCount
    // (679) exceeds prompt (347) + completion (20) because of thinking/tool-use
    // tokens. Telemetry records the provider's authoritative total honestly.
    const usage: InferenceUsage = {
      completionTokens: 20,
      promptTokens: 347,
      totalTokens: 679,
    }
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(VERTEX_GEMINI_ADAPTER_ID, captured, usage))

    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: KHALA_MINI_MODEL_ID,
        }),
        baseDeps({ lanePlan: selectAdapterPlan, registry }),
      ),
    )
    const body = (await response.json()) as OpenAgentsTelemetry
    // 679, not 367 — the provider's authoritative count, not a recomputed sum.
    expect(body.openagents?.telemetry?.totalTokens).toBe(679)
    expect(body.openagents?.telemetry?.promptTokens).toBe(347)
    expect(body.openagents?.telemetry?.completionTokens).toBe(20)
  })

  test('6. cache-aware routing picks the warm lane under a fixture (warm lane served)', async () => {
    // Two lanes registered; the warm oracle says the OpenAI passthrough lane is
    // warm for this session, so it should be tried (and serve) FIRST even though
    // the open-class plan lists Fireworks first.
    const fireworksCaptured: Array<Captured> = []
    const passthroughCaptured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, fireworksCaptured))
    registry.register(
      recordingAdapter('passthrough-openai', passthroughCaptured),
    )

    const response = await run(
      handleChatCompletions(
        chatRequest({ ...khalaCodeBody('build a crossy road game'), user: 'sess-warm' }),
        baseDeps({
          // The warm oracle promotes the passthrough lane for ANY affinity hash
          // (the fixture); health + pin policy allow it.
          cacheWarmthOracle: () => 'passthrough-openai',
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      openagents?: { worker?: string }
    }
    // The warm passthrough lane served; Fireworks was never dispatched.
    expect(body.openagents?.worker).toBe('passthrough-openai')
    expect(passthroughCaptured).toHaveLength(1)
    expect(fireworksCaptured).toHaveLength(0)
  })

  test('6. cache-aware routing does NOT promote an unhealthy warm lane (falls back to cheapest-viable)', async () => {
    const fireworksCaptured: Array<Captured> = []
    const passthroughCaptured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(FIREWORKS_ADAPTER_ID, fireworksCaptured))
    registry.register(
      recordingAdapter('passthrough-openai', passthroughCaptured),
    )

    const response = await run(
      handleChatCompletions(
        chatRequest({ ...khalaCodeBody('build a crossy road game'), user: 'sess-sick' }),
        baseDeps({
          cacheWarmthOracle: () => 'passthrough-openai',
          laneHealthOracle: lane =>
            lane === 'passthrough-openai' ? 'unhealthy' : 'healthy',
          lanePlan: selectAdapterPlan,
          registry,
        }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { openagents?: { worker?: string } }
    // The sick warm lane is skipped; the cheapest-viable Fireworks lane serves.
    expect(body.openagents?.worker).toBe(FIREWORKS_ADAPTER_ID)
    expect(fireworksCaptured).toHaveLength(1)
  })

  test('a non-Khala request gets NO gateway affinity params + NO reorder (byte-identical to before)', async () => {
    const captured: Array<Captured> = []
    const registry = new InferenceProviderRegistry()
    registry.register(recordingAdapter(STUB_ECHO_ADAPTER_ID, captured))

    await run(
      handleChatCompletions(
        chatRequest({ ...helloBody, user: 'should-pass-through' }),
        baseDeps({ registry }),
      ),
    )
    expect(captured).toHaveLength(1)
    // The gateway does not inject a derived affinity for non-Khala models; the
    // client's own `user` is forwarded unchanged (passthrough), not overwritten.
    expect(captured[0]!.passthroughParams['x-session-affinity']).toBeUndefined()
    expect(captured[0]!.passthroughParams['user']).toBe('should-pass-through')
  })
})

// TYPED COMPONENT CHANNEL (EPIC #6123, issue #6127) ------------------------
// The additive, opt-in `oa.component` SSE channel. These exercise the route
// wiring end-to-end: a Khala turn streams prose + >=1 validated component frame;
// the channel is inert by default + for non-opted-in / non-Khala requests; an
// invalid card is dropped; a provider-identity leak never crosses the channel;
// and a standard OpenAI client still parses the stream as normal text.
describe('POST /v1/chat/completions — typed component channel (#6127)', () => {
  // A buffered (non-streamSse) adapter that returns a scripted completion as a
  // single chunk. The component channel deliberately forces the buffered path,
  // so this is the adapter shape the channel re-frames.
  const bufferedAdapter = (
    id: string,
    content: string,
  ): InferenceProviderAdapter => ({
    complete: () =>
      Effect.sync(() => ({
        content,
        finishReason: 'stop',
        servedModel: id,
        usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
      })),
    id,
    stream: () =>
      Effect.sync(() => [
        { contentDelta: content },
        {
          contentDelta: '',
          finishReason: 'stop',
          servedModel: id,
          usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
        },
      ]),
  })

  const khalaRegistry = (content: string): InferenceProviderRegistry => {
    const registry = new InferenceProviderRegistry()
    registry.register(bufferedAdapter(VERTEX_GEMINI_ADAPTER_ID, content))
    return registry
  }

  // Parse an SSE body into BOTH the content deltas (default `data:` chunks) and
  // the custom `event: oa.component` frames a standard client would ignore.
  const parseChannelSse = (
    text: string,
  ): Readonly<{
    contentDeltas: ReadonlyArray<string>
    componentFrames: ReadonlyArray<{
      v: number
      component: string
      props: Record<string, unknown>
      id: string
    }>
    done: boolean
  }> => {
    const contentDeltas: Array<string> = []
    const componentFrames: Array<{
      v: number
      component: string
      props: Record<string, unknown>
      id: string
    }> = []
    let done = false
    for (const block of text.split('\n\n')) {
      const lines = block.split('\n')
      const eventLine = lines.find(l => l.startsWith('event: '))
      const dataLine = lines.find(l => l.startsWith('data: '))
      if (dataLine === undefined) continue
      const payload = dataLine.replace(/^data: /u, '').trim()
      if (payload === '[DONE]') {
        done = true
        continue
      }
      if (eventLine?.replace('event: ', '').trim() === OA_COMPONENT_SSE_EVENT) {
        componentFrames.push(JSON.parse(payload))
        continue
      }
      const chunk = JSON.parse(payload) as {
        choices?: Array<{ delta?: { content?: string } }>
      }
      const delta = chunk.choices?.[0]?.delta?.content
      if (typeof delta === 'string') contentDeltas.push(delta)
    }
    return { componentFrames, contentDeltas, done }
  }

  const channelDeps = (
    registry: InferenceProviderRegistry,
    overrides: Partial<ChatCompletionsDeps> = {},
  ): ChatCompletionsDeps =>
    baseDeps({
      lanePlan: selectAdapterPlan,
      nowEpochMillis: () => 0,
      registry,
      ...overrides,
    })

  const channelOnConfig = { enabled: true }

  const khalaStreamBody = (extra: Record<string, unknown> = {}) => ({
    messages: [{ content: 'help me get started', role: 'user' }],
    model: KHALA_MINI_MODEL_ID,
    stream: true,
    ...extra,
  })

  test('streams prose + >=1 validated oa.component frame when opted in', async () => {
    const completion = [
      'Great — here is how we kick this off.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Kick off with $500 in credits"}}',
      '```',
      'Click the card to continue.',
    ].join('\n\n')

    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig,
        }),
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')
    const parsed = parseChannelSse(await response.text())
    // Prose came through as normal content deltas...
    expect(parsed.contentDeltas.join('')).toContain('kick this off')
    expect(parsed.contentDeltas.join('')).not.toContain('oa-component')
    // ...and the validated card came through as one atomic oa.component frame.
    expect(parsed.componentFrames).toHaveLength(1)
    expect(parsed.componentFrames[0]?.component).toBe('credit_kickoff')
    expect(parsed.componentFrames[0]?.v).toBe(1)
    expect(parsed.componentFrames[0]?.props['amountCents']).toBe(50000)
    expect(parsed.done).toBe(true)
  })

  test('a standard OpenAI client still parses the stream as normal text', async () => {
    // A standard client only reads default `data:` chunks and ignores unknown
    // `event:` types — so it sees a valid chat.completion.chunk sequence + DONE.
    const completion = [
      'Onboarding prose.',
      '```oa-component',
      '{"component":"human_handoff","props":{"reason":"x","contact":"y"}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig,
        }),
      ),
    )
    const text = await response.text()
    // Every default data frame is a valid chat.completion.chunk; DONE terminates.
    const parsed = parseChannelSse(text)
    expect(parsed.done).toBe(true)
    expect(parsed.contentDeltas.join('')).toContain('Onboarding prose')
    // A standard client never sees a raw oa-component fence in its text channel.
    expect(parsed.contentDeltas.join('')).not.toContain('oa-component')
  })

  test('INERT when the gateway flag is off (no component frames, text unchanged)', async () => {
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        // componentChannel ABSENT => flag off => channel inert.
        channelDeps(khalaRegistry(completion)),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    // No oa.component frames; the fence rides in the raw text channel unchanged.
    expect(parsed.componentFrames).toHaveLength(0)
    expect(parsed.contentDeltas.join('')).toContain('oa-component')
  })

  test('INERT when the request does NOT opt in (default text-only shape)', async () => {
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody()), // no opt-in field/header
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig, // flag ON, but request didn't opt in
        }),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    expect(parsed.componentFrames).toHaveLength(0)
    expect(parsed.contentDeltas.join('')).toContain('oa-component')
  })

  test('opt-in via the x-oa-component-channel header also activates the channel', async () => {
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"quick_win_card","props":{"title":"NDA","scope":"one doc","etaDays":3}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody()),
          headers: { 'x-oa-component-channel': 'on' },
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig,
        }),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    expect(parsed.componentFrames).toHaveLength(1)
    expect(parsed.componentFrames[0]?.component).toBe('quick_win_card')
  })

  test('a non-Khala model never activates the channel even when opted in', async () => {
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      '```',
    ].join('\n\n')
    const registry = new InferenceProviderRegistry()
    registry.register(bufferedAdapter(STUB_ECHO_ADAPTER_ID, completion))
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify({
            messages: [{ content: 'hi', role: 'user' }],
            model: 'stub-model',
            oa_component_channel: true,
            stream: true,
          }),
          method: 'POST',
        }),
        channelDeps(registry, { componentChannel: channelOnConfig }),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    expect(parsed.componentFrames).toHaveLength(0)
  })

  test('an unknown/closed-enum-rejected component is DROPPED (never emitted)', async () => {
    const completion = [
      'Here is a valid card and an invalid one.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      '```',
      '```oa-component',
      '{"component":"exfiltrate_secrets","props":{"x":1}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig,
        }),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    // Only the valid card survives; the unknown component is dropped.
    expect(parsed.componentFrames).toHaveLength(1)
    expect(parsed.componentFrames[0]?.component).toBe('credit_kickoff')
    // The unknown component name never appears anywhere in the wire body.
    const allComponents = parsed.componentFrames.map(f => f.component)
    expect(allComponents).not.toContain('exfiltrate_secrets')
  })

  test('a component whose props leak a provider identity is DROPPED (non-leakage)', async () => {
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"quick_win_card","props":{"title":"We are built on Gemini","scope":"x","etaDays":1}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: channelOnConfig,
        }),
      ),
    )
    const text = await response.text()
    const parsed = parseChannelSse(text)
    // The leaking card is dropped (never emitted as an oa.component frame)...
    expect(parsed.componentFrames).toHaveLength(0)
    // ...and the forbidden provider identity never crosses the component channel
    // OR the prose content channel (the existing `openagents` disclosure block's
    // own lane/worker fields are a separate, pre-existing receipt surface, not
    // the model-authored channels this feature governs).
    expect(
      JSON.stringify(parsed.componentFrames).toLowerCase(),
    ).not.toContain('gemini')
    expect(parsed.contentDeltas.join('').toLowerCase()).not.toContain('gemini')
  })

  test('an invalid card is repaired via ONE bounded reask, then emitted', async () => {
    let reaskCalls = 0
    const repairReask: ComponentRepairReask = async () => {
      reaskCalls += 1
      return '```oa-component\n{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Repaired"}}\n```'
    }
    const completion = [
      'Prose.',
      '```oa-component',
      '{"component":"credit_kickoff","props":{"amountCents":"bad"}}',
      '```',
    ].join('\n\n')
    const response = await run(
      handleChatCompletions(
        new Request('https://openagents.com/v1/chat/completions', {
          body: JSON.stringify(khalaStreamBody({ oa_component_channel: true })),
          method: 'POST',
        }),
        channelDeps(khalaRegistry(completion), {
          componentChannel: { enabled: true, repairReask },
        }),
      ),
    )
    const parsed = parseChannelSse(await response.text())
    expect(reaskCalls).toBe(1)
    expect(parsed.componentFrames).toHaveLength(1)
    expect(parsed.componentFrames[0]?.props['label']).toBe('Repaired')
  })
})
