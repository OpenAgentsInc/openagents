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
