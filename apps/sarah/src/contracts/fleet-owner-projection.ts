import {
  FleetAccountRefHash,
  FleetApprovalStatus,
  FleetAttemptEntity,
  FleetAttemptMarginalCostClass,
  FleetAttemptUsageEvidence,
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
  FleetWorkUnitEntity,
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
 * Sarah-readable state. The caller establishes owner scope before calling this
 * mapper. Work units and attempts are the plan/execution authority;
 * assignments and workers are optional graph edges only.
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
  evidenceRefs: Schema.Array(FleetPublicRef),
  summary: SarahFleetSafeSummary,
})

const SarahFleetCloseout = Schema.Struct({
  status: Schema.Literals(["open", "submitted", "accepted", "rejected"]),
  closeoutRef: Schema.NullOr(FleetPublicRef),
  closeoutClass: Schema.NullOr(FleetClassToken),
  summary: SarahFleetSafeSummary,
})

const SarahFleetCapacity = Schema.Struct({
  harnessKind: FleetHarnessKind,
  pylonRef: FleetPublicRef,
  accountRefHash: Schema.NullOr(FleetAccountRefHash),
  capacityClass: Schema.Literal("owner_local"),
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

const SarahFleetAttemptProjection = Schema.Struct({
  attemptRef: FleetPublicRef,
  workUnitRef: FleetPublicRef,
  assignmentRef: Schema.NullOr(FleetPublicRef),
  assignmentStatus: Schema.NullOr(FleetClassToken),
  workerRef: Schema.NullOr(FleetPublicRef),
  workerKind: FleetHarnessKind,
  state: FleetAttemptEntity.fields.state,
  progressClass: FleetAttemptEntity.fields.progressClass,
  progress: SarahFleetProgress,
  approvalRefs: Schema.Array(FleetPublicRef),
  verification: SarahFleetVerification,
  artifactRefs: Schema.Array(FleetPublicRef),
  proofRefs: Schema.Array(FleetPublicRef),
  authorityReceiptRefs: Schema.Array(FleetPublicRef),
  closeout: SarahFleetCloseout,
  marginalCostClass: FleetAttemptMarginalCostClass,
  capacity: SarahFleetCapacity,
  usageEvidence: FleetAttemptUsageEvidence,
  blockerRefs: Schema.Array(FleetPublicRef),
  startedAt: FleetIsoTimestamp,
  /** Authoritative server-receipt clock used for freshness. */
  lastObservedAt: FleetIsoTimestamp,
  /** Remote worker clock retained for audit only; never freshness authority. */
  remoteObservedAtAudit: FleetIsoTimestamp,
  terminalAt: Schema.NullOr(FleetIsoTimestamp),
  updatedAt: FleetIsoTimestamp,
  summary: SarahFleetSafeSummary,
})
export type SarahFleetAttemptProjection =
  typeof SarahFleetAttemptProjection.Type

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
  workUnitRef: FleetPublicRef,
  issueRef: Schema.NullOr(FleetIssueRef),
  name: SarahFleetDisplayName,
  dependsOnRefs: Schema.Array(FleetPublicRef),
  state: FleetWorkUnitEntity.fields.state,
  latestAttemptRef: Schema.NullOr(FleetPublicRef),
  acceptedAttemptRef: Schema.NullOr(FleetPublicRef),
  /** Optional graph edge from the latest attempt, never work-unit identity. */
  assignmentRef: Schema.NullOr(FleetPublicRef),
  assignmentStatus: Schema.NullOr(FleetClassToken),
  workerRef: Schema.NullOr(FleetPublicRef),
  progress: SarahFleetProgress,
  approvalRefs: Schema.Array(FleetPublicRef),
  verification: SarahFleetVerification,
  artifactRefs: Schema.Array(FleetPublicRef),
  proofRefs: Schema.Array(FleetPublicRef),
  authorityReceiptRefs: Schema.Array(FleetPublicRef),
  closeout: SarahFleetCloseout,
  marginalCostClass: FleetAttemptMarginalCostClass,
  capacity: Schema.NullOr(SarahFleetCapacity),
  usageEvidence: FleetAttemptUsageEvidence,
  blockerRefs: Schema.Array(FleetPublicRef),
  attempts: Schema.Array(SarahFleetAttemptProjection),
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
  /** Optional until the live store wiring slice starts supplying these rows. */
  workUnits?: ReadonlyArray<typeof FleetWorkUnitEntity.Type>
  /** Optional until the live store wiring slice starts supplying these rows. */
  attempts?: ReadonlyArray<typeof FleetAttemptEntity.Type>
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

const decodeWorkUnit = Schema.decodeUnknownSync(FleetWorkUnitEntity)
const decodeAttempt = Schema.decodeUnknownSync(FleetAttemptEntity)

const titleCaseHarness = (
  harnessKind: typeof FleetHarnessKind.Type | undefined,
): string => {
  if (harnessKind === "codex") return "Codex"
  if (harnessKind === "claude") return "Claude"
  if (harnessKind === "grok") return "Grok"
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

const pendingApprovalForWorker = (
  approvals: ReadonlyArray<FleetApprovalEntity>,
  workerRef: string,
): FleetApprovalEntity | undefined =>
  approvals.find(
    (approval) =>
      approval.workerId === workerRef && approval.status === "pending",
  )

const progressFromHeartbeat = (
  heartbeatAt: string,
  harness: string,
  nowMs: number,
): SarahFleetProgress => {
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
      summary: `${harness} attempt reconnecting`,
    }
  }
  return {
    status: "fresh",
    phase: "dispatched",
    heartbeatAt,
    staleAt,
    ageMs,
    summary: `${harness} attempt progressing`,
  }
}

const progressForAttempt = (
  attempt: typeof FleetAttemptEntity.Type,
  nowMs: number,
): SarahFleetProgress => {
  const harness = titleCaseHarness(attempt.workerKind)
  if (attempt.state === "succeeded") {
    return {
      status: "completed",
      phase: "completed",
      observedAt: attempt.terminalAt ?? attempt.lastObservedAt,
      summary: `${harness} attempt completed`,
    }
  }
  if (attempt.state === "failed") {
    return {
      status: "blocked",
      phase: "failed",
      blockerRef: attempt.blockerRefs[0]!,
      blockerClass: "attempt_failed",
      observedAt: attempt.lastObservedAt,
      summary: `${harness} attempt failed`,
    }
  }
  if (attempt.state === "stale") {
    return {
      status: "blocked",
      phase: "circuit_broken",
      blockerRef: attempt.blockerRefs[0]!,
      blockerClass: "attempt_stale",
      observedAt: attempt.lastObservedAt,
      summary: `${harness} attempt stale`,
    }
  }
  if (attempt.state === "evidence_pending") {
    return {
      status: "blocked",
      phase: "blocked",
      blockerRef: attempt.closeoutRef!,
      blockerClass: "evidence_pending",
      observedAt: attempt.lastObservedAt,
      summary: `${harness} evidence pending`,
    }
  }
  if (attempt.progressClass === "blocked") {
    return {
      status: "blocked",
      phase: "blocked",
      blockerRef: attempt.blockerRefs[0]!,
      blockerClass: "attempt_blocked",
      observedAt: attempt.lastObservedAt,
      summary: `${harness} attempt blocked`,
    }
  }
  return progressFromHeartbeat(attempt.lastObservedAt, harness, nowMs)
}

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
  return progressFromHeartbeat(
    worker.lastProgressAt ?? worker.updatedAt,
    harness,
    nowMs,
  )
}

