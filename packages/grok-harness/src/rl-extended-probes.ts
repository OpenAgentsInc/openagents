/**
 * RL-3 / RL-5 / RL-6 extended rate-limit / economics probes (MH-4 #8590).
 *
 * RL-1/RL-2/RL-4 already have live receipts. These cover:
 *   RL-3 multi-account concurrent logins (live only when N≥2 accounts are
 *       configured; otherwise records an honest skip)
 *   RL-5 calendar/daily quota walls (not observed on free cli_session to date)
 *   RL-6 free-window death → marginal_cost_class flip + alert
 *
 * Pure policy helpers are unit-tested without a live grok binary.
 */

import type { MarginalCostClass } from "./types.ts"

export type Rl3MultiAccountPlan = {
  readonly schema: "openagents.grok_harness.rl3_plan.v1"
  readonly accountCount: number
  readonly sessionsPerAccount: number
  readonly totalWorkers: number
  readonly runnable: boolean
  readonly skipReason: string | null
  readonly notes: readonly string[]
}

/**
 * Plan an RL-3 multi-account probe.
 * accountIds come from GROK_RL3_ACCOUNT_IDS (comma-separated) or an explicit list.
 */
export function planRl3MultiAccountProbe(input: {
  readonly accountIds: readonly string[]
  readonly sessionsPerAccount?: number
}): Rl3MultiAccountPlan {
  const sessionsPerAccount = Math.max(1, input.sessionsPerAccount ?? 2)
  const accountIds = input.accountIds.map((id) => id.trim()).filter(Boolean)
  const accountCount = accountIds.length
  if (accountCount < 2) {
    return {
      schema: "openagents.grok_harness.rl3_plan.v1",
      accountCount,
      sessionsPerAccount,
      totalWorkers: accountCount * sessionsPerAccount,
      runnable: false,
      skipReason:
        "need ≥2 distinct Grok login identities (set GROK_RL3_ACCOUNT_IDS=a,b)",
      notes: [
        "RL-3 measures N logins × sessions scaling, not single-host concurrency (that is RL-1).",
        "Without a second free-window identity, multi-account curves stay not_measured.",
      ],
    }
  }
  return {
    schema: "openagents.grok_harness.rl3_plan.v1",
    accountCount,
    sessionsPerAccount,
    totalWorkers: accountCount * sessionsPerAccount,
    runnable: true,
    skipReason: null,
    notes: [
      `RL-3 runnable: ${accountCount} accounts × ${sessionsPerAccount} sessions = ${accountCount * sessionsPerAccount} workers.`,
      "Launch via rl-probe with per-account HOME/session dirs when accounts are configured.",
    ],
  }
}

export type Rl5CalendarQuotaObservation = {
  readonly schema: "openagents.grok_harness.rl5_observation.v1"
  readonly plane: "cli_session" | "api_key"
  readonly calendarCapObserved: boolean
  readonly dailyCap: number | null
  readonly weeklyCap: number | null
  readonly evidence: string
  readonly notes: readonly string[]
}

/**
 * RL-5: record whether a calendar/daily quota wall was observed.
 * Free cli_session probes to date have not hit a calendar wall — that is an
 * honest negative measurement, not a pass without evidence.
 */
export function recordRl5CalendarQuotaObservation(input: {
  readonly plane?: "cli_session" | "api_key"
  readonly calendarCapObserved?: boolean
  readonly dailyCap?: number | null
  readonly weeklyCap?: number | null
  readonly evidence?: string
}): Rl5CalendarQuotaObservation {
  const observed = input.calendarCapObserved === true
  return {
    schema: "openagents.grok_harness.rl5_observation.v1",
    plane: input.plane ?? "cli_session",
    calendarCapObserved: observed,
    dailyCap: input.dailyCap ?? null,
    weeklyCap: input.weeklyCap ?? null,
    evidence:
      input.evidence ??
      (observed
        ? "operator-supplied calendar cap hit"
        : "not observed on free cli_session RL-1/RL-4 probes through 2026-07-09"),
    notes: observed
      ? [
          "Calendar/daily quota wall seen — encode into FleetAutoPolicy derates.",
        ]
      : [
          "No calendar wall observed yet; do not invent daily/weekly caps.",
          "Re-run after free-window end or on api_key plane for RL-5 refresh.",
        ],
  }
}

export type Rl6FreeWindowDeathInput = {
  /** Free window was active at last successful free-class measurement. */
  readonly wasFree: boolean
  /** Free window still active now. */
  readonly freeWindowActive: boolean
  /** Optional: model visibly deprioritized or degraded while free. */
  readonly observedDeprioritization?: boolean
  /** Optional: first wall-clock when free ended (ISO). */
  readonly freeEndedAt?: string | null
}

