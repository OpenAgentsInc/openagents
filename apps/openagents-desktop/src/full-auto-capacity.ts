/**
 * FAV-04 (#9114): Full Auto per-lane capacity ledger and own-capacity-only
 * bounded concurrency.
 *
 * The Full Auto ProductSpec rev 13 (FA-AC-39) already admits up to
 * FULL_AUTO_RUN_ACTIVE_LIMIT independently active runs, and the run registry
 * enforces that total cap. What was missing is per-LANE capacity awareness, so
 * "those first agents at capacity" means saturating READY lanes without
 * oversubscribing a single exhausted or rate-limited account. This module adds
 * that: a typed capacity ledger derived from the SAME lane truth readiness and
 * routing use, plus an admission helper that spreads concurrent runs across
 * distinct available lanes under the existing total cap. It is own-capacity
 * only by construction — it never admits a run onto a busy, cooling, or
 * exhausted lane — so it cannot weaken the FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS
 * `own_capacity_only` invariant.
 */
import { FULL_AUTO_LANE_POLICIES } from "./full-auto-lane.ts"
import { type FullAutoRotationReason } from "./full-auto-registry.ts"
import { FULL_AUTO_RUN_ACTIVE_LIMIT } from "./full-auto-run-registry.ts"
import { type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"

export type FullAutoLaneCapacityState =
  | "available"
  | "busy"
  | "cooling"
  | "exhausted"
  | "unavailable"

export type FullAutoLaneCapacity = Readonly<{
  lane: string
  state: FullAutoLaneCapacityState
  activeRuns: number
  reason: string
}>

export type FullAutoCapacityInputs = Readonly<{
  /** Same lane gate readiness and routing use. */
  laneGate: FullAutoRoutingLaneGate
  /** Count of active runs currently bound to a lane. */
  activeRunsByLane: (lane: string) => number
  /** The most recent typed cooling reason for a lane, or null if none. */
  coolingReasonByLane: (lane: string) => FullAutoRotationReason | null
}>

/**
 * Project the per-lane capacity ledger. State precedence, most-constrained
 * first: a lane that is not ready is `unavailable`; an exhausted account is
 * `exhausted`; a rate-limited/errored lane is `cooling`; a lane with an active
 * run is `busy`; only a ready, idle, un-cooling lane is `available`.
 */
export const projectFullAutoCapacityLedger = (
  inputs: FullAutoCapacityInputs,
): ReadonlyArray<FullAutoLaneCapacity> =>
  Object.keys(FULL_AUTO_LANE_POLICIES)
    .sort()
    .map((lane): FullAutoLaneCapacity => {
      const active = inputs.activeRunsByLane(lane)
      const gate = inputs.laneGate(lane)
      if (gate === null || !gate.admitted || !gate.fullAuto) {
        return { lane, state: "unavailable", activeRuns: active, reason: "lane is not ready" }
      }
      const cooling = inputs.coolingReasonByLane(lane)
      if (cooling === "account_exhausted") {
        return { lane, state: "exhausted", activeRuns: active, reason: "account exhausted" }
      }
      if (cooling === "rate_limited" || cooling === "provider_error") {
        return { lane, state: "cooling", activeRuns: active, reason: cooling }
      }
      if (active > 0) {
        return { lane, state: "busy", activeRuns: active, reason: "a run is active on this lane" }
      }
      return { lane, state: "available", activeRuns: 0, reason: "ready and idle" }
    })

/** The total concurrent-run cap is the existing run-registry cap — this module
 * never raises it, it only decides which lane a permitted run should use. */
export const FULL_AUTO_MAX_CONCURRENT_RUNS = FULL_AUTO_RUN_ACTIVE_LIMIT

export type FullAutoConcurrentRunAdmission =
  | Readonly<{ ok: true; lane: string }>
  | Readonly<{ ok: false; reason: "active_run_limit_reached" | "no_available_lane" }>

/**
 * Admit a new concurrent run onto an available lane, own-capacity-only. It
 * refuses when the total active-run cap is reached, and otherwise returns the
 * first `available` (ready, idle, un-cooling) lane. It never returns a busy,
 * cooling, or exhausted lane, so concurrency spreads across distinct ready
 * lanes rather than oversubscribing one account.
 */
export const admitConcurrentRun = (
  ledger: ReadonlyArray<FullAutoLaneCapacity>,
  currentActiveRuns: number,
  maxConcurrentRuns: number = FULL_AUTO_MAX_CONCURRENT_RUNS,
): FullAutoConcurrentRunAdmission => {
  if (currentActiveRuns >= maxConcurrentRuns) {
    return { ok: false, reason: "active_run_limit_reached" }
  }
  const available = ledger.find((entry) => entry.state === "available")
  if (available === undefined) {
    return { ok: false, reason: "no_available_lane" }
  }
  return { ok: true, lane: available.lane }
}
