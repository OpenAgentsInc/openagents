/**
 * Effect->Promise runner that builds a Gemini `InferenceRequest` from an
 * Autopilot work-order input and drives an injected provider adapter to a
 * `Promise<InferenceResult | undefined>` — i.e. the `runInference` seam that
 * `createVertexGeminiHostedCaller` (autopilot-hosted-gemini-inference-bridge.ts)
 * consumes.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1. The chain was, until now:
 *
 *   work order  ->  ??? (MISSING)  ->  InferenceResult
 *               ->  projectGeminiResultToPublicSafeRefs  (bridge)
 *               ->  HostedGeminiInferenceResult          (executor)
 *               ->  AutopilotWorkExecutionCloseoutRecord (route harness)
 *
 * The "???" was the connective tissue that turns a work-order's REFS into a
 * normalized `InferenceRequest`, runs `adapter.complete` (an Effect), and folds
 * the typed failure channel into a clean `undefined`. This module is exactly
 * that piece.
 *
 * HONEST / INERT BY CONSTRUCTION:
 *   - FLAG-GATED + INERT by default: with `enabled: false` the runner returns
 *     `undefined` and never touches the adapter — exactly the current
 *     production behaviour.
 *   - The adapter is INJECTED, so this module never reaches live Vertex quota by
 *     itself: a caller must hand it a configured `makeVertexGeminiAdapter(...)`.
 *   - It carries NO secrets and NO credentials. The request prompt is built
 *     from the work-order REFS only (assignment/task/objective/work-order refs),
 *     not from any dereferenced task content — so nothing secret enters the
 *     request here. (A live deployment that wants the adapter to act on real
 *     task content must dereference those refs upstream; that resolver is part
 *     of what REMAINS, see the worklog.)
 *   - A typed `InferenceAdapterError` (or any failure) collapses to `undefined`
 *     rather than throwing, matching the "lane could not serve it" contract the
 *     bridge expects.
 *   - It does not settle, spend, pay out, or imply accepted work.
 *
 * It does NOT clear the blocker: this runner is still injected (not wired into
 * the worker dependency graph behind an armed flag), the upstream ref-resolver
 * that feeds the adapter real task content is still missing, and there is no
 * registered-agent production smoke. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import { Effect, Exit } from 'effect'

import type { HostedGeminiInferenceCallerInput } from './autopilot-hosted-gemini-inference-bridge'
import {
  type HostedGeminiRefContentResolver,
  type HostedGeminiResolvedContext,
  resolveHostedGeminiPromptContext,
} from './autopilot-hosted-gemini-content-resolver'
import {
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'

/** Default Gemini model alias the hosted lane requests when none is configured. */
export const DEFAULT_HOSTED_GEMINI_MODEL = 'gemini-3.5-flash'

/** Default output-token ceiling for a hosted Gemini closeout generation. */
export const DEFAULT_HOSTED_GEMINI_MAX_OUTPUT_TOKENS = 1024

// The hosted lane only ever serves cloud-allowed/public placements, so the
// system frame is fixed and refs-only: it instructs the model to produce a
// public-safe closeout for the referenced work without disclosing secrets.
const HOSTED_GEMINI_SYSTEM_INSTRUCTION =
  'You are an OpenAgents Autopilot hosted Gemini worker. Produce a concise, ' +
  'public-safe closeout for the referenced work order using only the ' +
  'referenced task and acceptance criteria. Do not disclose secrets, ' +
  'credentials, prompts, or raw provider tokens.'

export type HostedGeminiRequestRunnerConfig = Readonly<{
  /** Default-OFF arming flag. When false the runner is INERT (returns undefined). */
  enabled: boolean
  /** Injected provider adapter (e.g. `makeVertexGeminiAdapter(...)`). */
  adapter: InferenceProviderAdapter
  /** Requested model alias (defaults to DEFAULT_HOSTED_GEMINI_MODEL). */
  model?: string | undefined
  /** Output-token ceiling (defaults to DEFAULT_HOSTED_GEMINI_MAX_OUTPUT_TOKENS). */
  maxOutputTokens?: number | undefined
  /**
   * Optional INJECTED resolver that dereferences the work order's task +
   * acceptance refs into public-safe content. When provided (and the runner is
   * armed) the request embeds the resolved content; when absent or unresolvable
   * the request keeps the existing refs-only frame.
   */
  resolveRefContent?: HostedGeminiRefContentResolver | undefined
}>

