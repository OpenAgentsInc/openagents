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

export const FleetAutoPolicy = S.Struct({
  schema: S.Literal(FleetAutoPolicySchemaLiteral),
  // Ordered concrete-harness preference; first ready harness within the cost
  // ceiling wins. Dumb by design — no learned routing in v1.
  preferenceOrder: S.Array(FleetHarnessKind),
  // Optional ceiling: skip harnesses whose marginal cost class is more
  // expensive than this. Absent = no ceiling.
  maxMarginalCostClass: S.optional(MarginalCostClass),
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
  }>,
): FleetAutoTargetResolution => {
  const ceilingRank = input.policy.maxMarginalCostClass === undefined
    ? undefined
    : marginalCostClassRank[input.policy.maxMarginalCostClass]

  const evaluationOrder: FleetAutoTargetCandidate[] = []
  for (const harnessKind of input.policy.preferenceOrder) {
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
