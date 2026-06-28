// Live Vertex AgentCL runner (CL-4b, public issue #6788).
//
// This is the RUNNABLE enforcement layer the CL-4 guardrail
// (`agentcl.ts`) was missing. Up to #6788 the AgentCL Vertex contract was a
// typed plan + a pure circuit-breaker decision function referenced ONLY by
// tests: there was no live Vertex HTTP client, no per-iteration runtime
// enforcement, no live spend metering from real usage, and no kill-switch that
// aborts an in-flight paid run or blocks a GLM/free fallback. CL-5 cannot run
// safely until that exists; this module supplies it.
//
// What it owns:
//   1. A real per-iteration loop that wires the contract circuit-breaker
//      (`assessAgentClVertexRunnerCircuitBreaker`) into the run BEFORE each call
//      and AFTER each result, hard-stopping the loop the instant accumulated
//      spend reaches the cap OR three consecutive billing/quota errors trip.
//   2. Live spend metering computed from REAL provider usage tokens
//      (`InferenceResult.usage`) x the Vertex Gemini 3.5 Flash price from the
//      canonical pricing table - never a caller-supplied estimate.
//   3. A no-fallback guard: the runner only ever targets the `vertex-gemini`
//      lane. It refuses to start when routing for the requested model would
//      include any non-Vertex (GLM / free / Fireworks / OpenRouter) lane, and it
//      aborts mid-run if a call is served by any adapter other than
//      `vertex-gemini`.
//
// The loop is driven by an injected `modelFn` so the SAME enforcement runs over
// either a real Vertex call (`makeVertexGeminiRunnerModelFn`, live/owner-armed)
// or a no-network stub (dry-run proofs + unit tests). The loop NEVER itself
// decides to spend: it only enforces.
//
// Public-safety: this module computes spend cents and token totals only. It
// never prints prompts, provider payloads, the service-account key, or any
// secret. The CLI wrapper (`scripts/agentcl-vertex-runner.ts`) gates the live
// paid path behind an explicit `CL_VERTEX_ARMED=1` env + `--live` flag.
import { Effect } from 'effect'

import {
  assessAgentClVertexRunnerCircuitBreaker,
  buildAgentClVertexStressExperiment,
  runAgentClRepoReuseFixtureEval,
  type AgentClVertexStressCircuitBreakerReason,
} from './agentcl'
import {
  selectAdapterPlan,
  VERTEX_GEMINI_ADAPTER_ID,
} from '../model-router'
import {
  DEFAULT_GEMINI_MODEL_ID,
  makeVertexGeminiAdapter,
} from '../vertex-gemini-adapter'
import { type VertexTokenProvider } from '../vertex-anthropic-adapter'
import { lookupModel, type ModelCostPerMtok } from '../pricing'
import {
  type InferenceAdapterError,
  type InferenceRequest,
  type InferenceUsage,
} from '../provider-adapter'
import { tokenProviderFromSecret } from '../vertex-token'

export const AGENTCL_VERTEX_RUNNER_RECEIPT_SCHEMA =
  'openagents.gym.agentcl_vertex_runner_receipt.v0' as const

// The ONLY model id this runner serves. Routing classifies any `gemini-*` id to
// the gemini class, whose lane plan is `['vertex-gemini']` with NO fallback
// (model-router.ts `LANE_PLAN_BY_CLASS.gemini`). Using the raw Vertex-native id
// (not the public `openagents/khala` alias, which DOES carry GLM/free overflow)
// is what makes the no-fallback guarantee hold.
export const AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID = DEFAULT_GEMINI_MODEL_ID

// The GCP project that holds the first-party Vertex Gemini quota.
export const AGENTCL_VERTEX_GEMINI_PROJECT = 'openagentsgemini' as const

// Default spend cap: $50 = 5000 cents, matching the CL-4 contract budget guard
// (`buildAgentClVertexGeminiRunnerPlan().budgetGuard.spendCapUsdCents`). The CLI
// allows an env override (CL_VERTEX_CAP_USD) but the contract breaker also
// enforces an absolute 5000-cent ceiling, so a HIGHER override can never raise
// the real ceiling above $50 - it can only LOWER it.
export const DEFAULT_CL_VERTEX_CAP_USD_CENTS = 5000

