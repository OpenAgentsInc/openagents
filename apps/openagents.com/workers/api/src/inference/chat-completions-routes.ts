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
  type FairShareDecision,
  type SpendCapDecision,
} from './inference-abuse-controls'
import { type PremiumAccessDecision } from './inference-premium-allowlist'
// PURE verdict type only — the route NEVER imports the headless runner (playwright)
// into the Worker bundle. The runner executes out of the Worker; its verdict shape is
// the only thing the route consumes (EPIC #6017).
import { type AcceptanceVerdict } from './acceptance-runner/verdict'
// PURE dispatch producer only — enqueues an out-of-Worker verification job; never
// imports the headless runner (playwright) into the Worker bundle (EPIC #6017).
import {
  type AcceptanceJobQueue,
  enqueueAcceptanceJob,
} from './acceptance-dispatch'
import { intentToAcceptanceSpec } from './acceptance-spec'
import {
  KHALA_CODE_VERIFIER_WORKER_ID,
  type KhalaCodeVerificationVerdict,
  prescreenKhalaCodeArtifact,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'
import {
  type MeteringHook,
  type MeteringOutcome,
  stubMeteringHook,
} from './metering-hook'
import {
  type DispatchDeps,
  classifyModel,
  dispatchWithOverflow,
} from './model-router'
import {
  type SupplyLaneArming,
  resolveNamedModelServability,
} from './model-serving-policy'
import { type FundingKind, KHALA_CODE_MODEL_ID, isKhalaModel } from './pricing'
import {
  InferenceAdapterError,
  type InferenceMessage,
  type InferenceProviderRegistry,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamSource,
} from './provider-adapter'
import { STUB_ECHO_ADAPTER_ID } from './stub-echo-adapter'

// DEFAULT MODEL ------------------------------------------------------------
// The model served when a request omits `model`. The free-tier default is
// Gemini 3.5 Flash (gateway free-tier enablement §2). Kept here as the single
// route-level default so an unspecified request routes to the free lane.
export const DEFAULT_CHAT_MODEL = 'gemini-3.5-flash'

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
export type InferenceBalanceReader = (accountRef: string) => Promise<number>

// FUNDING-KIND SEAM -------------------------------------------------------
// Resolves how an account funds its balance (card | bitcoin) so the metering
// hook (#5477) applies the Bitcoin funding discount in `priceRequest`. Defaults
// to card. A real per-account card-vs-Bitcoin funding preference wires here once
// the credit top-up paths record it; until then every account is treated as
// card-funded (the conservative, no-discount default).
export type InferenceFundingResolver = (
  accountRef: string,
) => Promise<FundingKind>

export const defaultCardFundingResolver: InferenceFundingResolver = async () =>
  'card'

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
  // PROVIDER SERVING POLICY (blocker.product_promises.public_paid_model_gateway_missing
  // on api.hosted_gemini.v1). The SAME presence-derived lane arming the public
  // catalog (`/v1/models`) and the pre-purchase quote (`/v1/quote`) are gated on.
  // When supplied, a request for a KNOWN model whose supply lane is NOT armed is
  // rejected with a clean `model_unavailable` (400) BEFORE any account-state gate
  // or provider dispatch — so the gateway serves exactly what it advertises and
  // quotes, instead of accepting the request and failing deep at dispatch with a
  // generic `provider_error` (502). An UNKNOWN model id is not gated (the
  // estimator prices it at the conservative fallback rate, consistent with
  // `/v1/quote`). Omitting it preserves the prior serve-everything behaviour.
  // Presence-only; no secret value is read here.
  laneArming?: SupplyLaneArming
  // Routing overflow knobs (backoff + injected sleep) forwarded to
  // `dispatchWithOverflow`. Tests inject `sleep: () => Effect.void` so overflow
  // never waits. Ignored unless `lanePlan` is supplied.
  dispatch?: Omit<DispatchDeps, 'registry' | 'plan'>
  // Defaults to the no-op/log metering stub. The Worker supplies the live
  // ledger hook (`makeLedgerMeteringHook`, #5477) when the gateway is enabled.
  meteringHook?: MeteringHook
  // Resolves the account's funding kind (card | bitcoin) for the metering hook.
  // Defaults to card (no Bitcoin discount).
  resolveFundingKind?: InferenceFundingResolver
  // Minimum available balance (msat) required to accept a request. Until #5477
  // prices per-model, any positive balance clears the gate; an account with
  // zero/negative available balance is rejected.
  minimumAvailableMsat?: number
  // FREE-ALLOWANCE PRE-FLIGHT (EPIC #5474 §1). Read-only mirror of the gate
  // inside `withFreeAllowance`. The balance gate calls this BEFORE rejecting a
  // zero/insufficient-balance account: if the (account, model) is free-eligible
  // and the resolving owner still has remaining free allowance, the request is
  // allowed through (the metering hook then eats the cost and accrues it), so a
  // genuinely-free request is never falsely 402'd. Default undefined => the gate
  // is unchanged (a zero-balance account is rejected). Resolution errors return
  // not-eligible, so the balance gate stands. Wired by the Worker to
  // `checkFreeAllowancePreflight` against the SAME owner-identity resolver the
  // metering hook uses, keeping the bypass and the accrual consistent.
  checkFreeAllowance?: (
    accountRef: string,
    model: string,
  ) => Promise<{ readonly eligible: boolean }>
  // ABUSE-CONTROL SEAMS (#5486). Both default to undefined => the gate is OPEN
  // (no-op), so the inert/flag-off path and the unconfigured-account path are
  // byte-for-byte unchanged. The decisions are computed by the pure deciders in
  // inference-abuse-controls.ts (`decideFairShare` / `decideSpendCap`); the
  // Worker wires these to per-account window counters (D1/KV) so one customer
  // cannot starve the shared Vertex quota (fair-share) or drain its whole balance
  // via a compromised key (spend cap), both DISTINCT from the raw balance gate.
  //
  // PER-CUSTOMER RATE / FAIR-SHARE GATE. Returns the fair-share decision for the
  // account in the current window. When `allowed` is false the route rejects with
  // the decision's statusCode (429) and sets RateLimit-* headers from the
  // bounded counters. Checked AFTER auth (so it is keyed to the account) and
  // BEFORE provider dispatch (so a starve-attempt never reaches a provider).
  checkFairShare?: (accountRef: string) => Promise<FairShareDecision>
  // PER-ACCOUNT SPEND-CAP GATE. Returns the spend-cap decision for the account in
  // the current window. When `allowed` is false the route rejects with the
  // decision's statusCode (402, distinct from the balance gate's
  // insufficient_credits). Checked after the balance gate; pre-flight it bounds
  // the already-spent window total (no per-request estimate exists yet).
  checkSpendCap?: (accountRef: string) => Promise<SpendCapDecision>
  // PREMIUM-MODEL OWNER-GRANT GATE (free-tier enablement §2). Returns the
  // premium-access decision for the (account, model) pair. Premium models
  // (Claude / GPT / partner-passthrough) require the account's resolved OWNER
  // identity to be on the owner-controlled allowlist; non-allowlisted premium
  // requests are DENIED (403) with a clear, actionable message. Non-premium
  // models (Gemini free default, Fireworks open) always pass. Default undefined
  // => the gate is OPEN (no-op), so the inert/flag-off path is unchanged.
  // Checked AFTER auth (so it is keyed to the resolved owner) and BEFORE
  // provider dispatch (so a denied premium request never reaches a provider).
  checkPremiumAccess?: (
    accountRef: string,
    model: string,
  ) => Promise<PremiumAccessDecision>
  // Deterministic id/timestamp injection for tests. `nowEpochSeconds` defaults
  // to the runtime-primitives clock; `newId` to a runtime-primitives random id.
  nowEpochSeconds?: () => number
  newId?: () => string
  // ACCEPTANCE-DISPATCH SEAM (EPIC #6017). When a khala-code completion produces an
  // EXECUTABLE artifact (it passes the cheap pre-screen), the gateway enqueues an
  // out-of-Worker verification job: a node-side runner (Pylon / sandbox / Cloud Run)
  // runs the real headless acceptance suite and posts the verdict back to the
  // authenticated callback, which backfills the receipt (`unverified` ->
  // `test_passed`/`failed`). DEFAULT OFF + UNWIRED: with `acceptanceDispatch.enabled`
  // false or no `queue`/`artifactStore` wired, NOTHING is enqueued and the receipt
  // stays the honest `unverified`. The Worker wires this once a runner host is
  // deployed and the flag (KHALA_ACCEPTANCE_DISPATCH_ENABLED) is on. Chromium NEVER
  // runs in the Worker; this only enqueues a job (pure producer).
  acceptanceDispatch?: Readonly<{
    enabled: boolean
    queue: AcceptanceJobQueue | undefined
    // Persist the artifact bytes and return a dereferenceable ref the runner
    // resolves (an R2 key). Absent => no job is enqueued (the runner would have no
    // artifact to fetch). Receives the request id + the runnable HTML.
    storeArtifact?: (
      input: Readonly<{ requestId: string; html: string }>,
    ) => Promise<string>
  }>
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
  // `model` is OPTIONAL: an unspecified model defaults to DEFAULT_CHAT_MODEL
  // (Gemini 3.5 Flash, the free lane) in `resolveRequestedModel` below.
  model: S.optionalKey(S.String),
  messages: S.Array(ChatMessage),
  stream: S.optionalKey(S.Boolean),
})

