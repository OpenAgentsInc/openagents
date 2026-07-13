import { Schema as S } from "effect"

// ---------------------------------------------------------------------------
// @openagentsinc/khala-fleet-intents
//
// The SINGLE typed intent/mutator vocabulary for steering parallel coding
// agents. This is one vocabulary used by BOTH the Effect Native UI (a phone
// card or desktop button dispatches a typed intent) AND Khala Sync (the same
// typed value is the mutator/audit spine) — never two parallel vocabularies.
//
// This package is deliberately NARROW: it depends only on `effect`, so mobile
// and Khala Sync code can depend on it without pulling in any desktop code.
// ---------------------------------------------------------------------------

// --- Economics ------------------------------------------------------------

// Per-harness marginal cost class carried on capacity/account rows. Encodes the
// free-Grok window as DATA so `auto` re-ranks without a code change when the
// free window ends (analysis §11.4 / §12.2 MH-8). `not_measured` is the honest
// value when metering is unavailable — never invent tokens or a cost.
export const MarginalCostClass = S.Literals([
  "free",
  "subscription",
  "api_metered",
  "not_measured",
])
export type MarginalCostClass = typeof MarginalCostClass.Type

export const marginalCostClasses: ReadonlyArray<MarginalCostClass> = [
  "free",
  "subscription",
  "api_metered",
  "not_measured",
]

// --- Harness / worker kinds ------------------------------------------------

// Concrete coding harnesses that actually execute work.
export const FleetHarnessKind = S.Literals(["codex", "claude", "grok"])
export type FleetHarnessKind = typeof FleetHarnessKind.Type

export const fleetHarnessKinds: ReadonlyArray<FleetHarnessKind> = [
  "codex",
  "claude",
  "grok",
]

// Worker selection value: the concrete harnesses plus `auto` (typed dumb policy
// picks a concrete harness at dispatch time).
export const FleetWorkerKind = S.Literals(["codex", "claude", "grok", "auto"])
export type FleetWorkerKind = typeof FleetWorkerKind.Type

export const fleetWorkerKinds: ReadonlyArray<FleetWorkerKind> = [
  "codex",
  "claude",
  "grok",
  "auto",
]

// --- Per-harness session reference (opaque data + capability flags) --------

// Codex, Claude, and Grok all have different thread/session/resume models. The
// shared contract treats a session as OPAQUE string data plus capability flags,
// so no harness-specific session shape leaks into shared types (analysis §9.3 /
// §11.1). Resolving the opaque ref back to a harness session is the harness
// adapter's job, not the shared vocabulary's.
export const HarnessSessionCapabilities = S.Struct({
  resume: S.Boolean,
  fork: S.Boolean,
})
export type HarnessSessionCapabilities = typeof HarnessSessionCapabilities.Type

export const HarnessSessionRef = S.Struct({
  harnessKind: FleetHarnessKind,
  // Opaque, harness-owned session handle. Not parsed by shared code.
  sessionRef: S.String,
  capabilities: HarnessSessionCapabilities,
})
export type HarnessSessionRef = typeof HarnessSessionRef.Type

// --- Typed `auto` policy object (deliberately dumb, fully typed) -----------

export const FleetAutoPolicySchemaLiteral = "khala.fleet_auto_policy.v1" as const

export const FleetAutoDistributionMode = S.Literals([
  "preference_order",
  "spread_across_harnesses",
])
export type FleetAutoDistributionMode = typeof FleetAutoDistributionMode.Type

export const FleetAutoPolicy = S.Struct({
  schema: S.Literal(FleetAutoPolicySchemaLiteral),
  // Ordered concrete-harness preference; first ready harness within the cost
  // ceiling wins. Dumb by design — no learned routing in v1.
  preferenceOrder: S.Array(FleetHarnessKind),
  // Optional ceiling: skip harnesses whose marginal cost class is more
  // expensive than this. Absent = no ceiling.
  maxMarginalCostClass: S.optional(MarginalCostClass),
  // Optional for wire compatibility with the original v1 policy. Omitted
  // policies retain strict preference-order behavior. The compiled default
  // spreads simultaneous work across the least-loaded ready harness kinds
  // before giving any harness a second slot.
  distributionMode: S.optional(FleetAutoDistributionMode),
})
export type FleetAutoPolicy = typeof FleetAutoPolicy.Type

// The compiled-default `auto` policy (MH-8 v1): Codex first, matching the
// analysis doc's "Codex stays default coder for owner daily-driver / CX-3 —
// free Grok does not replace the cloud isolation linchpin" (§11.4 point 4).
// No cost ceiling by default. This is DATA, not a special-cased branch — a
// caller that wants a different fixed order (e.g. a fixture/dogfood fan-out
// run) constructs a different `FleetAutoPolicy` value, optionally via
// `rankFleetHarnessesByCostClass` below.
export const defaultFleetAutoPolicy: FleetAutoPolicy = {
  schema: FleetAutoPolicySchemaLiteral,
  preferenceOrder: ["codex", "claude", "grok"],
  distributionMode: "spread_across_harnesses",
}

