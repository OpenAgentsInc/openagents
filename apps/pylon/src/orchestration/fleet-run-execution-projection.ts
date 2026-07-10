import { createHash } from "node:crypto"

import type {
  PylonFleetRunExecutionEventInput,
  PylonFleetRunUsageEvidenceV2,
} from "./fleet-run-execution-reporter.js"
import type { FleetRunSupervisorObservedEvent } from "./fleet-run-supervisor.js"
import type { PylonOrchestrationStore } from "./store.js"

const projectedRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u
const projectedUnitRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$/u
const projectedBlockerPattern = /^blocker\.[A-Za-z0-9][A-Za-z0-9._:-]{0,171}$/u

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

const projectedRefs = (
  values: readonly string[],
  prefix: string,
  maximum = 64,
): string[] => [...new Set(values.map(value => projectedRef(value, prefix)))].slice(0, maximum)

const blockerRef = (value: string): string =>
  projectedBlockerPattern.test(value)
    ? value
    : `blocker.pylon.fleet_run.opaque.${digest(value)}`

const blockerRefs = (values: readonly string[]): string[] =>
  [...new Set(values.map(blockerRef))].slice(0, 32)

const terminalUsageProjection = (
  event: Extract<FleetRunSupervisorObservedEvent, { readonly kind: "dispatch" }>,
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
      evidenceRef: projectedRef(evidence.evidenceRef, "evidence.public.pylon.opaque"),
      receiptRef: projectedRef(evidence.receiptRef, "receipt.public.pylon.opaque"),
      tokenUsageRefs: [],
      caveatRefs: projectedRefs(evidence.caveatRefs, "caveat.pylon.fleet_run.opaque", 100),
    }
  }
  return {
    ...evidence,
    assignmentRef,
    evidenceRef: projectedRef(evidence.evidenceRef, "evidence.public.pylon.opaque"),
    tokenUsageRefs: projectedRefs(
      evidence.tokenUsageRefs,
      "token_usage.public.pylon.opaque",
      100,
    ),
    proofRefs: projectedRefs(evidence.proofRefs, "proof.public.pylon.opaque", 100),
    closeoutChecklistRefs: projectedRefs(
      evidence.closeoutChecklistRefs,
      "check.public.pylon.closeout.opaque",
      100,
    ),
    proofChecklistRefs: projectedRefs(
      evidence.proofChecklistRefs,
      "check.public.pylon.proof.opaque",
      100,
    ),
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

  if (input.event.kind === "lifecycle") {
    const claim = input.store.getWorkClaim(input.event.claimRef)
    if (claim === null || claim.runRef !== input.event.runRef) return [started]
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v2",
      kind: "work_progress",
      observedAt,
      unitRef: projectedUnitRef(claim.workUnitRef),
      workClaimRef: projectedRef(claim.claimRef, "work_claim.pylon.opaque"),
      ...(input.event.event.assignmentRef === undefined
        ? {}
        : {
            assignmentRef: projectedRef(
              input.event.event.assignmentRef,
              "assignment.public.pylon.opaque",
            ),
          }),
      workerKind: workerKindForTask(input.store, input.event.taskId),
      ...(input.event.event.accountRefHash === undefined
        ? {}
        : { accountRefHash: input.event.event.accountRefHash }),
      blockerRefs: blockerRefs(input.event.event.blockerRefs ?? []),
    }]
  }

  if (input.event.kind === "dispatch") {
    const usageEvidence = terminalUsageProjection(input.event)
    const unitRef = projectedUnitRef(input.event.workUnitRef)
    const workClaimRef = projectedRef(input.event.claimRef, "work_claim.pylon.opaque")
    const assignmentRef = input.event.assignmentRef === null
      ? null
      : projectedRef(input.event.assignmentRef, "assignment.public.pylon.opaque")
    const closeoutRef = input.event.closeoutRef === null
      ? null
      : projectedRef(input.event.closeoutRef, "closeout.public.pylon.opaque")
    const verification = input.event.verification?.truth === "passed"
      ? {
          truth: "passed" as const,
          verifierRef: projectedRef(
            input.event.verification.verifierRef,
            "verifier.public.pylon.opaque",
          ),
          evidenceRefs: projectedRefs(
            input.event.verification.evidenceRefs,
            "verification.public.pylon.opaque",
          ),
        }
      : null
    const artifactRefs = projectedRefs(
      input.event.artifactRefs ?? [],
      "artifact.public.pylon.opaque",
    )
    const proofRefs = projectedRefs(input.event.proofRefs ?? [], "proof.public.pylon.opaque")
    const authorityReceiptRefs = projectedRefs(
      input.event.authorityReceiptRefs ?? [],
      "receipt.public.pylon.authority.opaque",
    )
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
      const failedBlockers = blockerRefs(input.event.blockerRefs)
      if (input.event.status === "completed") {
        failedBlockers.push("blocker.pylon.fleet_run.evidence_incomplete")
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
        terminalState: "failed",
        blockerRefs: [...new Set(failedBlockers)].slice(0, 32),
        ...(closeoutRef === null ? {} : { closeoutRef }),
        ...(input.event.verification?.truth !== "failed"
          ? {}
          : {
              verification: {
                truth: "failed" as const,
                ...(input.event.verification.verifierRef === undefined
                  ? {}
                  : {
                      verifierRef: projectedRef(
                        input.event.verification.verifierRef,
                        "verifier.public.pylon.opaque",
                      ),
                    }),
                evidenceRefs: projectedRefs(
                  input.event.verification.evidenceRefs,
                  "verification.public.pylon.opaque",
                ),
              },
            }),
        ...(artifactRefs.length === 0 ? {} : { artifactRefs }),
        ...(proofRefs.length === 0 ? {} : { proofRefs }),
        ...(authorityReceiptRefs.length === 0 ? {} : { authorityReceiptRefs }),
        ...(usageEvidence === null ? {} : { usageEvidence }),
      }]
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
      blockerRefs: blockerRefs(input.event.blockerRefs),
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
  return [started]
}
