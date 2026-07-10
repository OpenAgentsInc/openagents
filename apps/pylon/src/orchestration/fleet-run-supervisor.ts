import { createHash } from "node:crypto"

import { Effect, Schema as S, Scope } from "effect"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"
import {
  defaultFleetAutoPolicy,
  marginalCostClassRank,
  resolveFleetAutoTarget,
  type FleetAutoPolicy,
  type FleetAutoTargetCandidate,
  type FleetAutoTargetFallbackEvent,
  type FleetAutoTargetSkipReason,
} from "@openagentsinc/khala-fleet-intents"
import type { PylonAccountMarginalCostClass as MarginalCostClass } from "@openagentsinc/pylon-core/custody/account-registry"
import { hashPylonAccountRef } from "../account-registry.js"

import type {
  FleetRun,
  OrchestrationTaskStatus,
  PylonOrchestrationStore,
  WorkClaim,
} from "./store.js"
import { isAutoRevivableFleetRun } from "./store.js"
import type {
  WorkPlannerClaimableUnit,
  WorkPlannerOutput,
} from "./work-planner.js"
import { fleetRunTaskIdForClaim } from "./fleet-run-refs.js"
import {
  PylonFleetRunUsageEvidenceCarrierSchema,
  type PylonFleetRunUsageEvidence,
  type PylonFleetRunUsageEvidenceCarrier,
} from "./fleet-run-usage-evidence.js"

export const FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT = 10

// A FleetRun (MH-0) or an individual account may be *labeled* with any
// run-selection worker kind. Concrete kinds with a live executor
// (`codex`, `claude`, `grok`) can be dispatched. `auto` defaults to codex
// at the run level; per-account kinds override for mixed pools.
export type FleetRunSupervisorWorkerKind = "codex" | "claude" | "grok" | "auto"
export type FleetRunSupervisorConcreteWorkerKind = "codex" | "claude" | "grok"

export type FleetRunSupervisorWorkerKindResolution =
  | { readonly available: true; readonly workerKind: FleetRunSupervisorConcreteWorkerKind }
  | { readonly available: false; readonly requestedWorkerKind: FleetRunSupervisorWorkerKind }

// Resolves a run-selection kind to a concrete dispatchable kind.
// NEVER silently substitute an unavailable kind onto another harness.
// `auto` at the run level defaults to `codex`; genuine per-account kinds override it.
// MH-4: `grok` is dispatchable via the Grok headless/ACP worker executor.
export function resolveSupervisorWorkerKind(
  workerKind: FleetRunSupervisorWorkerKind,
): FleetRunSupervisorWorkerKindResolution {
  if (workerKind === "auto") return { available: true, workerKind: "codex" }
  return { available: true, workerKind }
}

export type FleetRunSupervisorAccount = {
  readonly accountRef: string
  readonly advertisedCapacity: number
  readonly cooldownUntil?: Date | string | null
  readonly paused?: boolean
  // Optional per-account worker kind. When present it lets ONE FleetRun hold a
  // MIXED pool (e.g. codex + claude + grok accounts under an `auto` run); when
  // absent the account inherits the run's concrete kind.
  readonly workerKind?: FleetRunSupervisorWorkerKind
  // MH-8 (#8587): DATA-DRIVEN marginal cost class carried on the capacity row.
  // Both concrete selection and the shared `auto` policy treat absence as
  // `"not_measured"`; cost is never inferred from account or harness names.
  readonly marginalCostClass?: MarginalCostClass
  // When this row is retained at zero capacity for `auto` policy visibility,
  // carry the exact bounded reason. It is policy input only: a reasoned row is
  // never dispatchable, even if a malformed producer also advertises slots.
  readonly unavailabilityReason?: FleetAutoTargetSkipReason
}

export type FleetRunSupervisorLifecycleEvent = PylonAssignmentRunLifecycleEvent

const FleetRunSupervisorProjectedPublicRef = S.String.check(
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u),
)

export const FleetRunSupervisorApprovalRequestSchema = S.Struct({
  approvalRef: FleetRunSupervisorProjectedPublicRef,
  assignmentRef: FleetRunSupervisorProjectedPublicRef,
  toolClass: S.String.check(
    S.isMinLength(1),
    S.isMaxLength(64),
    S.isPattern(/^[a-z][a-z0-9_]*$/u),
  ),
})
export type FleetRunSupervisorApprovalRequest =
  typeof FleetRunSupervisorApprovalRequestSchema.Type

export type FleetRunSupervisorDispatchInput = {
  readonly accountRef: string
  readonly claim: WorkClaim
  readonly run: FleetRun
  readonly taskId: string
  readonly workUnit: WorkPlannerClaimableUnit
  // The concrete dispatchable kind the supervisor resolved for the account
  // that claimed this unit. In a mixed-kind FleetRun this is the per-account
  // kind, never the raw run kind, so the runner dispatches to the right harness.
  readonly workerKind: FleetRunSupervisorConcreteWorkerKind
  /** Streams executor lifecycle as it happens; buffered terminal replay is deduplicated. */
  readonly onLifecycle?: ((event: FleetRunSupervisorLifecycleEvent) => void | Promise<void>) | undefined
  /**
   * A real executor calls this only when it has actually paused for a named
   * tool approval. Unattended harnesses must not synthesize this signal.
   */
  readonly onApprovalRequested?: (
    request: FleetRunSupervisorApprovalRequest,
  ) => void | Promise<void>
}

export type FleetRunSupervisorVerificationEvidence =
  | {
    readonly truth: "passed"
    readonly verifierRef: string
    readonly evidenceRefs: readonly string[]
  }
  | {
    readonly truth: "failed"
    readonly verifierRef?: string | undefined
    readonly evidenceRefs: readonly string[]
  }

export type FleetRunSupervisorDispatchResult = {
  readonly assignmentRef: string | null
  readonly lifecycle: readonly FleetRunSupervisorLifecycleEvent[]
  readonly status: "accepted" | "blocked" | "failed" | "completed"
  readonly summary?: string | null
  readonly accountRefHash?: string | null
  readonly closeoutRef?: string | null
  readonly usageEvidence?: PylonFleetRunUsageEvidence | null
  readonly marginalCostClass?: MarginalCostClass | undefined
  readonly verification?: FleetRunSupervisorVerificationEvidence | undefined
  readonly artifactRefs?: readonly string[] | undefined
  readonly proofRefs?: readonly string[] | undefined
  readonly authorityReceiptRefs?: readonly string[] | undefined
}

export type FleetRunSupervisorActiveAssignment = {
  readonly accountRef: string
  readonly claim: WorkClaim
  readonly contextId: string
  readonly taskId: string
}

export type FleetRunSupervisorReconcileResult = FleetRunSupervisorDispatchResult & {
  readonly taskId: string
}

