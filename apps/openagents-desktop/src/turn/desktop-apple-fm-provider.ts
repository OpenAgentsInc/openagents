import {
  makeAppleFmProviderRegistry,
  type AppleFmCompletionTurn,
  type AppleFmReadinessSnapshot,
} from "@openagentsinc/apple-fm-runtime"
import type { ProviderRegistryInterface } from "@openagentsinc/agent-turn-runtime"

import type { AppleFmHost } from "../apple-fm-host.ts"
import type { AppleFmTurnResult } from "../apple-fm-contract.ts"
import type { makeThreadStore } from "../thread-store.ts"
import { buildOpenAgentsAppleFmPrompt, type AppleFmAvailableAgent } from "./apple-fm-prompt.ts"

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

/**
 * Assemble the honesty-bounded, history-aware prompt on the HOST side. By the
 * time the Apple FM provider starts, the kernel has already appended the user's
 * message to the canonical thread store, so the store carries the full window
 * including the current turn. The renderer never builds this prompt.
 */
const honestPromptFor = (
  store: ThreadStore | null,
  threadRef: string,
  fallback: string,
  availableAgents: ReadonlyArray<AppleFmAvailableAgent>,
): string => {
  const thread = store?.open(threadRef) ?? null
  const turns = thread === null ? [] : thread.notes
  return buildOpenAgentsAppleFmPrompt(turns.length > 0 ? turns : [{ role: "user", text: fallback }], availableAgents)
}

export const makeDesktopAppleFmProviderRegistry = (
  getHost: () => AppleFmHost | null,
  getThreadStore: () => ThreadStore | null = () => null,
  getAvailableAgents: AppleFmAvailableAgentsSource = () => [],
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
      // Build the authoritative, honesty-bounded prompt from the canonical
      // thread history the host owns — never from renderer-supplied prose.
      const honest = honestPromptFor(getThreadStore(), meta.threadRef, prompt, availableAgents)
      return turnResultToCompletion(await host.runTurn(honest))
    },
  })
