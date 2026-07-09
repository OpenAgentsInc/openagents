/**
 * MH-8 helper (Grok-owned economics input): free-window preference order.
 *
 * Does not implement fleet auto dispatch — only the typed preference data
 * MH-8 / FleetAutoPolicy can consume. Uses provisional types if fleet-intents
 * package is not imported (keep this package dependency-free of monorepo
 * cycles).
 */

import type { MarginalCostClass } from "./types.ts"

export type AutoHarnessKind = "codex" | "claude" | "grok"

export type AutoPreferenceInput = {
  readonly freeWindowActive: boolean
  /** Measured full-success concurrency floor for Grok CLI plane. */
  readonly grokMeasuredFullSuccessConcurrency?: number
  /** Soft derate factor (default 0.5 of measured floor). */
  readonly derate?: number
}

export type AutoPreferenceResult = {
  readonly preferenceOrder: readonly AutoHarnessKind[]
  readonly maxConcurrentGrokWorkers: number
  readonly marginalCostClassForGrok: MarginalCostClass
  readonly notes: readonly string[]
}

/**
 * While free-window holds, prefer Grok workers first for soak/fan-out.
 * Otherwise prefer Codex (daily-driver / subscription density), then Claude,
 * then Grok as API-metered.
 */
export function buildFreeWindowAutoPreference(
  input: AutoPreferenceInput,
): AutoPreferenceResult {
  const derate = input.derate ?? 0.5
  const measured = input.grokMeasuredFullSuccessConcurrency ?? 4
  const maxConcurrentGrokWorkers = Math.max(
    1,
    Math.floor(measured * derate),
  )

  if (input.freeWindowActive) {
    return {
      preferenceOrder: ["grok", "codex", "claude"],
      maxConcurrentGrokWorkers,
      marginalCostClassForGrok: "free",
      notes: [
        "Free window active: prefer Grok for parallel fan-out.",
        `Soft cap ${maxConcurrentGrokWorkers} = floor(${measured} * ${derate}) from RL-1 receipt.`,
        "CX-3 / owner dogfood should still pin coder role to Codex explicitly.",
      ],
    }
  }

  return {
    preferenceOrder: ["codex", "claude", "grok"],
    maxConcurrentGrokWorkers: Math.min(maxConcurrentGrokWorkers, 4),
    marginalCostClassForGrok: "api_metered",
    notes: [
      "Free window inactive: Grok is api_metered; prefer Codex then Claude.",
      "Re-run RL probes after free ends — ceilings may change by plane.",
    ],
  }
}