// Resolve the effective requested model: a present, non-blank `model` as given,
// otherwise the free-tier default. Centralized so the default applies uniformly
// to routing, premium gating, response echo, and metering.
export const resolveRequestedModel = (model: string | undefined): string => {
  const trimmed = model?.trim()
  return trimmed === undefined || trimmed === '' ? DEFAULT_CHAT_MODEL : trimmed
}

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
  requestedModel: string,
): InferenceRequest => {
  const messages: ReadonlyArray<InferenceMessage> = body.messages.map(
    message => ({ content: message.content, role: message.role }),
  )
  const { messages: _messages, model: _model, stream: _stream, ...rest } = raw
  return {
    messages,
    model: requestedModel,
    passthroughParams: rest,
    stream: body.stream === true,
  }
}

const defaultId = () => compactRandomId('chatcmpl')

// The OpenAgents disclosure block attached to a Khala response (M0 / #6008). A
// Khala model id is one endpoint over a pool; this NON-BREAKING `openagents`
// field discloses which concrete model/worker actually served the request so a
// Khala completion is auditable rather than opaque. `verification` is `none` in
// M0 (no verifier pass yet — that is a later milestone); the field exists so the
// receipt shape is stable as verification classes land. No prompts, credentials,
// or chain-of-thought are exposed.
type OpenAgentsReceipt = Readonly<{
  requested_model: string
  served_model: string
  worker: string
  lane: string
  // `unverified` is the HONEST default for an executable artifact we have not actually
  // run yet (EPIC #6017): the regex pre-screen passed but the out-of-Worker headless
  // acceptance runner has not executed it, so we do NOT certify it. `test_passed` is
  // reserved for an EXECUTED acceptance suite that fully passed.
  verification: 'none' | 'test_passed' | 'unverified' | 'failed'
  // Whether a real headless acceptance run produced this verdict. False => the
  // verdict is the honest pre-screen-only downgrade, not an execution result.
  executed?: boolean | undefined
  verified?: boolean | undefined
  receipt?: string | undefined
  receipt_url?: string | undefined
  route?: 'coding' | undefined
  workers?: ReadonlyArray<string> | undefined
  verification_receipt?: string | undefined
  verification_command?: string | undefined
  scalar_reward?: number | undefined
  reward_handoff?: string | undefined
  rubric?:
    | Readonly<{
        ref: string
        passed_checks: ReadonlyArray<string>
        failed_checks: ReadonlyArray<string>
      }>
    | undefined
}>