// Consecutive billing/quota errors that trip the breaker (matches the contract
// plan `abortOnConsecutiveBillingOrQuotaErrors`).
export const CL_VERTEX_CONSECUTIVE_ERROR_BREAKER = 3

// Capacity-error refs the breaker treats as "billing/quota" (matches the
// contract plan `trackedCapacityErrorRefs`).
export type AgentClVertexCapacityErrorRef =
  | 'billing_error'
  | 'quota_error'
  | 'http_429'
  | 'resource_exhausted'

// Resolve the Vertex Gemini 3.5 Flash marginal cost from the canonical pricing
// table. Falls back to the documented list rate if the entry is ever removed so
// metering never silently becomes free.
export const resolveVertexGeminiFlashCost = (): ModelCostPerMtok =>
  lookupModel(AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID)?.cost ?? {
    inputUsdPerMtok: 0.075,
    outputUsdPerMtok: 0.3,
  }

// Compute the USD-cents cost of one served call from its REAL provider usage.
// Deliberately bills cached prompt tokens at the FULL input rate (no cache
// discount): for a hard spend cap, over-estimating spend is the safe direction -
// it can only trip the breaker EARLIER, never later.
export const priceAgentClVertexCallUsdCents = (
  usage: Readonly<{
    promptTokens: number
    completionTokens: number
  }>,
  cost: ModelCostPerMtok = resolveVertexGeminiFlashCost(),
): number => {
  const promptTokens = Math.max(0, usage.promptTokens)
  const completionTokens = Math.max(0, usage.completionTokens)
  const usd =
    (promptTokens / 1_000_000) * cost.inputUsdPerMtok +
    (completionTokens / 1_000_000) * cost.outputUsdPerMtok
  return usd * 100
}

// The candidate adapter plan routing would use for a model id. Exposed so the
// CLI / tests can show the real routing decision.
export const resolveAgentClVertexAdapterPlan = (
  model: string,
): ReadonlyArray<string> => selectAdapterPlan(model)

// True only when routing for `model` resolves to the single Vertex Gemini lane
// with NO other (GLM / free / Fireworks / OpenRouter) adapter in the plan.
export const isVertexGeminiOnlyPlan = (
  plan: ReadonlyArray<string>,
): boolean => plan.length === 1 && plan[0] === VERTEX_GEMINI_ADAPTER_ID

// The combined runner stop decision. Wires the CL-4 contract breaker (3-error +
// absolute $50 ceiling) and layers the env-overridable cap with a HARD `>=`
// boundary so the loop halts the instant accumulated spend reaches the cap.
export const assessAgentClVertexRunnerStop = (
  input: Readonly<{
    accumulatedSpendUsdCents: number
    consecutiveBillingOrQuotaErrors: number
    capUsdCents: number
  }>,
): Readonly<{
  tripped: boolean
  reason: AgentClVertexStressCircuitBreakerReason
}> => {
  // Canonical contract breaker: 3 consecutive billing/quota errors OR strictly
  // above the absolute 5000-cent ($50) contract ceiling.
  const contract = assessAgentClVertexRunnerCircuitBreaker({
    consecutiveBillingOrQuotaErrors: input.consecutiveBillingOrQuotaErrors,
    estimatedSpendUsdCents: input.accumulatedSpendUsdCents,
  })
  if (contract.tripped) {
    return contract
  }
  // Env-overridable cap with a hard `>=` boundary (default equals the contract
  // ceiling). Halts AT the cap, not just above it.
  if (input.accumulatedSpendUsdCents >= input.capUsdCents) {
    return { reason: 'spend_cap_exceeded', tripped: true }
  }
  return { reason: 'none', tripped: false }
}