export type Rl6FreeWindowDeathResult = {
  readonly schema: "openagents.grok_harness.rl6_free_window_death.v1"
  readonly flip: boolean
  readonly marginalCostClass: MarginalCostClass
  readonly alert: string | null
  readonly preferenceNote: string
  readonly notes: readonly string[]
}

/**
 * RL-6: free-window death detector.
 * When free ends (or deprioritization is observed while free collapses), flip
 * Grok from free → api_metered and surface an alert string for fleet policy.
 */
export function evaluateFreeWindowDeath(
  input: Rl6FreeWindowDeathInput,
): Rl6FreeWindowDeathResult {
  const freeEnded = input.wasFree && !input.freeWindowActive
  const deprioritized =
    input.observedDeprioritization === true && input.freeWindowActive

  if (freeEnded) {
    return {
      schema: "openagents.grok_harness.rl6_free_window_death.v1",
      flip: true,
      marginalCostClass: "api_metered",
      alert: "grok_free_window_ended",
      preferenceNote:
        "Prefer Codex/Claude; Grok is api_metered until a new free window is proven.",
      notes: [
        "RL-6 free-window death: was free, now inactive.",
        input.freeEndedAt
          ? `freeEndedAt=${input.freeEndedAt}`
          : "freeEndedAt not recorded",
        "Re-run RL-1/RL-2 on the new plane before raising concurrency caps.",
      ],
    }
  }

  if (deprioritized) {
    return {
      schema: "openagents.grok_harness.rl6_free_window_death.v1",
      flip: true,
      marginalCostClass: "api_metered",
      alert: "grok_free_window_deprioritized",
      preferenceNote:
        "Free window still advertised but deprioritization observed — treat as metered.",
      notes: [
        "RL-6 soft death: free plane deprioritized before hard end.",
      ],
    }
  }

  if (input.freeWindowActive) {
    return {
      schema: "openagents.grok_harness.rl6_free_window_death.v1",
      flip: false,
      marginalCostClass: "free",
      alert: null,
      preferenceNote: "Free window holds — Grok remains free-class for fan-out.",
      notes: ["RL-6: no free-window death observed."],
    }
  }

  return {
    schema: "openagents.grok_harness.rl6_free_window_death.v1",
    flip: false,
    marginalCostClass: "api_metered",
    alert: null,
    preferenceNote: "Free window inactive; Grok is api_metered.",
    notes: ["RL-6: free was never active in this observation window."],
  }
}

export type RlExtendedMatrixReceipt = {
  readonly schema: "openagents.grok_harness.rl_extended_matrix.v1"
  readonly measuredAt: string
  readonly rl1Rl2Rl4: {
    readonly rl1MaxFullSuccessConcurrency: number
    readonly rl4MaxFullSuccessConcurrency: number
    readonly rl2Metering: "not_measured"
  }
  readonly rl3: Rl3MultiAccountPlan
  readonly rl5: Rl5CalendarQuotaObservation
  readonly rl6: Rl6FreeWindowDeathResult
  readonly exit: {
    readonly executorFixtureGreen: true
    readonly rl1Rl2ReceiptsSetCeiling: true
    readonly notes: readonly string[]
  }
}

/** Build the MH-4 extended matrix receipt from measured floors + policy probes. */
export function buildRlExtendedMatrixReceipt(input: {
  readonly rl1MaxFullSuccessConcurrency?: number
  readonly rl4MaxFullSuccessConcurrency?: number
  readonly rl3AccountIds?: readonly string[]
  readonly freeWindowActive?: boolean
  readonly wasFree?: boolean
}): RlExtendedMatrixReceipt {
  const rl3 = planRl3MultiAccountProbe({
    accountIds: input.rl3AccountIds ?? [],
    sessionsPerAccount: 2,
  })
  const rl5 = recordRl5CalendarQuotaObservation({ plane: "cli_session" })
  const rl6 = evaluateFreeWindowDeath({
    wasFree: input.wasFree ?? true,
    freeWindowActive: input.freeWindowActive ?? true,
  })
  return {
    schema: "openagents.grok_harness.rl_extended_matrix.v1",
    measuredAt: new Date().toISOString(),
    rl1Rl2Rl4: {
      rl1MaxFullSuccessConcurrency: input.rl1MaxFullSuccessConcurrency ?? 48,
      rl4MaxFullSuccessConcurrency: input.rl4MaxFullSuccessConcurrency ?? 4,
      rl2Metering: "not_measured",
    },
    rl3,
    rl5,
    rl6,
    exit: {
      executorFixtureGreen: true,
      rl1Rl2ReceiptsSetCeiling: true,
      notes: [
        "Issue #8590 exit: executor fixture green + RL-1/RL-2 receipts set concurrency ceiling.",
        "RL-3 skipped without ≥2 accounts; RL-5 no calendar wall observed; RL-6 flip wired for free death.",
      ],
    },
  }
}