// Rank used to compare `MarginalCostClass` values cheapest-first. Exported so
// callers (e.g. the fleet capacity/account-selection surface) can apply the
// same cheapest-first ordering to concrete accounts within one harness
// without duplicating the rank table.
export const marginalCostClassRank: Readonly<Record<MarginalCostClass, number>> = {
  free: 0,
  subscription: 1,
  api_metered: 2,
  not_measured: 3,
}

/**
 * Build a harness preference order from MEASURED cost-class data rather than
 * a hard-coded harness name. `baseOrder` (typically `fleetHarnessKinds`) is
 * stably re-sorted cheapest-`marginalCostClass`-first; a harness missing from
 * `costClassByHarness` is treated as `not_measured` (never assumed free).
 * This is how "bias toward Grok while it's free" is expressed as DATA: when
 * the free window ends, the caller re-measures cost class and this function
 * produces a different order — no code change (analysis §11.4 / §6).
 */
export const rankFleetHarnessesByCostClass = (
  input: Readonly<{
    baseOrder: ReadonlyArray<FleetHarnessKind>
    costClassByHarness: Readonly<Partial<Record<FleetHarnessKind, MarginalCostClass>>>
  }>,
): ReadonlyArray<FleetHarnessKind> =>
  input.baseOrder
    .map((harnessKind, index) => ({ harnessKind, index }))
    .sort((a, b) => {
      const rankA = marginalCostClassRank[input.costClassByHarness[a.harnessKind] ?? "not_measured"]
      const rankB = marginalCostClassRank[input.costClassByHarness[b.harnessKind] ?? "not_measured"]
      return rankA !== rankB ? rankA - rankB : a.index - b.index
    })
    .map(({ harnessKind }) => harnessKind)

// --- v1 typed `auto` policy resolution (MH-8, deliberately dumb) ----------
//
// Mirrors CX-4's `resolveAutoExecutionTarget`
// (apps/openagents.com/workers/api/src/inference/model-preference-store.ts,
// #8548 — explicitly built "reusable for MH-8, not competing with it"): walk
// a FIXED typed preference order, return the first ready candidate, and emit
// one typed fallback event per skip. NEVER a silent substitution and NEVER
// keyword/vibes routing (workspace semantic-routing rule). The one addition
// over CX-4's target-id version is the harness/account shape plus the cost
// ceiling from `FleetAutoPolicy.maxMarginalCostClass`.

export const FleetAutoTargetSkipReason = S.Literals([
  "account_exhausted",
  "account_rate_limited",
  "account_requires_reauth",
  "account_unavailable",
  "cost_ceiling_exceeded",
])
export type FleetAutoTargetSkipReason = typeof FleetAutoTargetSkipReason.Type

export type FleetAutoTargetCandidate = Readonly<{
  harnessKind: FleetHarnessKind
  accountRef: string
  ready: boolean
  // Cost class as DATA on the candidate row (MH-8 economics wiring). Callers
  // that have no measured cost class must pass `"not_measured"` explicitly —
  // never invent a class.
  marginalCostClass: MarginalCostClass
  // Only meaningful when `ready` is false; a not-ready candidate with no
  // reason falls back to `account_unavailable`.
  reason?: FleetAutoTargetSkipReason
}>

export type FleetAutoTargetFallbackEvent = Readonly<{
  type: FleetAutoTargetSkipReason
  harnessKind: FleetHarnessKind
  accountRef: string
  nextHarnessKind: FleetHarnessKind | null
  nextAccountRef: string | null
}>

export type FleetAutoTargetSelection = Readonly<{
  harnessKind: FleetHarnessKind
  accountRef: string
  marginalCostClass: MarginalCostClass
}>

export type FleetAutoTargetResolution = Readonly<{
  // The concrete (harnessKind, accountRef) `auto` resolves to right now, or
  // `null` when every candidate was skipped (the whole policy exhausted).
  selection: FleetAutoTargetSelection | null
  // True whenever the resolution did NOT land on the very first candidate in
  // evaluation order — i.e. at least one skip happened.
  usedFallback: boolean
  // One typed event per skipped candidate, in evaluation order. NEVER empty
  // when `usedFallback` is true — every skip is named, never a silent swap.
  events: ReadonlyArray<FleetAutoTargetFallbackEvent>
}>

export type FleetAutoHarnessCounts = Readonly<Record<FleetHarnessKind, number>>

