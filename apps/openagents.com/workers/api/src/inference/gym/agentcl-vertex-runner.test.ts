import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID,
  AGENTCL_VERTEX_RUNNER_RECEIPT_SCHEMA,
  assessAgentClVertexRunnerStop,
  classifyVertexBillingOrQuotaError,
  DEFAULT_CL_VERTEX_CAP_USD_CENTS,
  isVertexGeminiOnlyPlan,
  makeVertexGeminiRunnerEffectFn,
  priceAgentClVertexCallUsdCents,
  resolveAgentClVertexAdapterPlan,
  resolveVertexGeminiFlashCost,
  runAgentClVertexRunnerLoop,
  type AgentClVertexCallOutcome,
} from './agentcl-vertex-runner'
import { KHALA_MODEL_ID } from '../pricing'
import { InferenceAdapterError } from '../provider-adapter'

// A served-call stub whose REAL metered cost is exactly `cents`.
const servedUsageForCents = (cents: number) => {
  const cost = resolveVertexGeminiFlashCost()
  const completionTokens = Math.ceil(
    cents / ((cost.outputUsdPerMtok / 1_000_000) * 100),
  )
  return { completionTokens, promptTokens: 0, totalTokens: completionTokens }
}

const servedVertex = (
  usage: ReturnType<typeof servedUsageForCents>,
): AgentClVertexCallOutcome => ({
  _tag: 'served',
  finishReason: 'stop',
  servedAdapterId: 'vertex-gemini',
  usage,
})

describe('AgentCL Vertex runner — pricing + routing', () => {
  test('prices a call from real gemini-3.5-flash rates', () => {
    const cost = resolveVertexGeminiFlashCost()
    expect(cost.inputUsdPerMtok).toBeGreaterThan(0)
    expect(cost.outputUsdPerMtok).toBeGreaterThan(0)
    // 1M input + 1M output tokens = (input + output) USD = cents * 100.
    const cents = priceAgentClVertexCallUsdCents({
      completionTokens: 1_000_000,
      promptTokens: 1_000_000,
    })
    expect(cents).toBeCloseTo(
      (cost.inputUsdPerMtok + cost.outputUsdPerMtok) * 100,
      6,
    )
  })

  test('gemini-3.5-flash routes ONLY to the vertex-gemini lane', () => {
    const plan = resolveAgentClVertexAdapterPlan(
      AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID,
    )
    expect(plan).toEqual(['vertex-gemini'])
    expect(isVertexGeminiOnlyPlan(plan)).toBe(true)
  })

  test('the public khala alias carries a non-Vertex fallback lane', () => {
    const plan = resolveAgentClVertexAdapterPlan(KHALA_MODEL_ID)
    expect(plan.some(id => id !== 'vertex-gemini')).toBe(true)
    expect(isVertexGeminiOnlyPlan(plan)).toBe(false)
  })
})

describe('AgentCL Vertex runner — stop decision', () => {
  test('does not trip below the cap with no errors', () => {
    expect(
      assessAgentClVertexRunnerStop({
        accumulatedSpendUsdCents: 4999,
        capUsdCents: 5000,
        consecutiveBillingOrQuotaErrors: 0,
      }),
    ).toEqual({ reason: 'none', tripped: false })
  })

  test('trips at exactly the cap (>= boundary)', () => {
    const stop = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: 5000,
      capUsdCents: 5000,
      consecutiveBillingOrQuotaErrors: 0,
    })
    expect(stop.tripped).toBe(true)
    expect(stop.reason).toBe('spend_cap_exceeded')
  })

  test('trips on the 3-consecutive-error breaker', () => {
    const stop = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: 0,
      capUsdCents: 5000,
      consecutiveBillingOrQuotaErrors: 3,
    })
    expect(stop.tripped).toBe(true)
    expect(stop.reason).toBe('consecutive_billing_or_quota_errors')
  })

  test('a higher env cap cannot exceed the absolute $50 contract ceiling', () => {
    // Even with a 10000-cent override, the contract breaker trips above 5000.
    const stop = assessAgentClVertexRunnerStop({
      accumulatedSpendUsdCents: 5001,
      capUsdCents: 10_000,
      consecutiveBillingOrQuotaErrors: 0,
    })
    expect(stop.tripped).toBe(true)
    expect(stop.reason).toBe('spend_cap_exceeded')
  })
})

