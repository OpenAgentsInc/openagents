import {
  probePylonOwnerLocalAssignmentLiveness,
  type ProbePylonOwnerLocalAssignmentLivenessInput,
} from "../assignment.js"
import type { FleetRunOwnerLocalLivenessProbe } from "./fleet-run-recovery.js"

export type CreatePylonAssignmentFleetRunOwnerLocalLivenessProbeInput = {
  readonly assignmentStatePath: string
  readonly heartbeatStaleAfterMs?: ProbePylonOwnerLocalAssignmentLivenessInput["heartbeatStaleAfterMs"]
  readonly now?: ProbePylonOwnerLocalAssignmentLivenessInput["now"]
  readonly processIsAlive?: ProbePylonOwnerLocalAssignmentLivenessInput["processIsAlive"]
}

/** Pylon-owned bridge from private assignment process evidence to refs-only recovery. */
export function createPylonAssignmentFleetRunOwnerLocalLivenessProbe(
  input: CreatePylonAssignmentFleetRunOwnerLocalLivenessProbeInput,
): FleetRunOwnerLocalLivenessProbe {
  return async ({ assignmentRef }) => {
    if (assignmentRef === null) return "unknown"
    return await probePylonOwnerLocalAssignmentLiveness({
      assignmentRef,
      assignmentStatePath: input.assignmentStatePath,
      ...(input.heartbeatStaleAfterMs === undefined
        ? {}
        : { heartbeatStaleAfterMs: input.heartbeatStaleAfterMs }),
      ...(input.now === undefined ? {} : { now: input.now }),
      ...(input.processIsAlive === undefined ? {} : { processIsAlive: input.processIsAlive }),
    })
  }
}
