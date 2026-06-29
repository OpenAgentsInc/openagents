/**
 * Public-safe bridge from the live Vertex Gemini provider adapter result to the
 * hosted Gemini executor's `HostedGeminiInferenceCaller` seam.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1. The route harness already has:
 *   - a flag-gated production executor binding
 *     (`createHostedGeminiWorkExecutor`, autopilot-hosted-gemini-executor.ts)
 *     that consumes an injected `HostedGeminiInferenceCaller`, and
 *   - a real Vertex Gemini provider adapter
 *     (`makeVertexGeminiAdapter`, inference/vertex-gemini-adapter.ts) that
 *     returns a receipt-first `InferenceResult` (content + usage + servedModel).
 *
 * The MISSING connective tissue was a public-safe projection that turns a real
 * Gemini `InferenceResult` into the executor's REFS-ONLY result contract without
 * leaking the raw model output. This module is exactly that projection plus a
 * thin, INERT-by-default caller factory.
 *
 * HONEST / INERT BY CONSTRUCTION:
 *   - `createVertexGeminiHostedCaller` is FLAG-GATED and INERT by default: with
 *     `enabled: false` it returns `undefined` and never invokes the injected
 *     runner — exactly the current production behaviour.
 *   - It carries NO secrets, NO provider credentials, and NO raw model output.
 *     The completion text is reduced to a SHA-256 digest ref (the plaintext
 *     never leaves this module); only public-safe REFS are returned (a model
 *     ref, a response-digest proof ref, a token-count usage ref). Every emitted
 *     ref is re-validated with the same public-safe guard the route enforces,
 *     and any unsafe ref aborts the projection (returns `undefined`).
 *   - It does not settle, spend, pay out, or imply accepted work.
 *
 * It does NOT clear the blocker: the live Effect->Promise runner that builds the
 * Gemini request from a work order and drives the adapter against real Vertex
 * quota, the worker-dependency-graph wiring behind an armed flag, and a
 * registered-agent production smoke all remain. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import type {
  HostedGeminiInferenceCaller,
  HostedGeminiInferenceResult,
} from './autopilot-hosted-gemini-executor'
import { publicSafeExecutionCloseoutRef } from './autopilot-work-routes'
import type {
  InferenceResult,
  InferenceUsage,
} from './inference/provider-adapter'

/** Input contract of a `HostedGeminiInferenceCaller` (derived, never duplicated). */
export type HostedGeminiInferenceCallerInput =
  Parameters<HostedGeminiInferenceCaller>[0]

/** Computes the digest hex for a completion (injectable for deterministic tests). */
export type HostedGeminiContentDigest = (content: string) => Promise<string>

export type VertexGeminiHostedCallerConfig = Readonly<{
  /** Default-OFF arming flag. When false the caller is INERT (returns undefined). */
  enabled: boolean
  /**
   * Drives the real Gemini inference for a work input and returns the
   * receipt-first provider result, or `undefined` when the lane cannot serve it.
   * This is the (still-missing in production) Effect->Promise runner over
   * `makeVertexGeminiAdapter().complete`; it is injected so this module stays
   * pure, testable, and free of Effect/transport concerns.
   */
  runInference: (
    input: HostedGeminiInferenceCallerInput,
  ) => Promise<InferenceResult | undefined>
  /** Override the content digest (defaults to SHA-256 via WebCrypto). */
  digest?: HostedGeminiContentDigest | undefined
}>

// Collapse an arbitrary provider string into the ref-safe charset
// ([a-z0-9._-]). Returns '' when nothing survives, which the caller treats as
// an unprojectable (and therefore aborted) result.
const sanitizeRefSegment = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/gu, '-')
    .replace(/^[-.]+|[-.]+$/gu, '')

const nonNegInt = (value: number): number =>
  Number.isSafeInteger(value) && value >= 0 ? value : 0

// Token COUNTS only — never any content. Cached prompt tokens are appended when
// the provider reported them so metering can read the cache split from the ref.
const usageRefFromUsage = (usage: InferenceUsage): string => {
  const base =
    `usage.hosted_gemini.prompt_${nonNegInt(usage.promptTokens)}` +
    `.completion_${nonNegInt(usage.completionTokens)}` +
    `.total_${nonNegInt(usage.totalTokens)}`
  return usage.cachedPromptTokens === undefined
    ? base
    : `${base}.cached_${nonNegInt(usage.cachedPromptTokens)}`
}

/**
 * SHA-256 hex digest of the completion text. The raw content is hashed here and
 * never returned, so a public closeout can prove WHICH response was produced
 * without disclosing it.
 */
export const hostedGeminiResponseDigestHex: HostedGeminiContentDigest = async (
  content,
) => {
  const bytes = new TextEncoder().encode(content)
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return Array.from(new Uint8Array(digest))
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Project a real Vertex Gemini `InferenceResult` (+ its precomputed content
 * digest hex) into the executor's public-safe REFS-ONLY result. Returns
 * `undefined` when any derived ref is empty or fails the public-safe guard,
 * rather than emitting a partial or leaky result.
 */
export const projectGeminiResultToPublicSafeRefs = (
  result: InferenceResult,
  digestHex: string,
): HostedGeminiInferenceResult | undefined => {
  const model = sanitizeRefSegment(result.servedModel)
  const digest = sanitizeRefSegment(digestHex)
  if (model === '' || digest === '') {
    return undefined
  }

  const candidate: HostedGeminiInferenceResult = {
    modelRef: `model.hosted_gemini.${model}`,
    responseDigestRef: `proof.hosted_gemini.response_digest.sha256.${digest}`,
    usageRef: usageRefFromUsage(result.usage),
  }

  if (
    !publicSafeExecutionCloseoutRef(candidate.modelRef) ||
    !publicSafeExecutionCloseoutRef(candidate.responseDigestRef) ||
    candidate.usageRef === undefined ||
    !publicSafeExecutionCloseoutRef(candidate.usageRef)
  ) {
    return undefined
  }

  return candidate
}

/**
 * Build a `HostedGeminiInferenceCaller` over an injected Vertex Gemini runner.
 * Wire the result to `HostedGeminiExecutorConfig.inferenceCaller` to feed the
 * production executor binding real, public-safe Gemini inference results.
 */
export const createVertexGeminiHostedCaller = (
  config: VertexGeminiHostedCallerConfig,
): HostedGeminiInferenceCaller => {
  const digest = config.digest ?? hostedGeminiResponseDigestHex
  return async input => {
    // INERT by default: an un-armed caller behaves exactly like "no inference".
    if (!config.enabled) {
      return undefined
    }
    const result = await config.runInference(input)
    if (result === undefined) {
      return undefined
    }
    return projectGeminiResultToPublicSafeRefs(result, await digest(result.content))
  }
}
