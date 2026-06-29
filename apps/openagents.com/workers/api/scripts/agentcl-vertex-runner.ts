// Runnable CL-4b AgentCL Vertex runner CLI (public issue #6788).
//
// This is the operator entrypoint for the $50-bounded Vertex Gemini 3.5 Flash
// AgentCL run. It wraps the enforced loop in `../src/inference/gym/
// agentcl-vertex-runner.ts`. It is NOT part of the unit suite.
//
// MODES
//   --dry-run (default): exercises the FULL enforcement loop with no-network
//     stub model_fns and PROVES, with assertions, that:
//       (a) the loop halts exactly when simulated cumulative spend reaches $50,
//       (b) it never calls again after the halt,
//       (c) it refuses/aborts a simulated GLM-fallback (both a mid-run fallback
//           and a routing plan that carries a non-Vertex lane),
//       (d) the 3 consecutive billing/quota-error breaker trips.
//     Dry-run makes NO network calls and incurs NO spend.
//
//   --live: makes a REAL authorized paid call to gemini-3.5-flash on project
//     openagentsgemini via the registered Vertex Gemini adapter + SA-key token
//     path. GATED: refuses unless CL_VERTEX_ARMED=1 AND a VERTEX_SA_KEY is
//     present, so it can never spend by accident. Even then it runs the SAME
//     enforced loop with the SAME $50 cap and breaker.
//
// ENV
//   CL_VERTEX_ARMED=1            arm the live paid path (required for --live)
//   CL_VERTEX_CAP_USD=50         spend cap in DOLLARS (default 50 => 5000 cents;
//                                a higher value cannot exceed the absolute $50
//                                contract ceiling, it can only lower the cap)
//   CL_VERTEX_ITERATIONS=10      max loop iterations
//   CL_VERTEX_PROMPT="..."       prompt for the live call (owner-private; never
//                                printed)
//   CL_VERTEX_MODEL=gemini-3.5-flash   model id (must route Vertex-only)
//   CL_VERTEX_MAX_TOKENS=256     per-call max output tokens
//   VERTEX_SA_KEY=<sa json>      GCP service-account key (or ~/work/.secrets/vertex.env)
//   VERTEX_PROJECT_ID            override project (default openagentsgemini)
//   VERTEX_LOCATION              override location (default global)
//
// ONE LIVE $50 TRANCHE (run by the owner/overseer, NOT in the build lane):
//   CL_VERTEX_ARMED=1 CL_VERTEX_CAP_USD=50 VERTEX_SA_KEY="$(cat sa.json)" \
//     bun run apps/openagents.com/workers/api/scripts/agentcl-vertex-runner.ts --live
//
// This script NEVER prints the SA key, the prompt body, or any provider payload.
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import {
  AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID,
  DEFAULT_CL_VERTEX_CAP_USD_CENTS,
  makeVertexGeminiRunnerEffectFn,
  priceAgentClVertexCallUsdCents,
  resolveAgentClVertexAdapterPlan,
  resolveVertexGeminiFlashCost,
  runAgentClVertexRunnerLoop,
  type AgentClVertexCallOutcome,
  type AgentClVertexRunnerReceipt,
} from '../src/inference/gym/agentcl-vertex-runner'
import { KHALA_MODEL_ID } from '../src/inference/pricing'
import { type InferenceUsage } from '../src/inference/provider-adapter'

const argv = process.argv.slice(2)
const wantsLive = argv.includes('--live')
const wantsDryRun = argv.includes('--dry-run') || !wantsLive

