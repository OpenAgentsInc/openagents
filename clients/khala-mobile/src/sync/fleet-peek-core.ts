import type {
  FleetRunControlAction,
  KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"
import type {
  FleetApprovalEntity,
  FleetHarnessKind,
  FleetRunEntity,
  FleetSteerEntity,
  FleetWorkerEntity,
} from "@openagentsinc/khala-sync"

/**
 * MH-6 (#8585) mobile fleet-peek CORE — the PURE, renderer-free half of the
 * phone peek screen: it derives the view-model from projected Sync state and
 * builds the three MH-0 typed steering intents the screen dispatches.
 *
 * The phone is NEVER a second supervisor. It reads the desktop-authority
 * projection (run / per-harness worker cards / pending approvals / steer
 * receipts) and, when the operator taps an action, it builds a
 * `KhalaFleetIntent` value here and hands it to `session.mutate(...)`. All
 * authority-bearing behavior change happens server/desktop-side; this module
 * has no I/O, no clock, and no randomness of its own (both are injected), so
 * it is exhaustively unit-testable.
 */

const FLEET_INTENT_SCHEMA = "khala.fleet_intent.v1" as const

// ---------------------------------------------------------------------------
// View-model derivation
// ---------------------------------------------------------------------------

/** Which concrete harness backs a worker card, resolved defensively. */
export const harnessOfWorker = (
  worker: FleetWorkerEntity,
): FleetHarnessKind | "unknown" => {
  if (worker.harnessKind !== undefined) return worker.harnessKind
  // Fall back to the account lane (`account.pylon.<harness>.<hash>`) for
  // pre-multi-harness post-images that never carried an explicit harnessKind.
  const lane = worker.accountRefHash?.split(".")[2]
  if (lane === "codex" || lane === "claude" || lane === "grok") return lane
  return "unknown"
}

export type FleetWorkerCard = Readonly<{
  workerId: string
  harness: FleetHarnessKind | "unknown"
  phase: FleetWorkerEntity["phase"]
  assignmentRef: string | undefined
  updatedAt: string
}>

export type FleetPeekViewModel = Readonly<{
  runStatus: FleetRunEntity["status"] | "unknown"
  desiredSlots: number
  counters: FleetRunEntity["counters"] | null
  workers: ReadonlyArray<FleetWorkerCard>
  /** Count of worker cards per harness, for the harness pill row. */
  harnessCounts: Readonly<Record<FleetHarnessKind | "unknown", number>>
  pendingApprovals: ReadonlyArray<FleetApprovalEntity>
  resolvedApprovals: ReadonlyArray<FleetApprovalEntity>
  recentSteers: ReadonlyArray<FleetSteerEntity>
  /** The run-control actions that make sense in the current run status. */
  availableRunControls: ReadonlyArray<FleetRunControlAction>
}>

export type FleetPeekInput = Readonly<{
  run: FleetRunEntity | null
  workers: ReadonlyArray<FleetWorkerEntity>
  approvals: ReadonlyArray<FleetApprovalEntity>
  steers: ReadonlyArray<FleetSteerEntity>
}>

const availableRunControlsFor = (
  status: FleetRunEntity["status"] | "unknown",
): ReadonlyArray<FleetRunControlAction> => {
  switch (status) {
    case "running":
      return ["pause", "drain", "stop"]
    case "paused":
      return ["resume", "drain", "stop"]
    case "draining":
      return ["pause", "resume", "stop"]
    case "draft":
      return ["stop"]
    case "stopped":
    case "completed":
      return []
    default:
      return ["pause", "resume", "drain", "stop"]
  }
}

const emptyHarnessCounts = (): Record<FleetHarnessKind | "unknown", number> => ({
  claude: 0,
  codex: 0,
  grok: 0,
  unknown: 0,
})

const byUpdatedDesc = <T extends { readonly updatedAt: string }>(
  a: T,
  b: T,
): number => (a.updatedAt < b.updatedAt ? 1 : a.updatedAt > b.updatedAt ? -1 : 0)

export const deriveFleetPeekViewModel = (
  input: FleetPeekInput,
): FleetPeekViewModel => {
  const workers: ReadonlyArray<FleetWorkerCard> = [...input.workers]
    .sort((a, b) => (a.workerId < b.workerId ? -1 : a.workerId > b.workerId ? 1 : 0))
    .map((w) => ({
      assignmentRef: w.assignmentRef,
      harness: harnessOfWorker(w),
      phase: w.phase,
      updatedAt: w.updatedAt,
      workerId: w.workerId,
    }))

  const harnessCounts = emptyHarnessCounts()
  for (const w of workers) harnessCounts[w.harness] += 1

  const pendingApprovals = input.approvals
    .filter((a) => a.status === "pending")
    .sort(byUpdatedDesc)
  const resolvedApprovals = input.approvals
    .filter((a) => a.status !== "pending")
    .sort(byUpdatedDesc)
  const recentSteers = [...input.steers].sort(byUpdatedDesc)

  const runStatus = input.run?.status ?? "unknown"
  return {
    availableRunControls: availableRunControlsFor(runStatus),
    counters: input.run?.counters ?? null,
    desiredSlots: input.run?.desiredSlots ?? 0,
    harnessCounts,
    pendingApprovals,
    recentSteers,
    resolvedApprovals,
    runStatus,
    workers,
  }
}

// ---------------------------------------------------------------------------
// Typed intent factories (the exact values dispatched over session.mutate)
// ---------------------------------------------------------------------------

/**
 * Injected identity/clock source so intent construction is deterministic and
 * replay-safe (no `Date.now()` / `Math.random()` inside pure code). The screen
 * wires a real source; tests wire a fixture one.
 */
export type FleetIntentIds = Readonly<{
  intentId: string
  idempotencyKey: string
  createdAt: string
}>

export type FleetIntentIdSource = () => FleetIntentIds

const MOBILE_ORIGIN = { surface: "mobile" } as const

export const makeRunControlIntent = (input: {
  runRef: string
  action: FleetRunControlAction
  ids: FleetIntentIds
}): Extract<KhalaFleetIntent, { kind: "fleet_run_control" }> => ({
  action: input.action,
  createdAt: input.ids.createdAt,
  idempotencyKey: input.ids.idempotencyKey,
  intentId: input.ids.intentId,
  kind: "fleet_run_control",
  origin: MOBILE_ORIGIN,
  runRef: input.runRef,
  schema: FLEET_INTENT_SCHEMA,
})

export const makeApprovalDecisionIntent = (input: {
  runRef: string
  approvalRef: string
  decision: "allow" | "deny"
  ids: FleetIntentIds
}): Extract<KhalaFleetIntent, { kind: "approval_decision" }> => ({
  approvalRef: input.approvalRef,
  createdAt: input.ids.createdAt,
  decision: input.decision,
  idempotencyKey: input.ids.idempotencyKey,
  intentId: input.ids.intentId,
  kind: "approval_decision",
  origin: MOBILE_ORIGIN,
  runRef: input.runRef,
  schema: FLEET_INTENT_SCHEMA,
})

export const makeSteerMessageIntent = (input: {
  runRef: string
  body: string
  targetRef?: string
  ids: FleetIntentIds
}): Extract<KhalaFleetIntent, { kind: "steer_message" }> => ({
  body: input.body,
  createdAt: input.ids.createdAt,
  idempotencyKey: input.ids.idempotencyKey,
  intentId: input.ids.intentId,
  kind: "steer_message",
  origin: MOBILE_ORIGIN,
  runRef: input.runRef,
  schema: FLEET_INTENT_SCHEMA,
  ...(input.targetRef === undefined ? {} : { targetRef: input.targetRef }),
})
