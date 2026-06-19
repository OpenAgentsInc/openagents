// OpenAI-compatible chat-completions route for the inference gateway
// (EPIC #5474, #5476). This is the FOUNDATION the rest of the inference build
// plugs into:
//
//   - flag-gated INERT by default (INFERENCE_GATEWAY_ENABLED, default off)
//   - per-account API-key auth (reuses the agent bearer-token credential)
//   - read-only credit-balance gate (rejects on insufficient balance; #5477
//     owns the real decrement/top-up paths, not this route)
//   - provider-adapter seam dispatch (registry resolves model -> adapter; ships
//     wired to the stub/echo adapter so the route works end-to-end in tests)
//   - metering-hook seam (#5477 decrements credits from provider `usage`;
//     stubbed here as a no-op/log)
//
// Streaming and non-streaming shapes are both supported. The response mirrors
// the OpenAI Chat Completions contract so off-the-shelf clients work by changing
// only base URL + key. Anthropic Messages compatibility is a parallel surface;
// a clean spot is left for it (see ANTHROPIC SEAM below) but is out of scope for
// #5476.

import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { compactRandomId, currentEpochSeconds } from '../runtime-primitives'
import {
  type InferenceMessage,
  type InferenceProviderRegistry,
  type InferenceRequest,
  type InferenceResult,
} from './provider-adapter'
import { type MeteringHook, stubMeteringHook } from './metering-hook'
import { STUB_ECHO_ADAPTER_ID } from './stub-echo-adapter'
import { type DispatchDeps, dispatchWithOverflow } from './model-router'

// AUTH SEAM ---------------------------------------------------------------
// Resolves the per-account API key (the OpenAgents agent bearer token) to an
// account ref. Returns undefined when the key is missing/invalid. The Worker
// wires this to `authenticateProgrammaticAgent`; tests inject a fake.
export type InferenceAuth = (
  request: Request,
) => Promise<Readonly<{ accountRef: string }> | undefined>

// BALANCE SEAM ------------------------------------------------------------
// Read-only available-credit check (msat) for the account. The Worker wires
// this to `readAgentBalance(...).availableMsat`. #5476 only GATES on balance;
// it never decrements (that is #5477's metering path).
export type InferenceBalanceReader = (
  accountRef: string,
) => Promise<number>

// ROUTING SEAM ------------------------------------------------------------
// Resolves a requested model alias to an adapter id. #5476 shipped a resolver
// that always selects the stub/echo adapter so the route is exercisable
// end-to-end. #5482 (routing & supply selection) adds the real cheapest-viable
// path via `lanePlan` below; the single-id `router` seam is retained for the
// stub/test path and as the gate when no `lanePlan` is supplied.
export type ModelRouter = (model: string) => string | undefined

export const stubModelRouter: ModelRouter = () => STUB_ECHO_ADAPTER_ID

// SUPPLY-SELECTION SEAM (#5482) -------------------------------------------
// Resolves a requested model to an ORDERED list of candidate adapter ids
// (cheapest viable lane first, then overflow fallbacks). When supplied, the
// route dispatches across this plan with bounded-backoff overflow on retryable
// provider failures (429 / 503 / 5xx / transport) — see `dispatchWithOverflow`
// in model-router.ts. The Worker wires this to `selectAdapterPlan`; when it is
// absent the route falls back to the single-id `router` seam (the #5476 path).
export type ModelLanePlanner = (model: string) => ReadonlyArray<string>