const numEnv = (name: string, fallback: number): number => {
  const raw = process.env[name]?.trim()
  if (raw === undefined || raw === '') return fallback
  const parsed = Number(raw)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

// CL_VERTEX_CAP_USD is in DOLLARS; convert to cents. Default 50 => 5000 cents.
const capUsdCents = Math.round(numEnv('CL_VERTEX_CAP_USD', 50) * 100)
const maxIterations = Math.max(1, Math.floor(numEnv('CL_VERTEX_ITERATIONS', 10)))
const model =
  process.env.CL_VERTEX_MODEL?.trim() || AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID

const printReceipt = (label: string, receipt: AgentClVertexRunnerReceipt): void => {
  console.log(`[agentcl-vertex-runner] ${label}`)
  console.log(
    `  model=${receipt.model} laneRef=${receipt.laneRef} ` +
      `adapterPlan=[${receipt.adapterPlan.join(', ')}] ` +
      `noFallbackPlanVerified=${receipt.noFallbackPlanVerified}`,
  )
  console.log(
    `  capUsdCents=${receipt.capUsdCents} ($${(receipt.capUsdCents / 100).toFixed(2)}) ` +
      `iterationsRequested=${receipt.iterationsRequested} ` +
      `iterationsAttempted=${receipt.iterationsAttempted} ` +
      `iterationsServed=${receipt.iterationsServed}`,
  )
  console.log(
    `  tokens prompt=${receipt.promptTokens} completion=${receipt.completionTokens} ` +
      `total=${receipt.totalTokens}`,
  )
  console.log(
    `  estimatedSpendUsdCents=${receipt.estimatedSpendUsdCents.toFixed(4)} ` +
      `($${(receipt.estimatedSpendUsdCents / 100).toFixed(4)})`,
  )
  console.log(
    `  http429Count=${receipt.http429Count} ` +
      `billingOrQuotaErrorCount=${receipt.billingOrQuotaErrorCount} ` +
      `consecutiveBillingOrQuotaErrors=${receipt.consecutiveBillingOrQuotaErrors}`,
  )
  console.log(
    `  forbiddenFallbackBlocked=${receipt.forbiddenFallbackBlocked} ` +
      `circuitBreakerTripped=${receipt.circuitBreakerTripped} ` +
      `circuitBreakerReason=${receipt.circuitBreakerReason} ` +
      `abortReason=${receipt.abortReason}`,
  )
  console.log(
    `  AgentCL PG=${receipt.agentClFixtureGains.plasticityGain} ` +
      `SG=${receipt.agentClFixtureGains.stabilityGain} ` +
      `GG=${receipt.agentClFixtureGains.generalizationGain} (fixture-eval)`,
  )
}

const assert = (condition: boolean, message: string): void => {
  if (!condition) {
    console.error(`[agentcl-vertex-runner] DRY-RUN ASSERT FAILED: ${message}`)
    process.exitCode = 1
    throw new Error(message)
  }
  console.log(`  PROOF OK: ${message}`)
}

const loadSaKeyFromSecretsFile = (): string | undefined => {
  const path = join(homedir(), 'work', '.secrets', 'vertex.env')
  try {
    const contents = readFileSync(path, 'utf8')
    for (const line of contents.split('\n')) {
      const match = line.match(/^\s*(?:export\s+)?VERTEX_SA_KEY\s*=\s*(.+)$/u)
      if (match) {
        return match[1].trim().replace(/^["']|["']$/gu, '')
      }
    }
  } catch {
    return undefined
  }
  return undefined
}

const resolveSaKey = (): string | undefined =>
  process.env.VERTEX_SA_KEY?.trim() || loadSaKeyFromSecretsFile()

// Build a deterministic served-call stub whose REAL metered cost (via the real
// price function + real Vertex Gemini 3.5 Flash rates) is exactly `targetCents`.
const stubServedUsageForCents = (targetCents: number): InferenceUsage => {
  const cost = resolveVertexGeminiFlashCost()
  // Use output tokens only; solve completionTokens for the target cents.
  const completionTokens = Math.ceil(
    targetCents / ((cost.outputUsdPerMtok / 1_000_000) * 100),
  )
  return { completionTokens, promptTokens: 0, totalTokens: completionTokens }
}

const runDryRun = async (): Promise<void> => {
  console.log('[agentcl-vertex-runner] MODE=dry-run (no network, no spend)\n')

  // Document the REAL routing decision that makes the no-fallback guarantee hold.
  const vertexPlan = resolveAgentClVertexAdapterPlan(
    AGENTCL_VERTEX_GEMINI_RUNNER_MODEL_ID,
  )
  const khalaPlan = resolveAgentClVertexAdapterPlan(KHALA_MODEL_ID)
  console.log(
    `[routing] gemini-3.5-flash -> [${vertexPlan.join(', ')}] (Vertex-only)`,
  )
  console.log(
    `[routing] ${KHALA_MODEL_ID} -> [${khalaPlan.join(', ')}] (carries fallback)\n`,
  )
  assert(
    vertexPlan.length === 1 && vertexPlan[0] === 'vertex-gemini',
    'gemini-3.5-flash routes ONLY to the vertex-gemini lane (no GLM/free fallback)',
  )
  assert(
    khalaPlan.some(id => id !== 'vertex-gemini'),
    `${KHALA_MODEL_ID} carries non-Vertex lanes, so it is refused by the runner`,
  )

  // (a)+(b) Cap-halt + no-call-after-halt. Each stub call costs exactly half the
  // cap, so the 2nd call lands cumulative spend exactly AT the cap and the loop
  // must halt with NO 3rd call.
  console.log('\n[scenario A] cap-halt at exactly $50 (per-call = cap/2):')
  const perCallUsage = stubServedUsageForCents(capUsdCents / 2)
  let capCalls = 0
  const capReceipt = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model,
    modelFn: async (): Promise<AgentClVertexCallOutcome> => {
      capCalls += 1
      return {
        _tag: 'served',
        finishReason: 'stop',
        servedAdapterId: 'vertex-gemini',
        usage: perCallUsage,
      }
    },
  })
  printReceipt('scenario A receipt', capReceipt)
  assert(
    capReceipt.abortReason === 'spend_cap_exceeded',
    'loop halts with abortReason=spend_cap_exceeded',
  )
  assert(
    capReceipt.estimatedSpendUsdCents >= capUsdCents,
    `accumulated spend (${capReceipt.estimatedSpendUsdCents.toFixed(2)}c) >= cap (${capUsdCents}c)`,
  )
  assert(
    capReceipt.iterationsServed === 2 && capReceipt.iterationsAttempted === 2,
    'exactly 2 calls served then HARD STOP (no 3rd call after the cap)',
  )
  assert(
    capCalls === 2 && capCalls < maxIterations,
    `model_fn invoked exactly 2 times, not the full ${maxIterations} iterations`,
  )

  // (c1) Mid-run GLM fallback is refused.
  console.log('\n[scenario B] mid-run GLM fallback refused:')
  let fallbackCalls = 0
  const fallbackReceipt = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model,
    modelFn: async (): Promise<AgentClVertexCallOutcome> => {
      fallbackCalls += 1
      return { _tag: 'fallback_attempted', toLaneRef: 'glm-free' }
    },
  })
  printReceipt('scenario B receipt', fallbackReceipt)
  assert(
    fallbackReceipt.abortReason === 'forbidden_fallback' &&
      fallbackReceipt.forbiddenFallbackBlocked,
    'a simulated GLM fallback aborts the loop (forbidden_fallback)',
  )
  assert(
    fallbackReceipt.estimatedSpendUsdCents === 0 && fallbackCalls === 1,
    'no spend on a refused fallback; aborts on first attempt',
  )

  // (c2) A served-by-non-Vertex-adapter is refused too.
  console.log('\n[scenario C] served by non-Vertex adapter refused:')
  const servedElsewhere = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model,
    modelFn: async (): Promise<AgentClVertexCallOutcome> => ({
      _tag: 'served',
      finishReason: 'stop',
      servedAdapterId: 'openrouter-khala-glm-fallback',
      usage: perCallUsage,
    }),
  })
  printReceipt('scenario C receipt', servedElsewhere)
  assert(
    servedElsewhere.abortReason === 'forbidden_fallback' &&
      servedElsewhere.estimatedSpendUsdCents === 0,
    'a call served by a non-vertex-gemini adapter aborts with no spend',
  )

  // (c3) Pre-flight: a model whose routing plan carries a fallback lane is
  // refused before ANY call.
  console.log('\n[scenario D] pre-flight refuse of a fallback-carrying model:')
  let preflightCalls = 0
  const preflightReceipt = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model: KHALA_MODEL_ID,
    modelFn: async (): Promise<AgentClVertexCallOutcome> => {
      preflightCalls += 1
      return {
        _tag: 'served',
        finishReason: 'stop',
        servedAdapterId: 'vertex-gemini',
        usage: perCallUsage,
      }
    },
  })
  printReceipt('scenario D receipt', preflightReceipt)
  assert(
    preflightReceipt.abortReason === 'no_fallback_plan_refused' &&
      preflightCalls === 0,
    'a fallback-carrying model is refused with ZERO calls made',
  )

  // (d) 3 consecutive billing/quota errors trip the breaker.
  console.log('\n[scenario E] 3-error billing/quota breaker:')
  let errCalls = 0
  const breakerReceipt = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model,
    modelFn: async (): Promise<AgentClVertexCallOutcome> => {
      errCalls += 1
      return { _tag: 'billing_or_quota_error', errorRef: 'http_429' }
    },
  })
  printReceipt('scenario E receipt', breakerReceipt)
  assert(
    breakerReceipt.abortReason === 'consecutive_billing_or_quota_errors',
    'loop halts on the 3-consecutive-billing/quota-error breaker',
  )
  assert(
    breakerReceipt.iterationsAttempted === 3 &&
      breakerReceipt.http429Count === 3 &&
      errCalls === 3,
    'breaker trips at exactly 3 errors (no 4th call)',
  )

  console.log('\n[agentcl-vertex-runner] DRY-RUN: ALL PROOFS PASSED')
}