export type FleetRunSupervisorRunner = {
  readonly dispatch: (input: FleetRunSupervisorDispatchInput) => Promise<FleetRunSupervisorDispatchResult>
  readonly reconcile?: (input: {
    readonly activeAssignments: readonly FleetRunSupervisorActiveAssignment[]
    readonly now: Date
    readonly run: FleetRun
  }) => Promise<readonly FleetRunSupervisorReconcileResult[]>
}

export type FleetRunSupervisorPlanner = {
  readonly plan: (input: { readonly run: FleetRun; readonly now: Date }) => Promise<WorkPlannerOutput>
}

export type FleetRunSupervisorCapacity = {
  readonly accounts: (input: { readonly run: FleetRun; readonly now: Date }) => Promise<readonly FleetRunSupervisorAccount[]>
}

export type FleetRunSupervisorClock = {
  readonly now: () => Date
  readonly sleep: (ms: number) => Promise<void>
}

export type FleetRunSupervisorOptions = {
  readonly store: PylonOrchestrationStore
  readonly pylonRef: string
  readonly runRef: string
  readonly planner: FleetRunSupervisorPlanner
  readonly runner: FleetRunSupervisorRunner
  readonly capacity: FleetRunSupervisorCapacity
  readonly clock?: Partial<FleetRunSupervisorClock>
  readonly tickIntervalMs?: number
  readonly claimTtlMs?: number
  readonly maxSpawnPerTick?: number
  readonly awaitDispatches?: boolean
  readonly startImmediately?: boolean
  // The shared fleet-intents policy is the only `auto` selection vocabulary.
  // Production uses the compiled default; fixtures/operators may inject a
  // stricter cost ceiling without changing scheduler code.
  readonly autoPolicy?: FleetAutoPolicy
  readonly onLifecycle?: (event: FleetRunSupervisorObservedEvent) => void | Promise<void>
}

export type FleetRunSupervisorObservedEvent =
  | {
    readonly kind: "tick"
    readonly runRef: string
    readonly activeAssignments: number
    readonly freeSlots: number
  }
  | {
    readonly kind: "dispatch"
    readonly runRef: string
    readonly taskId: string
    readonly claimRef: string
    readonly workUnitRef: string
    readonly accountRef: string
    readonly accountRefHash: string | null
    readonly assignmentRef: string | null
    readonly blockerRefs: readonly string[]
    readonly closeoutRef: string | null
    readonly status: FleetRunSupervisorDispatchResult["status"]
    readonly summary?: string | null
    readonly usageEvidence: PylonFleetRunUsageEvidence | null
    readonly workerKind: FleetRunSupervisorConcreteWorkerKind
    readonly marginalCostClass: MarginalCostClass
    readonly verification: FleetRunSupervisorVerificationEvidence | null
    readonly artifactRefs: readonly string[]
    readonly proofRefs: readonly string[]
    readonly authorityReceiptRefs: readonly string[]
  }
  | {
    readonly kind: "lifecycle"
    readonly runRef: string
    readonly taskId: string
    readonly claimRef: string
    readonly accountRef: string
    readonly event: FleetRunSupervisorLifecycleEvent
  }
  | {
    readonly kind: "approval_requested"
    readonly runRef: string
    readonly taskId: string
    readonly claimRef: string
    readonly approvalRef: string
  }
  | {
    readonly kind: "completed"
    readonly runRef: string
    readonly reason: "backlog_empty" | "drained"
  }
  | {
    readonly kind: "terminal"
    readonly runRef: string
    readonly terminalState: "failed" | "stopped"
    readonly blockerRefs: readonly string[]
  }
  | {
    // Typed skip/fallback for an unavailable worker kind. Emitted when a
    // grok-labeled run (fail closed, `accountRef: null`) or a grok-labeled
    // account in a mixed pool cannot be dispatched. This is the multi-harness
    // law: an unavailable kind is always reported, never silently substituted.
    readonly kind: "skip"
    readonly runRef: string
    readonly reason: "worker_kind_unavailable"
    readonly requestedWorkerKind: FleetRunSupervisorWorkerKind
    readonly accountRef: string | null
    readonly detail: string
  }
  | {
    // Exact public-safe projection of the shared typed auto-policy event. One
    // event is emitted per skipped candidate per tick; no fallback is silent.
    readonly kind: "fallback"
    readonly runRef: string
    readonly policySchema: FleetAutoPolicy["schema"]
    readonly event: FleetAutoTargetFallbackEvent
  }

export type FleetRunSupervisorTickResult = {
  readonly activeAssignments: number
  readonly claimed: number
  readonly dispatched: number
  readonly freeSlots: number
  readonly run: FleetRun
}

export type FleetRunSupervisorHandle = {
  readonly pylonRef: string
  readonly runRef: string
  readonly stop: () => Effect.Effect<void>
  readonly tick: () => Effect.Effect<FleetRunSupervisorTickResult, FleetRunSupervisorError>
}

export class FleetRunSupervisorError extends Error {
  readonly _tag = "FleetRunSupervisorError"
  override readonly cause: unknown

  constructor(message: string, cause?: unknown) {
    super(message)
    this.name = "FleetRunSupervisorError"
    this.cause = cause
  }
}

const activeSupervisors = new Map<string, string>()

const defaultClock: FleetRunSupervisorClock = {
  now: () => new Date(),
  sleep: ms => new Promise(resolve => setTimeout(resolve, ms)),
}

const liveTaskStatuses = new Set<OrchestrationTaskStatus>(["dispatched"])

const terminalStatusForDispatch = (
  result: FleetRunSupervisorDispatchResult,
): Extract<OrchestrationTaskStatus, "completed" | "failed" | "blocked"> | null => {
  if (result.status === "completed") return "completed"
  if (result.status === "failed") return "failed"
  if (result.status === "blocked") return "blocked"
  const terminal = [...result.lifecycle].reverse().find(event => {
    if (event.event === "assignment_run.completed" || event.event === "assignment_run.runtime_failed") return true
    return event.status === "cancelled" || event.status === "rejected" || event.status === "stale" ||
      event.status === "timed-out" || event.status === "closed"
  })
  if (terminal?.event === "assignment_run.completed") return "completed"
  if (terminal?.event === "assignment_run.runtime_failed") return "failed"
  if (terminal?.status === "closed") return "completed"
  if (
    terminal?.status === "cancelled" ||
    terminal?.status === "rejected" ||
    terminal?.status === "stale" ||
    terminal?.status === "timed-out"
  ) return "failed"
  return null
}

const dispatchFailureFor = (
  result: FleetRunSupervisorDispatchResult,
): { readonly blockerRefs: readonly string[]; readonly status: string } => ({
  blockerRefs: [...new Set(result.lifecycle.flatMap(event => event.blockerRefs ?? []))],
  status: result.status,
})