const isNonEmptyRef = (value: string): boolean => value.trim() !== ''

/**
 * Build a normalized, REFS-ONLY `InferenceRequest` from a work-order input.
 * Returns `undefined` when the work order is missing the refs needed to frame a
 * request (no work-order ref or no task ref), so the runner declines cleanly
 * rather than asking the model to act on an empty frame.
 *
 * The request is non-streaming (the executor needs the whole completion to
 * digest it) and carries only public-safe refs in its message content.
 */
export const buildHostedGeminiInferenceRequest = (
  input: HostedGeminiInferenceCallerInput,
  options: Readonly<{
    model: string
    maxOutputTokens: number
    resolvedContext?: HostedGeminiResolvedContext | undefined
  }>,
): InferenceRequest | undefined => {
  if (!isNonEmptyRef(input.workOrderRef) || !isNonEmptyRef(input.taskRef)) {
    return undefined
  }

  const objectiveRefs = input.objectiveRefs.filter(isNonEmptyRef)
  const userLines = [
    `work_order=${input.workOrderRef.trim()}`,
    `assignment=${input.assignmentRef.trim()}`,
    `task=${input.taskRef.trim()}`,
    ...(objectiveRefs.length > 0
      ? [`objectives=${objectiveRefs.join(',')}`]
      : []),
  ]

  // Embed dereferenced, public-safe content (when a resolver supplied it) so the
  // model acts on the real task instead of opaque refs. The content is already
  // secret-scrubbed by the resolver's sanitizer; refs above stay for provenance.
  const resolved = options.resolvedContext
  if (resolved !== undefined) {
    userLines.push('', '--- resolved task content (public-safe) ---')
    userLines.push(`task_content: ${resolved.taskContent}`)
    resolved.objectiveContents.forEach((content, index) => {
      userLines.push(`objective_content[${index}]: ${content}`)
    })
  }

  return {
    messages: [
      { content: HOSTED_GEMINI_SYSTEM_INSTRUCTION, role: 'system' },
      { content: userLines.join('\n'), role: 'user' },
    ],
    model: options.model,
    passthroughParams: { max_tokens: options.maxOutputTokens },
    stream: false,
  }
}

/**
 * Build the `runInference` seam over an injected provider adapter. Wire the
 * result to `VertexGeminiHostedCallerConfig.runInference` to drive real Gemini
 * inference from a work order. Returns `undefined` (never throws) when the
 * runner is un-armed, the work order is unframeable, or the adapter fails.
 */
export const createHostedGeminiRequestRunner = (
  config: HostedGeminiRequestRunnerConfig,
): ((
  input: HostedGeminiInferenceCallerInput,
) => Promise<InferenceResult | undefined>) => {
  const model = config.model ?? DEFAULT_HOSTED_GEMINI_MODEL
  const maxOutputTokens =
    config.maxOutputTokens ?? DEFAULT_HOSTED_GEMINI_MAX_OUTPUT_TOKENS
  return async input => {
    // INERT by default: an un-armed runner behaves exactly like "no inference".
    if (!config.enabled) {
      return undefined
    }
    // Best-effort dereference of the work order's refs into public-safe content;
    // `undefined` (no resolver, or nothing safe resolved) keeps the refs-only frame.
    const resolvedContext =
      config.resolveRefContent === undefined
        ? undefined
        : await resolveHostedGeminiPromptContext(input, config.resolveRefContent)
    const request = buildHostedGeminiInferenceRequest(input, {
      maxOutputTokens,
      model,
      ...(resolvedContext === undefined ? {} : { resolvedContext }),
    })
    if (request === undefined) {
      return undefined
    }
    // Fold the typed failure channel into a clean `undefined`: a provider /
    // transport / config error means "the lane could not serve it", which is
    // precisely the bridge's `undefined` contract — no throw escapes here.
    const exit = await Effect.runPromiseExit(config.adapter.complete(request))
    return Exit.isSuccess(exit) ? exit.value : undefined
  }
}