const verificationForAttempt = (
  attempt: typeof FleetAttemptEntity.Type,
): typeof SarahFleetVerification.Type => {
  if (attempt.verification.truth === "passed") {
    return {
      status: "ready",
      verificationRef: attempt.verification.verifierRef,
      evidenceRefs: [...attempt.verification.evidenceRefs],
      summary: "Verification passed",
    }
  }
  if (attempt.verification.truth === "failed") {
    return {
      status: "failed",
      verificationRef: attempt.verification.verifierRef,
      evidenceRefs: [...attempt.verification.evidenceRefs],
      summary: "Verification failed",
    }
  }
  return {
    status: "not_reported",
    verificationRef: null,
    evidenceRefs: [],
    summary:
      attempt.verification.truth === "pending"
        ? "Verification pending"
        : "Verification not reported",
  }
}

const emptyVerification = (): typeof SarahFleetVerification.Type => ({
  status: "not_reported",
  verificationRef: null,
  evidenceRefs: [],
  summary: "Verification not reported",
})

const closeoutForAttempt = (
  attempt: typeof FleetAttemptEntity.Type,
): typeof SarahFleetCloseout.Type => {
  if (attempt.state === "succeeded") {
    return {
      status: "accepted",
      closeoutRef: attempt.closeoutRef,
      closeoutClass: "succeeded",
      summary: "Closeout accepted",
    }
  }
  if (attempt.state === "evidence_pending") {
    return {
      status: "submitted",
      closeoutRef: attempt.closeoutRef,
      closeoutClass: "evidence_pending",
      summary: "Closeout evidence pending",
    }
  }
  if (attempt.state === "failed") {
    return {
      status: "rejected",
      closeoutRef: attempt.closeoutRef,
      closeoutClass: "failed",
      summary: "Closeout failed",
    }
  }
  if (attempt.state === "stale") {
    return {
      status: "rejected",
      closeoutRef: attempt.closeoutRef,
      closeoutClass: "stale",
      summary: "Closeout stale",
    }
  }
  return {
    status: "open",
    closeoutRef: null,
    closeoutClass: null,
    summary: "Closeout open",
  }
}