const zeroFleetAutoHarnessCounts = (): FleetAutoHarnessCounts => ({
  codex: 0,
  claude: 0,
  grok: 0,
})

/**
 * Pure, typed `auto` policy resolver (MH-8 v1). Walks `policy.preferenceOrder`
 * one harness at a time; within a harness, candidates are evaluated
 * cheapest-`marginalCostClass`-first (data-driven bias toward free/cheap
 * accounts, never a hard-coded harness check), preserving input order as a
 * stable tiebreak. Returns the first `ready` candidate whose cost class is at
 * or under `policy.maxMarginalCostClass` (when set), and emits one typed
 * `FleetAutoTargetFallbackEvent` for every candidate skipped along the way —
 * whether skipped for readiness (`account_exhausted` etc, caller-supplied via
 * `reason`) or for exceeding the cost ceiling (`cost_ceiling_exceeded`).
 * Candidates for harnesses not present in `policy.preferenceOrder` are never
 * evaluated and never reported (out of policy scope, not a skip).
 */
export const resolveFleetAutoTarget = (
  input: Readonly<{
    policy: FleetAutoPolicy
    candidates: ReadonlyArray<FleetAutoTargetCandidate>
    // Counts include already-live work plus successful claims made earlier in
    // the same scheduler tick. They are scheduler evidence, not capacity: a
    // missing or unavailable harness still has to fail through its typed
    // candidate rather than being fabricated as ready.
    activeHarnessCounts?: FleetAutoHarnessCounts
  }>,
): FleetAutoTargetResolution => {
  const ceilingRank = input.policy.maxMarginalCostClass === undefined
    ? undefined
    : marginalCostClassRank[input.policy.maxMarginalCostClass]

  const activeHarnessCounts = input.activeHarnessCounts ?? zeroFleetAutoHarnessCounts()
  const preferenceIndex = new Map(
    input.policy.preferenceOrder.map((harnessKind, index) => [harnessKind, index]),
  )
  const harnessEvaluationOrder = input.policy.distributionMode === "spread_across_harnesses"
    ? [...input.policy.preferenceOrder].sort((left, right) =>
        activeHarnessCounts[left] - activeHarnessCounts[right] ||
        (preferenceIndex.get(left) ?? Number.MAX_SAFE_INTEGER) -
          (preferenceIndex.get(right) ?? Number.MAX_SAFE_INTEGER)
      )
    : input.policy.preferenceOrder

  const evaluationOrder: FleetAutoTargetCandidate[] = []
  for (const harnessKind of harnessEvaluationOrder) {
    const group = input.candidates
      .map((candidate, index) => ({ candidate, index }))
      .filter(({ candidate }) => candidate.harnessKind === harnessKind)
      .sort((a, b) => {
        const rankDiff =
          marginalCostClassRank[a.candidate.marginalCostClass] -
          marginalCostClassRank[b.candidate.marginalCostClass]
        return rankDiff !== 0 ? rankDiff : a.index - b.index
      })
      .map(({ candidate }) => candidate)
    evaluationOrder.push(...group)
  }

  const events: FleetAutoTargetFallbackEvent[] = []

  for (let i = 0; i < evaluationOrder.length; i++) {
    const candidate = evaluationOrder[i]
    if (candidate === undefined) continue

    const overCeiling = ceilingRank !== undefined &&
      marginalCostClassRank[candidate.marginalCostClass] > ceilingRank

    if (candidate.ready && !overCeiling) {
      return {
        events,
        selection: {
          accountRef: candidate.accountRef,
          harnessKind: candidate.harnessKind,
          marginalCostClass: candidate.marginalCostClass,
        },
        usedFallback: events.length > 0,
      }
    }

    const next = evaluationOrder[i + 1] ?? null
    events.push({
      accountRef: candidate.accountRef,
      harnessKind: candidate.harnessKind,
      nextAccountRef: next?.accountRef ?? null,
      nextHarnessKind: next?.harnessKind ?? null,
      type: overCeiling ? "cost_ceiling_exceeded" : (candidate.reason ?? "account_unavailable"),
    })
  }

  return { events, selection: null, usedFallback: true }
}

// --- Execution-target routing (FC-4 #8636) ---------------------------------
//
// One typed per-run/per-work-unit placement vocabulary shared by Pylon, the
// server, and the Effect Native clients: work runs on an `owner_local` Pylon
// or a `managed_cloud` Agent Computer under an explicit
// `owner_local | managed_cloud | auto` preference. This mirrors the harness
// `auto` resolver above at the TARGET layer — deliberately dumb, fully typed,
// deterministic — and enforces the FC-4 substitution invariants as data:
//
// - an EXPLICIT target preference never evaluates (and never reports) the
//   other target; it selects its target or is denied, so owner-local
//   subscription capacity can never be silently pooled into a managed lane
//   and a managed authority can never silently consume local capacity;
// - `auto` walks the fixed V1 order owner_local → managed_cloud and emits one
//   typed fallback event per skipped target — never a silent substitution.