// Outcome of a single model call handed back to the loop. The loop owns ALL
// enforcement; the modelFn only reports what happened.
export type AgentClVertexCallOutcome =
  | Readonly<{
      _tag: 'served'
      servedAdapterId: string
      usage: InferenceUsage
      finishReason: string
    }>
  | Readonly<{
      _tag: 'billing_or_quota_error'
      errorRef: AgentClVertexCapacityErrorRef
    }>
  // Routing tried to leave the Vertex lane (or a non-Vertex adapter served the
  // call). The loop treats this as a hard, immediate abort.
  | Readonly<{ _tag: 'fallback_attempted'; toLaneRef: string }>
  // Any other (non-billing, non-fallback) error. Does NOT add spend and does NOT
  // trip the consecutive-error breaker.
  | Readonly<{ _tag: 'error'; errorRef: string }>

export type AgentClVertexRunnerModelFn = (
  iteration: number,
) => Promise<AgentClVertexCallOutcome>

export type AgentClVertexRunnerAbortReason =
  | 'completed'
  | 'spend_cap_exceeded'
  | 'consecutive_billing_or_quota_errors'
  | 'forbidden_fallback'
  | 'no_fallback_plan_refused'

export type AgentClVertexRunnerReceipt = Readonly<{
  schemaVersion: typeof AGENTCL_VERTEX_RUNNER_RECEIPT_SCHEMA
  model: string
  laneRef: 'vertex-gemini'
  adapterPlan: ReadonlyArray<string>
  noFallbackPlanVerified: boolean
  capUsdCents: number
  iterationsRequested: number
  iterationsAttempted: number
  iterationsServed: number
  promptTokens: number
  completionTokens: number
  totalTokens: number
  estimatedSpendUsdCents: number
  http429Count: number
  billingOrQuotaErrorCount: number
  consecutiveBillingOrQuotaErrors: number
  forbiddenFallbackBlocked: boolean
  circuitBreakerTripped: boolean
  circuitBreakerReason: AgentClVertexStressCircuitBreakerReason
  abortReason: AgentClVertexRunnerAbortReason
  // Fixture-eval AgentCL gains (PG/SG/GG). The live loop measures spend +
  // capacity; the learning gains stay fixture-derived (deterministic) until a
  // real CL-5 eval supplies them, so they are reported as a stable skeleton.
  agentClFixtureGains: Readonly<{
    plasticityGain: number
    stabilityGain: number
    generalizationGain: number
  }>
}>

const fixtureGains = (): AgentClVertexRunnerReceipt['agentClFixtureGains'] => {
  const { eval: evalResult } = runAgentClRepoReuseFixtureEval(
    buildAgentClVertexStressExperiment(
      'owner.approval.agentcl.vertex_stress.required',
    ),
  )
  return {
    generalizationGain: evalResult.generalizationGain,
    plasticityGain: evalResult.plasticityGain,
    stabilityGain: evalResult.stabilityGain,
  }
}