const accountHealthFailureFor = (
  blockerRefs: readonly string[],
  status: string,
): { readonly blockerRefs: readonly string[]; readonly status: string } | undefined => {
  const accountHealthRefs = blockerRefs.filter(ref =>
    /(?:account_(?:exhausted|quota_exhausted|rate_limited|unavailable)|credentials?_revoked|custody_invalid|auth_required)/u
      .test(ref)
  )
  return accountHealthRefs.length === 0
    ? undefined
    : { blockerRefs: accountHealthRefs, status }
}

const emptyUsageEvidenceCarrier: PylonFleetRunUsageEvidenceCarrier = {
  accountRefHash: null,
  closeoutRef: null,
  usageEvidence: null,
}

const usageEvidenceCarrierFor = (
  result: FleetRunSupervisorDispatchResult,
): PylonFleetRunUsageEvidenceCarrier => S.decodeUnknownSync(
  PylonFleetRunUsageEvidenceCarrierSchema,
)({
  accountRefHash: result.accountRefHash ?? null,
  closeoutRef: result.closeoutRef ?? null,
  usageEvidence: result.usageEvidence ?? null,
}, { onExcessProperty: "error" })

const workerKindForTask = (
  store: PylonOrchestrationStore,
  taskId: string,
): FleetRunSupervisorConcreteWorkerKind => {
  const runnerKind = store.getTask(taskId)?.spec.runnerKind
  if (runnerKind === "claude_agent") return "claude"
  if (runnerKind === "grok_cli") return "grok"
  return "codex"
}

type FleetRunTerminalDisposition = {
  readonly blockerRefs: readonly string[]
  readonly carrier: PylonFleetRunUsageEvidenceCarrier
  readonly status: Extract<OrchestrationTaskStatus, "completed" | "failed" | "blocked">
}

const terminalDispositionFor = (
  run: FleetRun,
  result: FleetRunSupervisorDispatchResult,
): FleetRunTerminalDisposition | null => {
  const status = terminalStatusForDispatch(result)
  if (status === null) return null
  const blockerRefs = [...dispatchFailureFor(result).blockerRefs]
  let carrier: PylonFleetRunUsageEvidenceCarrier
  try {
    carrier = usageEvidenceCarrierFor(result)
  } catch {
    return {
      blockerRefs: [...new Set([
        ...blockerRefs,
        "blocker.pylon.fleet_run.usage_evidence_invalid",
      ])],
      carrier: emptyUsageEvidenceCarrier,
      status: "failed",
    }
  }
  const exactAssignment = carrier.usageEvidence?.assignmentRef
  const acceptedSarahRun = run.authorityBinding?.phase === "accepted"
  const terminalEvidenceComplete =
    blockerRefs.length === 0 &&
    result.marginalCostClass !== undefined &&
    result.verification?.truth === "passed" &&
    result.verification.evidenceRefs.length > 0 &&
    (result.artifactRefs?.length ?? 0) > 0 &&
    (result.proofRefs?.length ?? 0) > 0 &&
    (result.authorityReceiptRefs?.length ?? 0) > 0
  if (
    status === "completed" &&
    acceptedSarahRun &&
    (
      carrier.usageEvidence === null ||
      result.assignmentRef === null ||
      exactAssignment !== result.assignmentRef ||
      !terminalEvidenceComplete
    )
  ) {
    return {
      blockerRefs: [...new Set([
        ...blockerRefs,
        terminalEvidenceComplete
          ? "blocker.pylon.fleet_run.usage_evidence_required"
          : "blocker.pylon.fleet_run.terminal_evidence_required",
      ])],
      carrier: emptyUsageEvidenceCarrier,
      status: "failed",
    }
  }
  const terminalBlockerRefs = status === "completed" || blockerRefs.length > 0
    ? blockerRefs
    : ["blocker.pylon.fleet_run.work_failed"]
  return { blockerRefs: terminalBlockerRefs, carrier, status }
}

const isoSafe = (date: Date | string): string =>
  typeof date === "string" ? date : date.toISOString()

const isCoolingDown = (account: FleetRunSupervisorAccount, now: Date): boolean => {
  if (account.cooldownUntil === null || account.cooldownUntil === undefined) return false
  const until = Date.parse(isoSafe(account.cooldownUntil))
  return !Number.isNaN(until) && until > now.getTime()
}

const activeAssignmentsForRun = (store: PylonOrchestrationStore, runRef: string): number =>
  store.listTasks().filter(task => task.spec.fleetRunRef === runRef && liveTaskStatuses.has(task.status)).length

const liveClaimsForRun = (store: PylonOrchestrationStore, runRef: string, now: Date): WorkClaim[] =>
  store.listLiveWorkClaims(now).filter(claim => claim.runRef === runRef)

const activeByAccount = (claims: readonly WorkClaim[]): Map<string, number> => {
  const counts = new Map<string, number>()
  for (const claim of claims) {
    if (claim.state === "closeout") continue
    counts.set(claim.workerAccountRef, (counts.get(claim.workerAccountRef) ?? 0) + 1)
  }
  return counts
}

const collectActiveAssignments = (
  store: PylonOrchestrationStore,
  runRef: string,
): FleetRunSupervisorActiveAssignment[] => {
  const tasks = store.listTasks("dispatched").filter(task => task.spec.fleetRunRef === runRef)
  const claims = new Map(
    store.listWorkClaims({ runRef }).map(claim => [fleetRunTaskIdForClaim(runRef, claim.claimRef), claim]),
  )
  return tasks.flatMap(task => {
    const claim = claims.get(task.id)
    if (claim === undefined || claim.state !== "in_progress") return []
    return [{
      accountRef: claim.workerAccountRef,
      claim,
      contextId: contextIdFor(claim.workerAccountRef, task.id),
      taskId: task.id,
    }]
  })
}

const refreshActiveAssignmentClaims = (
  store: PylonOrchestrationStore,
  runRef: string,
  now: Date,
): void => {
  for (const assignment of collectActiveAssignments(store, runRef)) {
    store.refreshLiveWorkClaim(assignment.claim.workUnitRef, now)
    if (store.getDispatchContext(assignment.contextId) !== null) {
      store.recordHeartbeat(assignment.contextId, { at: now, status: "dispatched" })
    }
  }
}

// Resolve the concrete dispatchable kind for one account under a given run. An
// explicit per-account kind wins (mixed pool); otherwise the account inherits
// the run's kind. MH-4: `grok` is a concrete dispatchable kind.
const resolveAccountWorkerKind = (
  account: FleetRunSupervisorAccount,
  run: FleetRun,
): FleetRunSupervisorWorkerKindResolution =>
  resolveSupervisorWorkerKind(account.workerKind ?? run.workerKind)

