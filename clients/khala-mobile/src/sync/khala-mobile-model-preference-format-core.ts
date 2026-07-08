/**
 * MM-F1 mobile wiring: pure display-label helpers for model ids and the
 * typed fallback reasons `resolveModelPreference` reports server-side.
 *
 * CX-4 (#8548) extends this with the per-thread picker's execution-target
 * option list (real connected accounts, never a static Codex/Claude label
 * guess) and the typed, never-silent `auto` notice line.
 */
import type { KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import type {
  KhalaAutoExecutionTargetFallbackReason,
  KhalaAutoExecutionTargetResolution,
  KhalaModelPreference,
  KhalaModelPreferenceAccountSummary,
  KhalaModelPreferenceFallback,
} from "./khala-mobile-model-preference-api"
import type { RuntimeControlIntentTarget } from "./khala-runtime-compose-core"

export const executionTargetDisplayLabel = (targetId: string): string => {
  if (targetId === "auto") return "Auto"
  if (targetId === "gemini") return "Gemini"
  if (targetId === "khala" || targetId === "openagents/khala") return "Khala"
  if (targetId.startsWith("codex:")) return "Your Codex"
  if (targetId.startsWith("claude:")) return "Claude"
  return targetId
    .split(/[-_]/)
    .filter(part => part.length > 0)
    .map(part => (part.length <= 3 ? part.toUpperCase() : part.charAt(0).toUpperCase() + part.slice(1)))
    .join(" ")
}

export const modelDisplayLabel = (modelId: string): string => {
  return executionTargetDisplayLabel(modelId)
}

export const modelPreferenceFallbackMessage = (fallback: KhalaModelPreferenceFallback): string | null => {
  switch (fallback) {
    case "none":
      return null
    case "no_preference_set":
      return null // expected, quiet default — not an error state
    case "preference_unavailable":
      return "Your chosen target isn't available right now, so Khala Code is using the default instead."
    case "default_unavailable":
      return "No execution target is currently available. Try again shortly."
  }
}

export const executionTargetReasonLabel = (
  reason: KhalaAutoExecutionTargetFallbackReason | undefined,
): string => {
  switch (reason) {
    case "account_exhausted":
      return "exhausted"
    case "account_rate_limited":
      return "cooling down"
    case "account_requires_reauth":
      return "needs reconnect"
    case "account_unavailable":
    case undefined:
      return "unavailable"
  }
}

const CODEX_LANE: KhalaRuntimeLane = "codex_app_server"
const CLAUDE_LANE: KhalaRuntimeLane = "claude_pylon"
const HOSTED_LANE: KhalaRuntimeLane = "hosted_khala"

export type KhalaExecutionTargetOption = Readonly<{
  label: string
  target: RuntimeControlIntentTarget
}>

const accountOption = (
  prefix: "claude" | "codex",
  lane: KhalaRuntimeLane,
  account: KhalaModelPreferenceAccountSummary,
): KhalaExecutionTargetOption => ({
  label: account.ready ? account.label : `${account.label} (${executionTargetReasonLabel(account.reason)})`,
  target: { executionTargetId: `${prefix}:${account.accountRefHash}`, lane },
})

/** Maps a resolved concrete target id (what `resolveAutoExecutionTarget`
 * picked server-side) to a dispatchable `{ lane, executionTargetId }` — the
 * "Auto" pill must always carry a CONCRETE target, never the literal `auto`
 * string, since no runtime dispatch consumer understands that literal.
 * Returns `null` for a target id this client doesn't recognize, so a future
 * target kind never gets silently misrouted onto the wrong lane. */
const resolveAutoOptionTarget = (targetId: string): RuntimeControlIntentTarget | null => {
  if (targetId.startsWith("codex:")) return { executionTargetId: targetId, lane: CODEX_LANE }
  if (targetId.startsWith("claude:")) return { executionTargetId: targetId, lane: CLAUDE_LANE }
  if (targetId === "khala" || targetId === "gemini") return { executionTargetId: targetId, lane: HOSTED_LANE }
  return null
}

/** Builds the per-thread composer's execution-target options from a fetched
 * `KhalaModelPreference` — real connected Codex/Claude accounts (labeled,
 * quota-aware) plus Khala and a pre-resolved "Auto" entry. Returns an empty
 * array when nothing is derivable yet (e.g. still loading); callers should
 * fall back to `ChatComposer`'s own built-in default list in that case
 * rather than passing an empty `executionTargets` prop. */
export const buildExecutionTargetOptions = (
  preference: Pick<
    KhalaModelPreference,
    "autoResolution" | "availableTargetIds" | "claudeAccounts" | "codexAccounts"
  >,
): ReadonlyArray<KhalaExecutionTargetOption> => {
  const options: Array<KhalaExecutionTargetOption> = []

  if (preference.availableTargetIds.includes("khala")) {
    options.push({ label: "Khala", target: { executionTargetId: "khala", lane: HOSTED_LANE } })
  }

  const autoEffectiveTargetId = preference.autoResolution?.effectiveTargetId ?? null
  if (preference.availableTargetIds.includes("auto") && autoEffectiveTargetId !== null) {
    const resolvedTarget = resolveAutoOptionTarget(autoEffectiveTargetId)
    if (resolvedTarget !== null) {
      options.push({ label: "Auto", target: resolvedTarget })
    }
  }

  for (const account of preference.codexAccounts ?? []) {
    options.push(accountOption("codex", CODEX_LANE, account))
  }
  for (const account of preference.claudeAccounts ?? []) {
    options.push(accountOption("claude", CLAUDE_LANE, account))
  }

  return options
}

/** The typed, never-silent "what did Auto just do" line — renders whenever
 * `auto` skipped at least one connected account, naming what was skipped,
 * why, and what it fell through to. `null` when there's nothing to report
 * (no resolution yet, or the first candidate was ready). */
export const autoResolutionNoticeMessage = (
  resolution: KhalaAutoExecutionTargetResolution | null | undefined,
): string | null => {
  if (resolution === null || resolution === undefined || resolution.events.length === 0) return null
  const first = resolution.events[0]
  if (first === undefined) return null
  const skippedLabel = executionTargetDisplayLabel(first.targetId)
  const reasonText = executionTargetReasonLabel(first.type)
  const extra = resolution.events.length > 1 ? ` (+${resolution.events.length - 1} more)` : ""
  const resolvedLabel =
    resolution.effectiveTargetId === null ? "nothing available" : executionTargetDisplayLabel(resolution.effectiveTargetId)
  return `Auto skipped ${skippedLabel} (${reasonText})${extra} → using ${resolvedLabel}.`
}
