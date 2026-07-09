import { createHash } from "node:crypto"
import { Schema as S } from "effect"

import { assertPublicProjectionSafe } from "../state.js"
import { fleetRunTaskIdForClaim } from "./fleet-run-refs.js"
import {
  isAutoRevivableFleetRun,
  type OrchestrationRunnerKind,
  type PylonOrchestrationStore,
  type WorkClaim,
} from "./store.js"

export const FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA =
  "openagents.pylon.fleet_run_interrupted_recovery.v0.1" as const
export const FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA =
  "openagents.pylon.fleet_run_interrupted_closeout.v0.1" as const
export const FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER =
  "blocker.assignment.local_run_interrupted" as const

export const FleetRunOwnerLocalLivenessSchema = S.Literals(["live", "dead", "unknown"])
export type FleetRunOwnerLocalLiveness = typeof FleetRunOwnerLocalLivenessSchema.Type

export const FleetRunInterruptedCloseoutSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA),
  recoveryRef: S.String,
  runRef: S.String,
  taskRef: S.String,
  claimRef: S.String,
  assignmentRef: S.NullOr(S.String),
  status: S.Literal("stale"),
  liveness: S.Literals(["dead", "unknown"]),
  taskState: S.Literal("failed"),
  claimState: S.Literal("released"),
  blockerRefs: S.Array(S.Literal(FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER)),
  receiptRefs: S.Array(S.String),
  resultRefs: S.Array(S.String),
  redacted: S.Literal(true),
  recoveredAt: S.String,
})
export type FleetRunInterruptedCloseout = typeof FleetRunInterruptedCloseoutSchema.Type

export const FleetRunInterruptedRecoveryReceiptSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA),
  observedAt: S.String,
  inspectedAssignments: S.Number,
  liveAssignments: S.Number,
  recoveredAssignments: S.Number,
  closeouts: S.Array(FleetRunInterruptedCloseoutSchema),
  contentRedacted: S.Literal(true),
})
export type FleetRunInterruptedRecoveryReceipt = typeof FleetRunInterruptedRecoveryReceiptSchema.Type

export type FleetRunOwnerLocalLivenessProbeInput = {
  readonly runRef: string
  readonly taskRef: string
  readonly claimRef: string
  readonly contextRef: string
  readonly assignmentRef: string | null
  readonly runnerKind: OrchestrationRunnerKind
}

export type FleetRunOwnerLocalLivenessProbe = (
  input: FleetRunOwnerLocalLivenessProbeInput,
) => FleetRunOwnerLocalLiveness | Promise<FleetRunOwnerLocalLiveness>

export type RecoverInterruptedFleetRunAssignmentsInput = {
  readonly store: PylonOrchestrationStore
  readonly probe: FleetRunOwnerLocalLivenessProbe
  readonly runRef?: string
  readonly now?: Date
}

const stableRef = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,255}$/u

const assertPublicRef = (name: string, ref: string): void => {
  if (!PUBLIC_REF_PATTERN.test(ref)) throw new Error(`${name} must be a public-safe ref`)
}

const assertInterruptedCloseoutRefs = (closeout: FleetRunInterruptedCloseout): void => {
  assertPublicRef("recoveryRef", closeout.recoveryRef)
  assertPublicRef("runRef", closeout.runRef)
  assertPublicRef("taskRef", closeout.taskRef)
  assertPublicRef("claimRef", closeout.claimRef)
  if (closeout.assignmentRef !== null) assertPublicRef("assignmentRef", closeout.assignmentRef)
  closeout.blockerRefs.forEach((ref, index) => assertPublicRef(`blockerRefs[${index}]`, ref))
  closeout.receiptRefs.forEach((ref, index) => assertPublicRef(`receiptRefs[${index}]`, ref))
  closeout.resultRefs.forEach((ref, index) => assertPublicRef(`resultRefs[${index}]`, ref))
}

const buildInterruptedCloseout = (
  claim: WorkClaim,
  taskRef: string,
  liveness: Exclude<FleetRunOwnerLocalLiveness, "live">,
  now: Date,
): FleetRunInterruptedCloseout => {
  assertPublicRef("runRef", claim.runRef)
  assertPublicRef("taskRef", taskRef)
  assertPublicRef("claimRef", claim.claimRef)
  if (claim.assignmentRef !== null) assertPublicRef("assignmentRef", claim.assignmentRef)
  const seed = `${claim.runRef}:${claim.claimRef}:${taskRef}:${claim.assignmentRef ?? "unassigned"}`
  const closeout = S.decodeUnknownSync(FleetRunInterruptedCloseoutSchema)({
    schema: FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA,
    recoveryRef: stableRef("recovery.public.pylon.fleet_run.local_interrupted", seed),
    runRef: claim.runRef,
    taskRef,
    claimRef: claim.claimRef,
    assignmentRef: claim.assignmentRef,
    status: "stale",
    liveness,
    taskState: "failed",
    claimState: "released",
    blockerRefs: [FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER],
    receiptRefs: [stableRef("receipt.public.pylon.fleet_run.local_interrupted", seed)],
    resultRefs: [stableRef("result.public.pylon.fleet_run.local_interrupted", seed)],
    redacted: true,
    recoveredAt: now.toISOString(),
  })
  assertInterruptedCloseoutRefs(closeout)
  assertPublicProjectionSafe(closeout, "fleetRunInterruptedCloseout")
  return closeout
}

