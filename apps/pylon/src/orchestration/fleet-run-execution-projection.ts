import { createHash } from "node:crypto"

import type {
  PylonFleetRunExecutionEventInput,
  PylonFleetRunUsageEvidenceV2,
} from "./fleet-run-execution-reporter.js"
import type { FleetRunSupervisorObservedEvent } from "./fleet-run-supervisor.js"
import type {
  FleetRunSteeringApprovalBinding,
  PylonOrchestrationStore,
} from "./store.js"

const projectedRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u
const projectedUnitRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$/u
const projectedBlockerPattern = /^blocker\.[A-Za-z0-9][A-Za-z0-9._:-]{0,171}$/u
const approvalToolClassPattern = /^[a-z][a-z0-9_]{0,63}$/u
const isoTimestampPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u
const evidenceCardinalityBlocker =
  "blocker.pylon.fleet_run.evidence_cardinality_invalid" as const
const evidenceIdentityBlocker =
  "blocker.pylon.fleet_run.evidence_identity_invalid" as const
const approvalRequiredBlocker =
  "blocker.pylon.fleet_run.approval_required" as const

const digest = (value: string): string =>
  createHash("sha256").update(value).digest("hex").slice(0, 24)

// Never repair a path-like or otherwise unsafe ref by replacing separators:
// that can preserve identifying fragments. Unsafe legacy/provider refs become
// deterministic opaque refs at the Pylon projection boundary.
const projectedRef = (value: string, prefix: string): string =>
  projectedRefPattern.test(value) ? value : `${prefix}.${digest(value)}`

const projectedUnitRef = (value: string): string =>
  projectedUnitRefPattern.test(value)
    ? value
    : `work_unit.pylon.opaque.${digest(value)}`

type EvidenceProjectionRegistry = {
  readonly projectedBySource: Map<string, string>
  readonly sourceByProjected: Map<string, string>
}

const openEvidenceProjectionRegistry = (): EvidenceProjectionRegistry => ({
  projectedBySource: new Map(),
  sourceByProjected: new Map(),
})

const projectEvidenceRef = (
  registry: EvidenceProjectionRegistry,
  value: string,
  prefix: string,
): string => {
  const existing = registry.projectedBySource.get(value)
  if (existing !== undefined) return existing
  const next = projectedRef(value, prefix)
  const existingSource = registry.sourceByProjected.get(next)
  if (existingSource !== undefined && existingSource !== value) {
    throw new Error("FleetRun evidence ref projection collided")
  }
  registry.projectedBySource.set(value, next)
  registry.sourceByProjected.set(next, value)
  return next
}

const projectedEvidenceGroups = <const Groups extends readonly {
  readonly values: readonly string[]
  readonly prefix: string
  readonly maximum: number
}[]>(
  registry: EvidenceProjectionRegistry,
  groups: Groups,
): { readonly [Index in keyof Groups]: string[] } => {
  // Arrays are role-bearing, so the same real receipt may legitimately appear
  // in (for example) both artifact and verification roles. Preserve both role
  // entries while mapping each unique source ref to exactly one projected ref.
  // Duplicates inside one role remain invalid; distinct refs may never collide.
  for (const group of groups) {
    if (
      group.values.length > group.maximum ||
      new Set(group.values).size !== group.values.length
    ) throw new Error("FleetRun evidence ref cardinality is invalid")
  }
  const projected = groups.map(group => group.values.map(value =>
    projectEvidenceRef(registry, value, group.prefix)
  ))
  return projected as { readonly [Index in keyof Groups]: string[] }
}

const blockerRef = (value: string): string =>
  projectedBlockerPattern.test(value)
    ? value
    : `blocker.pylon.fleet_run.opaque.${digest(value)}`

const blockerRefs = (values: readonly string[]): string[] => {
  if (values.length > 32 || new Set(values).size !== values.length) {
    throw new Error("FleetRun blocker ref cardinality is invalid")
  }
  const sourceByProjected = new Map<string, string>()
  return values.map(value => {
    const projected = blockerRef(value)
    const existing = sourceByProjected.get(projected)
    if (existing !== undefined && existing !== value) {
      throw new Error("FleetRun blocker ref projection collided")
    }
    sourceByProjected.set(projected, value)
    return projected
  })
}

const accountHashMatchesWorker = (
  workerKind: "codex" | "claude" | "grok",
  accountRefHash: string,
): boolean => accountRefHash.startsWith(
  `account.pylon.${workerKind === "claude" ? "claude_agent" : workerKind}.`,
)