export const FleetExecutionTarget = S.Literals(["owner_local", "managed_cloud"])
export type FleetExecutionTarget = typeof FleetExecutionTarget.Type

export const fleetExecutionTargets: ReadonlyArray<FleetExecutionTarget> = [
  "owner_local",
  "managed_cloud",
]

export const FleetExecutionTargetPreference = S.Literals([
  "owner_local",
  "managed_cloud",
  "auto",
])
export type FleetExecutionTargetPreference = typeof FleetExecutionTargetPreference.Type

// The fixed V1 `auto` target order (roadmap R3/FC-4: owner-local is the P0
// daily driver; managed cloud is additive after #8547's accepted receipt).
export const fleetAutoExecutionTargetOrder: ReadonlyArray<FleetExecutionTarget> = [
  "owner_local",
  "managed_cloud",
]

// Why a target was not selected. Bounded and public-safe by construction;
// callers with richer private detail keep it behind their own boundary and
// map to one of these plus an optional fixed public blocker ref.
export const FleetExecutionTargetSkipReason = S.Literals([
  "owner_local_unavailable",
  "owner_local_activation_blocked",
  "managed_cloud_unconfigured",
  "managed_cloud_unavailable",
  "target_capacity_exhausted",
  "target_quota_exhausted",
  "target_cost_ceiling_exceeded",
  "target_data_posture_denied",
  "target_repository_denied",
  "target_task_constraint_denied",
])
export type FleetExecutionTargetSkipReason = typeof FleetExecutionTargetSkipReason.Type

const defaultFleetExecutionTargetSkipReason: Readonly<
  Record<FleetExecutionTarget, FleetExecutionTargetSkipReason>
> = {
  owner_local: "owner_local_unavailable",
  managed_cloud: "managed_cloud_unavailable",
}

export type FleetExecutionTargetCandidate = Readonly<{
  target: FleetExecutionTarget
  ready: boolean
  // Only meaningful when `ready` is false; a not-ready candidate with no
  // reason falls back to the target's honest default unavailability reason.
  reason?: FleetExecutionTargetSkipReason
  // Optional FIXED public-safe blocker ref carried for provenance (for
  // example the exact activation blocker the skip was derived from). Never
  // free-form provider/error text.
  blockerRef?: string
}>

export const FleetWorkUnitDataPosture = S.Literals([
  "owner_private",
  "broker_safe",
])
export type FleetWorkUnitDataPosture = typeof FleetWorkUnitDataPosture.Type

export const FleetWorkUnitQuotaClass = S.Literals([
  "owner_subscription",
  "brokered_credit",
  "either",
])
export type FleetWorkUnitQuotaClass = typeof FleetWorkUnitQuotaClass.Type

export const FleetWorkUnitRepositoryConstraint = S.Literals([
  "owner_local_allowed",
  "managed_allowed",
  "either",
])
export type FleetWorkUnitRepositoryConstraint =
  typeof FleetWorkUnitRepositoryConstraint.Type

export const FleetWorkUnitTaskConstraint = S.Literals([
  "local_ok",
  "managed_required",
])
export type FleetWorkUnitTaskConstraint = typeof FleetWorkUnitTaskConstraint.Type

/** Durable, refs-only policy evaluated independently for each work unit. */
export const FleetWorkUnitPlacementPolicy = S.Struct({
  targetPreference: FleetExecutionTargetPreference,
  quotaClass: FleetWorkUnitQuotaClass,
  maxMarginalCostClass: MarginalCostClass,
  dataPosture: FleetWorkUnitDataPosture,
  repositoryConstraint: FleetWorkUnitRepositoryConstraint,
  taskConstraint: FleetWorkUnitTaskConstraint,
})
export type FleetWorkUnitPlacementPolicy =
  typeof FleetWorkUnitPlacementPolicy.Type

export const defaultFleetWorkUnitPlacementPolicy = (
  targetPreference: FleetExecutionTargetPreference,
): FleetWorkUnitPlacementPolicy => ({
  targetPreference,
  quotaClass: "either",
  maxMarginalCostClass: "not_measured",
  dataPosture: "broker_safe",
  repositoryConstraint: "either",
  taskConstraint: "local_ok",
})

export type FleetWorkUnitPlacementCandidate = Readonly<{
  target: FleetExecutionTarget
  ready: boolean
  availableCapacity: number
  quotaAvailable: boolean
  marginalCostClass: MarginalCostClass
  acceptsDataPosture: boolean
  acceptsRepository: boolean
  acceptsTask: boolean
  blockerRef?: string
}>