const decodeInterruptedCloseout = (value: string | null): FleetRunInterruptedCloseout | null => {
  if (value === null) return null
  try {
    const closeout = S.decodeUnknownSync(FleetRunInterruptedCloseoutSchema)(JSON.parse(value))
    assertInterruptedCloseoutRefs(closeout)
    assertPublicProjectionSafe(closeout, "fleetRunInterruptedCloseout")
    return closeout
  } catch {
    return null
  }
}

const belongsToRequestedRun = (claim: WorkClaim, runRef: string | undefined): boolean =>
  runRef === undefined || claim.runRef === runRef

/**
 * Recover owner-local FleetRun executors whose durable task and claim still
 * say they are active after the process disappeared. The probe is deliberately
 * ref-only: process IDs, commands, workspaces, output, and credentials remain
 * in the owner-local executor implementation and cannot enter this receipt.
 */
export async function recoverInterruptedFleetRunAssignments(
  input: RecoverInterruptedFleetRunAssignmentsInput,
): Promise<FleetRunInterruptedRecoveryReceipt> {
  const { store } = input
  const now = input.now ?? new Date()
  const closeouts = new Map<string, FleetRunInterruptedCloseout>()
  const originallyRunningRuns = new Set(
    store.listFleetRuns("running").map((run) => run.runRef),
  )
  let inspectedAssignments = 0
  let liveAssignments = 0

  // Heal a process interruption between the durable task closeout and claim
  // release. The typed task result is the durable idempotency marker.
  for (const claim of store.listWorkClaims({ state: "in_progress" })) {
    if (!belongsToRequestedRun(claim, input.runRef)) continue
    const taskRef = fleetRunTaskIdForClaim(claim.runRef, claim.claimRef)
    const task = store.getTask(taskRef)
    const closeout = task?.status === "failed" ? decodeInterruptedCloseout(task.result) : null
    if (
      closeout === null ||
      closeout.runRef !== claim.runRef ||
      closeout.claimRef !== claim.claimRef ||
      closeout.taskRef !== taskRef
    ) continue
    store.releaseWorkClaim(claim.claimRef, now)
    closeouts.set(closeout.recoveryRef, closeout)
  }

  const dispatchedTasks = new Map(
    store.listTasks("dispatched")
      .filter((task) => task.spec.fleetRunRef !== undefined)
      .filter((task) => input.runRef === undefined || task.spec.fleetRunRef === input.runRef)
      .map((task) => [task.id, task]),
  )
  const dispatchedContexts = new Map(
    store.listDispatchContexts("dispatched")
      .filter((context) => context.currentTaskId !== null)
      .map((context) => [context.currentTaskId as string, context]),
  )

  for (const claim of store.listWorkClaims({ state: "in_progress" })) {
    if (!belongsToRequestedRun(claim, input.runRef)) continue
    const taskRef = fleetRunTaskIdForClaim(claim.runRef, claim.claimRef)
    const task = dispatchedTasks.get(taskRef)
    const context = dispatchedContexts.get(taskRef)
    if (task === undefined || task.spec.fleetRunRef !== claim.runRef || context === undefined) continue

    inspectedAssignments += 1
    let liveness: FleetRunOwnerLocalLiveness
    try {
      liveness = await input.probe({
        runRef: claim.runRef,
        taskRef,
        claimRef: claim.claimRef,
        contextRef: context.id,
        assignmentRef: claim.assignmentRef,
        runnerKind: context.runnerKind,
      })
    } catch {
      liveness = "unknown"
    }

    if (liveness === "live") {
      liveAssignments += 1
      continue
    }

    // The liveness probe is asynchronous. Re-read every durable guard so two
    // supervisors cannot both close the same task after racing on one probe.
    const currentTask = store.getTask(taskRef)
    const currentContext = store.getDispatchContext(context.id)
    const currentClaim = store.getWorkClaim(claim.claimRef)
    if (
      currentTask?.status !== "dispatched" ||
      currentTask.spec.fleetRunRef !== claim.runRef ||
      currentContext?.status !== "dispatched" ||
      currentContext.currentTaskId !== taskRef ||
      currentClaim?.state !== "in_progress"
    ) continue

    const closeout = buildInterruptedCloseout(currentClaim, taskRef, liveness, now)
    store.recordWorkerDone({
      contextId: currentContext.id,
      taskId: taskRef,
      status: "failed",
      result: JSON.stringify(closeout),
      body: `fleet_run_local_assignment_stale ${FLEET_RUN_LOCAL_RUN_INTERRUPTED_BLOCKER}`,
      now,
    })
    store.releaseWorkClaim(currentClaim.claimRef, now)
    closeouts.set(closeout.recoveryRef, closeout)
  }

  for (const runRef of new Set([...closeouts.values()].map((closeout) => closeout.runRef))) {
    if (store.getFleetRun(runRef) === null) continue
    const reconciled = store.reconcileFleetRun(runRef, now)
    if (originallyRunningRuns.has(runRef) && isAutoRevivableFleetRun(reconciled)) {
      store.updateFleetRunState(runRef, "running", now, "reconcile")
    }
  }

  const receipt = S.decodeUnknownSync(FleetRunInterruptedRecoveryReceiptSchema)({
    schema: FLEET_RUN_INTERRUPTED_RECOVERY_SCHEMA,
    observedAt: now.toISOString(),
    inspectedAssignments,
    liveAssignments,
    recoveredAssignments: closeouts.size,
    closeouts: [...closeouts.values()],
    contentRedacted: true,
  })
  assertPublicProjectionSafe(receipt, "fleetRunInterruptedRecoveryReceipt")
  return receipt
}
