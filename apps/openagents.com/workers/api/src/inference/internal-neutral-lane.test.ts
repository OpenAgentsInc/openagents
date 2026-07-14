// Persona-neutral internal lane (`openagents/internal-neutral`) fixture suite
// (#8600 FC-BRAIN).
//
// WHY THIS SUITE EXISTS. On 2026-07-09 Sarah's inference was routed through the
// shared `openagents/khala` lane and the gateway-injected Khala collective
// identity ("We are Khala") intermittently WON over Sarah's own system prompt
// on short turns ("who are you" -> "We are Khala. How can we help?"), and the
// open lane served gemini-3.5-flash instead of the Gemma-led plan. Production
// was rolled back to direct-Google. The durable fix is this persona-neutral
// internal model id: SAME routing/receipts/caps as the khala conversational
// lane, ZERO gateway persona conditioning, internal-allowlist-only.
//
// The PERSONA PROBE fixtures below assert the exact mechanism that caused the
// bleed can never fire on this lane: for short-turn identity probes the
// provider receives EXACTLY the caller's messages (no injected identity /
// refusal-posture / discipline system prompts, no "We are Khala", no
// "collective intelligence") and the completion is returned VERBATIM (no Khala
// signature guard rewriting). The khala lane is asserted as the CONTRAST so a
// regression that aliases the neutral id back into the persona path fails
// loudly here.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  handleChatCompletions,
  INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
  isToolBearingKhalaRequest,
  khalaRequestForAdapter,
} from './chat-completions-routes'
import { KHALA_FIREWORKS_BACKING_MODEL_ID } from './fireworks-adapter'
import { isPremiumModel } from './inference-premium-allowlist'
import {
  KHALA_IDENTITY_STATEMENT,
  KHALA_IDENTITY_SYSTEM_PROMPT,
} from './khala-identity'
import {
  FIREWORKS_ADAPTER_ID,
  classifyModel,
  selectAdapterPlan,
  selectAdapterPlanForKhalaToolRequest,
} from './model-router'
import { buildModelCatalog } from './model-catalog'
import {
  ALL_LANES_UNARMED,
  resolveNamedModelServability,
} from './model-serving-policy'
import {
  INTERNAL_NEUTRAL_MODEL_ID,
  KHALA_MODEL_ID,
  MODEL_PRICING_TABLE,
  isInternalNeutralModel,
  isKhalaRoutedModel,
} from './pricing'
import {
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
  type InferenceRequest,
} from './provider-adapter'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const INTERNAL_ACCOUNT = 'agent:user_sarah_internal'
const EXTERNAL_ACCOUNT = 'agent:user_external'
const ORG_NO_METER_SECRET = 'internal-neutral-no-spend'

const chatRequest = (body: unknown, init: RequestInit = {}): Request => {
  const headers = new Headers(init.headers)
  headers.set(
    INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER,
    ORG_NO_METER_SECRET,
  )
  return new Request('https://openagents.com/v1/chat/completions', {
    ...init,
    body: JSON.stringify(body),
    headers,
    method: 'POST',
  })
}

// A capture adapter: records every InferenceRequest it serves and returns a
// FIXED completion so guard behavior is observable (the completion text is a
// deliberate provider-identity leak the khala guard would rewrite).
const LEAKY_COMPLETION =
  'I am Sarah, an AI sales agent. I am built on Gemini by Google.'

const makeCaptureAdapter = (
  id: string,
  completion: string,
): { adapter: InferenceProviderAdapter; requests: Array<InferenceRequest> } => {
  const requests: Array<InferenceRequest> = []
  return {
    requests,
    adapter: {
      id,
      complete: (request: InferenceRequest) =>
        Effect.sync(() => {
          requests.push(request)
          return {
            content: completion,
            finishReason: 'stop',
            servedModel: 'gemma-4-31b-it',
            usage: { completionTokens: 12, promptTokens: 30, totalTokens: 50 },
          }
        }),
      stream: (request: InferenceRequest) =>
        Effect.sync(() => {
          requests.push(request)
          return [
            { contentDelta: completion },
            {
              contentDelta: '',
              finishReason: 'stop',
              servedModel: 'gemma-4-31b-it',
              usage: {
                completionTokens: 12,
                promptTokens: 30,
                totalTokens: 50,
              },
            },
          ]
        }),
    },
  }
}

const CAPTURE_ADAPTER_ID = 'capture-lane'

const depsFor = (
  adapter: InferenceProviderAdapter,
  overrides: Partial<ChatCompletionsDeps> = {},
): ChatCompletionsDeps => {
  const registry = new InferenceProviderRegistry()
  registry.register(adapter)
  return {
    authenticate: async () => ({ accountRef: INTERNAL_ACCOUNT }),
    enabled: true,
    internalAccountRefs: new Set([INTERNAL_ACCOUNT]),
    lanePlan: () => [CAPTURE_ADAPTER_ID],
    nowEpochMillis: () => 0,
    orgCloudRuntimeNoMeterSecret: ORG_NO_METER_SECRET,
    registry,
    ...overrides,
  }
}