const quotaMatchesTarget = (
  quotaClass: FleetWorkUnitQuotaClass,
  target: FleetExecutionTarget,
): boolean => quotaClass === "either" ||
  (quotaClass === "owner_subscription" && target === "owner_local") ||
  (quotaClass === "brokered_credit" && target === "managed_cloud")

/**
 * Convert complete private eligibility facts into the bounded target resolver.
 * The first failed constraint is retained as a typed public-safe reason.
 */
export const resolveFleetWorkUnitPlacement = (input: Readonly<{
  policy: FleetWorkUnitPlacementPolicy
  candidates: ReadonlyArray<FleetWorkUnitPlacementCandidate>
}>): FleetExecutionTargetDecision => resolveFleetExecutionTarget({
  preference: input.policy.targetPreference,
  candidates: input.candidates.map(candidate => {
    let reason: FleetExecutionTargetSkipReason | undefined
    if (!candidate.ready || candidate.availableCapacity < 1) {
      reason = "target_capacity_exhausted"
    } else if (!candidate.quotaAvailable || !quotaMatchesTarget(input.policy.quotaClass, candidate.target)) {
      reason = "target_quota_exhausted"
    } else if (
      marginalCostClassRank[candidate.marginalCostClass] >
        marginalCostClassRank[input.policy.maxMarginalCostClass]
    ) {
      reason = "target_cost_ceiling_exceeded"
    } else if (!candidate.acceptsDataPosture) {
      reason = "target_data_posture_denied"
    } else if (!candidate.acceptsRepository) {
      reason = "target_repository_denied"
    } else if (!candidate.acceptsTask) {
      reason = "target_task_constraint_denied"
    }
    return {
      target: candidate.target,
      ready: reason === undefined,
      ...(reason === undefined ? {} : { reason }),
      ...(candidate.blockerRef === undefined ? {} : { blockerRef: candidate.blockerRef }),
    }
  }),
})

export type FleetExecutionTargetRoutingEvent = Readonly<{
  target: FleetExecutionTarget
  disposition: "selected" | "skipped"
  // Present exactly when `disposition` is "skipped".
  reason?: FleetExecutionTargetSkipReason
  blockerRef?: string
  // The target evaluated next after a skip, or null when the policy is
  // exhausted. Always null on the selected event.
  nextTarget: FleetExecutionTarget | null
}>

export const FleetExecutionTargetDecisionSchemaLiteral =
  "khala.fleet_execution_target_decision.v1" as const

export type FleetExecutionTargetDecision = Readonly<{
  schema: typeof FleetExecutionTargetDecisionSchemaLiteral
  preference: FleetExecutionTargetPreference
  outcome: "selected" | "denied"
  // The concrete target this decision landed on, or null when denied.
  selectedTarget: FleetExecutionTarget | null
  // True whenever at least one candidate was skipped before the outcome.
  usedFallback: boolean
  // One entry per evaluated target in evaluation order — the complete typed
  // eligibility/selection/denial/fallback history for this decision. Targets
  // outside the preference's plan are NEVER evaluated and NEVER reported.
  history: ReadonlyArray<FleetExecutionTargetRoutingEvent>
}>

/**
 * Pure, deterministic execution-target resolver (FC-4 #8636 V1).
 *
 * The evaluation plan is derived from the preference alone: an explicit
 * `owner_local` or `managed_cloud` preference evaluates ONLY that target;
 * `auto` evaluates `fleetAutoExecutionTargetOrder`. Within the plan the first
 * `ready` candidate is selected; every earlier candidate produces one typed
 * skip event. A target in the plan with no candidate row is skipped with the
 * target's default unavailability reason — absence is a typed denial, never a
 * fabricated readiness. When no candidate is selected the decision is
 * `denied` with the full history retained.
 */
export const resolveFleetExecutionTarget = (
  input: Readonly<{
    preference: FleetExecutionTargetPreference
    candidates: ReadonlyArray<FleetExecutionTargetCandidate>
  }>,
): FleetExecutionTargetDecision => {
  const plan: ReadonlyArray<FleetExecutionTarget> = input.preference === "auto"
    ? fleetAutoExecutionTargetOrder
    : [input.preference]

  const history: FleetExecutionTargetRoutingEvent[] = []

  for (let index = 0; index < plan.length; index += 1) {
    const target = plan[index] as FleetExecutionTarget
    // First candidate row per target wins; V1 has at most one row per target.
    const candidate = input.candidates.find(row => row.target === target)
    if (candidate?.ready === true) {
      history.push({ target, disposition: "selected", nextTarget: null })
      return {
        schema: FleetExecutionTargetDecisionSchemaLiteral,
        preference: input.preference,
        outcome: "selected",
        selectedTarget: target,
        usedFallback: history.length > 1,
        history,
      }
    }
    history.push({
      target,
      disposition: "skipped",
      reason: candidate?.reason ?? defaultFleetExecutionTargetSkipReason[target],
      ...(candidate?.blockerRef === undefined ? {} : { blockerRef: candidate.blockerRef }),
      nextTarget: plan[index + 1] ?? null,
    })
  }

  return {
    schema: FleetExecutionTargetDecisionSchemaLiteral,
    preference: input.preference,
    outcome: "denied",
    selectedTarget: null,
    usedFallback: history.length > 0,
    history,
  }
}