// Parse the INFERENCE_GATEWAY_ENABLED flag value. Default OFF: anything other
// than an explicit truthy token leaves the gateway inert.
export const isInferenceGatewayEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type ChatCompletionsDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // env.INFERENCE_GATEWAY_ENABLED parsed as a flag; default OFF.
  enabled: boolean
  authenticate: InferenceAuth
  readAvailableMsat: InferenceBalanceReader
  registry: InferenceProviderRegistry
  // Single-id router seam. Defaults to the stub router (always selects the
  // stub/echo adapter). Used to gate `model_unavailable` and to dispatch when
  // no multi-lane `lanePlan` is supplied (#5476 path).
  router?: ModelRouter
  // Ordered multi-lane plan for cheapest-viable selection + overflow (#5482).
  // When present, the route dispatches across the plan with bounded-backoff
  // overflow on retryable failures. The Worker wires this to `selectAdapterPlan`.
  lanePlan?: ModelLanePlanner
  // Routing overflow knobs (backoff + injected sleep) forwarded to
  // `dispatchWithOverflow`. Tests inject `sleep: () => Effect.void` so overflow
  // never waits. Ignored unless `lanePlan` is supplied.
  dispatch?: Omit<DispatchDeps, 'registry' | 'plan'>
  // Defaults to the no-op/log metering stub (#5477 supplies the live hook).
  meteringHook?: MeteringHook
  // Minimum available balance (msat) required to accept a request. Until #5477
  // prices per-model, any positive balance clears the gate; an account with
  // zero/negative available balance is rejected.
  minimumAvailableMsat?: number
  // Deterministic id/timestamp injection for tests. `nowEpochSeconds` defaults
  // to the runtime-primitives clock; `newId` to a runtime-primitives random id.
  nowEpochSeconds?: () => number
  newId?: () => string
}>

// REQUEST SCHEMA ----------------------------------------------------------
const ChatMessage = S.Struct({
  role: S.String,
  content: S.String,
})

// OpenAI Chat Completions request. Unknown sampling params are preserved and
// forwarded to the adapter via `passthroughParams`; only the load-bearing
// fields are decoded here.
const ChatCompletionsRequestBody = S.Struct({
  model: S.String,
  messages: S.Array(ChatMessage),
  stream: S.optionalKey(S.Boolean),
})

const decodeBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(ChatCompletionsRequestBody)(value)
  } catch {
    return undefined
  }
}

const toInferenceRequest = (
  body: typeof ChatCompletionsRequestBody.Type,
  raw: Record<string, unknown>,
): InferenceRequest => {
  const messages: ReadonlyArray<InferenceMessage> = body.messages.map(
    message => ({ content: message.content, role: message.role }),
  )
  const { messages: _messages, model: _model, stream: _stream, ...rest } = raw
  return {
    messages,
    model: body.model,
    passthroughParams: rest,
    stream: body.stream === true,
  }
}

const defaultId = () => compactRandomId('chatcmpl')

// OpenAI non-streaming response envelope.
const openAiResponse = (
  input: Readonly<{
    id: string
    created: number
    model: string
    result: InferenceResult
  }>,
) => ({
  choices: [
    {
      finish_reason: input.result.finishReason,
      index: 0,
      message: { content: input.result.content, role: 'assistant' },
    },
  ],
  created: input.created,
  id: input.id,
  model: input.model,
  object: 'chat.completion',
  usage: {
    completion_tokens: input.result.usage.completionTokens,
    prompt_tokens: input.result.usage.promptTokens,
    total_tokens: input.result.usage.totalTokens,
  },
})

const sseFrame = (payload: unknown): string =>
  `data: ${JSON.stringify(payload)}\n\n`