/**
 * Rebuild the exact approval-request wire from durable local custody. Legacy
 * bindings without worker/tool identity remain local and are never guessed.
 */
export function projectFleetRunApprovalBinding(
  binding: FleetRunSteeringApprovalBinding,
): PylonFleetRunExecutionEventInput | null {
  if (
    binding.workerKind === null ||
    binding.workerRef === null ||
    binding.accountRefHash === null ||
    binding.toolClass === null ||
    !approvalToolClassPattern.test(binding.toolClass) ||
    !isoTimestampPattern.test(binding.createdAt) ||
    !accountHashMatchesWorker(binding.workerKind, binding.accountRefHash)
  ) return null
  return {
    schema: "openagents.pylon.fleet_run_execution_event.v2",
    kind: "approval_requested",
    observedAt: binding.createdAt,
    unitRef: projectedUnitRef(binding.workUnitRef),
    workClaimRef: projectedRef(binding.workClaimRef, "work_claim.pylon.opaque"),
    assignmentRef: projectedRef(
      binding.assignmentRef,
      "assignment.public.pylon.opaque",
    ),
    workerKind: binding.workerKind,
    workerRef: projectedRef(binding.workerRef, "worker.public.pylon.opaque"),
    accountRefHash: binding.accountRefHash,
    approvalRef: projectedRef(
      binding.approvalRef,
      "approval.public.pylon.opaque",
    ),
    toolClass: binding.toolClass,
    blockerRefs: [approvalRequiredBlocker],
  }
}

const terminalUsageProjection = (
  event: Extract<FleetRunSupervisorObservedEvent, { readonly kind: "dispatch" }>,
  registry: EvidenceProjectionRegistry,
): PylonFleetRunUsageEvidenceV2 | null => {
  const evidence = event.usageEvidence
  if (evidence === null) return null
  const assignmentRef = projectedRef(
    evidence.assignmentRef,
    "assignment.public.pylon.opaque",
  )
  if (evidence.truth === "not_measured") {
    return {
      ...evidence,
      assignmentRef,
      evidenceRef: projectEvidenceRef(
        registry,
        evidence.evidenceRef,
        "evidence.public.pylon.opaque",
      ),
      receiptRef: projectEvidenceRef(
        registry,
        evidence.receiptRef,
        "receipt.public.pylon.opaque",
      ),
      tokenUsageRefs: [],
      caveatRefs: projectedEvidenceGroups(registry, [{
        values: evidence.caveatRefs,
        prefix: "caveat.pylon.fleet_run.opaque",
        maximum: 100,
      }] as const)[0],
    }
  }
  const [tokenUsageRefs, proofRefs, closeoutChecklistRefs, proofChecklistRefs] =
    projectedEvidenceGroups(registry, [
      {
        values: evidence.tokenUsageRefs,
        prefix: "token_usage.public.pylon.opaque",
        maximum: 100,
      },
      {
        values: evidence.proofRefs,
        prefix: "proof.public.pylon.opaque",
        maximum: 100,
      },
      {
        values: evidence.closeoutChecklistRefs,
        prefix: "check.public.pylon.closeout.opaque",
        maximum: 100,
      },
      {
        values: evidence.proofChecklistRefs,
        prefix: "check.public.pylon.proof.opaque",
        maximum: 100,
      },
    ] as const)
  return {
    ...evidence,
    assignmentRef,
    evidenceRef: projectEvidenceRef(
      registry,
      evidence.evidenceRef,
      "evidence.public.pylon.opaque",
    ),
    tokenUsageRefs,
    proofRefs,
    closeoutChecklistRefs,
    proofChecklistRefs,
  }
}

const observedAtFor = (
  store: PylonOrchestrationStore,
  event: FleetRunSupervisorObservedEvent,
): string => {
  if (event.kind === "lifecycle") return event.event.observedAt
  if (event.kind === "dispatch") {
    return store.getTask(event.taskId)?.updatedAt ??
      store.getFleetRun(event.runRef)?.updatedAt ??
      new Date(0).toISOString()
  }
  return store.getFleetRun(event.runRef)?.updatedAt ?? new Date(0).toISOString()
}

const workerKindForTask = (
  store: PylonOrchestrationStore,
  taskId: string,
): "codex" | "claude" | "grok" => {
  const runnerKind = store.getTask(taskId)?.spec.runnerKind
  if (runnerKind === "claude_agent") return "claude"
  if (runnerKind === "grok_cli") return "grok"
  return "codex"
}

/**
 * Convert private supervisor observations into the bounded v2 execution wire.
 * The server receipt clock owns freshness; `observedAt` remains the Pylon audit
 * clock and may legitimately arrive out of order across parallel callbacks.
 */