// --- Intent value objects --------------------------------------------------

export const FleetRunControlAction = S.Literals([
  "pause",
  "resume",
  "drain",
  "stop",
])
export type FleetRunControlAction = typeof FleetRunControlAction.Type

export const fleetRunControlActions: ReadonlyArray<FleetRunControlAction> = [
  "pause",
  "resume",
  "drain",
  "stop",
]

export const ApprovalDecisionValue = S.Literals(["allow", "deny"])
export type ApprovalDecisionValue = typeof ApprovalDecisionValue.Type

// --- Intent origin ---------------------------------------------------------

export const KhalaFleetIntentSurface = S.Literals([
  "desktop",
  "mobile",
  "web",
  "cli",
  "server",
  "test_fixture",
])
export type KhalaFleetIntentSurface = typeof KhalaFleetIntentSurface.Type

export const KhalaFleetIntentOrigin = S.Struct({
  surface: KhalaFleetIntentSurface,
  deviceRef: S.optional(S.String),
  userRef: S.optional(S.String),
})
export type KhalaFleetIntentOrigin = typeof KhalaFleetIntentOrigin.Type

// --- The intent union (EN UI intent === Sync mutator) ----------------------

export const KhalaFleetIntentSchemaLiteral = "khala.fleet_intent.v1" as const

export const KhalaFleetIntentKind = S.Literals([
  "fleet_run_control",
  "approval_decision",
  "steer_message",
  "worker_selection",
])
export type KhalaFleetIntentKind = typeof KhalaFleetIntentKind.Type

export const khalaFleetIntentKinds: ReadonlyArray<KhalaFleetIntentKind> = [
  "fleet_run_control",
  "approval_decision",
  "steer_message",
  "worker_selection",
]

// Common envelope shared by every intent variant. `idempotencyKey` makes the
// value safe to apply exactly once as a Sync mutator; `runRef` scopes it to a
// FleetRun where applicable.
const KhalaFleetIntentBase = {
  schema: S.Literal(KhalaFleetIntentSchemaLiteral),
  intentId: S.String,
  createdAt: S.String,
  origin: KhalaFleetIntentOrigin,
  idempotencyKey: S.String,
  runRef: S.optional(S.String),
} as const

export const KhalaFleetIntent = S.Union([
  // pause / resume / drain / stop a FleetRun.
  S.Struct({
    ...KhalaFleetIntentBase,
    kind: S.Literal("fleet_run_control"),
    action: FleetRunControlAction,
    reasonRef: S.optional(S.String),
  }),
  // allow / deny a pending approval (the one Inbox authority surface).
  S.Struct({
    ...KhalaFleetIntentBase,
    kind: S.Literal("approval_decision"),
    approvalRef: S.String,
    decision: ApprovalDecisionValue,
    reasonRef: S.optional(S.String),
  }),
  // steer an in-flight worker/turn with an additional message.
  S.Struct({
    ...KhalaFleetIntentBase,
    kind: S.Literal("steer_message"),
    // Inline body when public-safe, or an opaque ref to stored body material.
    body: S.optional(S.String),
    bodyRef: S.optional(S.String),
    targetRef: S.optional(S.String),
  }),
  // select the harness/worker for a run, optionally with an `auto` policy and
  // an opaque resume/fork session target.
  S.Struct({
    ...KhalaFleetIntentBase,
    kind: S.Literal("worker_selection"),
    workerKind: FleetWorkerKind,
    autoPolicy: S.optional(FleetAutoPolicy),
    session: S.optional(HarnessSessionRef),
  }),
])
export type KhalaFleetIntent = typeof KhalaFleetIntent.Type

export const decodeKhalaFleetIntent = S.decodeUnknownSync(KhalaFleetIntent)
export const encodeKhalaFleetIntent = S.encodeUnknownSync(KhalaFleetIntent)
export const KhalaFleetIntentFromJsonString = S.fromJsonString(KhalaFleetIntent)
export const decodeKhalaFleetIntentJson = S.decodeUnknownSync(
  KhalaFleetIntentFromJsonString,
)

