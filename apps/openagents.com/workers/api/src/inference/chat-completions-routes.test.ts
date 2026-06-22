import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  type InferenceAuth,
  type InferenceBalanceReader,
  handleChatCompletions,
  isInferenceGatewayEnabled,
} from './chat-completions-routes'
import { decideFairShare, decideSpendCap } from './inference-abuse-controls'
import {
  BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
  GOOD_CROSSY_ROAD_HTML,
} from './khala-code-verifier.fixtures'
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
import { KHALA_CODE_MODEL_ID, KHALA_MINI_MODEL_ID } from './pricing'
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
    const nonStreamedBody = (await nonStreamed.json()) as { openagents?: unknown }
    expect(finalOpenagents).toEqual(nonStreamedBody.openagents)
    expect(finalOpenagents).toEqual({
      lane: 'gemini',
      requested_model: KHALA_MINI_MODEL_ID,
      served_model: KHALA_MINI_MODEL_ID,
      verification: 'none',
      worker: VERTEX_GEMINI_ADAPTER_ID,
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
    expect(body.openagents).toEqual({
      lane: 'gemini',
      requested_model: KHALA_MINI_MODEL_ID,
      served_model: KHALA_MINI_MODEL_ID,
      verification: 'none',
      worker: VERTEX_GEMINI_ADAPTER_ID,
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
      verified: false,
      worker: FIREWORKS_ADAPTER_ID,
      workers: [FIREWORKS_ADAPTER_ID, 'khala-code-crossy-road-verifier'],
    })
    expect(body.openagents?.verification_receipt).toMatch(
      /^receipt\.inference\.khala_code\.verification\.chatcmpl-khala-code-pass\./u,
    )
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