const publicInferenceReceiptUrl = (receiptRef: string): string =>
  `/api/public/inference/receipts/${encodeURIComponent(receiptRef)}`

const khalaReceiptForResult = (
  input: Readonly<{
    adapterId: string
    metering: MeteringOutcome
    requestedModel: string
    responseId: string
    result: InferenceResult
    // The executed-acceptance verdict from the out-of-Worker headless runner, when an
    // execution actually ran (EPIC #6017). PRESENT => `verified`/`scalarReward` derive
    // from EXECUTION. ABSENT (the hot Worker path today, which cannot launch a browser)
    // => HONEST DOWNGRADE to `unverified`. Full prod wiring (async sandbox dispatch of
    // the runner + receipt backfill) threads a real verdict in here.
    acceptance?: AcceptanceVerdict | undefined
  }>,
): OpenAgentsReceipt | undefined => {
  if (!isKhalaModel(input.requestedModel)) {
    return undefined
  }

  const base = {
    lane: classifyModel(input.requestedModel),
    requested_model: input.requestedModel,
    served_model: input.result.servedModel,
    worker: input.adapterId,
  } satisfies Omit<OpenAgentsReceipt, 'verification'>

  if (input.requestedModel !== KHALA_CODE_MODEL_ID) {
    return { ...base, verification: 'none' }
  }

  const verdict: KhalaCodeVerificationVerdict = verifyKhalaCodeCompletion({
    content: input.result.content,
    meteringReceiptRef: input.metering.receiptRef,
    requestId: input.responseId,
    servedModel: input.result.servedModel,
    worker: input.adapterId,
    ...(input.acceptance === undefined ? {} : { acceptance: input.acceptance }),
  })
  const receipt = input.metering.receiptRef ?? verdict.receiptRef

  return {
    ...base,
    executed: verdict.executed,
    receipt,
    ...(input.metering.receiptRef === null
      ? {}
      : { receipt_url: publicInferenceReceiptUrl(input.metering.receiptRef) }),
    reward_handoff: verdict.reward.handoffRef,
    route: 'coding',
    rubric: {
      failed_checks: verdict.failedChecks,
      passed_checks: verdict.passedChecks,
      ref: verdict.rubricRef,
    },
    scalar_reward: verdict.scalarReward,
    verification: verdict.verification,
    verification_command: verdict.command.commandRef,
    verification_receipt: verdict.receiptRef,
    verified: verdict.verified,
    workers: [input.adapterId, KHALA_CODE_VERIFIER_WORKER_ID],
  }
}