export function projectFleetRunSupervisorObservation(input: {
  readonly event: FleetRunSupervisorObservedEvent
  readonly store: PylonOrchestrationStore
}): readonly PylonFleetRunExecutionEventInput[] {
  const run = input.store.getFleetRun(input.event.runRef)
  if (run === null || run.authorityBinding?.phase !== "accepted") return []
  const started: PylonFleetRunExecutionEventInput = {
    schema: "openagents.pylon.fleet_run_execution_event.v2",
    kind: "run_started",
    observedAt: run.startedAt ?? run.createdAt,
  }
  const observedAt = observedAtFor(input.store, input.event)

  const failedProjection = (failure: {
    readonly blockerRef: typeof evidenceCardinalityBlocker | typeof evidenceIdentityBlocker
    readonly unitRef: string
    readonly workClaimRef: string
    readonly workerKind: "codex" | "claude" | "grok"
    readonly marginalCostClass?: "free" | "subscription" | "api_metered" | "not_measured"
  }): readonly PylonFleetRunExecutionEventInput[] => [started, {
    schema: "openagents.pylon.fleet_run_execution_event.v2",
    kind: "work_terminal",
    observedAt,
    unitRef: failure.unitRef,
    workClaimRef: failure.workClaimRef,
    workerKind: failure.workerKind,
    ...(failure.marginalCostClass === undefined
      ? {}
      : { marginalCostClass: failure.marginalCostClass }),
    terminalState: "failed",
    blockerRefs: [failure.blockerRef],
  }]

  if (input.event.kind === "approval_requested") {
    const binding = input.store.getFleetRunSteeringApprovalBinding(
      input.event.approvalRef,
    )
    if (
      binding === null ||
      binding.pylonRef !== run.authorityBinding.pylonRef ||
      binding.runRef !== input.event.runRef ||
      binding.claimRef !== run.authorityBinding.claimRef ||
      binding.workClaimRef !== input.event.claimRef
    ) return [started]
    const approval = projectFleetRunApprovalBinding(binding)
    return approval === null ? [started] : [started, approval]
  }

  if (input.event.kind === "lifecycle") {
    const claim = input.store.getWorkClaim(input.event.claimRef)
    if (claim === null || claim.runRef !== input.event.runRef) return [started]
    const unitRef = projectedUnitRef(claim.workUnitRef)
    const workClaimRef = projectedRef(claim.claimRef, "work_claim.pylon.opaque")
    const workerKind = workerKindForTask(input.store, input.event.taskId)
    const accountRefHash = input.event.event.accountRefHash
    if (
      accountRefHash !== undefined &&
      !accountHashMatchesWorker(workerKind, accountRefHash)
    ) {
      return failedProjection({
        blockerRef: evidenceIdentityBlocker,
        unitRef,
        workClaimRef,
        workerKind,
        ...(claim.marginalCostClass === undefined
          ? {}
          : { marginalCostClass: claim.marginalCostClass }),
      })
    }
    let projectedBlockers: string[]
    try {
      projectedBlockers = blockerRefs(input.event.event.blockerRefs ?? [])
    } catch {
      return failedProjection({
        blockerRef: evidenceCardinalityBlocker,
        unitRef,
        workClaimRef,
        workerKind,
        ...(claim.marginalCostClass === undefined
          ? {}
          : { marginalCostClass: claim.marginalCostClass }),
      })
    }
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v2",
      kind: "work_progress",
      observedAt,
      unitRef,
      workClaimRef,
      ...(input.event.event.assignmentRef === undefined
        ? {}
        : {
            assignmentRef: projectedRef(
              input.event.event.assignmentRef,
              "assignment.public.pylon.opaque",
            ),
          }),
      workerKind,
      ...(accountRefHash === undefined
        ? {}
        : { accountRefHash }),
      ...(claim.marginalCostClass === undefined
        ? {}
        : { marginalCostClass: claim.marginalCostClass }),
      capacityClass: input.event.executionTarget,
      blockerRefs: projectedBlockers,
    }]
  }

  if (input.event.kind === "dispatch") {
    const unitRef = projectedUnitRef(input.event.workUnitRef)
    const workClaimRef = projectedRef(input.event.claimRef, "work_claim.pylon.opaque")
    const assignmentRef = input.event.assignmentRef === null
      ? null
      : projectedRef(input.event.assignmentRef, "assignment.public.pylon.opaque")
    const closeoutRef = input.event.closeoutRef === null
      ? null
      : projectedRef(input.event.closeoutRef, "closeout.public.pylon.opaque")
    if (
      input.event.accountRefHash !== null &&
      !accountHashMatchesWorker(input.event.workerKind, input.event.accountRefHash)
    ) {
      return failedProjection({
        blockerRef: evidenceIdentityBlocker,
        unitRef,
        workClaimRef,
        workerKind: input.event.workerKind,
        marginalCostClass: input.event.marginalCostClass,
      })
    }
    const rawUsageIdentityMatches = input.event.usageEvidence === null ||
      (input.event.assignmentRef !== null &&
        input.event.usageEvidence.assignmentRef === input.event.assignmentRef &&
        input.event.usageEvidence.harnessKind === input.event.workerKind)
    if (!rawUsageIdentityMatches) {
      return failedProjection({
        blockerRef: evidenceIdentityBlocker,
        unitRef,
        workClaimRef,
        workerKind: input.event.workerKind,
        marginalCostClass: input.event.marginalCostClass,
      })
    }
    let usageEvidence: PylonFleetRunUsageEvidenceV2 | null
    let verification: {
      readonly truth: "passed"
      readonly verifierRef: string
      readonly evidenceRefs: readonly string[]
    } | null
    let failedVerification: {
      readonly truth: "failed"
      readonly verifierRef?: string | undefined
      readonly evidenceRefs: readonly string[]
    } | null
    let artifactRefs: string[]
    let proofRefs: string[]
    let authorityReceiptRefs: string[]
    try {
      const evidenceRegistry = openEvidenceProjectionRegistry()
      const projectedVerification = input.event.verification?.truth === "passed"
        ? {
            truth: "passed" as const,
            verifierRef: projectEvidenceRef(
              evidenceRegistry,
              input.event.verification.verifierRef,
              "verifier.public.pylon.opaque",
            ),
            evidenceRefs: input.event.verification.evidenceRefs,
          }
        : null
      const projectedFailedVerification = input.event.verification?.truth === "failed"
        ? {
            truth: "failed" as const,
            ...(input.event.verification.verifierRef === undefined
              ? {}
              : {
                  verifierRef: projectEvidenceRef(
                    evidenceRegistry,
                    input.event.verification.verifierRef,
                    "verifier.public.pylon.opaque",
                  ),
                }),
            evidenceRefs: input.event.verification.evidenceRefs,
          }
        : null
      const verificationEvidenceRefs = projectedVerification?.evidenceRefs ??
        projectedFailedVerification?.evidenceRefs ?? []
      const projectedGroups = projectedEvidenceGroups(evidenceRegistry, [
        {
          values: verificationEvidenceRefs,
          prefix: "verification.public.pylon.opaque",
          maximum: 64,
        },
        {
          values: input.event.artifactRefs ?? [],
          prefix: "artifact.public.pylon.opaque",
          maximum: 64,
        },
        {
          values: input.event.proofRefs ?? [],
          prefix: "proof.public.pylon.opaque",
          maximum: 64,
        },
        {
          values: input.event.authorityReceiptRefs ?? [],
          prefix: "receipt.public.pylon.authority.opaque",
          maximum: 64,
        },
      ] as const)
      const [verificationRefs, projectedArtifacts, projectedProofs, projectedAuthority] =
        projectedGroups
      verification = projectedVerification === null
        ? null
        : { ...projectedVerification, evidenceRefs: verificationRefs }
      failedVerification = projectedFailedVerification === null
        ? null
        : { ...projectedFailedVerification, evidenceRefs: verificationRefs }
      artifactRefs = projectedArtifacts
      proofRefs = projectedProofs
      authorityReceiptRefs = projectedAuthority
      usageEvidence = terminalUsageProjection(input.event, evidenceRegistry)
    } catch {
      return failedProjection({
        blockerRef: evidenceCardinalityBlocker,
        unitRef,
        workClaimRef,
        workerKind: input.event.workerKind,
        marginalCostClass: input.event.marginalCostClass,
      })
    }
    const projectedUsageIdentityMatches = usageEvidence === null ||
      (usageEvidence.assignmentRef === assignmentRef &&
        usageEvidence.harnessKind === input.event.workerKind)
    if (!projectedUsageIdentityMatches) {
      return failedProjection({
        blockerRef: evidenceIdentityBlocker,
        unitRef,
        workClaimRef,
        workerKind: input.event.workerKind,
        marginalCostClass: input.event.marginalCostClass,
      })
    }
    if (
      input.event.status === "completed" &&
      assignmentRef !== null &&
      input.event.accountRefHash !== null &&
      closeoutRef !== null &&
      verification !== null &&
      verification.evidenceRefs.length > 0 &&
      artifactRefs.length > 0 &&
      proofRefs.length > 0 &&
      authorityReceiptRefs.length > 0 &&
      usageEvidence !== null &&
      usageEvidence.assignmentRef === assignmentRef
    ) {
      return [started, {
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_terminal",
        observedAt,
        unitRef,
        workClaimRef,
        assignmentRef,
        workerKind: input.event.workerKind,
        accountRefHash: input.event.accountRefHash,
        marginalCostClass: input.event.marginalCostClass ?? "not_measured",
        capacityClass: input.event.executionTarget,
        terminalState: "accepted",
        closeoutRef,
        verification,
        artifactRefs,
        proofRefs,
        authorityReceiptRefs,
        usageEvidence,
        blockerRefs: [],
      }]
    }
    if (
      input.event.status === "failed" ||
      input.event.status === "blocked" ||
      input.event.status === "completed"
    ) {
      let failedBlockers: string[]
      try {
        failedBlockers = blockerRefs(input.event.blockerRefs)
      } catch {
        return failedProjection({
          blockerRef: evidenceCardinalityBlocker,
          unitRef,
          workClaimRef,
          workerKind: input.event.workerKind,
          marginalCostClass: input.event.marginalCostClass,
        })
      }
      if (input.event.status === "completed") {
        if (
          !failedBlockers.includes("blocker.pylon.fleet_run.evidence_incomplete") &&
          failedBlockers.length >= 32
        ) {
          return failedProjection({
            blockerRef: evidenceCardinalityBlocker,
            unitRef,
            workClaimRef,
            workerKind: input.event.workerKind,
            marginalCostClass: input.event.marginalCostClass,
          })
        }
        if (!failedBlockers.includes("blocker.pylon.fleet_run.evidence_incomplete")) {
          failedBlockers.push("blocker.pylon.fleet_run.evidence_incomplete")
        }
      } else if (failedBlockers.length === 0) {
        failedBlockers.push("blocker.pylon.fleet_run.work_failed")
      }
      return [started, {
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_terminal",
        observedAt,
        unitRef,
        workClaimRef,
        ...(assignmentRef === null ? {} : { assignmentRef }),
        workerKind: input.event.workerKind,
        ...(input.event.accountRefHash === null
          ? {}
          : { accountRefHash: input.event.accountRefHash }),
        marginalCostClass: input.event.marginalCostClass ?? "not_measured",
        capacityClass: input.event.executionTarget,
        terminalState: "failed",
        blockerRefs: failedBlockers,
        ...(closeoutRef === null ? {} : { closeoutRef }),
        ...(failedVerification === null
          ? {}
          : { verification: failedVerification }),
        ...(artifactRefs.length === 0 ? {} : { artifactRefs }),
        ...(proofRefs.length === 0 ? {} : { proofRefs }),
        ...(authorityReceiptRefs.length === 0 ? {} : { authorityReceiptRefs }),
        ...(usageEvidence === null ? {} : { usageEvidence }),
      }]
    }
    let projectedBlockers: string[]
    try {
      projectedBlockers = blockerRefs(input.event.blockerRefs)
    } catch {
      return failedProjection({
        blockerRef: evidenceCardinalityBlocker,
        unitRef,
        workClaimRef,
        workerKind: input.event.workerKind,
        marginalCostClass: input.event.marginalCostClass,
      })
    }
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v2",
      kind: "work_progress",
      observedAt,
      unitRef,
      workClaimRef,
      ...(assignmentRef === null ? {} : { assignmentRef }),
      workerKind: input.event.workerKind,
      ...(input.event.accountRefHash === null
        ? {}
        : { accountRefHash: input.event.accountRefHash }),
      marginalCostClass: input.event.marginalCostClass ?? "not_measured",
      capacityClass: input.event.executionTarget,
      blockerRefs: projectedBlockers,
    }]
  }

  if (input.event.kind === "completed") {
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v2",
      kind: "run_terminal",
      observedAt,
      terminalState: "completed",
      blockerRefs: [],
    }]
  }
  if (input.event.kind === "terminal") {
    let projectedBlockers: string[]
    try {
      projectedBlockers = blockerRefs(input.event.blockerRefs)
    } catch {
      return [started, {
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "run_terminal",
        observedAt,
        terminalState: "failed",
        blockerRefs: [evidenceCardinalityBlocker],
      }]
    }
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v2",
      kind: "run_terminal",
      observedAt,
      terminalState: input.event.terminalState,
      blockerRefs: projectedBlockers,
    }]
  }
  return [started]
}