// Explicit concrete-harness runs retain cheapest-cost-first account selection.
// `auto` runs do not use this helper: they resolve through the shared typed
// fleet-intents policy below, which owns cross-harness preference, cost
// ceilings, and fallback evidence.
const pickAccount = (
  accounts: readonly FleetRunSupervisorAccount[],
  activeCounts: Map<string, number>,
  now: Date,
): FleetRunSupervisorAccount | null => {
  const eligible = accounts
    .filter(account => account.advertisedCapacity > 0)
    .filter(account => account.unavailabilityReason === undefined)
    .filter(account => account.paused !== true)
    .filter(account => !isCoolingDown(account, now))
    .map(account => ({
      account,
      costRank: marginalCostClassRank[account.marginalCostClass ?? "not_measured"],
      free: Math.max(0, account.advertisedCapacity - (activeCounts.get(account.accountRef) ?? 0)),
    }))
    .filter(entry => entry.free > 0)
    .sort((a, b) =>
      a.costRank - b.costRank ||
      b.free - a.free ||
      a.account.accountRef.localeCompare(b.account.accountRef)
    )
  return eligible[0]?.account ?? null
}

const autoCandidateFor = (
  account: FleetRunSupervisorAccount,
  workerKind: FleetRunSupervisorConcreteWorkerKind,
  activeCounts: ReadonlyMap<string, number>,
  now: Date,
): FleetAutoTargetCandidate => {
  const free = Math.max(
    0,
    account.advertisedCapacity - (activeCounts.get(account.accountRef) ?? 0),
  )
  const reason = account.unavailabilityReason ??
    (isCoolingDown(account, now) ? "account_rate_limited" : "account_unavailable")
  const ready = account.unavailabilityReason === undefined &&
    account.paused !== true &&
    !isCoolingDown(account, now) &&
    free > 0
  return {
    accountRef: account.accountRef,
    harnessKind: workerKind,
    marginalCostClass: account.marginalCostClass ?? "not_measured",
    ready,
    ...(ready ? {} : { reason }),
  }
}

const fallbackEventKey = (event: FleetAutoTargetFallbackEvent): string =>
  `${event.harnessKind}:${event.accountRef}:${event.type}`

const contextIdFor = (accountRef: string, taskId: string): string =>
  `${accountRef}.ctx.${taskId}`.replace(/[^a-zA-Z0-9_.-]/g, "_")

const stableWorkerRef = (input: {
  readonly accountRefHash: string
  readonly pylonRef: string
  readonly workerKind: FleetRunSupervisorConcreteWorkerKind
}): string => `worker.pylon.${input.workerKind}.${createHash("sha256")
  .update(`${input.pylonRef}:${input.accountRefHash}`)
  .digest("hex")
  .slice(0, 24)}`

const claimRefFor = (runRef: string, workUnitRef: string, now: Date, ordinal: number): string =>
  `${runRef}.claim.${workUnitRef.replace(/[^a-zA-Z0-9_.-]/g, "_")}.${now.getTime()}.${ordinal}`

const dependencyTaskIdsFor = (
  store: PylonOrchestrationStore,
  runRef: string,
  workUnit: WorkPlannerClaimableUnit,
): readonly string[] => {
  if (workUnit.dependsOn === undefined || workUnit.dependsOn.length === 0) return []
  const claimsByWorkUnit = new Map(
    store.listWorkClaims({ runRef }).map(claim => [claim.workUnitRef, claim]),
  )
  return workUnit.dependsOn.flatMap((depRef) => {
    const claim = claimsByWorkUnit.get(depRef)
    return claim === undefined ? [] : [fleetRunTaskIdForClaim(runRef, claim.claimRef)]
  })
}

const emit = async (
  sink: FleetRunSupervisorOptions["onLifecycle"],
  event: FleetRunSupervisorObservedEvent,
): Promise<void> => {
  if (sink !== undefined) await sink(event)
}

const emitRunTerminal = async (
  sink: FleetRunSupervisorOptions["onLifecycle"],
  run: FleetRun,
): Promise<void> => {
  if (run.state === "completed") {
    await emit(sink, { kind: "completed", runRef: run.runRef, reason: "drained" })
    return
  }
  if (run.state !== "stopped") return
  const failed = run.stateSource === "reconcile" &&
    (run.counters.failedAssignments > 0 || run.counters.blockedAssignments > 0)
  await emit(sink, {
    kind: "terminal",
    runRef: run.runRef,
    terminalState: failed ? "failed" : "stopped",
    blockerRefs: failed
      ? ["blocker.pylon.fleet_run.local_attempt_failed"]
      : [],
  })
}

const recordTerminalAssignment = async (
  options: FleetRunSupervisorOptions,
  assignment: FleetRunSupervisorActiveAssignment,
  result: FleetRunSupervisorDispatchResult,
  now: Date,
): Promise<boolean> => {
  const run = options.store.getFleetRun(options.runRef)
  if (run === null) return false
  const terminal = terminalDispositionFor(run, result)
  if (terminal === null) return false
  for (const event of result.lifecycle) {
    await emit(options.onLifecycle, {
      kind: "lifecycle",
      runRef: options.runRef,
      taskId: assignment.taskId,
      claimRef: assignment.claim.claimRef,
      accountRef: assignment.accountRef,
      event,
    })
  }
  if (result.assignmentRef !== null) {
    options.store.updateWorkClaimAssignmentRef(assignment.claim.claimRef, result.assignmentRef, now)
  }
  options.store.recordWorkerDone({
    contextId: assignment.contextId,
    taskId: assignment.taskId,
    status: terminal.status,
    result: JSON.stringify({
      assignmentRef: result.assignmentRef,
      summary: result.summary ?? null,
      ...terminal.carrier,
    }),
    ...(terminal.status === "failed" &&
        accountHealthFailureFor(terminal.blockerRefs, result.status) !== undefined
      ? { failure: accountHealthFailureFor(terminal.blockerRefs, result.status) }
      : {}),
    now,
  })
  options.store.updateWorkClaimState(
    assignment.claim.claimRef,
    terminal.status === "completed" ? "closeout" : "released",
    now,
  )
  await emit(options.onLifecycle, {
    kind: "dispatch",
    runRef: options.runRef,
    taskId: assignment.taskId,
    claimRef: assignment.claim.claimRef,
    workUnitRef: assignment.claim.workUnitRef,
    accountRef: assignment.accountRef,
    accountRefHash: terminal.carrier.accountRefHash,
    assignmentRef: result.assignmentRef,
    blockerRefs: terminal.blockerRefs,
    closeoutRef: terminal.carrier.closeoutRef,
    status: terminal.status === "completed"
      ? "completed"
      : terminal.status === "blocked"
        ? "blocked"
        : "failed",
    summary: result.summary ?? null,
    usageEvidence: terminal.carrier.usageEvidence,
    workerKind: workerKindForTask(options.store, assignment.taskId),
    marginalCostClass: assignment.claim.marginalCostClass ?? "not_measured",
    verification: result.verification ?? null,
    artifactRefs: result.artifactRefs ?? [],
    proofRefs: result.proofRefs ?? [],
    authorityReceiptRefs: result.authorityReceiptRefs ?? [],
  })
  return true
}