// Enqueue an out-of-Worker acceptance-verification job for a khala-code completion
// that produced an EXECUTABLE artifact (EPIC #6017). INERT + FAIL-SOFT: returns
// immediately (no enqueue) for a non-khala-code model, a flag-off dispatch, a missing
// queue/artifact store, or an artifact that fails the cheap pre-screen (not even worth
// running). When it does enqueue, it derives the spec from intent (reusing
// `intentToAcceptanceSpec`), persists the artifact to get a dereferenceable ref, and
// sends the typed job. A failure to store/enqueue is swallowed (the completion is
// already delivered; the receipt simply stays the honest `unverified`).
const maybeEnqueueAcceptanceJob = (
  input: Readonly<{
    dispatch: ChatCompletionsDeps['acceptanceDispatch']
    requestedModel: string
    requestMessages: ReadonlyArray<InferenceMessage>
    responseId: string
    result: InferenceResult
    adapterId: string
    meteringReceiptRef: string | null
  }>,
): Effect.Effect<void> =>
  Effect.gen(function* () {
    const dispatch = input.dispatch
    if (
      dispatch === undefined ||
      !dispatch.enabled ||
      dispatch.queue === undefined ||
      dispatch.storeArtifact === undefined ||
      input.requestedModel !== KHALA_CODE_MODEL_ID
    ) {
      return
    }

    // Only enqueue a RUNNABLE artifact (the cheap pre-screen gates execution; it is
    // NOT the verdict). A non-runnable artifact already verifies as `failed` on the
    // hot path; nothing to execute.
    const prescreen = prescreenKhalaCodeArtifact(input.result.content)
    if (!prescreen.attemptExecution || prescreen.html === undefined) {
      return
    }

    const spec = intentToAcceptanceSpec({
      messages: input.requestMessages.map(message => ({
        content: message.content,
        role: message.role,
      })),
      model: input.requestedModel,
    })
    if (spec === undefined) {
      return
    }

    // Persist the artifact -> dereferenceable ref the runner resolves. A store failure
    // means no runnable handle, so we do not enqueue (receipt stays `unverified`).
    const artifactRef = yield* Effect.tryPromise(() =>
      dispatch.storeArtifact!({
        html: prescreen.html as string,
        requestId: input.responseId,
      }),
    ).pipe(Effect.map(ref => ref as string | undefined), Effect.orElseSucceed(() => undefined))
    if (artifactRef === undefined) {
      return
    }

    yield* enqueueAcceptanceJob({
      artifactRef,
      enabled: dispatch.enabled,
      meteringReceiptRef: input.meteringReceiptRef,
      queue: dispatch.queue,
      requestId: input.responseId,
      servedModel: input.result.servedModel,
      spec,
      worker: input.adapterId,
    }).pipe(Effect.asVoid)
  })