// The Sarah-shaped system prompt: the ONLY conditioning the provider may see on
// the neutral lane.
const SARAH_SYSTEM_PROMPT =
  'You are Sarah, the OpenAgents relationship agent. Always speak as Sarah, first-person singular.'

// Short-turn identity probes — the exact turn shapes that triggered the
// 2026-07-09 collective-identity bleed on the shared khala lane.
const IDENTITY_PROBES: ReadonlyArray<string> = [
  'who are you',
  'what are you',
  'who am I talking to?',
  'hi',
  'you?',
  'ok',
]

describe('internal-neutral: routing parity with the khala lane (pure)', () => {
  test('id predicates', () => {
    expect(isInternalNeutralModel(INTERNAL_NEUTRAL_MODEL_ID)).toBe(true)
    expect(isInternalNeutralModel(' OPENAGENTS/INTERNAL-NEUTRAL ')).toBe(true)
    expect(isInternalNeutralModel(KHALA_MODEL_ID)).toBe(false)
    expect(isKhalaRoutedModel(INTERNAL_NEUTRAL_MODEL_ID)).toBe(true)
    expect(isKhalaRoutedModel(KHALA_MODEL_ID)).toBe(true)
    expect(isKhalaRoutedModel('gpt-oss-20b')).toBe(false)
  })

  test('adapter plan is EXACTLY the khala conversational plan', () => {
    expect(selectAdapterPlan(INTERNAL_NEUTRAL_MODEL_ID)).toEqual(
      selectAdapterPlan(KHALA_MODEL_ID),
    )
  })

  test('tool-bearing requests leave the Gemma lane exactly like khala', () => {
    expect(
      selectAdapterPlanForKhalaToolRequest(INTERNAL_NEUTRAL_MODEL_ID),
    ).toEqual(selectAdapterPlanForKhalaToolRequest(KHALA_MODEL_ID))
    expect(
      isToolBearingKhalaRequest({
        body: {
          messages: [{ content: 'hello', role: 'user' as const }],
        } as never,
        rawBody: { tools: [{ type: 'function' }] },
        requestedModel: INTERNAL_NEUTRAL_MODEL_ID,
      }),
    ).toBe(true)
  })

  test('per-adapter backing-model rewrite matches khala', () => {
    const request: InferenceRequest = {
      messages: [{ content: 'hi', role: 'user' }],
      model: INTERNAL_NEUTRAL_MODEL_ID,
      passthroughParams: {},
      stream: false,
    }
    expect(
      khalaRequestForAdapter(request, FIREWORKS_ADAPTER_ID).model,
    ).toBe(KHALA_FIREWORKS_BACKING_MODEL_ID)
  })

  test('classifies open and never premium', () => {
    expect(classifyModel(INTERNAL_NEUTRAL_MODEL_ID)).toBe(
      classifyModel(KHALA_MODEL_ID),
    )
    expect(isPremiumModel(INTERNAL_NEUTRAL_MODEL_ID)).toBe(false)
  })

  test('NEVER public: absent from the pricing table, catalog, and named servability', () => {
    expect(
      MODEL_PRICING_TABLE.some(
        entry => entry.model === INTERNAL_NEUTRAL_MODEL_ID,
      ),
    ).toBe(false)
    const catalog = buildModelCatalog()
    expect(
      catalog.some(entry => entry.id === INTERNAL_NEUTRAL_MODEL_ID),
    ).toBe(false)
    // Even with nothing armed the answer for the neutral id as a PUBLIC name is
    // a hard false (non-public id) — it can never be advertised or quoted.
    expect(
      resolveNamedModelServability(INTERNAL_NEUTRAL_MODEL_ID, ALL_LANES_UNARMED),
    ).toBe(false)
  })
})

describe('internal-neutral: internal-allowlist-only route gate', () => {
  test('a non-internal account gets the SAME model_unavailable an unknown id gets', async () => {
    const { adapter } = makeCaptureAdapter(CAPTURE_ADAPTER_ID, 'ok')
    const deps = depsFor(adapter, {
      authenticate: async () => ({ accountRef: EXTERNAL_ACCOUNT }),
    })
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: INTERNAL_NEUTRAL_MODEL_ID,
        }),
        deps,
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error?: string }
    expect(body.error).toBe('model_unavailable')
  })

  test('an empty allowlist refuses everyone (fail-closed)', async () => {
    const { adapter } = makeCaptureAdapter(CAPTURE_ADAPTER_ID, 'ok')
    const deps = depsFor(adapter, { internalAccountRefs: new Set<string>() })
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: INTERNAL_NEUTRAL_MODEL_ID,
        }),
        deps,
      ),
    )
    expect(response.status).toBe(400)
  })

  test('an internal-allowlist account is served', async () => {
    const { adapter } = makeCaptureAdapter(CAPTURE_ADAPTER_ID, 'hello there')
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [{ content: 'hi', role: 'user' }],
          model: INTERNAL_NEUTRAL_MODEL_ID,
        }),
        depsFor(adapter),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      choices: Array<{ message: { content: string } }>
    }
    expect(body.choices[0]!.message.content).toBe('hello there')
  })
})