export const handleChatCompletions = (
  request: Request,
  deps: ChatCompletionsDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE: flagged off in production until the EPIC lands.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    // ANTHROPIC SEAM: a parallel `/v1/messages` handler normalizes Anthropic
    // Messages into the same InferenceRequest and reuses the registry +
    // metering hook below. Out of scope for #5476.

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { headers, status: 401 },
      )
    }

    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })
    if (rawBody === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeBody(rawBody)
    if (body === undefined || body.messages.length === 0) {
      return noStoreJsonResponse(
        { error: 'invalid_request' },
        { status: 400 },
      )
    }

    // BALANCE GATE (read-only). #5477 owns the real per-model decrement.
    const minimum = deps.minimumAvailableMsat ?? 1
    const availableMsat = yield* Effect.promise(() =>
      deps.readAvailableMsat(session.accountRef),
    )
    if (availableMsat < minimum) {
      return noStoreJsonResponse(
        {
          error: 'insufficient_credits',
          availableMsat,
          requiredMsat: minimum,
        },
        { status: 402 },
      )
    }

    // SUPPLY SELECTION (#5482) -------------------------------------------
    // Resolve the ordered candidate adapter ids for this model. When a
    // multi-lane `lanePlan` is supplied (the Worker wires `selectAdapterPlan`)
    // the route dispatches across it with bounded-backoff overflow; otherwise
    // it falls back to the single-id `router` seam (the #5476 / stub path).
    const planFor: ModelLanePlanner =
      deps.lanePlan ??
      (model => {
        const id = (deps.router ?? stubModelRouter)(model)
        return id === undefined ? [] : [id]
      })
    const plannedIds = planFor(body.model)
    // model_unavailable when no lane is configured OR none of the planned lanes
    // is actually registered (e.g. an absent partner secret leaves the plan but
    // no resolvable adapter).
    const hasViableLane = plannedIds.some(
      id => deps.registry.resolve(id) !== undefined,
    )
    if (!hasViableLane) {
      return noStoreJsonResponse(
        { error: 'model_unavailable', model: body.model },
        { status: 400 },
      )
    }

    const inferenceRequest = toInferenceRequest(body, rawBody)
    const meteringHook = deps.meteringHook ?? stubMeteringHook
    const nowEpochSeconds = deps.nowEpochSeconds ?? currentEpochSeconds
    const newId = deps.newId ?? defaultId
    const created = nowEpochSeconds()
    const responseId = newId()

    // Dispatch deps for the overflow loop. The plan is pinned to `plannedIds`
    // (already resolved from lanePlan/router above) so selection + dispatch use
    // exactly the same ordering.
    const dispatchDeps: DispatchDeps = {
      registry: deps.registry,
      plan: () => plannedIds,
      ...(deps.dispatch?.backoff === undefined
        ? {}
        : { backoff: deps.dispatch.backoff }),
      ...(deps.dispatch?.sleep === undefined
        ? {}
        : { sleep: deps.dispatch.sleep }),
    }

    if (inferenceRequest.stream) {
      // Run the stream op across the lane plan; the served adapter id rides
      // alongside the chunks so metering reports the lane that actually served.
      const chunks = yield* dispatchWithOverflow(
        inferenceRequest,
        (adapter, request) =>
          adapter
            .stream(request)
            .pipe(Effect.map(value => ({ adapterId: adapter.id, value }))),
        dispatchDeps,
      ).pipe(
        Effect.map(served => ({ ok: true as const, served })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
      if (!chunks.ok) {
        return noStoreJsonResponse(
          { error: 'provider_error', reason: chunks.reason },
          { status: 502 },
        )
      }
      const servedChunks = chunks.served.value
      // Settle metering from the terminal usage frame (receipt-first).
      const terminal = [...servedChunks]
        .reverse()
        .find(chunk => chunk.usage !== undefined)
      if (terminal?.usage !== undefined) {
        yield* meteringHook({
          accountRef: session.accountRef,
          adapterId: chunks.served.adapterId,
          requestedModel: body.model,
          servedModel: body.model,
          streamed: true,
          usage: terminal.usage,
        })
      }

      const body_sse = servedChunks
        .map(chunk =>
          sseFrame({
            choices: [
              {
                delta:
                  chunk.contentDelta === ''
                    ? {}
                    : { content: chunk.contentDelta },
                finish_reason: chunk.finishReason ?? null,
                index: 0,
              },
            ],
            created,
            id: responseId,
            model: body.model,
            object: 'chat.completion.chunk',
          }),
        )
        .join('')
      const stream = `${body_sse}data: [DONE]\n\n`

      return new Response(stream, {
        headers: {
          'cache-control': 'no-store',
          'content-type': 'text/event-stream; charset=utf-8',
        },
        status: 200,
      })
    }

    const result = yield* dispatchWithOverflow(
      inferenceRequest,
      (adapter, request) =>
        adapter
          .complete(request)
          .pipe(Effect.map(value => ({ adapterId: adapter.id, value }))),
      dispatchDeps,
    ).pipe(
      Effect.map(served => ({ ok: true as const, served })),
      Effect.catch(error =>
        Effect.succeed({ ok: false as const, reason: error.reason }),
      ),
    )
    if (!result.ok) {
      return noStoreJsonResponse(
        { error: 'provider_error', reason: result.reason },
        { status: 502 },
      )
    }

    yield* meteringHook({
      accountRef: session.accountRef,
      adapterId: result.served.adapterId,
      requestedModel: body.model,
      servedModel: result.served.value.servedModel,
      streamed: false,
      usage: result.served.value.usage,
    })

    return noStoreJsonResponse(
      openAiResponse({
        created,
        id: responseId,
        model: body.model,
        result: result.served.value,
      }),
    )
  })
