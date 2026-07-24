/**
 * FAV-01 (#9111): readiness-gated Full Auto routing.
 *
 * The Desktop BOOT SEQUENCE scan (renderer/boot-sequence.ts) and the Full Auto
 * routing gate (full-auto-routing.ts) already read ONE substrate — the harness
 * lanes and provider lane-capability reports. This module makes that shared
 * truth a bind-time product fact: a per-candidate readiness snapshot the run
 * can record, and a reconciled lane scan that covers every Full-Auto-eligible
 * lane plus an advisory-only marker for Apple FM.
 *
 * It invents no authority. Readiness is projected from the SAME
 * `FullAutoRoutingLaneGate` that `validateFullAutoRoutingPolicy` uses, so the
 * snapshot and the fail-closed bind decision cannot drift. The snapshot is
 * public-safe by construction: lane refs, typed states, and typed reasons
 * only — never prompts, models, tokens, or paths.
 */
import { Schema } from "effect"

import { fullAutoLanePolicy, FULL_AUTO_LANE_POLICIES } from "./full-auto-lane.ts"
import { type FullAutoRoutingCandidate } from "./full-auto-registry.ts"
import { type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"

export const FULL_AUTO_READINESS_SNAPSHOT_SCHEMA =
  "openagents.desktop.full_auto_readiness_snapshot.v1" as const

/** Apple FM participates in the scan as advisory only — it has no Full Auto
 * action lane and no action authority (see FAV-03). */
export const FULL_AUTO_ADVISORY_LANE = "apple-fm" as const

const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
const AccountRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))

/**
 * The settled per-lane readiness at bind time. `checking` means a probe has
 * not yet reported — honest, mirroring the boot scan's rule that a lane with
 * no capability report yet reads `checking`, not `unavailable`.
 */
export const FullAutoLaneReadinessStateSchema = Schema.Literals([
  "available",
  "unavailable",
  "checking",
])
export type FullAutoLaneReadinessState = typeof FullAutoLaneReadinessStateSchema.Type

export const FullAutoCandidateReadinessReasonSchema = Schema.Literals([
  "ready",
  "lane_unknown",
  "lane_not_admitted",
  "lane_not_full_auto_eligible",
])
export type FullAutoCandidateReadinessReason =
  typeof FullAutoCandidateReadinessReasonSchema.Type

export const FullAutoCandidateReadinessSchema = Schema.Struct({
  lane: LaneRef,
  accountRef: Schema.optional(AccountRef),
  state: FullAutoLaneReadinessStateSchema,
  reason: FullAutoCandidateReadinessReasonSchema,
})
export type FullAutoCandidateReadiness = typeof FullAutoCandidateReadinessSchema.Type

export const FULL_AUTO_READINESS_CANDIDATE_LIMIT = 8

export const FullAutoReadinessSnapshotSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_READINESS_SNAPSHOT_SCHEMA),
  boundAt: Schema.String,
  candidates: Schema.Array(FullAutoCandidateReadinessSchema).check(
    Schema.isMaxLength(FULL_AUTO_READINESS_CANDIDATE_LIMIT),
  ),
  /** True only when every candidate is `available`. A run binds only a policy
   * whose candidates all pass — this echoes that decision for the report. */
  allReady: Schema.Boolean,
})
export type FullAutoReadinessSnapshot = typeof FullAutoReadinessSnapshotSchema.Type

export const decodeFullAutoReadinessSnapshot = Schema.decodeUnknownSync(
  FullAutoReadinessSnapshotSchema,
)

/** Whether a lane is still being probed (no settled capability report yet). */
export type FullAutoLaneScanningPredicate = (laneRef: string) => boolean

const NEVER_SCANNING: FullAutoLaneScanningPredicate = () => false

/**
 * Project one candidate's readiness from the shared lane gate. Unlike
 * `validateFullAutoRoutingPolicy` this does NOT short-circuit on the first
 * refusal — every candidate is evaluated so the whole picture is visible.
 */
const candidateReadiness = (
  candidate: FullAutoRoutingCandidate,
  laneGate: FullAutoRoutingLaneGate,
  isScanning: FullAutoLaneScanningPredicate,
): FullAutoCandidateReadiness => {
  const base = candidate.accountRef
    ? { lane: candidate.lane, accountRef: candidate.accountRef }
    : { lane: candidate.lane }
  const admission = laneGate(candidate.lane)
  if (admission === null) {
    return { ...base, state: isScanning(candidate.lane) ? "checking" : "unavailable", reason: "lane_unknown" }
  }
  const lanePolicy = fullAutoLanePolicy(candidate.lane)
  if (lanePolicy === null || !lanePolicy.autoResolveQuestions) {
    return { ...base, state: "unavailable", reason: "lane_not_full_auto_eligible" }
  }
  if (!admission.admitted || !admission.fullAuto) {
    return { ...base, state: "unavailable", reason: "lane_not_admitted" }
  }
  return { ...base, state: "available", reason: "ready" }
}

/**
 * Project the bind-time readiness snapshot for an ordered routing policy. Bind
 * this into the durable run so the report shows which lanes were ready, which
 * were not, and why — the shared truth the boot scan renders, at run start.
 */
export const projectFullAutoReadinessSnapshot = (
  policy: ReadonlyArray<FullAutoRoutingCandidate>,
  laneGate: FullAutoRoutingLaneGate,
  boundAt: string,
  isScanning: FullAutoLaneScanningPredicate = NEVER_SCANNING,
): FullAutoReadinessSnapshot => {
  const candidates = policy.map((candidate) =>
    candidateReadiness(candidate, laneGate, isScanning),
  )
  return {
    schema: FULL_AUTO_READINESS_SNAPSHOT_SCHEMA,
    boundAt,
    candidates,
    allReady: candidates.length > 0 && candidates.every((c) => c.state === "available"),
  }
}

export type FullAutoLaneRole = "action" | "advisory"

export type FullAutoLaneScanEntry = Readonly<{
  lane: string
  role: FullAutoLaneRole
  state: FullAutoLaneReadinessState
  reason: string
}>

/**
 * Reconcile the boot scan set with the Full Auto lane set. The boot scan shows
 * Codex, Claude, Grok, and Apple FM, but not Cursor; the routing lane set has
 * Cursor but not Apple FM. This projection shows EVERY Full-Auto-eligible
 * action lane (from `FULL_AUTO_LANE_POLICIES`, so Cursor is visible) plus Apple
 * FM as an advisory-only entry with no action authority.
 */
export const projectFullAutoLaneScan = (
  laneGate: FullAutoRoutingLaneGate,
  options?: {
    readonly appleFmState?: FullAutoLaneReadinessState
    readonly isScanning?: FullAutoLaneScanningPredicate
  },
): ReadonlyArray<FullAutoLaneScanEntry> => {
  const isScanning = options?.isScanning ?? NEVER_SCANNING
  const action = Object.keys(FULL_AUTO_LANE_POLICIES)
    .sort()
    .map((lane): FullAutoLaneScanEntry => {
      const readiness = candidateReadiness({ lane }, laneGate, isScanning)
      return { lane, role: "action", state: readiness.state, reason: readiness.reason }
    })
  const advisory: FullAutoLaneScanEntry = {
    lane: FULL_AUTO_ADVISORY_LANE,
    role: "advisory",
    state: options?.appleFmState ?? "checking",
    reason: "advisory_only_no_action_authority",
  }
  return [...action, advisory]
}