// Run the enforced AgentCL Vertex loop. Pure orchestration: deterministic given
// the injected `modelFn`. The same code path runs the dry-run stub and the live
// paid call - only `modelFn` differs.
export const runAgentClVertexRunnerLoop = async (
  input: Readonly<{
    modelFn: AgentClVertexRunnerModelFn
    maxIterations: number
    model?: string | undefined
    capUsdCents?: number | undefined
    // Verify the no-fallback routing plan before the first call. Defaults true.
    enforceNoFallbackPlan?: boolean | undefined
  }>,
): Promise<AgentClVertexRunnerReceipt> => {
  const model = input.model ?? AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID
  const capUsdCents = input.capUsdCents ?? DEFAULT_CL_VERTEX_CAP_USD_CENTS
  const enforceNoFallbackPlan = input.enforceNoFallbackPlan ?? true
  const adapterPlan = selectAdapterPlan(model)
  const noFallbackPlanVerified = isVertexGeminiOnlyPlan(adapterPlan)

  let promptTokens = 0
  let completionTokens = 0
  let totalTokens = 0
  let estimatedSpendUsdCents = 0
  let http429Count = 0
  let billingOrQuotaErrorCount = 0
  let consecutiveBillingOrQuotaErrors = 0
  let iterationsAttempted = 0
  let iterationsServed = 0
  let forbiddenFallbackBlocked = false
  let abortReason: AgentClVertexRunnerAbortReason = 'completed'

  const buildReceipt = (): AgentClVertexRunnerReceipt => {
    const stop = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: estimatedSpendUsdCents,
      capUsdCents,
      consecutiveBillingOrQuotaErrors,
    })
    const tripped =
      abortReason === 'spend_cap_exceeded' ||
      abortReason === 'consecutive_billing_or_quota_errors' ||
      stop.tripped
    const circuitBreakerReason: AgentClVertexStressCircuitBreakerReason =
      abortReason === 'spend_cap_exceeded'
        ? 'spend_cap_exceeded'
        : abortReason === 'consecutive_billing_or_quota_errors'
          ? 'consecutive_billing_or_quota_errors'
          : stop.reason
    return {
      adapterPlan,
      abortReason,
      agentClFixtureGains: fixtureGains(),
      billingOrQuotaErrorCount,
      capUsdCents,
      circuitBreakerReason,
      circuitBreakerTripped: tripped,
      completionTokens,
      consecutiveBillingOrQuotaErrors,
      estimatedSpendUsdCents,
      forbiddenFallbackBlocked,
      http429Count,
      iterationsAttempted,
      iterationsRequested: input.maxIterations,
      iterationsServed,
      laneRef: 'vertex-gemini',
      model,
      noFallbackPlanVerified,
      promptTokens,
      schemaVersion: AGENTCL_VERTEX_RUNNER_RECEIPT_SCHEMA,
      totalTokens,
    }
  }

  // Pre-flight: refuse to run at all if routing for this model would ever leave
  // the Vertex lane. No call is made.
  if (enforceNoFallbackPlan && !noFallbackPlanVerified) {
    forbiddenFallbackBlocked = true
    abortReason = 'no_fallback_plan_refused'
    return buildReceipt()
  }

  for (let iteration = 0; iteration < input.maxIterations; iteration += 1) {
    // ENFORCE BEFORE each call: never make another call once the cap or the
    // error breaker has tripped.
    const pre = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: estimatedSpendUsdCents,
      capUsdCents,
      consecutiveBillingOrQuotaErrors,
    })
    if (pre.tripped) {
      abortReason =
        pre.reason === 'spend_cap_exceeded'
          ? 'spend_cap_exceeded'
          : 'consecutive_billing_or_quota_errors'
      break
    }

    iterationsAttempted += 1
    const outcome = await input.modelFn(iteration)

    // Hard, immediate abort if routing left the Vertex lane.
    if (
      outcome._tag === 'fallback_attempted' ||
      (outcome._tag === 'served' &&
        outcome.servedAdapterId !== VERTEX_GEMINI_ADAPTER_ID)
    ) {
      forbiddenFallbackBlocked = true
      abortReason = 'forbidden_fallback'
      break
    }

    if (outcome._tag === 'billing_or_quota_error') {
      consecutiveBillingOrQuotaErrors += 1
      billingOrQuotaErrorCount += 1
      if (outcome.errorRef === 'http_429') {
        http429Count += 1
      }
    } else if (outcome._tag === 'served') {
      consecutiveBillingOrQuotaErrors = 0
      iterationsServed += 1
      promptTokens += outcome.usage.promptTokens
      completionTokens += outcome.usage.completionTokens
      totalTokens += outcome.usage.totalTokens
      estimatedSpendUsdCents += priceAgentClVertexCallUsdCents(outcome.usage)
    }
    // `error` outcomes add no spend and do not move the breaker.

    // ENFORCE AFTER each result: abort the instant the cap or error breaker
    // trips, before the next iteration's BEFORE check would even run.
    const post = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: estimatedSpendUsdCents,
      capUsdCents,
      consecutiveBillingOrQuotaErrors,
    })
    if (post.tripped) {
      abortReason =
        post.reason === 'spend_cap_exceeded'
          ? 'spend_cap_exceeded'
          : 'consecutive_billing_or_quota_errors'
      break
    }
  }

  return buildReceipt()
}

