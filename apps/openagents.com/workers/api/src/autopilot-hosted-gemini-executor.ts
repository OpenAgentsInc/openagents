/**
 * Production hosted Gemini executor binding for the Autopilot route harness.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1: until now the only thing that could be wired to
 * `dependencies.executeReadyWork` for a `hosted_gemini` placement was a test
 * fake. This module is the real, reusable production binding — a pure factory
 * that turns an injected hosted Gemini inference caller into an
 * `AutopilotWorkExecutor`, deriving a deterministic, public-safe execution
 * closeout from the work projection.
 *
 * HONEST / INERT BY CONSTRUCTION:
 *   - It is FLAG-GATED and INERT by default: when `enabled` is false the
 *     executor returns `undefined`, which is exactly the current production
 *     behaviour (no execution, no closeout) — so wiring this in changes nothing
 *     until an operator arms it.
 *   - It carries NO secrets, NO provider credentials, and NO raw model output.
 *     The injected caller returns public-safe REFS only (a response digest ref,
 *     an optional usage ref); every ref the executor emits is re-validated with
 *     the same public-safe guard the route uses, and any unsafe ref aborts the
 *     execution (returns `undefined`) rather than leaking it.
 *   - It does not settle, spend, pay out, or imply accepted work.
 *
 * It does NOT clear the blocker: a live binding still needs a real deployed
 * hosted Gemini inference caller wired into the worker dependency graph plus a
 * registered-agent production smoke. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import type { OpenAgentsAutopilotPrivacyTier } from './autopilot-work-request'
import type {
  AutopilotWorkExecutionCloseoutRecord,
  AutopilotWorkExecutor,
  AutopilotWorkOrderProjection,
} from './autopilot-work-routes'
import { publicSafeExecutionCloseoutRef } from './autopilot-work-routes'

export const HOSTED_GEMINI_RUNNER_KIND = 'hosted_gemini' as const

/**
 * Privacy tiers a cloud-hosted Gemini executor may serve. Anything that
 * promises local/customer/TEE confinement is intentionally excluded: a hosted
 * cloud provider cannot honour those tiers.
 */
const hostedGeminiAllowedPrivacyTiers: ReadonlySet<OpenAgentsAutopilotPrivacyTier> =
  new Set<OpenAgentsAutopilotPrivacyTier>(['cloud_allowed', 'public_beta'])

/**
 * Public-safe result of a single hosted Gemini inference call. The caller MUST
 * NOT return raw model text, prompts, provider tokens, or any secret material —
 * only dereferenceable refs that are safe to persist in a public closeout.
 */
export type HostedGeminiInferenceResult = Readonly<{
  modelRef: string
  responseDigestRef: string
  usageRef?: string
}>

export type HostedGeminiInferenceCaller = (
  input: Readonly<{
    assignmentRef: string
    objectiveRefs: ReadonlyArray<string>
    taskRef: string
    workOrderRef: string
  }>,
) => Promise<HostedGeminiInferenceResult | undefined>

export type HostedGeminiExecutorConfig = Readonly<{
  /** Default-OFF arming flag. When false the executor is INERT (returns undefined). */
  enabled: boolean
  inferenceCaller: HostedGeminiInferenceCaller
}>

const placementServesHostedGemini = (
  work: AutopilotWorkOrderProjection,
): boolean =>
  work.placementDecision.selectedRunnerKind === HOSTED_GEMINI_RUNNER_KIND &&
  work.fallbackLeaseIntents.length > 0 &&
  work.placementPolicy.publicTraceAllowed &&
  hostedGeminiAllowedPrivacyTiers.has(work.placementPolicy.privacyTier)

const resultRefsArePublicSafe = (
  result: HostedGeminiInferenceResult,
): boolean =>
  publicSafeExecutionCloseoutRef(result.modelRef) &&
  publicSafeExecutionCloseoutRef(result.responseDigestRef) &&
  (result.usageRef === undefined ||
    publicSafeExecutionCloseoutRef(result.usageRef))

/**
 * Build the production hosted Gemini `AutopilotWorkExecutor`. Wire the returned
 * function to `dependencies.executeReadyWork` to bind real hosted Gemini
 * execution to the Autopilot route harness for `hosted_gemini` placements.
 */
export const createHostedGeminiWorkExecutor = (
  config: HostedGeminiExecutorConfig,
): AutopilotWorkExecutor =>
  async ({ work }) => {
    // INERT by default: an un-armed binding behaves exactly like "no executor".
    if (!config.enabled) {
      return undefined
    }
    if (!placementServesHostedGemini(work)) {
      return undefined
    }

    const intents = work.fallbackLeaseIntents
    const results: Array<{
      assignmentRef: string
      result: HostedGeminiInferenceResult
    }> = []

    for (const intent of intents) {
      const result = await config.inferenceCaller({
        assignmentRef: intent.assignmentRef,
        objectiveRefs: intent.acceptanceCriteriaRefs,
        taskRef: intent.taskRef,
        workOrderRef: work.workOrderRef,
      })
      // Abort cleanly on any incomplete/unsafe inference rather than emitting a
      // partial or leaky closeout.
      if (result === undefined || !resultRefsArePublicSafe(result)) {
        return undefined
      }
      results.push({ assignmentRef: intent.assignmentRef, result })
    }

    const closeout: AutopilotWorkExecutionCloseoutRecord = {
      assignmentRefs: results.map(entry => entry.assignmentRef),
      closeoutRefs: results.flatMap(entry => [
        `closeout.${entry.assignmentRef}.public_safe_summary_delivered`,
        `closeout.${entry.assignmentRef}.tests_or_blocker_retained`,
      ]),
      proofRefs: results.flatMap(entry => [
        `proof.${entry.assignmentRef}.hosted_gemini_executor`,
        entry.result.modelRef,
        entry.result.responseDigestRef,
      ]),
      resultRefs: intents.flatMap(intent => intent.resultExpectationRefs),
      runnerKind: HOSTED_GEMINI_RUNNER_KIND,
      summaryRefs: results.map(
        entry => `summary.${entry.assignmentRef}.hosted_gemini_closeout`,
      ),
      ...(results.every(entry => entry.result.usageRef !== undefined)
        ? {
            verificationRefs: results.map(
              entry =>
                entry.result.usageRef ??
                `verification.${entry.assignmentRef}.usage_unavailable`,
            ),
          }
        : {}),
    }

    return closeout
  }