describe('internal-neutral: PERSONA PROBE fixtures (the 2026-07-09 bleed class)', () => {
  test.each(IDENTITY_PROBES)(
    'probe %j: provider receives EXACTLY the caller messages — zero injected persona',
    async probe => {
      const { adapter, requests } = makeCaptureAdapter(
        CAPTURE_ADAPTER_ID,
        'I am Sarah. How can I help?',
      )
      const response = await run(
        handleChatCompletions(
          chatRequest({
            messages: [
              { content: SARAH_SYSTEM_PROMPT, role: 'system' },
              { content: probe, role: 'user' },
            ],
            model: INTERNAL_NEUTRAL_MODEL_ID,
          }),
          depsFor(adapter),
        ),
      )
      expect(response.status).toBe(200)
      expect(requests).toHaveLength(1)
      const served = requests[0]!
      // Byte-identical pass-through: the caller's system prompt is the ONLY
      // conditioning; no gateway block was injected, prepended, or reordered.
      expect(served.messages).toEqual([
        { content: SARAH_SYSTEM_PROMPT, role: 'system' },
        { content: probe, role: 'user' },
      ])
      const serialized = JSON.stringify(served.messages)
      expect(serialized).not.toContain('We are Khala')
      expect(serialized).not.toContain('collective intelligence')
      expect(serialized).not.toContain(KHALA_IDENTITY_SYSTEM_PROMPT)
    },
  )

  test('the completion is returned VERBATIM — no Khala signature guard rewrite', async () => {
    // The adapter answer is a deliberate provider-identity leak: on the khala
    // lane the guard rewrites it; on the neutral lane it MUST pass through
    // untouched (persona conditioning is the caller's job, not the gateway's).
    const { adapter } = makeCaptureAdapter(CAPTURE_ADAPTER_ID, LEAKY_COMPLETION)
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [
            { content: SARAH_SYSTEM_PROMPT, role: 'system' },
            { content: 'who are you', role: 'user' },
          ],
          model: INTERNAL_NEUTRAL_MODEL_ID,
        }),
        depsFor(adapter),
      ),
    )
    const body = (await response.json()) as {
      choices: Array<{ message: { content: string } }>
    }
    expect(body.choices[0]!.message.content).toBe(LEAKY_COMPLETION)
    expect(body.choices[0]!.message.content).not.toContain(
      KHALA_IDENTITY_STATEMENT,
    )
  })

  test('CONTRAST: the khala lane injects the collective identity and guards the same completion', async () => {
    const { adapter, requests } = makeCaptureAdapter(
      CAPTURE_ADAPTER_ID,
      LEAKY_COMPLETION,
    )
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [
            { content: SARAH_SYSTEM_PROMPT, role: 'system' },
            { content: 'who are you', role: 'user' },
          ],
          model: KHALA_MODEL_ID,
        }),
        depsFor(adapter),
      ),
    )
    expect(response.status).toBe(200)
    // The khala lane DOES inject the identity block (this is what bled into
    // Sarah's turns) — proving the two lanes genuinely diverge, so a regression
    // that aliases the neutral id back into the persona path fails the probes
    // above.
    const injectedSystemContents = requests[0]!.messages
      .filter(message => message.role === 'system')
      .map(message => message.content)
    expect(injectedSystemContents).toContain(KHALA_IDENTITY_SYSTEM_PROMPT)
    // ...and the khala guard rewrites the provider-identity leak.
    const body = (await response.json()) as {
      choices: Array<{ message: { content: string } }>
    }
    expect(body.choices[0]!.message.content).not.toBe(LEAKY_COMPLETION)
  })

  test('exact usage telemetry rides the neutral lane response (Sarah reads it)', async () => {
    const { adapter } = makeCaptureAdapter(CAPTURE_ADAPTER_ID, 'short answer')
    const response = await run(
      handleChatCompletions(
        chatRequest({
          messages: [
            { content: SARAH_SYSTEM_PROMPT, role: 'system' },
            { content: 'hi', role: 'user' },
          ],
          model: INTERNAL_NEUTRAL_MODEL_ID,
        }),
        depsFor(adapter),
      ),
    )
    const body = (await response.json()) as {
      usage?: { total_tokens?: number }
      openagents?: { telemetry?: { totalTokens?: unknown } }
    }
    expect(body.usage?.total_tokens).toBe(50)
    // The `openagents` receipt/telemetry block (the exact-usage rail Sarah's
    // streaming transport reads from the terminal frame) is present for the
    // neutral lane, not khala-persona-gated.
    expect(body.openagents?.telemetry?.totalTokens).toBe(50)
  })
})
