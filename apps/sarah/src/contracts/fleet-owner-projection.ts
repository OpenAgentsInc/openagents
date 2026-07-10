import {
  FleetAccountRefHash,
  FleetApprovalStatus,
  FleetClassToken,
  FleetCommandOutcomeEntity,
  FleetHarnessKind,
  FleetIsoTimestamp,
  FleetIssueRef,
  FleetPublicRef,
  FleetRunCounters,
  FleetRunStatus,
  FleetWorkerKind,
  FleetWorkerPhase,
  type FleetApprovalEntity,
  type FleetAssignmentEntity,
  type FleetInboxFlagEntity,
  type FleetRunEntity,
  type FleetWorkerEntity,
} from "@openagentsinc/khala-sync"
import {
  ApprovalDecisionValue,
  FleetRunControlAction,
} from "@openagentsinc/khala-fleet-intents"
import { Schema } from "effect"

import { FC3_FRESHNESS_TIMEOUT_MS } from "./fleet-continuity-projection.ts"

/**
 * Pure allowlist projection from owner-scoped Khala Sync fleet entities into
 * Sarah-readable state. The caller must establish owner scope before calling
 * this mapper. `availableControls` is only a state-derived UI affordance; it
 * grants no authority and every action still crosses the existing typed,
 * authenticated fleet-intent boundary.
 *
 * Free-form worker material is not an input to this boundary. Prompts, steer
 * bodies, command output, credentials, local paths, and private events are
 * therefore structurally absent from both the schema and mapper output.
 */
export const SARAH_FLEET_OWNER_PROJECTION_SCHEMA =
  "sarah.fleet_owner_projection.v1" as const

export const SarahFleetSafeSummary = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9 .:_#-]*$/),
)

const SarahFleetDisplayName = Schema.Union([FleetPublicRef, FleetIssueRef])
const SarahElapsedMilliseconds = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
)

const SarahFleetProgress = Schema.Union([
  Schema.Struct({
    status: Schema.Literal("not_assigned"),
    summary: SarahFleetSafeSummary,
  }),
  Schema.Struct({
    status: Schema.Literal("waiting"),
    phase: Schema.Literals(["idle", "paused"]),
    observedAt: FleetIsoTimestamp,
    summary: SarahFleetSafeSummary,
  }),
  Schema.Struct({
    status: Schema.Literal("fresh"),
    phase: Schema.Literal("dispatched"),
    heartbeatAt: FleetIsoTimestamp,
    staleAt: FleetIsoTimestamp,
    ageMs: SarahElapsedMilliseconds,
    summary: SarahFleetSafeSummary,
  }),
  Schema.Struct({
    status: Schema.Literal("stalled"),
    phase: Schema.Literal("dispatched"),
    heartbeatAt: FleetIsoTimestamp,
    staleAt: FleetIsoTimestamp,
    ageMs: SarahElapsedMilliseconds,
    reconnect: Schema.Literal(true),
    summary: SarahFleetSafeSummary,
  }),
  Schema.Struct({
    status: Schema.Literal("blocked"),
    phase: Schema.Literals(["blocked", "failed", "circuit_broken"]),
    blockerRef: FleetPublicRef,
    blockerClass: FleetClassToken,
    observedAt: FleetIsoTimestamp,
    summary: SarahFleetSafeSummary,
  }),
  Schema.Struct({
    status: Schema.Literal("completed"),
    phase: Schema.Literal("completed"),
    observedAt: FleetIsoTimestamp,
    summary: SarahFleetSafeSummary,
  }),
])
export type SarahFleetProgress = typeof SarahFleetProgress.Type

const SarahFleetBlocker = Schema.Struct({
  blockerRef: FleetPublicRef,
  blockerClass: FleetClassToken,
  scope: Schema.Literals(["run", "worker"]),
  workerRef: Schema.NullOr(FleetPublicRef),
  observedAt: FleetIsoTimestamp,
  summary: SarahFleetSafeSummary,
})

