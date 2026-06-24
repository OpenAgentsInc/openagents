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
import {
  compactRandomId,
  currentEpochMillis,
  currentEpochSeconds,
} from '../runtime-primitives'
// PURE dispatch producer only — enqueues an out-of-Worker verification job; never
// imports the headless runner (playwright) into the Worker bundle (EPIC #6017).
import {
  type AcceptanceJobQueue,
  enqueueAcceptanceJob,
} from './acceptance-dispatch'
// PURE verdict type only — the route NEVER imports the headless runner (playwright)
// into the Worker bundle. The runner executes out of the Worker; its verdict shape is
// the only thing the route consumes (EPIC #6017).
import { type AcceptanceVerdict } from './acceptance-runner/verdict'
import {
  acceptanceContractGuidanceForRequest,
  intentToAcceptanceSpec,
} from './acceptance-spec'
import {
  type AutopilotConciergeRequestConfig,
  buildAutopilotConciergeSystemPrompt,
  isAutopilotConciergeModel,
  resolveAutopilotConciergeConfig,
} from './autopilot-concierge-model'
import {
  type AutopilotConciergeOutputSpec,
  extractConciergeOutputSpec,
} from './autopilot-concierge-output-spec'
import {
  type ConciergeToolDeclaration,
  autopilotConciergeToolDeclarations,
} from './autopilot-concierge-tools'
import {
  type CachePinPolicy,
  type CacheWarmthOracle,
  type LaneHealthOracle,
  decideCacheAwareRouting,
} from './cache-aware-routing'
import {
  type DurableInferenceStreamStore,
  type StreamStore,
  durableInferenceReadUrl,
  teeUpstreamToDurable,
} from './durable-inference-proxy'
import {
  type DurableStreamNamespace,
  teeUpstreamToDurableDO,
} from './durable-inference-do-transport'
import {
  type FairShareDecision,
  type SpendCapDecision,
} from './inference-abuse-controls'
import { type PremiumAccessDecision } from './inference-premium-allowlist'
import {
  KHALA_CODE_VERIFIER_WORKER_ID,
  type KhalaCodeVerificationVerdict,
  prescreenKhalaCodeArtifact,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'
import {
  type ComponentChannelOutput,
  type ComponentRepairReask,
  KHALA_COMPONENT_CATALOG_PROMPT,
  type KhalaComponentFrame,
  runComponentChannel,
  serializeComponentFrame,
} from './khala-component-channel'
import {
  KHALA_IDENTITY_REINFORCEMENT_PROMPT,
  KHALA_IDENTITY_SYSTEM_PROMPT,
  guardKhalaCompletion,
  verifyKhalaSignatures,
} from './khala-identity'
import {
  type KhalaExecutedVerdict,
  type KhalaRequestClass,
  type KhalaSettlementState,
  type KhalaTelemetryBlock,
  type KhalaVerificationClass,
  NOT_MEASURED,
  buildKhalaTelemetryBlock,
} from './khala-telemetry'
import {
  type MeteringHook,
  type MeteringOutcome,
  stubMeteringHook,
} from './metering-hook'
import {
  type DispatchDeps,
  type DispatchRouteMetadata,
  classifyModel,
  dispatchWithOverflow,
  dispatchWithOverflowWithMetadata,
} from './model-router'
import {
  type SupplyLaneArming,
  resolveNamedModelServability,
} from './model-serving-policy'
import {
  type FundingKind,
  KHALA_CODE_MODEL_ID,
  isKhalaModel,
  lookupModel,
  type SupplyLane,
} from './pricing'
import {
  type StableBlockKind,
  type TaggedPromptMessage,
  assembleStablePromptLayout,
  deriveCacheAffinityKey,
  deriveSessionAffinityValue,
  hashCacheAffinityKey,
  sessionAffinityParams,
} from './prompt-prefix-cache'
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

// Parse the INFERENCE_DURABLE_STREAM_ENABLED flag (durable-stream Rank-1, #6058).
// Default OFF: the streaming pass-through degrades to today's behaviour (no
// persistence/resume) unless this is an explicit truthy token AND a durable
// store factory is wired. Fail-safe + inert by default.
export const isInferenceDurableStreamEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// COMPONENT-CHANNEL CONFIG (issue #6127). `enabled` is the gateway-level flag
// (default off). When on, the channel is STILL only activated per-request via an
// explicit opt-in for a Khala model. `repairReask` (optional) wires the ONE
// bounded repair turn over a single non-streaming Khala call; absent => an
// invalid card is dropped without a repair attempt (still never shipped).
export type ComponentChannelConfig = Readonly<{
  enabled: boolean
  repairReask?: ComponentRepairReask | undefined
}>

// Resolve whether the typed component channel is active FOR THIS REQUEST. Three
// AND-gated conditions, all required, so the default `/v1` shape never changes:
//   1. the gateway-level flag is on (`config.enabled`)
//   2. the request explicitly opts in (header `x-oa-component-channel: on`, or a
//      truthy `oa_component_channel` body field) — a deterministic parse of an
//      explicit caller-supplied switch, never an intent inference
//   3. the model is a Khala model (the channel is a Khala capability)
export const resolveComponentChannelActive = (
  input: Readonly<{
    config: ComponentChannelConfig | undefined
    request: Request
    rawBody: Record<string, unknown>
    requestedModel: string
    autopilotConcierge?: boolean | undefined
  }>,
): boolean => {
  if (input.config?.enabled !== true) {
    return false
  }
  if (!isKhalaModel(input.requestedModel)) {
    return false
  }
  if (input.autopilotConcierge === true) {
    return true
  }
  const header = input.request.headers
    .get('x-oa-component-channel')
    ?.trim()
    .toLowerCase()
  const headerOptIn =
    header !== undefined && ['1', 'true', 'yes', 'on'].includes(header)
  const bodyField = input.rawBody['oa_component_channel']
  const bodyOptIn =
    bodyField === true ||
    (typeof bodyField === 'string' &&
      ['1', 'true', 'yes', 'on'].includes(bodyField.trim().toLowerCase()))
  return headerOptIn || bodyOptIn
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
  // DURABLE-STREAM RANK-1 SEAM (#6058, EPIC #6056). When `durableStreamEnabled`
  // is true AND `durableStream` resolves a per-request `StreamStore`, a streaming
  // completion is teed into a durable offset log keyed by the response id, so a
  // client disconnect mid-generation can be resumed by offset (the durable read
  // route). DEFAULT OFF + UNWIRED: with the flag off or no store factory, the
  // streaming path is byte-for-byte today's pass-through (no persistence/resume).
  // Metering still settles EXACTLY ONCE on the real upstream EOF and NEVER on a
  // resume/replay read (the resume route has no metering hook).
  durableStreamEnabled?: boolean | undefined
  durableStream?: DurableInferenceStreamStore | undefined
  // PRODUCTION DURABLE SUBSTRATE (#6058). When present AND `durableStreamEnabled`
  // is true, the streaming completion is teed into the per-request Durable Object
  // (`DurableInferenceStreamObject`, keyed `getByName(responseId)`) over the
  // `/v1/stream/{id}` HTTP contract — the DO is the single authoritative durable
  // log a LATER GET resume reads. This is the live wiring; `durableStream` (the
  // synchronous `StreamStore` factory) is the in-memory test/contract substrate.
  // When both are present the DO namespace wins. Absent (e.g. the binding is
  // unbound on an env) => fail-safe non-durable pass-through.
  durableStreamNamespace?: DurableStreamNamespace | undefined
  // TYPED COMPONENT CHANNEL SEAM (EPIC #6123, issue #6127). The ADDITIVE,
  // OPT-IN `oa.component` SSE channel: a Khala turn may stream prose PLUS one or
  // more validated, versioned `oa.component` cards from the CLOSED v1 catalog
  // (khala-component-channel.ts). DEFAULT OFF: with `componentChannel` absent or
  // `enabled: false`, `/v1/chat/completions` is byte-for-byte today's text-only
  // stream — standard OpenAI clients are unaffected. When `enabled` is true the
  // channel is STILL only activated for a request that explicitly opts in (the
  // `x-oa-component-channel: on` header or an `oa_component_channel: true` body
  // field) AND targets a Khala model, so the default response shape never
  // changes. On activation, the gateway:
  //   - injects the closed-catalog system prompt (stable prefix),
  //   - assembles the completion, splits prose vs fenced `oa-component` blocks,
  //   - validates each card's props against the closed catalog with Effect
  //     Schema (ONE bounded repair turn, then drop — never ship malformed),
  //   - honors the SAME provider-identity redaction backstop as the prose path,
  //   - re-emits prose as `{content}` deltas and each card as one atomic
  //     `event: oa.component` frame.
  componentChannel?: ComponentChannelConfig | undefined
  // CACHE-AWARE ROUTING SEAM (book P0-2 deliverable 6 / #6084). When wired, a
  // same-session/codebase/account follow-up is routed to the cache-WARM lane
  // first (the lane that previously served this affinity hash), subject to the
  // lane still being viable + healthy + privacy/region-allowed. All three are
  // INJECTED capabilities so routing stays pure + typed (no ad-hoc string
  // matching). DEFAULT UNWIRED: with `cacheWarmthOracle` absent the lane plan is
  // unchanged (cheapest-viable order), so existing behavior is unaffected.
  //   - `cacheWarmthOracle` : affinity HASH → the lane that last served it.
  //   - `laneHealthOracle`  : lane → health posture (warm hint ignored if sick).
  //   - `cachePinPolicy`    : lane → may this account pin here (privacy/region).
  cacheWarmthOracle?: CacheWarmthOracle | undefined
  laneHealthOracle?: LaneHealthOracle | undefined
  cachePinPolicy?: CachePinPolicy | undefined
  // Deterministic clock (epoch ms) for the durable log's TTL/offset bookkeeping.
  // Defaults to a fixed value in tests; the Worker threads `currentEpochMillis`.
  nowEpochMillis?: (() => number) | undefined
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

// GATEWAY-SIDE STABLE PROMPT LAYOUT (book P0-2 / #6084 deliverable 1+2). Build the
// outgoing messages as TAGGED blocks so the prefix-cache assembler can order them
// STABLE content first (acceptance contract → identity → tool schemas → stable
// policy) and NOVEL/volatile content last — the book's rule "novel tokens as late
// as possible" so the long shared prefix stays cacheable. Each gateway-injected
// block is tagged with its `StableBlockKind`, so ordering is deterministic and
// structural (classification of OUR OWN injected blocks, not a keyword match on
// user intent — honors the workspace semantic-routing rule).
//
// Identity injection (STEP 1, the PRIMARY identity mechanism): for every
// `openagents/khala-*` request the strong Khala identity system message is part of
// the STABLE prefix so the identity rule binds to every Khala consumer and the
// post-completion signature guard remains the backstop on top.
//
// Acceptance-contract injection (EPIC #6017): for a khala-code coding request whose
// intent maps to an executable acceptance lane, the contract guidance (the runner's
// `window` hooks) is a STABLE prefix block too. Both are additive; the assembler
// orders them canonically (acceptance contract leads, then identity) regardless of
// append order.
const buildTaggedKhalaMessages = (
  clientMessages: ReadonlyArray<InferenceMessage>,
  requestedModel: string,
  autopilotConcierge: AutopilotConciergeRequestConfig | undefined,
  // COMPONENT-CHANNEL (issue #6127): inject the closed-catalog system prompt as a
  // STABLE prefix block ONLY when the channel is active for this request. Absent /
  // false => no injection, prompt byte-identical to before (additive + opt-in).
  componentChannelActive?: boolean,
): ReadonlyArray<TaggedPromptMessage> => {
  const tagged: Array<TaggedPromptMessage> = []

  // Acceptance-contract stable block (khala-code coding lane only).
  if (requestedModel === KHALA_CODE_MODEL_ID) {
    const guidance = acceptanceContractGuidanceForRequest({
      messages: clientMessages.map(message => ({
        content: message.content,
        role: message.role,
      })),
      model: requestedModel,
    })
    if (guidance !== undefined) {
      tagged.push({
        message: { content: guidance, role: 'system' },
        stableKind: 'acceptanceContract' satisfies StableBlockKind,
      })
    }
  }

  // Identity stable block (every khala-* model).
  if (isKhalaModel(requestedModel)) {
    tagged.push({
      message: { content: KHALA_IDENTITY_SYSTEM_PROMPT, role: 'system' },
      stableKind: 'identity' satisfies StableBlockKind,
    })
  }

  // Autopilot Concierge product prompt (#6148): server-owned model config,
  // vertical enum, component catalog metadata, and Output Spec instructions. It
  // is injected by the gateway for the virtual model only; callers cannot supply
  // arbitrary vertical/system-prompt text.
  if (autopilotConcierge !== undefined) {
    tagged.push({
      message: {
        content: buildAutopilotConciergeSystemPrompt(autopilotConcierge),
        role: 'system',
      },
      stableKind: 'otherSystem' satisfies StableBlockKind,
    })
  }

  // Component-catalog stable block (issue #6127): only when the typed component
  // channel is active for this Khala request. The catalog prompt lists the closed
  // v1 component set + prop shapes so the model surfaces a card via the fenced
  // `oa-component` mechanism that works across all Khala backends.
  if (componentChannelActive === true && isKhalaModel(requestedModel)) {
    tagged.push({
      message: { content: KHALA_COMPONENT_CATALOG_PROMPT, role: 'system' },
      stableKind: 'identity' satisfies StableBlockKind,
    })
  }

  // Client messages: their own `system` messages are stable policy/steer (the
  // assembler classifies role==='system' as `otherSystem`, ordered after the
  // known stable blocks); everything else is volatile/novel (ordered last).
  for (const message of clientMessages) {
    tagged.push({ message })
  }
  return tagged
}

const toInferenceRequest = (
  body: typeof ChatCompletionsRequestBody.Type,
  raw: Record<string, unknown>,
  requestedModel: string,
  // The session-affinity passthrough params derived from the request's
  // cache-affinity key (book P0-2 deliverable 4). MERGED into passthroughParams
  // (overriding any stray client copy) so the adapters pin the session to one
  // cache-warm replica. Empty for non-Khala / no-affinity requests.
  affinityParams: Readonly<Record<string, string>>,
  // COMPONENT-CHANNEL (issue #6127): when active, the closed-catalog system prompt
  // is injected as a stable prefix block. Default false => no injection.
  componentChannelActive?: boolean,
  autopilotConcierge?: AutopilotConciergeRequestConfig | undefined,
): InferenceRequest => {
  const clientMessages: ReadonlyArray<InferenceMessage> = body.messages.map(
    message => ({ content: message.content, role: message.role }),
  )
  // For a non-Khala model, leave the messages exactly as the client sent them
  // (no gateway injection, no reordering) — byte-identical to prior behavior.
  // For a Khala model, assemble the cache-optimal stable layout.
  const messages = isKhalaModel(requestedModel)
    ? assembleStablePromptLayout(
        buildTaggedKhalaMessages(
          clientMessages,
          requestedModel,
          autopilotConcierge,
          componentChannelActive,
        ),
      ).messages
    : clientMessages
  const { messages: _messages, model: _model, stream: _stream, ...rest } = raw
  return {
    messages,
    model: requestedModel,
    // Gateway-derived session affinity wins over any stray client copy.
    passthroughParams: { ...rest, ...affinityParams },
    stream: body.stream === true,
  }
}

// Resolve the cache-affinity dimensions for a request from the authenticated
// account plus client-supplied session / codebase hints (book P0-2 deliverable
// 3). The session/codebase hints are read from the OpenAI-style `user` field and
// a non-standard `codebase` hint in the raw body, plus the `x-session-affinity` /
// `x-codebase` request headers — all OPTIONAL, all bounded fields (deterministic
// parse of explicit caller-supplied identifiers, never an intent parse). Only
// `account` is required; the others sharpen affinity when present.
const resolveCacheAffinity = (
  accountRef: string,
  raw: Record<string, unknown>,
  request: Request,
): Readonly<{
  rawKey: string
  hash: string
  params: Readonly<Record<string, string>>
}> => {
  const stringField = (value: unknown): string | undefined =>
    typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined
  const header = (name: string): string | undefined => {
    const value = request.headers.get(name)
    return value !== null && value.trim() !== '' ? value.trim() : undefined
  }
  const session =
    stringField(raw['user']) ?? header('x-session-affinity') ?? undefined
  const codebase =
    stringField(raw['codebase']) ?? header('x-codebase') ?? undefined
  const rawKey = deriveCacheAffinityKey({
    account: accountRef,
    ...(session === undefined ? {} : { session }),
    ...(codebase === undefined ? {} : { codebase }),
  })
  return {
    hash: hashCacheAffinityKey(rawKey),
    params: sessionAffinityParams(deriveSessionAffinityValue(rawKey)),
    rawKey,
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
  // Concrete priced supply lane, distinct from the legacy `lane` model class
  // (`open` / `gemini` / `claude`). Additive and public-safe; lets Khala expose
  // day-zero Hydralisk routing without changing older receipt readers.
  supply_lane?: SupplyLane | undefined
  routing?:
    | Readonly<{
        provider_health_score: number | typeof NOT_MEASURED
        region: string | typeof NOT_MEASURED
        fallback_reason: string | null
      }>
    | undefined
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
  // KHALA REQUEST-TELEMETRY SCORECARD (book P0-1 / Open Q #1-2). The SMALL,
  // immediate lifecycle summary — request class, tokens, TTFT, total wall-clock,
  // verification class + executed verdict + scalar reward, and a `detailRef`
  // pointer to the full dereferenceable record. Non-breaking additive field;
  // every numeric is a real measurement or the honest `not_measured` sentinel.
  // The full P0-1 record (time split, queue/batch wait, region, cache-affinity
  // hash, fallback reason, cost basis / margin bucket / settlement state /
  // blockers) is the depth behind the receipt detail, off this hot path.
  telemetry?: KhalaTelemetryBlock | undefined
  // Bitcoin/Spark settlement on a VERIFIED accepted outcome (#6011, EPIC #6017).
  // `settled` is the honest default `false` on the hot path: settlement fires
  // ASYNC after an out-of-Worker headless acceptance run verifies the outcome
  // (the verdict-callback path settles the worker + validator and flips this to
  // `true`), so a fresh completion is `verified:false`/`settled:false` until the
  // runner verifies it. The field exists so the receipt shape is stable and a
  // settled accepted outcome surfaces `settled:true` + the settlement receipt
  // refs alongside `verified`/`scalar_reward`.
  settled?: boolean | undefined
  settlement_receipts?: ReadonlyArray<string> | undefined
  // AUTOPILOT CONCIERGE STRUCTURED ARTIFACTS (#6148). Present ONLY for the
  // `openagents/autopilot-concierge` virtual model.
  //   - `output_spec` is the 10-section intake Output Spec reliably extracted
  //     from the completion (the fenced `oa-output-spec` JSON block, with a
  //     markdown-section fallback), so a programmatic consumer reads the
  //     accumulated intake state as a STRUCTURED field rather than parsing prose.
  //     Absent when the turn surfaced no parseable spec content.
  //   - `tools` is the bounded, server-declared Concierge tool set with each
  //     tool's review/effect posture and an honest `declared_not_executed`
  //     status (live execution is a deferred seam — see autopilot-concierge-tools.ts).
  output_spec?: AutopilotConciergeOutputSpec | undefined
  tools?: ReadonlyArray<ConciergeToolDeclaration> | undefined
}>

const publicInferenceReceiptUrl = (receiptRef: string): string =>
  `/api/public/inference/receipts/${encodeURIComponent(receiptRef)}`

// What the gateway can MEASURE about this request's lifecycle on the hot path
// today (book P0-1). All optional: an unmeasured value becomes the honest
// `not_measured` sentinel in the telemetry builder — never a fabricated number.
type KhalaTelemetryTiming = Readonly<{
  // True for the streaming path. Determines the request class (interactive_stream
  // vs async_job/batch later) and whether TTFT/ITL are measurable at all.
  streamed: boolean
  // Total wall-clock from request accept to completion (ms), gateway-edge
  // measured. Undefined on a path that did not capture it => sentinel.
  totalWallClockMs?: number | undefined
  // Time to first token (ms), measurable on the streaming path only.
  ttftMs?: number | undefined
  // Generation wall-clock (first byte -> last byte, ms) used to DERIVE perceived
  // TPS + mean inter-token latency. Streaming only; undefined => those derived
  // metrics are the sentinel (we never guess them from total wall-clock).
  generationWallClockMs?: number | undefined
}>

// Map a khala-code verifier verdict's CLASS onto the telemetry verification
// class vocabulary (khala.md §6). Reuses the existing values; never invents one.
const telemetryVerificationClass = (
  verification: KhalaCodeVerificationVerdict['verification'],
): KhalaVerificationClass => verification

// Map executed-ness + class onto the executed verdict. `not_executed` is the
// honest default whenever the headless run did not produce a verdict.
const telemetryExecutedVerdict = (
  verdict: KhalaCodeVerificationVerdict,
): KhalaExecutedVerdict => {
  if (!verdict.executed) {
    return 'not_executed'
  }
  return verdict.verification === 'test_passed' ? 'passed' : 'failed'
}

// Derive the request class from the measured shape. Streaming => interactive
// stream; otherwise an async/synchronous job. (batch/verifier_run lanes set
// their own class once those request paths land; this is the chat path.)
const telemetryRequestClass = (streamed: boolean): KhalaRequestClass =>
  streamed ? 'interactive_stream' : 'async_job'

const khalaReceiptForResult = (
  input: Readonly<{
    adapterId: string
    metering: MeteringOutcome
    requestedModel: string
    responseId: string
    result: InferenceResult
    routeMetadata?: DispatchRouteMetadata | undefined
    // The executed-acceptance verdict from the out-of-Worker headless runner, when an
    // execution actually ran (EPIC #6017). PRESENT => `verified`/`scalarReward` derive
    // from EXECUTION. ABSENT (the hot Worker path today, which cannot launch a browser)
    // => HONEST DOWNGRADE to `unverified`. Full prod wiring (async sandbox dispatch of
    // the runner + receipt backfill) threads a real verdict in here.
    acceptance?: AcceptanceVerdict | undefined
    // What the gateway measured about this request's lifecycle (book P0-1). When
    // absent, every telemetry numeric is the honest `not_measured` sentinel.
    timing?: KhalaTelemetryTiming | undefined
    // The RAW cache-affinity key for this request (book P0-2 deliverable 3:
    // account/session/codebase). It is hashed into `cacheAffinityKeyHash` by the
    // telemetry builder and NEVER stored/exposed raw. Undefined → no affinity key
    // applied → the hash is null and a blocker records why.
    cacheAffinityKeyRaw?: string | undefined
  }>,
): OpenAgentsReceipt | undefined => {
  if (!isKhalaModel(input.requestedModel)) {
    return undefined
  }

  const supplyLane = lookupModel(input.requestedModel)?.lane
  const base = {
    lane: classifyModel(input.requestedModel),
    requested_model: input.requestedModel,
    served_model: input.result.servedModel,
    ...(supplyLane === undefined ? {} : { supply_lane: supplyLane }),
    worker: input.adapterId,
  } satisfies Omit<OpenAgentsReceipt, 'verification'>
  const routing = {
    fallback_reason: input.routeMetadata?.fallbackReason ?? null,
    provider_health_score:
      input.routeMetadata?.providerHealthScore ?? NOT_MEASURED,
    region: input.routeMetadata?.region ?? NOT_MEASURED,
  } satisfies NonNullable<OpenAgentsReceipt['routing']>

  // Shared telemetry inputs measurable for EVERY Khala request (tokens from the
  // provider usage, latency from the gateway edge). The cache-affinity key (book
  // P0-2 deliverable 3) is now wired: when present it is hashed into the receipt
  // and the cached-input dimension is populated from provider usage; when absent
  // (no session/codebase context) the hash is null and a blocker records why.
  const streamed = input.timing?.streamed ?? false
  const settlementBlockers =
    input.metering.receiptRef === null ? ['cost_not_measured'] : []
  const cacheAffinityKeyRaw =
    input.cacheAffinityKeyRaw !== undefined &&
    input.cacheAffinityKeyRaw.trim() !== ''
      ? input.cacheAffinityKeyRaw
      : undefined
  const cacheBlockers =
    cacheAffinityKeyRaw === undefined ? ['cache_affinity_key_not_resolved'] : []

  if (input.requestedModel !== KHALA_CODE_MODEL_ID) {
    // Non-coding Khala lane: no verifier pass (verification class `none`). Still
    // carries the full lifecycle telemetry summary (tokens + latency + request
    // class) so EVERY Khala response is measurable, not just khala-code.
    const settlementState: KhalaSettlementState = 'not_applicable'
    const telemetry = buildKhalaTelemetryBlock(
      {
        completionTokens: input.result.usage.completionTokens,
        costBasisMsat: undefined,
        executedVerdict: 'not_executed',
        priceMsat: undefined,
        promptTokens: input.result.usage.promptTokens,
        provider: input.adapterId,
        ...(input.routeMetadata?.providerHealthScore === undefined
          ? {}
          : { providerHealthScore: input.routeMetadata.providerHealthScore }),
        requestClass: telemetryRequestClass(streamed),
        requestId: input.responseId,
        requestedModel: input.requestedModel,
        route: classifyModel(input.requestedModel),
        servedModel: input.result.servedModel,
        ...(input.routeMetadata?.region === undefined
          ? {}
          : { region: input.routeMetadata.region }),
        fallbackReason: input.routeMetadata?.fallbackReason ?? null,
        ...(cacheAffinityKeyRaw === undefined ? {} : { cacheAffinityKeyRaw }),
        ...(input.result.usage.cachedPromptTokens === undefined
          ? {}
          : { cachedInputTokens: input.result.usage.cachedPromptTokens }),
        ...(input.timing?.generationWallClockMs === undefined
          ? {}
          : { generationWallClockMs: input.timing.generationWallClockMs }),
        ...(input.timing?.totalWallClockMs === undefined
          ? {}
          : { totalWallClockMs: input.timing.totalWallClockMs }),
        ...(input.timing?.ttftMs === undefined
          ? {}
          : { ttftMs: input.timing.ttftMs }),
        scalarReward: undefined,
        settlementState,
        totalTokens: input.result.usage.totalTokens,
        verificationClass: 'none',
        blockerRefs: [...settlementBlockers, ...cacheBlockers],
      },
      input.metering.receiptRef === null
        ? null
        : publicInferenceReceiptUrl(input.metering.receiptRef),
    )
    // AUTOPILOT CONCIERGE artifacts (#6148): surface the structured Output Spec
    // (reliably extracted from the completion) and the declared bounded tool set
    // on the disclosure block, so a programmatic `/api/v1/chat/completions`
    // consumer reads them as structured fields. Only for the concierge virtual
    // model; every other Khala model is byte-identical to before.
    const conciergeOutputSpec = isAutopilotConciergeModel(input.requestedModel)
      ? extractConciergeOutputSpec(input.result.content)
      : undefined
    return {
      ...base,
      routing,
      telemetry,
      verification: 'none' as const,
      ...(isAutopilotConciergeModel(input.requestedModel)
        ? { tools: autopilotConciergeToolDeclarations() }
        : {}),
      ...(conciergeOutputSpec === undefined
        ? {}
        : { output_spec: conciergeOutputSpec }),
    }
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

  const verifierBlockers = verdict.executed ? [] : ['verifier_not_executed']
  const telemetry = buildKhalaTelemetryBlock(
    {
      completionTokens: input.result.usage.completionTokens,
      costBasisMsat: undefined,
      executedVerdict: telemetryExecutedVerdict(verdict),
      priceMsat: undefined,
      promptTokens: input.result.usage.promptTokens,
      provider: input.adapterId,
      ...(input.routeMetadata?.providerHealthScore === undefined
        ? {}
        : { providerHealthScore: input.routeMetadata.providerHealthScore }),
      requestClass: telemetryRequestClass(streamed),
      requestId: input.responseId,
      requestedModel: input.requestedModel,
      route: 'coding',
      servedModel: input.result.servedModel,
      ...(input.routeMetadata?.region === undefined
        ? {}
        : { region: input.routeMetadata.region }),
      fallbackReason: input.routeMetadata?.fallbackReason ?? null,
      scalarReward: verdict.scalarReward,
      settlementState: verdict.verified ? 'pending' : 'not_applicable',
      totalTokens: input.result.usage.totalTokens,
      verificationClass: telemetryVerificationClass(verdict.verification),
      verifierReceiptRef: verdict.receiptRef,
      ...(cacheAffinityKeyRaw === undefined ? {} : { cacheAffinityKeyRaw }),
      ...(input.result.usage.cachedPromptTokens === undefined
        ? {}
        : { cachedInputTokens: input.result.usage.cachedPromptTokens }),
      ...(input.timing?.generationWallClockMs === undefined
        ? {}
        : { generationWallClockMs: input.timing.generationWallClockMs }),
      ...(input.timing?.totalWallClockMs === undefined
        ? {}
        : { totalWallClockMs: input.timing.totalWallClockMs }),
      ...(input.timing?.ttftMs === undefined
        ? {}
        : { ttftMs: input.timing.ttftMs }),
      blockerRefs: [
        ...settlementBlockers,
        ...cacheBlockers,
        ...verifierBlockers,
      ],
    },
    input.metering.receiptRef === null
      ? null
      : publicInferenceReceiptUrl(input.metering.receiptRef),
  )

  return {
    ...base,
    executed: verdict.executed,
    receipt,
    ...(input.metering.receiptRef === null
      ? {}
      : { receipt_url: publicInferenceReceiptUrl(input.metering.receiptRef) }),
    reward_handoff: verdict.reward.handoffRef,
    route: 'coding',
    routing,
    rubric: {
      failed_checks: verdict.failedChecks,
      passed_checks: verdict.passedChecks,
      ref: verdict.rubricRef,
    },
    scalar_reward: verdict.scalarReward,
    // Honest hot-path default: settlement fires ASYNC after the headless acceptance
    // verdict callback verifies the outcome and pays the worker + validator (#6011).
    // A fresh completion is not yet settled; the public receipt read reflects the
    // settled state once the callback has run.
    settled: false,
    telemetry,
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
    ).pipe(
      Effect.map(ref => ref as string | undefined),
      Effect.orElseSucceed(() => undefined),
    )
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
// KHALA IDENTITY DEFENSE ON THE PASS-THROUGH PATH: because deltas are sent to
// the client AS THEY ARRIVE (the whole point of this path — to avoid the 524),
// the post-completion identity guard's redaction backstop cannot un-send an
// already-streamed token. The PRIMARY mechanism — the strong gateway-side
// `KHALA_IDENTITY_SYSTEM_PROMPT` injected for every `khala-*` request — is what
// protects this path: the model never volunteers its provenance in the first
// place. The buffered stream path (which materializes the whole completion
// before emitting) and the non-streaming path additionally run the guard
// backstop over the assembled content.
//
// This runs OUTSIDE the route's Effect: the metering hook is an Effect, so it is
// executed with `Effect.runPromise` in the flush step (the same hook used on the
// buffered/non-streaming paths). The metering hook never fails (it returns a
// typed outcome), so a metering error never breaks the already-delivered stream.
// Resolve the per-request durable store, swallowing a factory failure into
// `undefined` (fail-safe: a broken/absent durable substrate must NOT break the
// completion — it degrades to today's non-durable pass-through).
const resolveDurableStore = (
  factory: DurableInferenceStreamStore,
  requestId: string,
): StreamStore | undefined => {
  try {
    return factory(requestId)
  } catch {
    return undefined
  }
}

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
    routeMetadata?: DispatchRouteMetadata | undefined
    // DURABLE-PROXY SEAM (#6058). When present, every upstream frame is teed into
    // a per-request durable offset log (`@openagentsinc/durable-stream`) keyed by
    // `responseId` so a client disconnect mid-generation can be resumed by offset.
    // ABSENT => today's pure pass-through, byte-for-byte unchanged (fail-safe).
    durableStore?: StreamStore | undefined
    // PRODUCTION DURABLE SUBSTRATE (#6058). When present, every upstream frame is
    // teed into the per-request Durable Object (`getByName(responseId)`) over the
    // `/v1/stream/{id}` HTTP contract — the authoritative durable log a later GET
    // resume reads. Takes precedence over `durableStore` (the in-memory test
    // substrate) when both are present. Absent => non-durable pass-through.
    durableNamespace?: DurableStreamNamespace | undefined
    // Deterministic clock for the durable log (TTL/offset bookkeeping). Defaults
    // to a fixed epoch in tests; the route threads `currentEpochMillis`.
    durableNowMs?: number | undefined
    // TELEMETRY CLOCK (book P0-1). On the TRUE pass-through stream, TTFT and
    // generation wall-clock are GENUINELY measurable (first content delta and
    // EOF are observable here). `nowMs` is the wall-clock source (ms);
    // `requestStartMs` is the request-accept instant the route captured. Both
    // optional: absent => the telemetry numerics degrade to honest sentinels.
    nowMs?: (() => number) | undefined
    requestStartMs?: number | undefined
    // The RAW cache-affinity key (book P0-2 deliverable 3) so the terminal
    // disclosure records its public-safe HASH. Undefined → no affinity key.
    cacheAffinityKeyRaw?: string | undefined
  }>,
): ReadableStream<Uint8Array> => {
  const encoder = new TextEncoder()
  const telemetryNow = input.nowMs ?? currentEpochMillis
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

  // Settle metering (receipt-first) from the terminal usage frame and build the
  // terminal SSE frame carrying the `openagents` disclosure. THIS IS THE SINGLE
  // METERING-ONCE BOUNDARY: it runs only on the real upstream EOF (the producer
  // drain), never on a resume/replay read. `terminal.usage === undefined` (no
  // terminal usage frame served) means no settlement, exactly as the buffered
  // path behaves.
  const buildTerminalFrame = async (
    terminal: ReturnType<InferenceStreamSource['terminal']>,
    content: string,
    // Telemetry boundaries captured by the producer drain (book P0-1): the
    // first-token instant (for TTFT) and the EOF instant (for total + generation
    // wall-clock). `undefined` => that boundary was never observed => sentinel.
    timing?: Readonly<{ firstTokenMs?: number | undefined; eofMs: number }>,
  ): Promise<string> => {
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
      content,
      finishReason: terminal.finishReason ?? 'stop',
      servedModel,
      usage: terminal.usage ?? {
        completionTokens: 0,
        promptTokens: 0,
        totalTokens: 0,
      },
    }
    // Derive the measurable lifecycle timing for the TRUE streaming path. TTFT =
    // first-token − request-accept; total wall-clock = EOF − request-accept;
    // generation wall-clock = EOF − first-token (the decode window, used to derive
    // perceived TPS + ITL). Any boundary we did not observe stays undefined =>
    // honest sentinel downstream.
    const start = input.requestStartMs
    const streamTiming =
      timing === undefined
        ? { streamed: true as const }
        : {
            streamed: true as const,
            ...(start === undefined
              ? {}
              : { totalWallClockMs: Math.max(0, timing.eofMs - start) }),
            ...(start === undefined || timing.firstTokenMs === undefined
              ? {}
              : { ttftMs: Math.max(0, timing.firstTokenMs - start) }),
            ...(timing.firstTokenMs === undefined
              ? {}
              : {
                  generationWallClockMs: Math.max(
                    0,
                    timing.eofMs - timing.firstTokenMs,
                  ),
                }),
          }
    const openagents = khalaReceiptForResult({
      adapterId: input.adapterId,
      ...(input.cacheAffinityKeyRaw === undefined
        ? {}
        : { cacheAffinityKeyRaw: input.cacheAffinityKeyRaw }),
      metering: streamMetering ?? { metered: false, receiptRef: null },
      requestedModel: input.requestedModel,
      responseId: input.responseId,
      result: streamResult,
      ...(input.routeMetadata === undefined
        ? {}
        : { routeMetadata: input.routeMetadata }),
      timing: streamTiming,
    })
    return chunkFrame('', terminal.finishReason ?? 'stop', openagents)
  }

  return new ReadableStream<Uint8Array>({
    async start(controller) {
      // DURABLE PROXY PATH (#6058). Tee each upstream frame into the durable log
      // AND to the client; persist+close on EOF; settle metering EXACTLY ONCE in
      // the `onEof` callback above. The producer drain is the only consumer of
      // the live upstream, so a replay read can never re-bill.
      // Telemetry: the first observed content delta (TTFT boundary). Captured on
      // BOTH the durable and non-durable drains so streaming TTFT is real.
      let firstTokenMs: number | undefined
      // PRODUCTION DURABLE PATH (#6058). Tee each upstream frame into the
      // per-request Durable Object over the `/v1/stream/{id}` HTTP contract AND
      // to the client. The DO is the authoritative durable log a later GET
      // resume reads. Metering settles EXACTLY ONCE in the `onEof` callback (the
      // producer drain), never in the DO and never on replay. A DO-fetch fault
      // degrades the durable mirror but never breaks the live completion.
      if (input.durableNamespace !== undefined) {
        await teeUpstreamToDurableDO({
          emit: frame => {
            if (firstTokenMs === undefined) {
              firstTokenMs = telemetryNow()
            }
            controller.enqueue(encoder.encode(frame))
          },
          frameForDelta: delta => chunkFrame(delta, null),
          namespace: input.durableNamespace,
          onEof: (terminal, content) =>
            buildTerminalFrame(terminal, content, {
              eofMs: telemetryNow(),
              firstTokenMs,
            }),
          requestId: input.responseId,
          source: input.source,
        })
        controller.enqueue(encoder.encode('data: [DONE]\n\n'))
        controller.close()
        return
      }
      // IN-MEMORY DURABLE PATH (test/contract substrate). Same guarantees against
      // the synchronous `StreamStore` port the DO implements.
      if (input.durableStore !== undefined) {
        await teeUpstreamToDurable({
          emit: frame => {
            if (firstTokenMs === undefined) {
              // First emitted frame on the durable path marks first-token.
              firstTokenMs = telemetryNow()
            }
            controller.enqueue(encoder.encode(frame))
          },
          frameForDelta: delta => chunkFrame(delta, null),
          nowMs: input.durableNowMs ?? 0,
          onEof: (terminal, content) =>
            buildTerminalFrame(terminal, content, {
              eofMs: telemetryNow(),
              firstTokenMs,
            }),
          requestId: input.responseId,
          source: input.source,
          store: input.durableStore,
        })
        controller.enqueue(encoder.encode('data: [DONE]\n\n'))
        controller.close()
        return
      }

      // NON-DURABLE PASS-THROUGH (today's behaviour, unchanged).
      const contentParts: Array<string> = []
      try {
        // Pump every upstream content delta to the client immediately.
        for await (const event of input.source.frames) {
          if (event.contentDelta !== '') {
            if (firstTokenMs === undefined) {
              firstTokenMs = telemetryNow()
            }
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
      const terminalFrame = await buildTerminalFrame(
        input.source.terminal(),
        contentParts.join(''),
        { eofMs: telemetryNow(), firstTokenMs },
      )
      controller.enqueue(encoder.encode(terminalFrame))
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
    const autopilotConcierge = isAutopilotConciergeModel(requestedModel)
      ? resolveAutopilotConciergeConfig(rawBody)
      : undefined
    if (autopilotConcierge?.ok === false) {
      return noStoreJsonResponse(
        {
          allowed: autopilotConcierge.allowed,
          error: autopilotConcierge.error,
        },
        { status: 400 },
      )
    }

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
    const basePlannedIds = planFor(requestedModel)

    // CACHE-AFFINITY RESOLUTION (book P0-2 deliverables 3+4). Compose the
    // account/session/codebase key, its public-safe hash (for the receipt), and
    // the provider session-affinity passthrough params (for replica pinning).
    // Only Khala models get gateway-managed affinity; non-Khala requests carry no
    // affinity key (empty params) so their behavior is byte-identical to before.
    const affinity = isKhalaModel(requestedModel)
      ? resolveCacheAffinity(session.accountRef, rawBody, request)
      : { hash: null as string | null, params: {}, rawKey: undefined }

    // CACHE-AWARE ROUTING (book P0-2 deliverable 6). Reorder — never widen — the
    // viable lane plan so a same-session/codebase/account follow-up tries the
    // cache-WARM lane first, subject to the warm lane still being in the plan and
    // passing health + privacy/region gates. Pure reorder: the overflow tail is
    // preserved behind the warm lane. Inert when no oracle is wired (plan
    // unchanged) so existing behavior is unaffected until the Worker wires it.
    const routingDecision = decideCacheAwareRouting({
      affinityHash: affinity.hash,
      plannedLanes: basePlannedIds,
      ...(deps.cacheWarmthOracle === undefined
        ? {}
        : { warmthOracle: deps.cacheWarmthOracle }),
      ...(deps.laneHealthOracle === undefined
        ? {}
        : { healthOracle: deps.laneHealthOracle }),
      ...(deps.cachePinPolicy === undefined
        ? {}
        : { pinPolicy: deps.cachePinPolicy }),
    })
    const plannedIds = routingDecision.lanes

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

    // COMPONENT-CHANNEL ACTIVATION (issue #6127). AND-gated: gateway flag on +
    // explicit per-request opt-in + Khala model. Default false => the channel is
    // inert and the response is byte-for-byte today's text-only stream.
    const componentChannelActive = resolveComponentChannelActive({
      autopilotConcierge: autopilotConcierge?.ok === true,
      config: deps.componentChannel,
      rawBody,
      request,
      requestedModel,
    })

    const inferenceRequest = toInferenceRequest(
      body,
      rawBody,
      requestedModel,
      affinity.params,
      componentChannelActive,
      autopilotConcierge?.ok === true ? autopilotConcierge.config : undefined,
    )
    // The raw cache-affinity key threaded into every telemetry build site so the
    // receipt records its public-safe HASH (never the raw key). Undefined for
    // non-Khala / no-affinity requests.
    const cacheAffinityKeyRaw = affinity.rawKey
    const meteringHook = deps.meteringHook ?? stubMeteringHook
    const resolveFundingKind =
      deps.resolveFundingKind ?? defaultCardFundingResolver
    const nowEpochSeconds = deps.nowEpochSeconds ?? currentEpochSeconds
    const newId = deps.newId ?? defaultId
    const nowEpochMillis = deps.nowEpochMillis ?? currentEpochMillis
    const created = nowEpochSeconds()
    const responseId = newId()
    // Request-accept wall-clock start (ms). Telemetry total wall-clock is measured
    // from here to completion (book P0-1). Deterministic in tests via nowEpochMillis.
    const requestStartMs = nowEpochMillis()
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
      ...(deps.dispatch?.routingSignals === undefined
        ? {}
        : { routingSignals: deps.dispatch.routingSignals }),
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
      //
      // COMPONENT-CHANNEL (issue #6127): when the typed component channel is
      // active for this request, we deliberately take the BUFFERED path instead
      // of the true pass-through. A component frame is ATOMIC (one complete
      // validated card) and the prose/component split + schema validation + the
      // bounded repair turn all need the WHOLE assembled completion — which the
      // true pass-through (delta-as-it-arrives) cannot provide. So we signal the
      // SAME non-retryable `stream_not_supported` the no-`streamSse` lane signals,
      // and the existing fallthrough re-frames the completion through the channel.
      // This keeps the default (channel-off) path byte-for-byte unchanged.
      const sseDispatch = yield* dispatchWithOverflowWithMetadata(
        inferenceRequest,
        (adapter, request) => {
          if (componentChannelActive || adapter.streamSse === undefined) {
            // Signal "this lane cannot stream incrementally" as a NON-retryable
            // typed failure so the overflow loop surfaces it without retrying;
            // the route catches it and falls back to the buffered path.
            //
            // COMPONENT-CHANNEL (issue #6127): when the typed component channel is
            // active for this request, we deliberately force the BUFFERED path. A
            // component frame is ATOMIC (one complete validated card) and the
            // prose/component split + schema validation + the bounded repair turn
            // all need the WHOLE assembled completion — which the true pass-through
            // (delta-as-it-arrives) cannot provide. The default (channel-off) path
            // is byte-for-byte unchanged.
            return Effect.fail(
              new InferenceAdapterError({
                adapterId: adapter.id,
                kind: 'stream_not_supported',
                reason: componentChannelActive
                  ? 'component channel active: buffered re-frame required'
                  : 'adapter does not support incremental streaming',
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
        const { adapterId, source } = sseDispatch.served.value
        // DURABLE-STREAM RANK-1 (#6058). Resolve a per-request durable store when
        // the flag is on AND a store factory is wired; a failing/absent factory
        // leaves `durableStore` undefined so the stream degrades to today's
        // pass-through (fail-safe, idempotent). The durable read URL lets a client
        // reconnect `?offset=<last>` and replay the suffix without re-billing.
        const durableStore: StreamStore | undefined =
          deps.durableStreamEnabled === true && deps.durableStream !== undefined
            ? resolveDurableStore(deps.durableStream, responseId)
            : undefined
        // PRODUCTION DURABLE SUBSTRATE (#6058): the per-request Durable Object,
        // resolved only when the flag is on AND the binding is wired. Takes
        // precedence over the in-memory `durableStore` test substrate.
        const durableNamespace: DurableStreamNamespace | undefined =
          deps.durableStreamEnabled === true &&
          deps.durableStreamNamespace !== undefined
            ? deps.durableStreamNamespace
            : undefined
        // The completion is being persisted (and is resumable) when EITHER durable
        // substrate is active.
        const durablePersisting =
          durableNamespace !== undefined || durableStore !== undefined
        const responseStream = makePassThroughResponseStream({
          accountRef: session.accountRef,
          adapterId,
          ...(cacheAffinityKeyRaw === undefined ? {} : { cacheAffinityKeyRaw }),
          created,
          durableNowMs: nowEpochMillis(),
          durableStore,
          ...(durableNamespace === undefined ? {} : { durableNamespace }),
          fundingKind,
          meteringHook,
          // TELEMETRY CLOCK (book P0-1): the TRUE pass-through path is where TTFT
          // and generation wall-clock are genuinely observable (first delta + EOF).
          nowMs: nowEpochMillis,
          requestStartMs,
          requestedModel,
          responseId,
          source,
          routeMetadata: sseDispatch.served.route,
        })
        return new Response(responseStream, {
          headers: {
            'cache-control': 'no-store',
            'content-type': 'text/event-stream; charset=utf-8',
            // Advertise the resumable read URL only when the completion is being
            // persisted. The opaque request id carries no prompt/credential
            // material, so this header is safe (INVARIANTS: no leakage).
            ...(durablePersisting
              ? {
                  'openagents-durable-stream-url':
                    durableInferenceReadUrl(responseId),
                }
              : {}),
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
      const chunks = yield* dispatchWithOverflowWithMetadata(
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
      const servedChunks = chunks.served.value.value
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
          adapterId: chunks.served.value.adapterId,
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
      const assembledContent = servedChunks
        .map(chunk => chunk.contentDelta)
        .join('')

      // KHALA IDENTITY GUARD (buffered stream path). This path materializes the
      // WHOLE upstream completion before emitting a byte (it builds `body_sse`
      // from the full chunk array), so the deterministic identity backstop runs
      // here on the assembled content. No re-ask on this path (the chunks are
      // already buffered); the gateway identity system prompt is the primary
      // defense and this is the fail-closed redaction backstop. A clean stream is
      // re-emitted delta-for-delta unchanged; only an identity leak is rewritten.
      let guardedStreamContent = assembledContent
      let streamIdentityCorrected = false
      // COMPONENT-CHANNEL TRANSFORM (issue #6127). When the channel is active for
      // this request, run the assembled completion through the gateway transform:
      // split prose vs fenced `oa-component` blocks, validate each card against
      // the closed catalog (ONE bounded repair turn, then drop), identity-guard
      // the prose, and surface the validated frames. The transform ALSO runs the
      // identity backstop over the prose, so it subsumes the plain identity guard
      // for this request. Inert for every channel-off request (the `else` below is
      // the unchanged prior behavior).
      let componentFrames: ReadonlyArray<KhalaComponentFrame> = []
      if (componentChannelActive) {
        const channel: ComponentChannelOutput = yield* Effect.promise(() =>
          runComponentChannel(assembledContent, {
            ...(deps.componentChannel?.repairReask === undefined
              ? {}
              : { reask: deps.componentChannel.repairReask }),
          }),
        )
        guardedStreamContent = channel.prose
        componentFrames = channel.frames
        // The transform already identity-guarded the prose; force the re-frame
        // path below (a single corrected content frame + the component frames),
        // since the per-delta chunks no longer match the stripped prose.
        streamIdentityCorrected = true
      } else if (isKhalaModel(requestedModel)) {
        const guard = yield* Effect.promise(() =>
          guardKhalaCompletion({ completion: assembledContent }),
        )
        guardedStreamContent = guard.text
        streamIdentityCorrected = guard.corrected
      }

      const streamResult: InferenceResult = {
        content: guardedStreamContent,
        finishReason: terminal?.finishReason ?? 'stop',
        servedModel: terminal?.servedModel ?? requestedModel,
        usage: terminal?.usage ?? {
          completionTokens: 0,
          promptTokens: 0,
          totalTokens: 0,
        },
      }
      const streamOpenagents = khalaReceiptForResult({
        adapterId: chunks.served.value.adapterId,
        ...(cacheAffinityKeyRaw === undefined ? {} : { cacheAffinityKeyRaw }),
        // `khalaReceiptForResult` requires a MeteringOutcome; when no terminal
        // usage frame was served (so no metering ran) fall back to the stub
        // outcome shape so a Khala stream without usage still discloses.
        metering: streamMetering ?? { metered: false, receiptRef: null },
        requestedModel,
        responseId,
        result: streamResult,
        routeMetadata: chunks.served.route,
        // Buffered fallback (adapter has no incremental `streamSse`): the WHOLE
        // completion is materialized before a byte is emitted, so there is no
        // observable first-token boundary here — TTFT/ITL stay honest sentinels.
        // Total wall-clock IS measurable from request accept to drain.
        timing: {
          streamed: true,
          totalWallClockMs: Math.max(0, nowEpochMillis() - requestStartMs),
        },
      })

      // When the identity guard rewrote the content, the original per-delta
      // chunks no longer match the corrected text, so re-frame the stream as a
      // single corrected content frame + a terminal frame carrying the finish
      // reason and disclosure. When nothing was corrected, re-emit the chunks
      // exactly as served (byte-for-byte the prior behavior).
      const frameStrings: Array<string> = []
      if (streamIdentityCorrected) {
        // Prose content frame. When the component channel stripped all prose to
        // empty (a card-only turn), emit an empty-delta frame so the stream shape
        // stays a valid OpenAI chunk sequence.
        frameStrings.push(
          sseFrame({
            choices: [
              {
                delta:
                  guardedStreamContent === ''
                    ? {}
                    : { content: guardedStreamContent },
                finish_reason: null,
                index: 0,
              },
            ],
            created,
            id: responseId,
            model: requestedModel,
            object: 'chat.completion.chunk',
          }),
        )
        // COMPONENT-CHANNEL FRAMES (issue #6127). Each validated card is emitted
        // as one ATOMIC `event: oa.component` SSE frame, AFTER the prose and
        // BEFORE the terminal `chat.completion.chunk`. Standard OpenAI clients
        // ignore the unknown `event:` type and still parse the stream as text;
        // the Foldkit client switches on `oa.component` to render the typed card.
        for (const frame of componentFrames) {
          frameStrings.push(serializeComponentFrame(frame))
        }
        frameStrings.push(
          sseFrame({
            choices: [
              {
                delta: {},
                finish_reason: terminal?.finishReason ?? 'stop',
                index: 0,
              },
            ],
            created,
            id: responseId,
            model: requestedModel,
            object: 'chat.completion.chunk',
            ...(streamOpenagents !== undefined
              ? { openagents: streamOpenagents }
              : {}),
          }),
        )
      } else {
        const lastIndex = servedChunks.length - 1
        for (const [index, chunk] of servedChunks.entries()) {
          frameStrings.push(
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
        }
      }
      const body_sse = frameStrings.join('')
      const stream = `${body_sse}data: [DONE]\n\n`

      return new Response(stream, {
        headers: {
          'cache-control': 'no-store',
          'content-type': 'text/event-stream; charset=utf-8',
        },
        status: 200,
      })
    }

    const result = yield* dispatchWithOverflowWithMetadata(
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

    // KHALA IDENTITY GUARD (STEP 2). Run the typed identity signature over the
    // completion BEFORE metering/response. If the completion asserts a forbidden
    // provider/model identity ("I am built on Gemini / by Google", Claude, GPT,
    // …) the guard corrects it: first by RE-ASKING the provider with a stronger
    // identity instruction, and as a fail-closed backstop by deterministically
    // redacting the offending identity claim to the Khala identity statement.
    // It NEVER mangles a normal answer — a clean completion passes through
    // unchanged — and only runs for Khala models (non-Khala responses are
    // byte-identical to before). The re-ask re-dispatches across the same lane
    // plan with the reinforcement appended as a leading system message.
    const servedValue = result.served.value.value
    const servedAdapterId = result.served.value.adapterId
    let guardedContent = servedValue.content
    if (isKhalaModel(requestedModel)) {
      // Verify the original completion against the identity signatures. Only on
      // a violation do we re-ask (the native Effect dispatch below), so a clean
      // answer never triggers an extra provider call. The re-ask result is then
      // handed to `guardKhalaCompletion` (which re-verifies it and applies the
      // deterministic backstop if it STILL leaks) — keeping the dispatch in the
      // Effect topology rather than nesting `Effect.runPromise`.
      const leaked = verifyKhalaSignatures(servedValue.content).some(
        verdict => !verdict.satisfied,
      )
      const reaskedContent = leaked
        ? yield* dispatchWithOverflow(
            {
              ...inferenceRequest,
              messages: [
                {
                  content: KHALA_IDENTITY_REINFORCEMENT_PROMPT,
                  role: 'system',
                },
                ...inferenceRequest.messages,
              ],
            },
            (adapter, request) => adapter.complete(request),
            dispatchDeps,
          ).pipe(
            Effect.map(value => value.content as string | undefined),
            Effect.orElseSucceed(() => undefined),
          )
        : undefined
      const guard = yield* Effect.promise(() =>
        guardKhalaCompletion({
          completion: servedValue.content,
          // The re-ask is already resolved (above); the guard just consumes it.
          reask:
            reaskedContent === undefined
              ? undefined
              : async () => reaskedContent,
        }),
      )
      guardedContent = guard.text
    }
    // Apply the (possibly corrected) content. A clean answer is identical to
    // `servedValue`; only an identity leak changes `content`.
    const guardedResult: InferenceResult =
      guardedContent === servedValue.content
        ? servedValue
        : { ...servedValue, content: guardedContent }

    const metering = yield* meteringHook({
      accountRef: session.accountRef,
      adapterId: servedAdapterId,
      fundingKind,
      requestId: responseId,
      requestedModel: requestedModel,
      servedModel: guardedResult.servedModel,
      streamed: false,
      usage: guardedResult.usage,
    })

    // ACCEPTANCE-DISPATCH (EPIC #6017): when khala-code produced a runnable artifact,
    // enqueue an out-of-Worker verification job (flagged; inert by default). The
    // node-side runner executes the suite and the verdict callback backfills the
    // receipt from `unverified` -> `test_passed`/`failed`. Fire-and-forget: never
    // blocks or fails the already-priced completion.
    yield* maybeEnqueueAcceptanceJob({
      adapterId: servedAdapterId,
      dispatch: deps.acceptanceDispatch,
      meteringReceiptRef: metering.receiptRef,
      requestMessages: inferenceRequest.messages,
      requestedModel,
      responseId,
      result: guardedResult,
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
          adapterId: servedAdapterId,
          ...(cacheAffinityKeyRaw === undefined ? {} : { cacheAffinityKeyRaw }),
          metering,
          requestedModel,
          responseId,
          result: guardedResult,
          routeMetadata: result.served.route,
          // Non-streaming: total wall-clock is measurable; TTFT/ITL are not (no
          // first-token boundary on a buffered completion) => honest sentinels.
          timing: {
            streamed: false,
            totalWallClockMs: Math.max(0, nowEpochMillis() - requestStartMs),
          },
        }),
        result: guardedResult,
      }),
    )
  })