export async function tickFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Promise<FleetRunSupervisorTickResult> {
  const clock = { ...defaultClock, ...options.clock }
  const now = clock.now()
  const store = options.store
  const maxSpawnPerTick = Math.min(
    Math.max(1, Math.trunc(options.maxSpawnPerTick ?? FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT)),
    FLEET_RUN_SUPERVISOR_MAX_SPAWN_COUNT,
  )
  const claimTtlMs = options.claimTtlMs ?? 30 * 60 * 1000

  refreshActiveAssignmentClaims(store, options.runRef, now)
  store.reconcileWorkClaims({ now })
  const expectedWorkUnitsTotal = store.getFleetRun(options.runRef)?.counters.workUnitsTotal ?? 0
  let run = store.reconcileFleetRun(options.runRef, now)
  // Reconciliation may auto-close a running run whose created tasks are all
  // terminal even though the planner backlog still has uncreated work units.
  // Only that auto-close may be undone: operator lifecycle states (paused,
  // draining, operator-stopped) are authority and must never be auto-revived
  // (#7975).
  if (
    run.state !== "running" &&
    isAutoRevivableFleetRun(run) &&
    run.counters.workUnitsTotal < expectedWorkUnitsTotal
  ) {
    run = store.upsertFleetRun({
      ...run,
      state: "running",
      counters: {
        ...run.counters,
        workUnitsTotal: expectedWorkUnitsTotal,
      },
      updatedAt: now.toISOString(),
    })
  }
  if (run.state !== "running") {
    // An operator stop forbids new dispatch immediately, but an already
    // accepted attempt still owns its claim until terminal reconciliation.
    // Do not publish run_terminal ahead of that residual attempt.
    if (run.state === "stopped") {
      const residual = collectActiveAssignments(store, run.runRef)
      if (options.runner.reconcile !== undefined && residual.length > 0) {
        const activeByTask = new Map(residual.map(assignment => [assignment.taskId, assignment]))
        const reconciled = await options.runner.reconcile({
          activeAssignments: residual,
          now,
          run,
        })
        for (const result of reconciled) {
          const assignment = activeByTask.get(result.taskId)
          if (assignment === undefined) continue
          await recordTerminalAssignment(options, assignment, result, now)
        }
        run = store.reconcileFleetRun(run.runRef, now)
      }
      if (activeAssignmentsForRun(store, run.runRef) === 0) {
        await emitRunTerminal(options.onLifecycle, run)
      }
    } else if (
      run.state === "completed" &&
      activeAssignmentsForRun(store, run.runRef) === 0
    ) {
      await emitRunTerminal(options.onLifecycle, run)
    }
    return {
      activeAssignments: activeAssignmentsForRun(store, run.runRef),
      claimed: 0,
      dispatched: 0,
      freeSlots: 0,
      run,
    }
  }

  // Fail closed when the whole run is labeled with an unavailable kind.
  // Emit a typed skip event and dispatch nothing — never silently substitute.
  const runWorkerKindResolution = resolveSupervisorWorkerKind(run.workerKind)
  if (!runWorkerKindResolution.available) {
    await emit(options.onLifecycle, {
      kind: "skip",
      runRef: run.runRef,
      reason: "worker_kind_unavailable",
      requestedWorkerKind: runWorkerKindResolution.requestedWorkerKind,
      accountRef: null,
      detail:
        `FleetRun worker kind '${run.workerKind}' has no executor yet; failing closed with zero dispatch (no silent substitution).`,
    })
    return {
      activeAssignments: activeAssignmentsForRun(store, run.runRef),
      claimed: 0,
      dispatched: 0,
      freeSlots: 0,
      run,
    }
  }

  const activeBeforeReconcile = collectActiveAssignments(store, run.runRef)
  if (options.runner.reconcile !== undefined && activeBeforeReconcile.length > 0) {
    const activeByTask = new Map(activeBeforeReconcile.map(assignment => [assignment.taskId, assignment]))
    const reconciled = await options.runner.reconcile({ activeAssignments: activeBeforeReconcile, now, run })
    for (const result of reconciled) {
      const assignment = activeByTask.get(result.taskId)
      if (assignment === undefined) continue
      await recordTerminalAssignment(options, assignment, result, now)
    }
    run = store.reconcileFleetRun(options.runRef, now)
    if (
      run.state !== "running" &&
      isAutoRevivableFleetRun(run) &&
      run.counters.workUnitsTotal < expectedWorkUnitsTotal
    ) {
      run = store.upsertFleetRun({
        ...run,
        state: "running",
        counters: {
          ...run.counters,
          workUnitsTotal: expectedWorkUnitsTotal,
        },
        updatedAt: now.toISOString(),
      })
    }
    if (run.state !== "running") {
      await emitRunTerminal(options.onLifecycle, run)
      return {
        activeAssignments: activeAssignmentsForRun(store, run.runRef),
        claimed: 0,
        dispatched: 0,
        freeSlots: 0,
        run,
      }
    }
  }

  const liveClaims = liveClaimsForRun(store, run.runRef, now)
  const rawAccounts = await options.capacity.accounts({ run, now })
  // Partition the (possibly mixed-kind) pool into dispatchable accounts and
  // accounts whose kind has no executor. Unavailable kinds emit typed skips
  // and never silently substitute another harness.
  const accounts: FleetRunSupervisorAccount[] = []
  const accountWorkerKinds = new Map<string, FleetRunSupervisorConcreteWorkerKind>()
  for (const account of rawAccounts) {
    const resolution = resolveAccountWorkerKind(account, run)
    if (!resolution.available) {
      await emit(options.onLifecycle, {
        kind: "skip",
        runRef: run.runRef,
        reason: "worker_kind_unavailable",
        requestedWorkerKind: resolution.requestedWorkerKind,
        accountRef: account.accountRef,
        detail:
          `Account ${account.accountRef} worker kind '${resolution.requestedWorkerKind}' has no executor yet; skipping it without substitution.`,
      })
      continue
    }
    if (run.workerKind !== "auto" && resolution.workerKind !== run.workerKind) {
      await emit(options.onLifecycle, {
        kind: "skip",
        runRef: run.runRef,
        reason: "worker_kind_unavailable",
        requestedWorkerKind: account.workerKind ?? resolution.workerKind,
        accountRef: account.accountRef,
        detail:
          `Account ${account.accountRef} does not match explicit FleetRun worker kind '${run.workerKind}'; skipping it without substitution.`,
      })
      continue
    }
    accounts.push(account)
    accountWorkerKinds.set(account.accountRef, resolution.workerKind)
  }
  const activeCounts = activeByAccount(liveClaims)
  const autoPolicy = options.autoPolicy ?? defaultFleetAutoPolicy
  const emittedAutoFallbacks = new Set<string>()
  const resolveAutoAccount = async (): Promise<FleetRunSupervisorAccount | null> => {
    const resolution = resolveFleetAutoTarget({
      policy: autoPolicy,
      candidates: accounts.map(account => autoCandidateFor(
        account,
        accountWorkerKinds.get(account.accountRef) ?? "codex",
        activeCounts,
        now,
      )),
    })
    for (const event of resolution.events) {
      const key = fallbackEventKey(event)
      if (emittedAutoFallbacks.has(key)) continue
      emittedAutoFallbacks.add(key)
      await emit(options.onLifecycle, {
        kind: "fallback",
        runRef: run.runRef,
        policySchema: autoPolicy.schema,
        event,
      })
    }
    if (resolution.selection === null) return null
    return accounts.find(account =>
      account.accountRef === resolution.selection?.accountRef &&
      accountWorkerKinds.get(account.accountRef) === resolution.selection?.harnessKind
    ) ?? null
  }
  const activeAssignments = activeAssignmentsForRun(store, run.runRef)
  const targetFreeSlots = Math.max(0, run.targetConcurrency - activeAssignments)
  const autoPolicyHarnesses = new Set(autoPolicy.preferenceOrder)
  const autoPolicyCostCeiling = autoPolicy.maxMarginalCostClass === undefined
    ? undefined
    : marginalCostClassRank[autoPolicy.maxMarginalCostClass]
  const advertisedFreeSlots = accounts.reduce((total, account) => {
    if (account.unavailabilityReason !== undefined) return total
    if (account.paused === true) return total
    if (isCoolingDown(account, now)) return total
    const workerKind = accountWorkerKinds.get(account.accountRef) ?? "codex"
    if (run.workerKind === "auto" && !autoPolicyHarnesses.has(workerKind)) return total
    if (
      run.workerKind === "auto" &&
      autoPolicyCostCeiling !== undefined &&
      marginalCostClassRank[account.marginalCostClass ?? "not_measured"] > autoPolicyCostCeiling
    ) return total
    return total + Math.max(0, account.advertisedCapacity - (activeCounts.get(account.accountRef) ?? 0))
  }, 0)
  const freeSlots = Math.min(targetFreeSlots, advertisedFreeSlots, maxSpawnPerTick)

  await emit(options.onLifecycle, {
    kind: "tick",
    runRef: run.runRef,
    activeAssignments,
    freeSlots,
  })

  if (freeSlots <= 0) {
    if (run.workerKind === "auto" && targetFreeSlots > 0) {
      await resolveAutoAccount()
    }
    if (targetFreeSlots > 0 && activeAssignments === 0 && run.refillPolicy.stopCondition === "backlog_empty") {
      const plan = await options.planner.plan({ run, now })
      const dependencyPending = plan.skipped.some(unit => unit.skipReason === "dependency_pending")
      const plannedWorkUnitsTotal = Math.max(expectedWorkUnitsTotal, run.counters.workUnitsTotal, plan.units.length)
      let reconciled = store.reconcileFleetRun(run.runRef, clock.now())
      if (reconciled.counters.workUnitsTotal < plannedWorkUnitsTotal) {
        reconciled = store.upsertFleetRun({
          ...reconciled,
          counters: {
            ...reconciled.counters,
            workUnitsTotal: plannedWorkUnitsTotal,
          },
          updatedAt: clock.now().toISOString(),
        })
      }
      if (plan.claimable.length === 0 && !dependencyPending && activeAssignmentsForRun(store, run.runRef) === 0) {
        const completed = store.updateFleetRunState(run.runRef, "completed", clock.now(), "reconcile")
        await emit(options.onLifecycle, { kind: "completed", runRef: run.runRef, reason: "backlog_empty" })
        return {
          activeAssignments: activeAssignmentsForRun(store, run.runRef),
          claimed: 0,
          dispatched: 0,
          freeSlots,
          run: completed,
        }
      }
      return { activeAssignments, claimed: 0, dispatched: 0, freeSlots, run: reconciled }
    }
    return { activeAssignments, claimed: 0, dispatched: 0, freeSlots, run }
  }

  const plan = await options.planner.plan({ run, now })
  const dependencyPending = plan.skipped.some(unit => unit.skipReason === "dependency_pending")
  const plannedWorkUnitsTotal = Math.max(expectedWorkUnitsTotal, run.counters.workUnitsTotal, plan.units.length)
  let claimed = 0
  const claimOrdinalBase = store.listWorkClaims({ runRef: run.runRef }).length
  const dispatches: Promise<void>[] = []

  for (const workUnit of plan.claimable) {
    if (dispatches.length >= freeSlots) break
    const account = run.workerKind === "auto"
      ? await resolveAutoAccount()
      : pickAccount(accounts, activeCounts, now)
    if (account === null) break
    const claim = store.tryClaimWorkUnit({
      claimRef: claimRefFor(run.runRef, workUnit.workUnitRef, now, claimOrdinalBase + claimed),
      workUnitRef: workUnit.workUnitRef,
      runRef: run.runRef,
      workerAccountRef: account.accountRef,
      marginalCostClass: account.marginalCostClass ?? "not_measured",
      ttl: claimTtlMs,
      now,
    })
    if (claim === null) continue
    claimed += 1
    activeCounts.set(account.accountRef, (activeCounts.get(account.accountRef) ?? 0) + 1)

    // Resolve the concrete dispatch kind for THIS account (mixed pools carry a
    // per-account kind; otherwise the account inherited the run's kind). Grok
    // accounts with no executor were partitioned out above.
    const workerKind = accountWorkerKinds.get(account.accountRef) ?? "codex"
    const runnerKind =
      workerKind === "claude" ? "claude_agent" :
      workerKind === "grok" ? "grok_cli" :
      "codex"

    const taskId = fleetRunTaskIdForClaim(run.runRef, claim.claimRef)
    const contextId = contextIdFor(account.accountRef, taskId)
    const accountProvider = workerKind === "claude" ? "claude_agent" : workerKind
    const accountRefHash = hashPylonAccountRef(accountProvider, account.accountRef)
    if (store.getDispatchContext(contextId) === null) {
      store.createDispatchContext({
        id: contextId,
        assigneeHandle: account.accountRef,
        runnerKind,
        accountRefHash,
        lastHeartbeatAt: now,
        maxConcurrentSlots: 1,
        now,
      })
    }
    store.createTask({
      id: taskId,
      deps: dependencyTaskIdsFor(store, run.runRef, workUnit),
      spec: {
        title: workUnit.title,
        prompt: workUnit.body ?? run.objective,
        runnerKind,
        ...(workUnit.branch === undefined ? {} : { branch: workUnit.branch }),
        ...(workUnit.baseCommit === undefined ? {} : { baseCommit: workUnit.baseCommit }),
        ...(workUnit.repo === undefined ? {} : { repo: workUnit.repo }),
        ...(workUnit.verify === undefined ? {} : { verifyCommand: workUnit.verify }),
        issueRef: workUnit.number === undefined ? workUnit.workUnitRef : `#${workUnit.number}`,
        fleetRunRef: run.runRef,
      },
      status: "ready",
      now,
    })
    store.markDispatched(taskId, contextId, now)
    store.updateWorkClaimState(claim.claimRef, "in_progress", now)

    // The durable claim is already live. Project it before invoking an
    // executor so Sarah does not wait for a long-running dispatch to return.
    await emit(options.onLifecycle, {
      kind: "dispatch",
      runRef: run.runRef,
      taskId,
      claimRef: claim.claimRef,
      workUnitRef: workUnit.workUnitRef,
      accountRef: account.accountRef,
      accountRefHash,
      assignmentRef: null,
      blockerRefs: [],
      closeoutRef: null,
      status: "accepted",
      summary: null,
      usageEvidence: null,
      workerKind,
      marginalCostClass: claim.marginalCostClass ?? "not_measured",
      verification: null,
      artifactRefs: [],
      proofRefs: [],
      authorityReceiptRefs: [],
    })

    dispatches.push((async () => {
      let result: FleetRunSupervisorDispatchResult
      const streamedLifecycle = new Set<string>()
      try {
        result = await options.runner.dispatch({
          accountRef: account.accountRef,
          claim,
          run,
          taskId,
          workUnit,
          workerKind,
          onLifecycle: async event => {
            await emit(options.onLifecycle, {
              kind: "lifecycle",
              runRef: run.runRef,
              taskId,
              claimRef: claim.claimRef,
              accountRef: account.accountRef,
              event,
            })
            streamedLifecycle.add(JSON.stringify(event))
          },
          onApprovalRequested: async rawRequest => {
            const request = S.decodeUnknownSync(
              FleetRunSupervisorApprovalRequestSchema,
            )(rawRequest, { onExcessProperty: "error" })
            const observedAt = clock.now()
            if (!Number.isFinite(observedAt.getTime())) {
              throw new Error("FleetRun approval request clock is invalid")
            }
            const authority = store.getFleetRun(run.runRef)?.authorityBinding
            if (
              authority?.phase !== "accepted" ||
              authority.pylonRef !== options.pylonRef
            ) {
              throw new Error("FleetRun approval request lacks accepted Sarah authority")
            }
            store.bindFleetRunSteeringApproval({
              approvalRef: request.approvalRef,
              pylonRef: options.pylonRef,
              runRef: run.runRef,
              claimRef: authority.claimRef,
              workUnitRef: claim.workUnitRef,
              workClaimRef: claim.claimRef,
              assignmentRef: request.assignmentRef,
              workerKind,
              workerRef: stableWorkerRef({
                accountRefHash,
                pylonRef: options.pylonRef,
                workerKind,
              }),
              accountRefHash,
              toolClass: request.toolClass,
              now: observedAt,
            })
            await emit(options.onLifecycle, {
              kind: "approval_requested",
              runRef: run.runRef,
              taskId,
              claimRef: claim.claimRef,
              approvalRef: request.approvalRef,
            })
          },
        })
      } catch {
        store.recordWorkerDone({
          contextId,
          taskId,
          status: "failed",
          result: JSON.stringify({
            assignmentRef: null,
            summary: "The named FleetRun executor failed safely.",
          }),
          now,
        })
        store.releaseWorkClaim(claim.claimRef, now)
        activeCounts.set(account.accountRef, Math.max(0, (activeCounts.get(account.accountRef) ?? 1) - 1))
        await emit(options.onLifecycle, {
          kind: "dispatch",
          runRef: run.runRef,
          taskId,
          claimRef: claim.claimRef,
          workUnitRef: workUnit.workUnitRef,
          accountRef: account.accountRef,
          accountRefHash: null,
          assignmentRef: null,
          blockerRefs: ["blocker.pylon.fleet_run.runner_throw"],
          closeoutRef: null,
          status: "failed",
          summary: "The named FleetRun executor failed safely.",
          usageEvidence: null,
          workerKind,
          marginalCostClass: claim.marginalCostClass ?? "not_measured",
          verification: null,
          artifactRefs: [],
          proofRefs: [],
          authorityReceiptRefs: [],
        })
        return
      }

      // The dispatch call above already succeeded, so the underlying work likely
      // completed; everything from here on is local bookkeeping (store updates and
      // lifecycle emits). Wrap it separately so a throw here can never reject this
      // item's promise in the shared `Promise.all(dispatches)` and discard visibility
      // into every OTHER in-flight or already-succeeded dispatch in the same batch.
      try {
        for (const event of result.lifecycle) {
          if (streamedLifecycle.has(JSON.stringify(event))) continue
          await emit(options.onLifecycle, {
            kind: "lifecycle",
            runRef: run.runRef,
            taskId,
            claimRef: claim.claimRef,
            accountRef: account.accountRef,
            event,
          })
        }
        if (result.assignmentRef !== null) {
          store.updateWorkClaimAssignmentRef(claim.claimRef, result.assignmentRef, now)
        }

        const terminal = terminalDispositionFor(run, result)
        if (terminal === null) {
          const carrier = usageEvidenceCarrierFor(result)
          await emit(options.onLifecycle, {
            kind: "dispatch",
            runRef: run.runRef,
            taskId,
            claimRef: claim.claimRef,
            workUnitRef: workUnit.workUnitRef,
            accountRef: account.accountRef,
            accountRefHash: carrier.accountRefHash,
            assignmentRef: result.assignmentRef,
            blockerRefs: dispatchFailureFor(result).blockerRefs,
            closeoutRef: carrier.closeoutRef,
            status: result.status,
            summary: result.summary ?? null,
            usageEvidence: carrier.usageEvidence,
            workerKind,
            marginalCostClass: claim.marginalCostClass ?? "not_measured",
            verification: result.verification ?? null,
            artifactRefs: result.artifactRefs ?? [],
            proofRefs: result.proofRefs ?? [],
            authorityReceiptRefs: result.authorityReceiptRefs ?? [],
          })
          return
        }

        store.recordWorkerDone({
          contextId,
          taskId,
          status: terminal.status,
          result: JSON.stringify({
            assignmentRef: result.assignmentRef,
            summary: result.summary ?? null,
            ...terminal.carrier,
          }),
          ...(terminal.status === "failed" &&
              accountHealthFailureFor(terminal.blockerRefs, result.status) !== undefined
            ? { failure: accountHealthFailureFor(terminal.blockerRefs, result.status) }
            : {}),
          now,
        })
        store.updateWorkClaimState(
          claim.claimRef,
          terminal.status === "completed" ? "closeout" : "released",
          now,
        )
        await emit(options.onLifecycle, {
          kind: "dispatch",
          runRef: run.runRef,
          taskId,
          claimRef: claim.claimRef,
          workUnitRef: workUnit.workUnitRef,
          accountRef: account.accountRef,
          accountRefHash: terminal.carrier.accountRefHash,
          assignmentRef: result.assignmentRef,
          blockerRefs: terminal.blockerRefs,
          closeoutRef: terminal.carrier.closeoutRef,
          status: terminal.status === "completed"
            ? "completed"
            : terminal.status === "blocked"
              ? "blocked"
              : "failed",
          summary: result.summary ?? null,
          usageEvidence: terminal.carrier.usageEvidence,
          workerKind,
          marginalCostClass: claim.marginalCostClass ?? "not_measured",
          verification: result.verification ?? null,
          artifactRefs: result.artifactRefs ?? [],
          proofRefs: result.proofRefs ?? [],
          authorityReceiptRefs: result.authorityReceiptRefs ?? [],
        })
      } catch (error) {
        console.error("[fleet-run-supervisor] post-dispatch bookkeeping failed", {
          runRef: run.runRef,
          taskId,
          claimRef: claim.claimRef,
          accountRef: account.accountRef,
          error: error instanceof Error ? error.message : String(error),
        })
      }
    })())
  }

  if (options.awaitDispatches === false) {
    for (const dispatch of dispatches) {
      void dispatch.catch(error => {
        console.error("[fleet-run-supervisor] fire-and-forget dispatch failed", error)
      })
    }
  } else {
    await Promise.all(dispatches)
  }

  let reconciled = store.reconcileFleetRun(run.runRef, clock.now())
  const hasClaimableBacklog = plan.claimable.length > claimed || dependencyPending
  const reviveForBacklog = hasClaimableBacklog &&
    (reconciled.state === "running" || isAutoRevivableFleetRun(reconciled))
  if (reconciled.counters.workUnitsTotal < plannedWorkUnitsTotal) {
    reconciled = store.upsertFleetRun({
      ...reconciled,
      state: reviveForBacklog ? "running" : reconciled.state,
      counters: {
        ...reconciled.counters,
        workUnitsTotal: plannedWorkUnitsTotal,
      },
      updatedAt: clock.now().toISOString(),
    })
  } else if (reconciled.state !== "running" && reviveForBacklog) {
    reconciled = store.updateFleetRunState(run.runRef, "running", clock.now(), "reconcile")
  }
  if (reconciled.state === "completed") {
    await emit(options.onLifecycle, { kind: "completed", runRef: run.runRef, reason: "drained" })
  } else if (
    reconciled.state === "stopped" &&
    activeAssignmentsForRun(store, run.runRef) === 0
  ) {
    await emitRunTerminal(options.onLifecycle, reconciled)
  }
  if (
    reconciled.state === "running" &&
    plan.claimable.length === 0 &&
    !dependencyPending &&
    activeAssignmentsForRun(store, run.runRef) === 0 &&
    reconciled.refillPolicy.stopCondition === "backlog_empty"
  ) {
    const completed = store.updateFleetRunState(run.runRef, "completed", clock.now(), "reconcile")
    await emit(options.onLifecycle, { kind: "completed", runRef: run.runRef, reason: "backlog_empty" })
    return {
      activeAssignments: activeAssignmentsForRun(store, run.runRef),
      claimed,
      dispatched: dispatches.length,
      freeSlots,
      run: completed,
    }
  }

  return {
    activeAssignments: activeAssignmentsForRun(store, run.runRef),
    claimed,
    dispatched: dispatches.length,
    freeSlots,
    run: reconciled,
  }
}