const runLive = async (): Promise<void> => {
  if (process.env.CL_VERTEX_ARMED !== '1') {
    console.error(
      '[agentcl-vertex-runner] REFUSED: --live requires CL_VERTEX_ARMED=1 ' +
        '(paid-safe guard). No call made.',
    )
    process.exitCode = 2
    return
  }
  const serviceAccountKey = resolveSaKey()
  if (serviceAccountKey === undefined || serviceAccountKey === '') {
    console.error(
      '[agentcl-vertex-runner] REFUSED: no VERTEX_SA_KEY in env or ' +
        '~/work/.secrets/vertex.env. No call made.',
    )
    process.exitCode = 2
    return
  }
  const plan = resolveAgentClVertexAdapterPlan(model)
  if (!(plan.length === 1 && plan[0] === 'vertex-gemini')) {
    console.error(
      `[agentcl-vertex-runner] REFUSED: model=${model} routes to ` +
        `[${plan.join(', ')}] which would allow a non-Vertex fallback. No call made.`,
    )
    process.exitCode = 2
    return
  }

  const prompt =
    process.env.CL_VERTEX_PROMPT?.trim() ||
    'Reply with exactly: OPENAGENTS AGENTCL VERTEX OK'
  const maxTokens = Math.max(1, Math.floor(numEnv('CL_VERTEX_MAX_TOKENS', 256)))

  console.log(
    `[agentcl-vertex-runner] MODE=live ARMED model=${model} ` +
      `cap=$${(capUsdCents / 100).toFixed(2)} maxIterations=${maxIterations}`,
  )
  const effectFn = makeVertexGeminiRunnerEffectFn({
    location: process.env.VERTEX_LOCATION?.trim(),
    maxTokens,
    model,
    project: process.env.VERTEX_PROJECT_ID?.trim(),
    prompt,
    serviceAccountKey,
  })
  // Effect->Promise bridge stays at the runnable edge (this CLI), not in the
  // domain module.
  const receipt = await runAgentClVertexRunnerLoop({
    capUsdCents,
    maxIterations,
    model,
    modelFn: iteration => Effect.runPromise(effectFn(iteration)),
  })
  printReceipt('LIVE receipt', receipt)
}

const main = async (): Promise<void> => {
  console.log(
    `[agentcl-vertex-runner] cap=$${(capUsdCents / 100).toFixed(2)} ` +
      `(${capUsdCents}c, default ${DEFAULT_CL_VERTEX_CAP_USD_CENTS}c) ` +
      `priceBasis=${JSON.stringify(resolveVertexGeminiFlashCost())} ` +
      `perCallCentExample=${priceAgentClVertexCallUsdCents({
        completionTokens: 256,
        promptTokens: 500,
      }).toFixed(6)}\n`,
  )
  if (wantsLive) {
    await runLive()
    return
  }
  if (wantsDryRun) {
    await runDryRun()
  }
}

void main()