// OpenAI non-streaming response envelope.
const openAiResponse = (
  input: Readonly<{
    id: string
    created: number
    model: string
    result: InferenceResult
    openagents?: OpenAgentsReceipt | undefined
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
  ...(input.openagents === undefined ? {} : { openagents: input.openagents }),
  usage: {
    completion_tokens: input.result.usage.completionTokens,
    prompt_tokens: input.result.usage.promptTokens,
    total_tokens: input.result.usage.totalTokens,
  },
})

const sseFrame = (payload: unknown): string =>
  `data: ${JSON.stringify(payload)}\n\n`

// Build a TRUE incremental SSE Response body that pumps the upstream stream
// source to the client frame-by-frame (the khala-code 524 fix). Each content
// delta is emitted AS IT ARRIVES — bytes flow continuously so the Cloudflare
// edge idle-timer resets and a multi-minute generation never 524s. The terminal
// `openagents` disclosure block (built by the SAME `khalaReceiptForResult` the
// non-streaming + buffered paths use) and metering settlement happen AFTER the
// upstream closes, attached to the final `chat.completion.chunk` before
// `data: [DONE]`. Metering settles receipt-first from the terminal usage frame.
//
// This runs OUTSIDE the route's Effect: the metering hook is an Effect, so it is
// executed with `Effect.runPromise` in the flush step (the same hook used on the
// buffered/non-streaming paths). The metering hook never fails (it returns a
// typed outcome), so a metering error never breaks the already-delivered stream.
const makePassThroughResponseStream = (
  input: Readonly<{
    accountRef: string
    adapterId: string
    created: number
    fundingKind: FundingKind
    meteringHook: MeteringHook
    requestedModel: string
    responseId: string
    source: InferenceStreamSource
  }>,
): ReadableStream<Uint8Array> => {
  const encoder = new TextEncoder()
  const contentParts: Array<string> = []
  return new ReadableStream<Uint8Array>({
    async start(controller) {
      const chunkFrame = (
        delta: string,
        finishReason: string | null,
        openagents?: OpenAgentsReceipt | undefined,
      ): string =>
        sseFrame({
          choices: [
            {
              delta: delta === '' ? {} : { content: delta },
              finish_reason: finishReason,
              index: 0,
            },
          ],
          created: input.created,
          id: input.responseId,
          model: input.requestedModel,
          object: 'chat.completion.chunk',
          ...(openagents === undefined ? {} : { openagents }),
        })

      try {
        // Pump every upstream content delta to the client immediately.
        for await (const event of input.source.frames) {
          if (event.contentDelta !== '') {
            contentParts.push(event.contentDelta)
            controller.enqueue(
              encoder.encode(chunkFrame(event.contentDelta, null)),
            )
          }
        }
      } catch {
        // The upstream stream faulted mid-flight. The client already has partial
        // content; close the SSE cleanly (DONE) so it is not left hanging. There
        // is no terminal usage frame, so metering does NOT settle (receipt-first
        // — never an estimate), exactly as the buffered path would on a stream
        // with no terminal usage.
        controller.enqueue(encoder.encode('data: [DONE]\n\n'))
        controller.close()
        return
      }

      // Stream drained: settle metering + build the disclosure from the terminal
      // state, receipt-first. No re-buffering of content beyond the join needed
      // for the verifier's full-output rubric.
      const terminal = input.source.terminal()
      const servedModel = terminal.servedModel ?? input.requestedModel
      let streamMetering: MeteringOutcome | undefined
      if (terminal.usage !== undefined) {
        streamMetering = await Effect.runPromise(
          input.meteringHook({
            accountRef: input.accountRef,
            adapterId: input.adapterId,
            fundingKind: input.fundingKind,
            requestId: input.responseId,
            requestedModel: input.requestedModel,
            servedModel,
            streamed: true,
            usage: terminal.usage,
          }),
        )
      }

      const streamResult: InferenceResult = {
        content: contentParts.join(''),
        finishReason: terminal.finishReason ?? 'stop',
        servedModel,
        usage: terminal.usage ?? {
          completionTokens: 0,
          promptTokens: 0,
          totalTokens: 0,
        },
      }
      const openagents = khalaReceiptForResult({
        adapterId: input.adapterId,
        metering: streamMetering ?? { metered: false, receiptRef: null },
        requestedModel: input.requestedModel,
        responseId: input.responseId,
        result: streamResult,
      })

      // Terminal frame: empty delta + finish reason + disclosure block.
      controller.enqueue(
        encoder.encode(
          chunkFrame(
            '',
            terminal.finishReason ?? 'stop',
            openagents,
          ),
        ),
      )
      controller.enqueue(encoder.encode('data: [DONE]\n\n'))
      controller.close()
    },
  })
}

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

    // FAIR-SHARE GATE (#5486). Keyed to the authenticated account so one customer
    // cannot starve the shared Vertex quota / Fireworks limits. Open (no-op) when
    // unwired. Rejected requests carry RateLimit-* headers from the bounded
    // counters so well-behaved clients back off; nothing reaches a provider.
    if (deps.checkFairShare !== undefined) {
      const fairShare = yield* Effect.promise(() =>
        deps.checkFairShare!(session.accountRef),
      )
      if (!fairShare.allowed) {
        const headers = new Headers({
          'ratelimit-limit': String(fairShare.limit),
          'ratelimit-policy': `${fairShare.limit};w=${fairShare.windowSeconds}`,
          'ratelimit-remaining': String(fairShare.remainingRequests),
          'ratelimit-reset': String(fairShare.windowSeconds),
          'retry-after': String(fairShare.windowSeconds),
        })
        return noStoreJsonResponse(
          {
            error: 'rate_limited',
            reason: fairShare.status,
            remainingRequests: fairShare.remainingRequests,
            remainingTokens: fairShare.remainingTokens,
          },
          { headers, status: fairShare.statusCode },
        )
      }
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
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    // Resolve the effective model once: an unspecified/blank model defaults to
    // the free-tier default (Gemini 3.5 Flash). Used for premium gating,
    // routing, response echo, and metering.
    const requestedModel = resolveRequestedModel(body.model)

    // PROVIDER SERVING-POLICY GATE (public_paid_model_gateway_missing). Reject a
    // KNOWN model whose supply lane is NOT armed with the SAME clean
    // `model_unavailable` the catalog hides and the quote 404s — keeping
    // advertise == quote == serve. Checked BEFORE the account-state gates
    // (premium / balance / spend-cap) and before dispatch, because servability is
    // a property of the model + supply, independent of the account: an unservable
    // model is never the customer's balance/allowlist problem. An UNKNOWN model id
    // (servability `undefined`) falls through unchanged, as on `/v1/quote`. Open
    // (no-op) when `laneArming` is omitted.
    if (
      deps.laneArming !== undefined &&
      resolveNamedModelServability(requestedModel, deps.laneArming) === false
    ) {
      return noStoreJsonResponse(
        { error: 'model_unavailable', model: requestedModel },
        { status: 400 },
      )
    }

    // PREMIUM-MODEL OWNER-GRANT GATE (free-tier enablement §2). Premium models
    // require the account's resolved OWNER identity to be allowlisted. Checked
    // here, after auth and before any balance/dispatch work, so a non-allowlisted
    // premium request is denied with an actionable message and never reaches a
    // provider. Open (no-op) when unwired; non-premium models always pass.
    if (deps.checkPremiumAccess !== undefined) {
      const premium = yield* Effect.promise(() =>
        deps.checkPremiumAccess!(session.accountRef, requestedModel),
      )
      if (!premium.allowed) {
        return noStoreJsonResponse(
          {
            error: 'premium_model_not_allowed',
            message: premium.message,
            model: requestedModel,
            reason: premium.reasonRef,
          },
          { status: 403 },
        )
      }
    }

    // BALANCE GATE (read-only). #5477 owns the real per-model decrement.
    const minimum = deps.minimumAvailableMsat ?? 1
    const availableMsat = yield* Effect.promise(() =>
      deps.readAvailableMsat(session.accountRef),
    )
    if (availableMsat < minimum) {
      // FREE-ALLOWANCE BYPASS (EPIC #5474 §1). A zero/insufficient-balance
      // account is NOT rejected when the request is free-eligible AND the
      // resolving owner still has remaining free allowance — that request would
      // be eaten by `withFreeAllowance` after dispatch, so 402'ing it here would
      // make the free tier untestable/unreachable without a funded balance. The
      // pre-flight is read-only and conservative: non-free models, exhausted
      // pools, and resolution errors all fall through to the normal 402.
      const freeAllowed =
        deps.checkFreeAllowance === undefined
          ? false
          : (yield* Effect.promise(() =>
              deps.checkFreeAllowance!(session.accountRef, requestedModel),
            )).eligible
      if (!freeAllowed) {
        return noStoreJsonResponse(
          {
            error: 'insufficient_credits',
            availableMsat,
            requiredMsat: minimum,
          },
          { status: 402 },
        )
      }
    }

    // SPEND-CAP GATE (#5486). DISTINCT from the balance gate above: an account
    // can be flush with credits yet still be capped at a configurable per-window
    // spend ceiling so a compromised key cannot drain the whole balance. Open
    // (no-op) when unwired or when no cap is configured for the account.
    if (deps.checkSpendCap !== undefined) {
      const spendCap = yield* Effect.promise(() =>
        deps.checkSpendCap!(session.accountRef),
      )
      if (!spendCap.allowed) {
        return noStoreJsonResponse(
          {
            error: 'spend_cap_exceeded',
            capMsat: spendCap.capMsat,
            remainingMsat: spendCap.remainingMsat,
            spentMsatInWindow: spendCap.spentMsatInWindow,
            windowSeconds: spendCap.windowSeconds,
          },
          { status: spendCap.statusCode },
        )
      }
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
    const plannedIds = planFor(requestedModel)
    // model_unavailable when no lane is configured OR none of the planned lanes
    // is actually registered (e.g. an absent partner secret leaves the plan but
    // no resolvable adapter).
    const hasViableLane = plannedIds.some(
      id => deps.registry.resolve(id) !== undefined,
    )
    if (!hasViableLane) {
      return noStoreJsonResponse(
        { error: 'model_unavailable', model: requestedModel },
        { status: 400 },
      )
    }

    const inferenceRequest = toInferenceRequest(body, rawBody, requestedModel)
    const meteringHook = deps.meteringHook ?? stubMeteringHook
    const resolveFundingKind =
      deps.resolveFundingKind ?? defaultCardFundingResolver
    const nowEpochSeconds = deps.nowEpochSeconds ?? currentEpochSeconds
    const newId = deps.newId ?? defaultId
    const created = nowEpochSeconds()
    const responseId = newId()
    // Funding kind (card | bitcoin) for the metering charge. Resolved once per
    // request so the Bitcoin discount in `priceRequest` applies; defaults card.
    const fundingKind = yield* Effect.promise(() =>
      resolveFundingKind(session.accountRef),
    )

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
      // TRUE PASS-THROUGH STREAM (the khala-code 524 fix). When the served
      // adapter exposes `streamSse`, pump the upstream SSE to the client
      // frame-by-frame so every chunk resets the Cloudflare edge idle-timer and
      // a multi-minute generation never 524s (the old buffered array path read
      // the WHOLE upstream completion before emitting a single byte — exactly
      // what tripped the edge ~100s timeout on a long generation). Connect-time
      // dispatch still overflows across the lane plan on a retryable failure;
      // once bytes are flowing there is no overflow (the client already has
      // partial output). Metering settles receipt-first from the terminal usage
      // frame after the upstream stream closes. Adapters without `streamSse`
      // (stub/echo, simple test adapters) fall through to the buffered path.
      const sseDispatch = yield* dispatchWithOverflow(
        inferenceRequest,
        (adapter, request) => {
          if (adapter.streamSse === undefined) {
            // Signal "this lane cannot stream incrementally" as a NON-retryable
            // typed failure so the overflow loop surfaces it without retrying;
            // the route catches it and falls back to the buffered path.
            return Effect.fail(
              new InferenceAdapterError({
                adapterId: adapter.id,
                kind: 'stream_not_supported',
                reason: 'adapter does not support incremental streaming',
                retryable: false,
              }),
            )
          }
          return adapter
            .streamSse(request)
            .pipe(Effect.map(source => ({ adapterId: adapter.id, source })))
        },
        dispatchDeps,
      ).pipe(
        Effect.map(served => ({ ok: true as const, served })),
        Effect.catch(error =>
          Effect.succeed({
            kind: error.kind,
            ok: false as const,
            reason: error.reason,
          }),
        ),
      )

      if (sseDispatch.ok) {
        const { adapterId, source } = sseDispatch.served
        const responseStream = makePassThroughResponseStream({
          accountRef: session.accountRef,
          adapterId,
          created,
          fundingKind,
          meteringHook,
          requestedModel,
          responseId,
          source,
        })
        return new Response(responseStream, {
          headers: {
            'cache-control': 'no-store',
            'content-type': 'text/event-stream; charset=utf-8',
          },
          status: 200,
        })
      }

      // Only fall back to the buffered path when the served lane genuinely could
      // not stream incrementally. A real upstream/connect failure (provider
      // error) must surface as the 502 it is, not be retried via the buffered
      // path (which would double-dispatch and could 524 again).
      if (sseDispatch.kind !== 'stream_not_supported') {
        return noStoreJsonResponse(
          { error: 'provider_error', reason: sseDispatch.reason },
          { status: 502 },
        )
      }

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
      // Capture the metering outcome so the disclosure block (below) carries the
      // same receipt the non-streaming path attaches. `undefined` when no
      // terminal usage frame was served (no metering ran).
      let streamMetering: MeteringOutcome | undefined
      if (terminal?.usage !== undefined) {
        const streamServedModel = terminal.servedModel ?? requestedModel
        streamMetering = yield* meteringHook({
          accountRef: session.accountRef,
          adapterId: chunks.served.adapterId,
          fundingKind,
          requestId: responseId,
          requestedModel: requestedModel,
          servedModel: streamServedModel,
          streamed: true,
          usage: terminal.usage,
        })
      }

      // OPENAGENTS DISCLOSURE (M0 / #6008 follow-up). Streaming carries the SAME
      // non-breaking `openagents` block the non-streaming path emits, built by the
      // SAME `khalaReceiptForResult(...)` (non-Khala => undefined). Reconstruct an
      // `InferenceResult` from the served chunks — content concatenated, the
      // terminal finishReason/usage, and the terminal served model when the
      // adapter reports one — then attach the block to the FINAL
      // `chat.completion.chunk` frame only, before `data: [DONE]`.
      const streamResult: InferenceResult = {
        content: servedChunks.map(chunk => chunk.contentDelta).join(''),
        finishReason: terminal?.finishReason ?? 'stop',
        servedModel: terminal?.servedModel ?? requestedModel,
        usage: terminal?.usage ?? {
          completionTokens: 0,
          promptTokens: 0,
          totalTokens: 0,
        },
      }
      const streamOpenagents = khalaReceiptForResult({
        adapterId: chunks.served.adapterId,
        // `khalaReceiptForResult` requires a MeteringOutcome; when no terminal
        // usage frame was served (so no metering ran) fall back to the stub
        // outcome shape so a Khala stream without usage still discloses.
        metering: streamMetering ?? { metered: false, receiptRef: null },
        requestedModel,
        responseId,
        result: streamResult,
      })

      const lastIndex = servedChunks.length - 1
      const body_sse = servedChunks
        .map((chunk, index) =>
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
            model: requestedModel,
            object: 'chat.completion.chunk',
            // Disclosure rides on the terminal frame only (non-breaking field).
            ...(index === lastIndex && streamOpenagents !== undefined
              ? { openagents: streamOpenagents }
              : {}),
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

    const metering = yield* meteringHook({
      accountRef: session.accountRef,
      adapterId: result.served.adapterId,
      fundingKind,
      requestId: responseId,
      requestedModel: requestedModel,
      servedModel: result.served.value.servedModel,
      streamed: false,
      usage: result.served.value.usage,
    })

    // ACCEPTANCE-DISPATCH (EPIC #6017): when khala-code produced a runnable artifact,
    // enqueue an out-of-Worker verification job (flagged; inert by default). The
    // node-side runner executes the suite and the verdict callback backfills the
    // receipt from `unverified` -> `test_passed`/`failed`. Fire-and-forget: never
    // blocks or fails the already-priced completion.
    yield* maybeEnqueueAcceptanceJob({
      adapterId: result.served.adapterId,
      dispatch: deps.acceptanceDispatch,
      meteringReceiptRef: metering.receiptRef,
      requestMessages: inferenceRequest.messages,
      requestedModel,
      responseId,
      result: result.served.value,
    })

    return noStoreJsonResponse(
      openAiResponse({
        created,
        id: responseId,
        model: requestedModel,
        // Khala requests carry the disclosure block (which concrete model/worker
        // served this one endpoint). `khala-code` additionally runs the
        // deterministic verifier and attaches the test verdict; non-Khala
        // responses are byte-identical to before. Streaming carries the same
        // disclosure in a later slice.
        openagents: khalaReceiptForResult({
          adapterId: result.served.adapterId,
          metering,
          requestedModel,
          responseId,
          result: result.served.value,
        }),
        result: result.served.value,
      }),
    )
  })
