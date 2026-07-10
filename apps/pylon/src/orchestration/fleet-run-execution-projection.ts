import type {
  PylonFleetRunExecutionEventInput,
  PylonFleetRunProjectedUsageEvidence,
} from "./fleet-run-execution-reporter.js"
import type { FleetRunSupervisorObservedEvent } from "./fleet-run-supervisor.js"
import type { PylonOrchestrationStore } from "./store.js"

const terminalUsageProjection = (
  event: Extract<FleetRunSupervisorObservedEvent, { readonly kind: "dispatch" }>,
): PylonFleetRunProjectedUsageEvidence | null => {
  const evidence = event.usageEvidence
  if (evidence === null) return null
  return evidence.truth === "exact"
    ? { truth: "exact", tokenUsageRefs: evidence.tokenUsageRefs }
    : { truth: "not_measured", tokenUsageRefs: [] }
}

const observedAtFor = (
  store: PylonOrchestrationStore,
  event: FleetRunSupervisorObservedEvent,
): string => {
  if (event.kind === "dispatch" || event.kind === "lifecycle") {
    return store.getTask(event.taskId)?.updatedAt ??
      store.getFleetRun(event.runRef)?.updatedAt ??
      new Date(0).toISOString()
  }
  return store.getFleetRun(event.runRef)?.updatedAt ?? new Date(0).toISOString()
}

/**
 * Convert private supervisor observations into the bounded execution wire.
 * Prompts, summaries, raw account refs, local paths, and provider text are
 * deliberately not members of the returned vocabulary.
 */
export function projectFleetRunSupervisorObservation(input: {
  readonly event: FleetRunSupervisorObservedEvent
  readonly store: PylonOrchestrationStore
}): readonly PylonFleetRunExecutionEventInput[] {
  const run = input.store.getFleetRun(input.event.runRef)
  if (run === null || run.authorityBinding?.phase !== "accepted") return []
  const started: PylonFleetRunExecutionEventInput = {
    schema: "openagents.pylon.fleet_run_execution_event.v1",
    kind: "run_started",
    observedAt: run.startedAt ?? run.createdAt,
  }
  const observedAt = observedAtFor(input.store, input.event)
  if (input.event.kind === "dispatch") {
    const usageEvidence = terminalUsageProjection(input.event)
    if (
      input.event.status === "completed" &&
      input.event.assignmentRef !== null &&
      input.event.accountRefHash !== null &&
      input.event.closeoutRef !== null &&
      usageEvidence !== null
    ) {
      return [started, {
        schema: "openagents.pylon.fleet_run_execution_event.v1",
        kind: "work_terminal",
        observedAt,
        unitRef: input.event.workUnitRef,
        workClaimRef: input.event.claimRef,
        assignmentRef: input.event.assignmentRef,
        workerKind: input.event.workerKind,
        accountRefHash: input.event.accountRefHash,
        terminalState: "accepted",
        closeoutRef: input.event.closeoutRef,
        usageEvidence,
        blockerRefs: [...new Set(input.event.blockerRefs)].slice(0, 32),
      }]
    }
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v1",
      kind: "work_progress",
      observedAt,
      unitRef: input.event.workUnitRef,
      workClaimRef: input.event.claimRef,
      ...(input.event.assignmentRef === null ? {} : { assignmentRef: input.event.assignmentRef }),
      workerKind: input.event.workerKind,
      ...(input.event.accountRefHash === null ? {} : { accountRefHash: input.event.accountRefHash }),
      blockerRefs: [...new Set(input.event.blockerRefs)].slice(0, 32),
    }]
  }
  if (input.event.kind === "completed") {
    return [started, {
      schema: "openagents.pylon.fleet_run_execution_event.v1",
      kind: "run_terminal",
      observedAt,
      terminalState: "completed",
      blockerRefs: [],
    }]
  }
  return [started]
}