describe('AgentCL Vertex runner — enforced loop', () => {
  test('hard-stops at exactly $50 and makes NO further call', async () => {
    const cap = DEFAULT_CL_VERTEX_CAP_USD_CENTS
    const usage = servedUsageForCents(cap / 2)
    let calls = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: cap,
      maxIterations: 10,
      modelFn: async () => {
        calls += 1
        return servedVertex(usage)
      },
    })
    expect(receipt.schemaVersion).toBe(AGENTCL_VERTEX_RUNNER_RECEIPT_SCHEMA)
    expect(receipt.abortReason).toBe('spend_cap_exceeded')
    expect(receipt.estimatedSpendUsdCents).toBeGreaterThanOrEqual(cap)
    expect(receipt.iterationsServed).toBe(2)
    expect(receipt.iterationsAttempted).toBe(2)
    // The crucial proof: model_fn was invoked exactly twice, never a 3rd time.
    expect(calls).toBe(2)
    expect(receipt.circuitBreakerTripped).toBe(true)
  })

  test('never calls again after the cap even with many iterations', async () => {
    const cap = DEFAULT_CL_VERTEX_CAP_USD_CENTS
    // Each call costs the WHOLE cap -> halts after the first served call.
    const usage = servedUsageForCents(cap)
    let calls = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: cap,
      maxIterations: 1000,
      modelFn: async () => {
        calls += 1
        return servedVertex(usage)
      },
    })
    expect(calls).toBe(1)
    expect(receipt.iterationsAttempted).toBe(1)
    expect(receipt.abortReason).toBe('spend_cap_exceeded')
  })

  test('aborts on a simulated GLM fallback (no spend)', async () => {
    let calls = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 10,
      modelFn: async () => {
        calls += 1
        return { _tag: 'fallback_attempted', toLaneRef: 'glm-free' }
      },
    })
    expect(receipt.abortReason).toBe('forbidden_fallback')
    expect(receipt.forbiddenFallbackBlocked).toBe(true)
    expect(receipt.estimatedSpendUsdCents).toBe(0)
    expect(calls).toBe(1)
  })

  test('aborts when a non-vertex adapter serves the call (no spend)', async () => {
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 10,
      modelFn: async () => ({
        _tag: 'served',
        finishReason: 'stop',
        servedAdapterId: 'openrouter-khala-glm-fallback',
        usage: servedUsageForCents(100),
      }),
    })
    expect(receipt.abortReason).toBe('forbidden_fallback')
    expect(receipt.estimatedSpendUsdCents).toBe(0)
  })

  test('refuses a model whose routing plan carries a fallback lane (zero calls)', async () => {
    let calls = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 10,
      model: KHALA_MODEL_ID,
      modelFn: async () => {
        calls += 1
        return servedVertex(servedUsageForCents(100))
      },
    })
    expect(receipt.abortReason).toBe('no_fallback_plan_refused')
    expect(receipt.noFallbackPlanVerified).toBe(false)
    expect(calls).toBe(0)
  })

  test('trips the breaker at exactly 3 consecutive billing/quota errors', async () => {
    let calls = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 10,
      modelFn: async () => {
        calls += 1
        return { _tag: 'billing_or_quota_error', errorRef: 'http_429' }
      },
    })
    expect(receipt.abortReason).toBe('consecutive_billing_or_quota_errors')
    expect(receipt.iterationsAttempted).toBe(3)
    expect(receipt.http429Count).toBe(3)
    expect(receipt.billingOrQuotaErrorCount).toBe(3)
    expect(calls).toBe(3)
  })

  test('a generic error does not trip the breaker and adds no spend', async () => {
    const usage = servedUsageForCents(10)
    let call = 0
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 4,
      modelFn: async () => {
        call += 1
        // error, error, served, served — generic errors never break the loop.
        return call <= 2
          ? { _tag: 'error', errorRef: 'transient' }
          : servedVertex(usage)
      },
    })
    expect(receipt.abortReason).toBe('completed')
    expect(receipt.iterationsServed).toBe(2)
    expect(receipt.billingOrQuotaErrorCount).toBe(0)
    expect(receipt.estimatedSpendUsdCents).toBeCloseTo(20, 4)
  })

  test('a dry-run-style error-only loop incurs zero spend', async () => {
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 2,
      modelFn: async () => ({ _tag: 'error', errorRef: 'stub' }),
    })
    expect(receipt.estimatedSpendUsdCents).toBe(0)
    expect(receipt.iterationsServed).toBe(0)
  })
})

