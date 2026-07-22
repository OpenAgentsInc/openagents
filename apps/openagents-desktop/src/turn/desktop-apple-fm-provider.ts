import {
  decodeAppleFmRouteOutput,
  makeAppleFmProviderRegistry,
  type AppleFmCompletionTurn,
  type AppleFmReadinessSnapshot,
} from "@openagentsinc/apple-fm-runtime"
import type { TurnProviderCandidate } from "@openagentsinc/agent-runtime-schema"
import type { ProviderRegistryInterface } from "@openagentsinc/agent-turn-runtime"

import type { AppleFmHost } from "../apple-fm-host.ts"
import type { AppleFmTurnResult } from "../apple-fm-contract.ts"
import type { makeThreadStore } from "../thread-store.ts"
import {
  buildOpenAgentsAppleFmPrompt,
  shouldOfferAppleFmDelegation,
  type AppleFmAvailableAgent,
  type AppleFmEnvironmentContext,
} from "./apple-fm-prompt.ts"
import {
  buildCompiledAppleFmPrompt,
  honestChatRelease,
  resolveAppleFmPromptPlan,
  type AppleFmDseRelease,
} from "./dse/release-channel.ts"

/**
 * The host-owned connected-agent snapshot the prompt names. It is resolved
 * lazily at turn time from the SAME main-owned lane readiness the boot sequence
 * and the AFS-04 router use — never renderer input. Absent → the prompt keeps
 * its plain honesty preamble (no invented agents).
 */
export type AppleFmAvailableAgentsSource = () =>
  | ReadonlyArray<AppleFmAvailableAgent>
  | Promise<ReadonlyArray<AppleFmAvailableAgent>>

/**
 * The host-owned ambient-context snapshot the prompt states as fact. Resolved
 * lazily at turn time from main-owned host facts (working directory, platform,
 * app name, public identity `npub`, an injected clock) — never renderer input.
 * Absent or a throw → the prompt omits the context block (plain preamble).
 */
export type AppleFmEnvironmentContextSource = () =>
  | AppleFmEnvironmentContext
  | Promise<AppleFmEnvironmentContext>

/**
 * AFS-02 (#9080): register Apple FM into the AFS-01 `InferenceProviderRegistry`
 * so the shared turn kernel can drive it as a local advisory inference lane.
 *
 * This thin Desktop adapter maps the main-owned `AppleFmHost` supervisor state
 * into the neutral provider config: readiness comes from `host.status()` with
 * NO renderer input, and one bounded read-only completion comes from
 * `host.runTurn`. The neutral `@openagentsinc/apple-fm-runtime` provider adapter
 * converts the text completion into an `AnswerCandidate` and applies the
 * empty/oversized/action-claim refusals. No helper path, URL, token, or raw
 * transport detail crosses into the kernel.
 *
 * The host is resolved lazily (`getHost`) because the Electron main process
 * constructs the supervisor after the kernel composition is installed.
 */
export const APPLE_FM_LOCAL_PROVIDER_REF = "provider.apple_fm.local" as const

/** Map the Desktop IPC turn result into the neutral completion-turn shape. */
const turnResultToCompletion = (result: AppleFmTurnResult): AppleFmCompletionTurn => {
  if (result.outcome !== "completed" || result.text === null) {
    return {
      outcome: "failed",
      usageTruth: result.usageTruth,
      ...(result.failureClass !== null ? { failureClass: result.failureClass } : {}),
    }
  }
  return {
    outcome: "completed",
    text: result.text,
    usageTruth: result.usageTruth,
    ...(result.promptTokens !== null ? { promptTokens: result.promptTokens } : {}),
    ...(result.completionTokens !== null ? { completionTokens: result.completionTokens } : {}),
    ...(result.totalTokens !== null ? { totalTokens: result.totalTokens } : {}),
  }
}

const readinessOf = (host: AppleFmHost | null): AppleFmReadinessSnapshot => {
  if (host === null) return { ready: false, unavailableReason: "not_ready" }
  const status = host.status()
  return status.ready
    ? { ready: true }
    : { ready: false, unavailableReason: status.unavailableReason ?? "not_ready" }
}

type ThreadStore = ReturnType<typeof makeThreadStore>
const LOCAL_ANSWER_CANDIDATE = "apple_fm" as const

const addCounts = (left: number | null, right: number | null): number | null =>
  left === null && right === null ? null : (left ?? 0) + (right ?? 0)

/** Include the private first-stage route cost in a two-stage local answer. */
const combineRoutedAnswerUsage = (
  route: AppleFmTurnResult,
  answer: AppleFmTurnResult,
): AppleFmTurnResult => ({
  ...answer,
  promptTokens: addCounts(route.promptTokens, answer.promptTokens),
  completionTokens: addCounts(route.completionTokens, answer.completionTokens),
  totalTokens: addCounts(route.totalTokens, answer.totalTokens),
})