// Classify an adapter error into a tracked billing/quota capacity-error ref, or
// undefined for a generic (non-breaker) error. The Vertex Gemini adapter encodes
// the HTTP status + Google error status in its `reason` string, so we parse that
// (the adapter does not currently populate `httpStatus`/`kind`). Bounded,
// case-insensitive substring classification of provider-neutral error text.
export const classifyVertexBillingOrQuotaError = (
  error: InferenceAdapterError,
): AgentClVertexCapacityErrorRef | undefined => {
  const haystack =
    `${error.reason} ${error.kind ?? ''} ${error.httpStatus ?? ''}`.toLowerCase()
  if (error.httpStatus === 429 || haystack.includes('http 429')) {
    return 'http_429'
  }
  if (haystack.includes('resource_exhausted')) {
    return 'resource_exhausted'
  }
  if (haystack.includes('quota')) {
    return 'quota_error'
  }
  if (
    haystack.includes('billing') ||
    haystack.includes('permission_denied') ||
    haystack.includes('consumer') ||
    haystack.includes('http 402') ||
    haystack.includes('http 403')
  ) {
    return 'billing_error'
  }
  return undefined
}

// A single-call Effect that performs ONE real Vertex Gemini call and maps the
// result into the loop's outcome union. Never fails (errors are folded into the
// outcome) so the loop owns all control flow. Exposed as an Effect so the
// Effect->Promise bridge (`Effect.runPromise`) stays at the runnable edge (the
// scripts CLI + tests), keeping this domain module free of the temporary
// runPromise bridge budget.
export type AgentClVertexRunnerEffectFn = (
  iteration: number,
) => Effect.Effect<AgentClVertexCallOutcome>

// Build the LIVE Vertex Gemini call effect: a real authorized paid call to
// gemini-3.5-flash on project openagentsgemini, via the SAME adapter + SA-key
// token path the inference gateway registers (`makeVertexGeminiAdapter` +
// `tokenProviderFromSecret(VERTEX_SA_KEY)`). Returns the receipt-first provider
// usage on success and a classified capacity-error / generic-error on failure.
// It NEVER falls back to another lane - there is no other adapter here.
export const makeVertexGeminiRunnerEffectFn = (
  input: Readonly<{
    prompt: string
    // Raw VERTEX_SA_KEY service-account-key JSON string. The LIVE path supplies
    // this; the adapter mints a short-lived GCP token from it.
    serviceAccountKey?: string | undefined
    // Pre-built token provider (tests inject this to exercise the served/error
    // mapping without SA-key crypto or a real token mint).
    tokenProvider?: VertexTokenProvider | undefined
    model?: string | undefined
    project?: string | undefined
    location?: string | undefined
    maxTokens?: number | undefined
    // Injected for tests; defaults to global fetch inside the adapter.
    fetchImpl?: typeof fetch | undefined
  }>,
): AgentClVertexRunnerEffectFn => {
  const model = input.model ?? AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID
  const tokenProvider =
    input.tokenProvider ??
    (input.serviceAccountKey === undefined
      ? undefined
      : tokenProviderFromSecret(input.serviceAccountKey))
  const adapter = makeVertexGeminiAdapter({
    fetchImpl: input.fetchImpl,
    location: input.location,
    project: input.project ?? AGENTCL_VERTEX_GEMINI_PROJECT,
    tokenProvider,
  })

  return () => {
    const request: InferenceRequest = {
      messages: [{ content: input.prompt, role: 'user' }],
      model,
      passthroughParams: {
        max_tokens: input.maxTokens ?? 256,
        temperature: 0,
      },
      stream: false,
    }
    return adapter.complete(request).pipe(
      Effect.map(
        (result): AgentClVertexCallOutcome => ({
          _tag: 'served',
          finishReason: result.finishReason,
          servedAdapterId: VERTEX_GEMINI_ADAPTER_ID,
          usage: result.usage,
        }),
      ),
      Effect.catch(
        (error): Effect.Effect<AgentClVertexCallOutcome> => {
          const capacityErrorRef = classifyVertexBillingOrQuotaError(error)
          return Effect.succeed(
            capacityErrorRef !== undefined
              ? { _tag: 'billing_or_quota_error', errorRef: capacityErrorRef }
              : { _tag: 'error', errorRef: 'vertex_adapter_error' },
          )
        },
      ),
    )
  }
}