const emptyCloseout = (): typeof SarahFleetCloseout.Type => ({
  status: "open",
  closeoutRef: null,
  closeoutClass: null,
  summary: "Closeout open",
})

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

const uniqueBy = <A>(
  values: ReadonlyArray<A>,
  keyOf: (value: A) => string,
  label: string,
): Map<string, A> => {
  const result = new Map<string, A>()
  for (const value of values) {
    const key = keyOf(value)
    if (result.has(key)) {
      throw new Error(`duplicate ${label}`)
    }
    result.set(key, value)
  }
  return result
}

export function projectSarahFleetOwnerRun(
  input: SarahFleetOwnerProjectionInput,
  projectedAtMs: number,
): SarahFleetOwnerProjection {
  if (
    !Number.isSafeInteger(projectedAtMs) ||
    projectedAtMs < 0 ||
    !Number.isFinite(projectedAtMs)
  ) {
    throw new Error("invalid projection time")
  }
  const workers = [...input.workers].sort((left, right) =>
    left.workerId.localeCompare(right.workerId),
  )
  const assignments = [...input.assignments].sort((left, right) =>
    left.assignmentRef.localeCompare(right.assignmentRef),
  )
  const workUnits = (input.workUnits ?? [])
    .map((workUnit) => decodeWorkUnit(workUnit))
    .sort((left, right) => left.workUnitRef.localeCompare(right.workUnitRef))
  const attempts = (input.attempts ?? [])
    .map((attempt) => decodeAttempt(attempt))
    .sort((left, right) => left.attemptRef.localeCompare(right.attemptRef))
  const approvals = [...input.approvals].sort((left, right) =>
    left.approvalRef.localeCompare(right.approvalRef),
  )
  const commandOutcomes = [...(input.commandOutcomes ?? [])].sort(
    (left, right) =>
      left.seq !== right.seq
        ? left.seq - right.seq
        : left.intentId.localeCompare(right.intentId),
  )

  const workUnitByRef = uniqueBy(
    workUnits,
    (workUnit) => workUnit.workUnitRef,
    "work-unit ref",
  )
  const attemptByRef = uniqueBy(
    attempts,
    (attempt) => attempt.attemptRef,
    "attempt ref",
  )
  const assignmentByRef = uniqueBy(
    assignments,
    (assignment) => assignment.assignmentRef,
    "assignment ref",
  )
  const workersByAssignment = new Map<string, Array<FleetWorkerEntity>>()
  for (const worker of workers) {
    if (worker.assignmentRef === undefined) continue
    const assignmentWorkers = workersByAssignment.get(worker.assignmentRef) ?? []
    assignmentWorkers.push(worker)
    workersByAssignment.set(worker.assignmentRef, assignmentWorkers)
  }
  const attemptsByAssignment = new Map<
    string,
    Array<typeof FleetAttemptEntity.Type>
  >()
  for (const attempt of attempts) {
    if (attempt.assignmentRef === null) continue
    const assignmentAttempts =
      attemptsByAssignment.get(attempt.assignmentRef) ?? []
    assignmentAttempts.push(attempt)
    attemptsByAssignment.set(attempt.assignmentRef, assignmentAttempts)
  }
  const attemptsByWorkUnit = new Map<
    string,
    Array<typeof FleetAttemptEntity.Type>
  >()
  for (const attempt of attempts) {
    if (!workUnitByRef.has(attempt.workUnitRef)) {
      throw new Error("attempt names an unknown work unit")
    }
    const unitAttempts = attemptsByWorkUnit.get(attempt.workUnitRef) ?? []
    unitAttempts.push(attempt)
    attemptsByWorkUnit.set(attempt.workUnitRef, unitAttempts)
  }
  for (const workUnit of workUnits) {
    const unitAttempts = attemptsByWorkUnit.get(workUnit.workUnitRef) ?? []
    if (workUnit.latestAttemptRef === null && unitAttempts.length !== 0) {
      throw new Error("work unit with attempts has no latest attempt pointer")
    }
    for (const pointer of [
      workUnit.latestAttemptRef,
      workUnit.acceptedAttemptRef,
    ]) {
      if (pointer === null) continue
      const pointedAttempt = attemptByRef.get(pointer)
      if (
        pointedAttempt === undefined ||
        pointedAttempt.workUnitRef !== workUnit.workUnitRef
      ) {
        throw new Error("work-unit attempt pointer is unresolved or cross-unit")
      }
    }
    if (workUnit.latestAttemptRef === null) continue
    const latestAttempt = attemptByRef.get(workUnit.latestAttemptRef)!
    const expectedAttemptState =
      workUnit.state === "verification_pending"
        ? "evidence_pending"
        : workUnit.state
    if (latestAttempt.state !== expectedAttemptState) {
      throw new Error("work-unit state disagrees with its latest attempt")
    }
    if (
      workUnit.acceptedAttemptRef !== null &&
      attemptByRef.get(workUnit.acceptedAttemptRef)?.state !== "succeeded"
    ) {
      throw new Error("accepted attempt is not succeeded")
    }
  }

  const currentAttemptForAssignment = (
    assignmentRef: string,
  ): typeof FleetAttemptEntity.Type | undefined => {
    const current = (attemptsByAssignment.get(assignmentRef) ?? []).filter(
      (attempt) =>
        workUnitByRef.get(attempt.workUnitRef)?.latestAttemptRef ===
        attempt.attemptRef,
    )
    return current.length === 1 ? current[0] : undefined
  }

  const attemptProjection = (
    attempt: typeof FleetAttemptEntity.Type,
  ): SarahFleetAttemptProjection => {
    const assignment =
      attempt.assignmentRef === null
        ? undefined
        : assignmentByRef.get(attempt.assignmentRef)
    const assignmentWorkers =
      attempt.assignmentRef === null
        ? []
        : (workersByAssignment.get(attempt.assignmentRef) ?? [])
    const worker = assignmentWorkers.length === 1 ? assignmentWorkers[0] : undefined
    return {
      attemptRef: attempt.attemptRef,
      workUnitRef: attempt.workUnitRef,
      assignmentRef: attempt.assignmentRef,
      assignmentStatus: assignment?.status ?? null,
      workerRef: worker?.workerId ?? null,
      workerKind: attempt.workerKind,
      state: attempt.state,
      progressClass: attempt.progressClass,
      progress: progressForAttempt(attempt, projectedAtMs),
      // Worker slots and assignment edges can be reused across retries. Until
      // the approval entity binds an exact attempt, no approval is attributed
      // to attempt evidence or made actionable here.
      approvalRefs: [],
      verification: verificationForAttempt(attempt),
      artifactRefs: [...attempt.artifactRefs],
      proofRefs: [...attempt.proofRefs],
      authorityReceiptRefs: [...attempt.authorityReceiptRefs],
      closeout: closeoutForAttempt(attempt),
      marginalCostClass: attempt.marginalCostClass,
      capacity: {
        harnessKind: attempt.workerKind,
        pylonRef: attempt.pylonRef,
        accountRefHash: attempt.accountRefHash,
        capacityClass: attempt.capacityClass,
      },
      usageEvidence: attempt.usageEvidence,
      blockerRefs: [...attempt.blockerRefs],
      startedAt: attempt.startedAt,
      lastObservedAt: attempt.lastObservedAt,
      remoteObservedAtAudit: attempt.remoteObservedAt,
      terminalAt: attempt.terminalAt,
      updatedAt: attempt.updatedAt,
      summary: `${titleCaseHarness(attempt.workerKind)} attempt ${humanizeToken(attempt.state)}`,
    }
  }

  const projectedWorkUnits = workUnits.map((workUnit) => {
    const projectedAttempts = (
      attemptsByWorkUnit.get(workUnit.workUnitRef) ?? []
    )
      .sort((left, right) => left.attemptRef.localeCompare(right.attemptRef))
      .map(attemptProjection)
    const latestAttempt =
      workUnit.latestAttemptRef === null
        ? undefined
        : projectedAttempts.find(
            (attempt) => attempt.attemptRef === workUnit.latestAttemptRef,
          )
    return {
      workUnitRef: workUnit.workUnitRef,
      issueRef: workUnit.issueRef,
      name: workUnit.issueRef ?? workUnit.workUnitRef,
      dependsOnRefs: [...workUnit.dependsOnRefs],
      state: workUnit.state,
      latestAttemptRef: workUnit.latestAttemptRef,
      acceptedAttemptRef: workUnit.acceptedAttemptRef,
      assignmentRef: latestAttempt?.assignmentRef ?? null,
      assignmentStatus: latestAttempt?.assignmentStatus ?? null,
      workerRef: latestAttempt?.workerRef ?? null,
      progress:
        latestAttempt?.progress ?? {
          status: "not_assigned" as const,
          summary: "Work unit planned",
        },
      approvalRefs: latestAttempt?.approvalRefs ?? [],
      verification: latestAttempt?.verification ?? emptyVerification(),
      artifactRefs: latestAttempt?.artifactRefs ?? [],
      proofRefs: latestAttempt?.proofRefs ?? [],
      authorityReceiptRefs: latestAttempt?.authorityReceiptRefs ?? [],
      closeout: latestAttempt?.closeout ?? emptyCloseout(),
      marginalCostClass: latestAttempt?.marginalCostClass ?? "not_measured",
      capacity: latestAttempt?.capacity ?? null,
      usageEvidence: latestAttempt?.usageEvidence ?? { truth: "pending" as const },
      blockerRefs: latestAttempt?.blockerRefs ?? [],
      attempts: projectedAttempts,
      summary: `Work unit ${humanizeToken(workUnit.state)}`,
      updatedAt: workUnit.updatedAt,
    }
  })

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
    workUnits: projectedWorkUnits,
    workers: workers.map((worker) => {
      const attempt =
        worker.assignmentRef === undefined
          ? undefined
          : currentAttemptForAssignment(worker.assignmentRef)
      const workUnit =
        attempt === undefined
          ? undefined
          : workUnitByRef.get(attempt.workUnitRef)
      return {
        workerRef: worker.workerId,
        name: `${titleCaseHarness(worker.harnessKind)} worker`,
        phase: worker.phase,
        harnessKind: worker.harnessKind ?? null,
        workUnitRef: workUnit?.workUnitRef ?? null,
        accountRefHash: worker.accountRefHash ?? null,
        progress:
          attempt === undefined
            ? progressForWorker(worker, approvals, projectedAtMs)
            : progressForAttempt(attempt, projectedAtMs),
        approvalRefs: approvalRefsForWorker(approvals, worker.workerId),
        updatedAt: worker.updatedAt,
      }
    }),
    approvals: approvals.map((approval) => {
      return {
        approvalRef: approval.approvalRef,
        status: approval.status,
        workerRef: approval.workerId ?? null,
        // The current approval entity is worker-bound. A worker and assignment
        // may be reused by retries, so projecting a work-unit target here
        // would manufacture attempt authority. Keep it unbound until the
        // shared approval contract carries an exact attempt ref.
        workUnitRef: null,
        toolClass: approval.toolClass ?? null,
        openedAt: approval.openedAt ?? null,
        decidedAt: approval.decidedAt ?? null,
        // Exact attempt-bound approval actions are a separate authority lane.
        availableDecisions: [] as const,
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