const SarahFleetVerification = Schema.Struct({
  status: Schema.Literals(["not_reported", "ready", "failed"]),
  verificationRef: Schema.NullOr(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const SarahFleetCloseout = Schema.Struct({
  status: Schema.Literals(["open", "submitted", "accepted", "rejected"]),
  closeoutRef: Schema.NullOr(FleetPublicRef),
  closeoutClass: Schema.NullOr(FleetClassToken),
  summary: SarahFleetSafeSummary,
})

const SarahFleetRunProjection = Schema.Struct({
  runRef: FleetPublicRef,
  name: SarahFleetSafeSummary,
  status: FleetRunStatus,
  desiredSlots: Schema.Number,
  workerKind: FleetWorkerKind,
  startedAt: Schema.NullOr(FleetIsoTimestamp),
  counters: FleetRunCounters,
  updatedAt: FleetIsoTimestamp,
  availableControls: Schema.Array(FleetRunControlAction),
  blockers: Schema.Array(SarahFleetBlocker),
})

const SarahFleetWorkerProjection = Schema.Struct({
  workerRef: FleetPublicRef,
  name: SarahFleetSafeSummary,
  phase: FleetWorkerPhase,
  harnessKind: Schema.NullOr(FleetHarnessKind),
  workUnitRef: Schema.NullOr(SarahFleetDisplayName),
  accountRefHash: Schema.NullOr(FleetAccountRefHash),
  progress: SarahFleetProgress,
  approvalRefs: Schema.Array(FleetPublicRef),
  updatedAt: FleetIsoTimestamp,
})

const SarahFleetWorkUnitProjection = Schema.Struct({
  workUnitRef: SarahFleetDisplayName,
  assignmentRef: FleetPublicRef,
  name: SarahFleetDisplayName,
  assignmentStatus: FleetClassToken,
  workerRef: Schema.NullOr(FleetPublicRef),
  progress: SarahFleetProgress,
  approvalRefs: Schema.Array(FleetPublicRef),
  verification: SarahFleetVerification,
  closeout: SarahFleetCloseout,
  summary: SarahFleetSafeSummary,
  updatedAt: FleetIsoTimestamp,
})

const SarahFleetApprovalProjection = Schema.Struct({
  approvalRef: FleetPublicRef,
  status: FleetApprovalStatus,
  workerRef: Schema.NullOr(FleetPublicRef),
  workUnitRef: Schema.NullOr(SarahFleetDisplayName),
  toolClass: Schema.NullOr(FleetClassToken),
  openedAt: Schema.NullOr(FleetIsoTimestamp),
  decidedAt: Schema.NullOr(FleetIsoTimestamp),
  availableDecisions: Schema.Array(ApprovalDecisionValue),
  summary: SarahFleetSafeSummary,
  updatedAt: FleetIsoTimestamp,
})

export const SarahFleetOwnerProjection = Schema.Struct({
  schema: Schema.Literal(SARAH_FLEET_OWNER_PROJECTION_SCHEMA),
  run: SarahFleetRunProjection,
  workUnits: Schema.Array(SarahFleetWorkUnitProjection),
  workers: Schema.Array(SarahFleetWorkerProjection),
  approvals: Schema.Array(SarahFleetApprovalProjection),
  /** Body-free request/delivery/effective receipts, durable across reconnect. */
  commandOutcomes: Schema.optionalKey(Schema.Array(FleetCommandOutcomeEntity)),
  projectedAt: FleetIsoTimestamp,
})
export type SarahFleetOwnerProjection = typeof SarahFleetOwnerProjection.Type

export type SarahFleetOwnerProjectionInput = Readonly<{
  run: FleetRunEntity
  workers: ReadonlyArray<FleetWorkerEntity>
  assignments: ReadonlyArray<FleetAssignmentEntity>
  approvals: ReadonlyArray<FleetApprovalEntity>
  inboxFlags: ReadonlyArray<FleetInboxFlagEntity>
  commandOutcomes?: ReadonlyArray<FleetCommandOutcomeEntity>
}>

const runControlsByStatus: Readonly<
  Record<FleetRunStatus, ReadonlyArray<FleetRunControlAction>>
> = {
  draft: ["stop"],
  running: ["pause", "drain", "stop"],
  paused: ["resume", "drain", "stop"],
  draining: ["stop"],
  stopped: [],
  completed: [],
}

const verificationReadyStatuses = new Set([
  "proof_ready",
  "verified",
  "closeout_submitted",
  "accepted_work",
])
const verificationFailedStatuses = new Set([
  "verification_failed",
  "rejected",
])
const submittedCloseoutStatuses = new Set(["closeout_submitted"])

const titleCaseHarness = (
  harnessKind: typeof FleetHarnessKind.Type | undefined,
): string => {
  if (harnessKind === "codex") {
    return "Codex"
  }
  if (harnessKind === "claude") {
    return "Claude"
  }
  if (harnessKind === "grok") {
    return "Grok"
  }
  return "Fleet"
}

const humanizeToken = (token: string): string => token.replaceAll("_", " ")

const addMilliseconds = (iso: string, milliseconds: number): string =>
  new Date(Date.parse(iso) + milliseconds).toISOString()

const approvalRefsForWorker = (
  approvals: ReadonlyArray<FleetApprovalEntity>,
  workerRef: string,
): ReadonlyArray<string> =>
  approvals
    .filter((approval) => approval.workerId === workerRef)
    .map((approval) => approval.approvalRef)
    .sort()

// NOTE: Khala Sync's current assignment post-image does not carry the planner's
// canonical workUnitRef. An issue ref is stable across assignment retries;
// non-issue/plan work falls back to its stable assignment identity until the
// shared projection grows that additive field.
const workUnitRefForAssignment = (
  assignment: FleetAssignmentEntity,
): typeof SarahFleetDisplayName.Type =>
  assignment.issueRef ?? assignment.assignmentRef

const pendingApprovalForWorker = (
  approvals: ReadonlyArray<FleetApprovalEntity>,
  workerRef: string,
): FleetApprovalEntity | undefined =>
  approvals.find(
    (approval) =>
      approval.workerId === workerRef && approval.status === "pending",
  )

const progressForWorker = (
  worker: FleetWorkerEntity,
  approvals: ReadonlyArray<FleetApprovalEntity>,
  nowMs: number,
): SarahFleetProgress => {
  const harness = titleCaseHarness(worker.harnessKind)
  if (
    worker.phase === "blocked" ||
    worker.phase === "failed" ||
    worker.phase === "circuit_broken"
  ) {
    const approval = pendingApprovalForWorker(approvals, worker.workerId)
    return {
      status: "blocked",
      phase: worker.phase,
      blockerRef: approval?.approvalRef ?? worker.workerId,
      blockerClass: approval === undefined ? worker.phase : "approval_pending",
      observedAt: worker.updatedAt,
      summary: `${harness} worker ${humanizeToken(worker.phase)}`,
    }
  }
  if (worker.phase === "completed") {
    return {
      status: "completed",
      phase: "completed",
      observedAt: worker.updatedAt,
      summary: `${harness} worker completed`,
    }
  }
  if (worker.phase === "idle" || worker.phase === "paused") {
    return {
      status: "waiting",
      phase: worker.phase,
      observedAt: worker.updatedAt,
      summary: `${harness} worker ${worker.phase}`,
    }
  }

  const heartbeatAt = worker.lastProgressAt ?? worker.updatedAt
  const ageMs = Math.max(0, nowMs - Date.parse(heartbeatAt))
  const staleAt = addMilliseconds(heartbeatAt, FC3_FRESHNESS_TIMEOUT_MS)
  if (ageMs >= FC3_FRESHNESS_TIMEOUT_MS) {
    return {
      status: "stalled",
      phase: "dispatched",
      heartbeatAt,
      staleAt,
      ageMs,
      reconnect: true,
      summary: `${harness} worker reconnecting`,
    }
  }
  return {
    status: "fresh",
    phase: "dispatched",
    heartbeatAt,
    staleAt,
    ageMs,
    summary: `${harness} worker progressing`,
  }
}

const verificationForAssignment = (
  assignment: FleetAssignmentEntity,
): typeof SarahFleetVerification.Type => {
  if (
    verificationFailedStatuses.has(assignment.status) ||
    assignment.closeoutClass === "rejected"
  ) {
    return {
      status: "failed",
      verificationRef: assignment.assignmentRef,
      summary: "Verification failed",
    }
  }
  if (
    verificationReadyStatuses.has(assignment.status) ||
    assignment.closeoutClass !== undefined
  ) {
    return {
      status: "ready",
      verificationRef: assignment.assignmentRef,
      summary: "Verification available",
    }
  }
  return {
    status: "not_reported",
    verificationRef: null,
    summary: "Verification not reported",
  }
}

const closeoutForAssignment = (
  assignment: FleetAssignmentEntity,
): typeof SarahFleetCloseout.Type => {
  if (assignment.closeoutClass === "accepted_work") {
    return {
      status: "accepted",
      closeoutRef: assignment.assignmentRef,
      closeoutClass: assignment.closeoutClass,
      summary: "Closeout accepted",
    }
  }
  if (assignment.closeoutClass === "rejected") {
    return {
      status: "rejected",
      closeoutRef: assignment.assignmentRef,
      closeoutClass: assignment.closeoutClass,
      summary: "Closeout rejected",
    }
  }
  if (
    assignment.closeoutClass !== undefined ||
    submittedCloseoutStatuses.has(assignment.status)
  ) {
    return {
      status: "submitted",
      closeoutRef: assignment.assignmentRef,
      closeoutClass: assignment.closeoutClass ?? null,
      summary: "Closeout submitted",
    }
  }
  return {
    status: "open",
    closeoutRef: null,
    closeoutClass: null,
    summary: "Closeout open",
  }
}

const runBlockers = (
  workers: ReadonlyArray<FleetWorkerEntity>,
  approvals: ReadonlyArray<FleetApprovalEntity>,
  inboxFlags: ReadonlyArray<FleetInboxFlagEntity>,
): ReadonlyArray<typeof SarahFleetBlocker.Type> => {
  const flags = inboxFlags
    .filter((flag) => flag.status === "open")
    .map((flag) => ({
      blockerRef: flag.flagRef,
      blockerClass: flag.kind,
      scope: "run" as const,
      workerRef: null,
      observedAt: flag.openedAt ?? flag.updatedAt,
      summary: `Run ${humanizeToken(flag.kind)}`,
    }))
  const blockedWorkers = workers
    .filter(
      (worker) =>
        worker.phase === "blocked" ||
        worker.phase === "failed" ||
        worker.phase === "circuit_broken",
    )
    .map((worker) => {
      const approval = pendingApprovalForWorker(approvals, worker.workerId)
      return {
        blockerRef: approval?.approvalRef ?? worker.workerId,
        blockerClass:
          approval === undefined ? worker.phase : "approval_pending",
        scope: "worker" as const,
        workerRef: worker.workerId,
        observedAt: worker.updatedAt,
        summary: `${titleCaseHarness(worker.harnessKind)} worker blocked`,
      }
    })
  return [...flags, ...blockedWorkers].sort((left, right) =>
    left.blockerRef.localeCompare(right.blockerRef),
  )
}

export function projectSarahFleetOwnerRun(
  input: SarahFleetOwnerProjectionInput,
  projectedAtMs: number,
): SarahFleetOwnerProjection {
  const workers = [...input.workers].sort((left, right) =>
    left.workerId.localeCompare(right.workerId),
  )
  const assignments = [...input.assignments].sort((left, right) =>
    left.assignmentRef.localeCompare(right.assignmentRef),
  )
  const approvals = [...input.approvals].sort((left, right) =>
    left.approvalRef.localeCompare(right.approvalRef),
  )
  const commandOutcomes = [...(input.commandOutcomes ?? [])].sort(
    (left, right) =>
      left.seq !== right.seq
        ? left.seq - right.seq
        : left.intentId.localeCompare(right.intentId),
  )

  const workerByAssignment = new Map(
    workers.flatMap((worker) =>
      worker.assignmentRef === undefined
        ? []
        : [[worker.assignmentRef, worker] as const],
    ),
  )
  const assignmentByRef = new Map(
    assignments.map((assignment) => [assignment.assignmentRef, assignment]),
  )

  const projection = {
    schema: SARAH_FLEET_OWNER_PROJECTION_SCHEMA,
    run: {
      runRef: input.run.runId,
      name: "Fleet run",
      status: input.run.status,
      desiredSlots: input.run.desiredSlots,
      workerKind: input.run.workerKind,
      startedAt: input.run.startedAt,
      counters: input.run.counters,
      updatedAt: input.run.updatedAt,
      availableControls: [...runControlsByStatus[input.run.status]],
      blockers: runBlockers(workers, approvals, input.inboxFlags),
    },
    workUnits: assignments.map((assignment) => {
      const worker = workerByAssignment.get(assignment.assignmentRef)
      const progress =
        worker === undefined
          ? ({
              status: "not_assigned",
              summary: "Work unit not assigned",
            } as const)
          : progressForWorker(worker, approvals, projectedAtMs)
      return {
        workUnitRef: workUnitRefForAssignment(assignment),
        assignmentRef: assignment.assignmentRef,
        name: assignment.issueRef ?? assignment.assignmentRef,
        assignmentStatus: assignment.status,
        workerRef: worker?.workerId ?? null,
        progress,
        approvalRefs:
          worker === undefined
            ? []
            : approvalRefsForWorker(approvals, worker.workerId),
        // NOTE: Owner proof and closeout lookups already resolve by assignmentRef;
        // this mapper does not mint a synthetic evidence identity.
        verification: verificationForAssignment(assignment),
        closeout: closeoutForAssignment(assignment),
        summary: `Work unit ${humanizeToken(assignment.status)}`,
        updatedAt: assignment.updatedAt,
      }
    }),
    workers: workers.map((worker) => {
      const assignment =
        worker.assignmentRef === undefined
          ? undefined
          : assignmentByRef.get(worker.assignmentRef)
      return {
        workerRef: worker.workerId,
        name: `${titleCaseHarness(worker.harnessKind)} worker`,
        phase: worker.phase,
        harnessKind: worker.harnessKind ?? null,
        workUnitRef:
          assignment === undefined ? null : workUnitRefForAssignment(assignment),
        accountRefHash: worker.accountRefHash ?? null,
        progress: progressForWorker(worker, approvals, projectedAtMs),
        approvalRefs: approvalRefsForWorker(approvals, worker.workerId),
        updatedAt: worker.updatedAt,
      }
    }),
    approvals: approvals.map((approval) => {
      const worker =
        approval.workerId === undefined
          ? undefined
          : workers.find((candidate) => candidate.workerId === approval.workerId)
      const assignment =
        worker?.assignmentRef === undefined
          ? undefined
          : assignmentByRef.get(worker.assignmentRef)
      return {
        approvalRef: approval.approvalRef,
        status: approval.status,
        workerRef: approval.workerId ?? null,
        workUnitRef:
          assignment === undefined ? null : workUnitRefForAssignment(assignment),
        toolClass: approval.toolClass ?? null,
        openedAt: approval.openedAt ?? null,
        decidedAt: approval.decidedAt ?? null,
        availableDecisions:
          approval.status === "pending"
            ? (["allow", "deny"] as const)
            : ([] as const),
        summary:
          approval.status === "pending"
            ? "Approval needs decision"
            : `Approval ${approval.status}`,
        updatedAt: approval.updatedAt,
      }
    }),
    commandOutcomes,
    projectedAt: new Date(projectedAtMs).toISOString(),
  }

  return Schema.decodeUnknownSync(SarahFleetOwnerProjection)(projection)
}