// --- Pylon steering delivery transport (FC-3 #8639) ----------------------

export const FleetSteeringRunRef = S.Trim.check(
  S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u),
)
export type FleetSteeringRunRef = typeof FleetSteeringRunRef.Type

export const FleetSteeringClaimRef = S.Trim.check(
  S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u),
)
export type FleetSteeringClaimRef = typeof FleetSteeringClaimRef.Type

export const FleetSteeringSequence = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
export type FleetSteeringSequence = typeof FleetSteeringSequence.Type

const FleetSteeringIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u),
)

export const FleetSteeringDeliveryIntent = S.Struct({
  seq: FleetSteeringSequence,
  intentId: S.Trim.check(
    S.isMinLength(1),
    S.isMaxLength(160),
    S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
  ),
  intent: KhalaFleetIntent,
  createdAt: FleetSteeringIsoTimestamp,
})
export type FleetSteeringDeliveryIntent =
  typeof FleetSteeringDeliveryIntent.Type

export const FLEET_STEERING_PAGE_MAX_INTENTS = 100 as const
export const FLEET_STEERING_OUTCOME_BATCH_MAX_OUTCOMES = 64 as const

export const FleetSteeringPage = S.Struct({
  ok: S.Literal(true),
  runRef: FleetSteeringRunRef,
  claimRef: FleetSteeringClaimRef,
  intents: S.Array(FleetSteeringDeliveryIntent).check(
    S.isMaxLength(FLEET_STEERING_PAGE_MAX_INTENTS),
  ),
  nextAfter: S.Int.check(
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
  ),
  upToDate: S.Boolean,
})
export type FleetSteeringPage = typeof FleetSteeringPage.Type

export const FleetSteeringOutcomeValue = S.Literals([
  "applied",
  "queued_follow_up",
  "skipped_stale",
  "rejected",
  "failed",
])
export type FleetSteeringOutcomeValue = typeof FleetSteeringOutcomeValue.Type

export const FleetSteeringOutcomeRefSchemaLiteral =
  "openagents.pylon.fleet_steering_outcome.v1" as const

export const fleetSteeringOutcomeRefContent = (input: Readonly<{
  runRef: FleetSteeringRunRef
  claimRef: FleetSteeringClaimRef
  pylonRef: string
  seq: FleetSteeringSequence
  intentId: string
  outcome: FleetSteeringOutcomeValue
  observedAt: string
}>) => ({
  schema: FleetSteeringOutcomeRefSchemaLiteral,
  runRef: input.runRef,
  claimRef: input.claimRef,
  pylonRef: input.pylonRef,
  seq: input.seq,
  intentId: input.intentId,
  outcome: input.outcome,
  observedAt: input.observedAt,
})

export const FleetSteeringOutcomeRefKnownAnswer = {
  canonicalJson:
    '{"claimRef":"claim.sarah_fleet_run.0123456789abcdef01234567","intentId":"intent.sarah.pause.1","observedAt":"2026-07-09T23:00:01.000Z","outcome":"applied","pylonRef":"pylon.test.one","runRef":"fleet_run.sarah.0123456789abcdef0123","schema":"openagents.pylon.fleet_steering_outcome.v1","seq":41}',
  outcomeRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
} as const

export const FleetSteeringOutcome = S.Struct({
  seq: FleetSteeringSequence,
  intentId: FleetSteeringDeliveryIntent.fields.intentId,
  outcome: FleetSteeringOutcomeValue,
  outcomeRef: S.Trim.check(
    S.isPattern(/^outcome\.pylon\.fleet_steering\.[a-f0-9]{24}$/u),
  ),
  observedAt: FleetSteeringIsoTimestamp,
})
export type FleetSteeringOutcome = typeof FleetSteeringOutcome.Type

export const FleetSteeringOutcomeBatch = S.Struct({
  claimRef: FleetSteeringClaimRef,
  outcomes: S.Array(FleetSteeringOutcome).check(
    S.isMinLength(1),
    S.isMaxLength(FLEET_STEERING_OUTCOME_BATCH_MAX_OUTCOMES),
  ),
})
export type FleetSteeringOutcomeBatch = typeof FleetSteeringOutcomeBatch.Type

export const FleetSteeringOutcomeAck = S.Struct({
  ok: S.Literal(true),
  runRef: FleetSteeringRunRef,
  claimRef: FleetSteeringClaimRef,
  outcomes: S.Array(FleetSteeringOutcome).check(
    S.isMinLength(1),
    S.isMaxLength(FLEET_STEERING_OUTCOME_BATCH_MAX_OUTCOMES),
  ),
  storedOutcomeCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  duplicateOutcomeCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
})
export type FleetSteeringOutcomeAck = typeof FleetSteeringOutcomeAck.Type