export function makeFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Effect.Effect<FleetRunSupervisorHandle, FleetRunSupervisorError> {
  return Effect.sync(() => {
    const existing = activeSupervisors.get(options.pylonRef)
    if (existing !== undefined) {
      throw new FleetRunSupervisorError(`fleet run supervisor already active for pylon ${options.pylonRef}: ${existing}`)
    }
    activeSupervisors.set(options.pylonRef, options.runRef)
    let stopped = false
    let inFlightTick: Promise<FleetRunSupervisorTickResult> | null = null
    return {
      pylonRef: options.pylonRef,
      runRef: options.runRef,
      stop: () => Effect.sync(() => {
        stopped = true
        if (activeSupervisors.get(options.pylonRef) === options.runRef) activeSupervisors.delete(options.pylonRef)
      }),
      tick: () => Effect.tryPromise({
        try: async () => {
          if (stopped) throw new FleetRunSupervisorError(`fleet run supervisor stopped: ${options.runRef}`)
          if (inFlightTick !== null) return await inFlightTick
          const tick = tickFleetRunSupervisor({ ...options, awaitDispatches: false })
            .finally(() => {
              if (inFlightTick === tick) inFlightTick = null
            })
          inFlightTick = tick
          return await tick
        },
        catch: (error: unknown) => error instanceof FleetRunSupervisorError
          ? error
          : new FleetRunSupervisorError("fleet run supervisor tick failed", error),
      }),
    }
  })
}

export function startFleetRunSupervisor(
  options: FleetRunSupervisorOptions,
): Effect.Effect<FleetRunSupervisorHandle, FleetRunSupervisorError, Scope.Scope> {
  return Effect.gen(function* () {
    const handle = yield* makeFleetRunSupervisor(options)
    const scope = yield* Effect.scope
    let loopStopped = false
    yield* Scope.addFinalizer(
      scope,
      Effect.gen(function* () {
        loopStopped = true
        yield* handle.stop()
      }),
    )
    // tickFleetRunSupervisor intentionally bypasses the one-supervisor guard as the direct test seam.
    const context = yield* Effect.context<never>()
    void (async () => {
      const clock = { ...defaultClock, ...options.clock }
      let firstLoop = true
      while (!loopStopped) {
        if (!firstLoop || options.startImmediately !== false) {
          try {
            await Effect.runPromiseWith(context)(handle.tick())
          } catch {
            // Keep the scoped supervisor alive; individual dispatch failures are recorded by tick.
          }
        }
        firstLoop = false
        if (!loopStopped) await clock.sleep(options.tickIntervalMs ?? 1000)
      }
    })()
    return handle
  })
}