describe('AgentCL Vertex runner — error classification', () => {
  const err = (reason: string, httpStatus?: number) =>
    new InferenceAdapterError({
      adapterId: 'vertex-gemini',
      ...(httpStatus === undefined ? {} : { httpStatus }),
      reason,
      retryable: false,
    })

  test('classifies HTTP 429 as http_429', () => {
    expect(
      classifyVertexBillingOrQuotaError(
        err('Vertex Gemini returned HTTP 429: rate limit'),
      ),
    ).toBe('http_429')
    expect(classifyVertexBillingOrQuotaError(err('boom', 429))).toBe('http_429')
  })

  test('classifies RESOURCE_EXHAUSTED / quota / billing', () => {
    expect(
      classifyVertexBillingOrQuotaError(
        err('Vertex Gemini returned HTTP 400: RESOURCE_EXHAUSTED'),
      ),
    ).toBe('resource_exhausted')
    expect(
      classifyVertexBillingOrQuotaError(err('quota exceeded for project')),
    ).toBe('quota_error')
    expect(
      classifyVertexBillingOrQuotaError(
        err('Vertex Gemini returned HTTP 403: billing disabled'),
      ),
    ).toBe('billing_error')
  })

  test('returns undefined for a generic non-billing error', () => {
    expect(
      classifyVertexBillingOrQuotaError(err('Vertex Gemini returned HTTP 500')),
    ).toBeUndefined()
    expect(
      classifyVertexBillingOrQuotaError(err('response was not valid JSON')),
    ).toBeUndefined()
  })
})

describe('AgentCL Vertex runner — live modelFn over a stubbed adapter', () => {
  // Exercises the REAL vertex-gemini adapter wiring (no SA-key crypto, no
  // network): inject a token provider + a fake fetch.
  const tokenProvider = () => Effect.succeed('fake-token')

  test('maps a 200 Gemini response to a served outcome with usage', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          candidates: [
            { content: { parts: [{ text: 'ok' }] }, finishReason: 'STOP' },
          ],
          usageMetadata: {
            candidatesTokenCount: 7,
            promptTokenCount: 5,
            totalTokenCount: 12,
          },
        }),
        { status: 200 },
      )) as unknown as typeof fetch
    const effectFn = makeVertexGeminiRunnerEffectFn({
      fetchImpl,
      prompt: 'hello',
      tokenProvider,
    })
    const outcome = await Effect.runPromise(effectFn(0))
    expect(outcome._tag).toBe('served')
    if (outcome._tag === 'served') {
      expect(outcome.servedAdapterId).toBe('vertex-gemini')
      expect(outcome.usage.promptTokens).toBe(5)
      expect(outcome.usage.completionTokens).toBe(7)
      expect(outcome.usage.totalTokens).toBe(12)
    }
  })

  test('maps a 429 to a billing_or_quota_error http_429 outcome', async () => {
    const fetchImpl = (async () =>
      new Response('rate limited', { status: 429 })) as unknown as typeof fetch
    const effectFn = makeVertexGeminiRunnerEffectFn({
      fetchImpl,
      prompt: 'hello',
      tokenProvider,
    })
    const outcome = await Effect.runPromise(effectFn(0))
    expect(outcome).toEqual({
      _tag: 'billing_or_quota_error',
      errorRef: 'http_429',
    })
  })

  test('the loop meters real served usage from the stubbed adapter', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          candidates: [
            { content: { parts: [{ text: 'ok' }] }, finishReason: 'STOP' },
          ],
          usageMetadata: {
            candidatesTokenCount: 1_000_000,
            promptTokenCount: 0,
            totalTokenCount: 1_000_000,
          },
        }),
        { status: 200 },
      )) as unknown as typeof fetch
    const effectFn = makeVertexGeminiRunnerEffectFn({
      fetchImpl,
      prompt: 'hello',
      tokenProvider,
    })
    const receipt = await runAgentClVertexRunnerLoop({
      capUsdCents: DEFAULT_CL_VERTEX_CAP_USD_CENTS,
      maxIterations: 1,
      modelFn: iteration => Effect.runPromise(effectFn(iteration)),
    })
    expect(receipt.iterationsServed).toBe(1)
    // 1M output tokens * $0.30/Mtok = $0.30 = 30 cents.
    expect(receipt.estimatedSpendUsdCents).toBeCloseTo(
      resolveVertexGeminiFlashCost().outputUsdPerMtok * 100,
      4,
    )
  })
})