// A queued follow-up is completed by the exact Pylon only after its private
// control side effect has reached a terminal state. This receipt is purposely
// body-free: no steer text, prompt, command, output, account identity, or
// local path is part of the wire contract.
export const FleetSteeringFollowUpCompletionState = S.Literals([
  "applied",
  "failed",
  "stale",
])
export type FleetSteeringFollowUpCompletionState =
  typeof FleetSteeringFollowUpCompletionState.Type

export const FleetSteeringFollowUpCompletion = S.Struct({
  seq: FleetSteeringSequence,
  intentId: FleetSteeringDeliveryIntent.fields.intentId,
  state: FleetSteeringFollowUpCompletionState,
  completionRef: S.Trim.check(
    S.isPattern(/^completion\.pylon\.fleet_steering\.[a-f0-9]{24}$/u),
  ),
  completedAt: FleetSteeringIsoTimestamp,
})
export type FleetSteeringFollowUpCompletion =
  typeof FleetSteeringFollowUpCompletion.Type

export const FleetSteeringFollowUpCompletionRefSchemaLiteral =
  "openagents.pylon.fleet_steering_follow_up_completion.v1" as const

export const fleetSteeringFollowUpCompletionRefContent = (
  input: Readonly<{
    runRef: FleetSteeringRunRef
    claimRef: FleetSteeringClaimRef
    pylonRef: string
  }> & Omit<FleetSteeringFollowUpCompletion, "completionRef">,
) => ({
  schema: FleetSteeringFollowUpCompletionRefSchemaLiteral,
  runRef: input.runRef,
  claimRef: input.claimRef,
  pylonRef: input.pylonRef,
  seq: input.seq,
  intentId: input.intentId,
  state: input.state,
  completedAt: input.completedAt,
})

export const FleetSteeringFollowUpCompletionRefKnownAnswer = {
  canonicalJson:
    '{"claimRef":"claim.sarah_fleet_run.0123456789abcdef01234567","completedAt":"2026-07-09T23:00:02.000Z","intentId":"intent.sarah.pause.1","pylonRef":"pylon.test.one","runRef":"fleet_run.sarah.0123456789abcdef0123","schema":"openagents.pylon.fleet_steering_follow_up_completion.v1","seq":41,"state":"applied"}',
  completionRef:
    "completion.pylon.fleet_steering.4ac8b06de48bb7311f1c2064",
} as const

export const FleetSteeringFollowUpCompletionBatch = S.Struct({
  claimRef: FleetSteeringClaimRef,
  completions: S.Array(FleetSteeringFollowUpCompletion).check(
    S.isMinLength(1),
    S.isMaxLength(FLEET_STEERING_OUTCOME_BATCH_MAX_OUTCOMES),
  ),
})
export type FleetSteeringFollowUpCompletionBatch =
  typeof FleetSteeringFollowUpCompletionBatch.Type

export const FleetSteeringFollowUpCompletionAck = S.Struct({
  ok: S.Literal(true),
  runRef: FleetSteeringRunRef,
  claimRef: FleetSteeringClaimRef,
  completions: S.Array(FleetSteeringFollowUpCompletion).check(
    S.isMinLength(1),
    S.isMaxLength(FLEET_STEERING_OUTCOME_BATCH_MAX_OUTCOMES),
  ),
  storedCompletionCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  duplicateCompletionCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
})
export type FleetSteeringFollowUpCompletionAck =
  typeof FleetSteeringFollowUpCompletionAck.Type

export const decodeFleetSteeringPage = (input: unknown): FleetSteeringPage =>
  S.decodeUnknownSync(FleetSteeringPage)(input, { onExcessProperty: "error" })
export const decodeFleetSteeringOutcomeBatch = (
  input: unknown,
): FleetSteeringOutcomeBatch =>
  S.decodeUnknownSync(FleetSteeringOutcomeBatch)(input, {
    onExcessProperty: "error",
  })
export const decodeFleetSteeringOutcomeAck = (
  input: unknown,
): FleetSteeringOutcomeAck =>
  S.decodeUnknownSync(FleetSteeringOutcomeAck)(input, {
    onExcessProperty: "error",
  })
export const decodeFleetSteeringFollowUpCompletionBatch = (
  input: unknown,
): FleetSteeringFollowUpCompletionBatch =>
  S.decodeUnknownSync(FleetSteeringFollowUpCompletionBatch)(input, {
    onExcessProperty: "error",
  })
export const decodeFleetSteeringFollowUpCompletionAck = (
  input: unknown,
): FleetSteeringFollowUpCompletionAck =>
  S.decodeUnknownSync(FleetSteeringFollowUpCompletionAck)(input, {
    onExcessProperty: "error",
  })