/**
 * Assemble the honesty-bounded, history-aware prompt on the HOST side. By the
 * time the Apple FM provider starts, the kernel has already appended the user's
 * message to the canonical thread store, so the store carries the full window
 * including the current turn. The renderer never builds this prompt.
 *
 * AFS-09: the honest-answer prompt is gated through a DSE release channel. In
 * SHADOW (the checked-in default) the hand-written prompt is served unchanged;
 * only an explicit canary or promotion serves the compiled artifact's preamble,
 * and a rollback restores the hand-written baseline without an app rebuild.
 */
const honestPromptFor = (
  store: ThreadStore | null,
  threadRef: string,
  fallback: string,
  availableAgents: ReadonlyArray<AppleFmAvailableAgent>,
  environment: AppleFmEnvironmentContext | undefined,
  release: AppleFmDseRelease,
): string => {
  const thread = store?.open(threadRef) ?? null
  const turns = thread === null ? [] : thread.notes
  const window = turns.length > 0 ? turns : [{ role: "user", text: fallback }]
  const plan = resolveAppleFmPromptPlan({ release, requestKey: threadRef })
  return plan.kind === "compiled"
    ? buildCompiledAppleFmPrompt(plan.program, window, environment)
    : buildOpenAgentsAppleFmPrompt(window, availableAgents, environment)
}

export const makeDesktopAppleFmProviderRegistry = (
  getHost: () => AppleFmHost | null,
  getThreadStore: () => ThreadStore | null = () => null,
  getAvailableAgents: AppleFmAvailableAgentsSource = () => [],
  getEnvironmentContext: AppleFmEnvironmentContextSource = () => ({}),
  release: AppleFmDseRelease = honestChatRelease,
): ProviderRegistryInterface =>
  makeAppleFmProviderRegistry({
    providerRef: APPLE_FM_LOCAL_PROVIDER_REF,
    readiness: () => readinessOf(getHost()),
    complete: async (prompt, meta) => {
      const host = getHost()
      if (host === null) return { outcome: "failed", usageTruth: "unknown", failureClass: "not_ready" }
      // Resolve the host-owned connected-agent snapshot so the prompt names the
      // agents that are actually ready and can be delegated to. Fail-soft: if the
      // availability probe throws, fall back to no agents (plain honesty preamble)
      // rather than blocking the local turn.
      let availableAgents: ReadonlyArray<AppleFmAvailableAgent> = []
      try {
        availableAgents = await getAvailableAgents()
      } catch {
        availableAgents = []
      }
      // Resolve the host-owned ambient context so the prompt can answer
      // environment/identity questions with real facts. Fail-soft: if the probe
      // throws, omit the context block rather than blocking the local turn.
      let environment: AppleFmEnvironmentContext | undefined
      try {
        environment = await getEnvironmentContext()
      } catch {
        environment = undefined
      }
      const delegateCandidates = availableAgents
        .filter((agent) => agent.ready && agent.canDelegate)
        .map((agent) => agent.candidate)
      const shouldRoute =
        delegateCandidates.length > 0 && shouldOfferAppleFmDelegation(prompt)
      // Ordinary chat stays on the direct, honesty-bounded OpenAgents path even
      // when delegate lanes are ready. This is an authority decision in main,
      // not a best-effort instruction to the model.
      if (!shouldRoute) {
        const directPrompt = honestPromptFor(
          getThreadStore(),
          meta.threadRef,
          prompt,
          [],
          environment,
          release,
        )
        return turnResultToCompletion(await host.runTurn(directPrompt))
      }

      // An explicit action request uses constrained routing. `apple_fm` is a
      // first-class local route. If selected, the host runs a separate direct
      // answer turn and never exposes the route-control JSON as assistant prose.
      const routeCandidates: ReadonlyArray<TurnProviderCandidate> = [
        LOCAL_ANSWER_CANDIDATE,
        ...delegateCandidates,
      ]
      const routePrompt = honestPromptFor(
        getThreadStore(),
        meta.threadRef,
        prompt,
        availableAgents,
        environment,
        release,
      )
      const routed = await host.runTurn(routePrompt, routeCandidates)
      if (routed.outcome !== "completed" || routed.text === null) {
        return turnResultToCompletion(routed)
      }
      const decision = decodeAppleFmRouteOutput({
        raw: routed.text,
        admittedCandidates: routeCandidates,
      })
      if (
        decision._tag === "Recommendation" &&
        decision.recommendation.candidate !== LOCAL_ANSWER_CANDIDATE
      ) {
        return turnResultToCompletion(routed)
      }
      const directPrompt = honestPromptFor(
        getThreadStore(),
        meta.threadRef,
        prompt,
        [],
        environment,
        release,
      )
      const answer = await host.runTurn(directPrompt)
      return turnResultToCompletion(combineRoutedAnswerUsage(routed, answer))
    },
  })
