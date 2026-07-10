import { Schema as S } from "effect"

import {
  FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA,
  FleetRunInterruptedCloseoutSchema,
} from "./fleet-run-recovery.js"
import { fleetRunTaskIdForClaim } from "./fleet-run-refs.js"
import type { FleetRunSupervisorPlanner } from "./fleet-run-supervisor.js"
import type { PylonOrchestrationStore, WorkClaim } from "./store.js"
import {
  planDagWork,
  planFixtureWork,
  planGithubBacklogWork,
  planIssueListWork,
  type GithubBacklogGhRunner,
} from "./work-planner.js"

export type FleetRunDurablePlannerFailure =
  | "corrupt_descriptor"
  | "github_runner_missing"
  | "missing_descriptor"
  | "unknown_run"

const blockerForFailure = (failure: FleetRunDurablePlannerFailure): string =>
  `blocker.pylon.fleet_run.work_source_${failure}`

const publicRunRef = (runRef: string): string => {
  const normalized = runRef.trim()
  return /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u.test(normalized)
    ? normalized
    : "fleet_run.unavailable"
}

export class FleetRunDurablePlannerError extends Error {
  readonly blockerRefs: readonly string[]
  readonly failure: FleetRunDurablePlannerFailure
  readonly runRef: string

  constructor(runRef: string, failure: FleetRunDurablePlannerFailure) {
    const safeRunRef = publicRunRef(runRef)
    super(`FleetRun ${safeRunRef} planner unavailable: ${failure}`)
    this.name = "FleetRunDurablePlannerError"
    this.runRef = safeRunRef
    this.failure = failure
    this.blockerRefs = [blockerForFailure(failure)]
  }
}

export type CreatePylonDurableFleetRunPlannerInput = {
  readonly gh?: GithubBacklogGhRunner | undefined
  readonly store: PylonOrchestrationStore
}

const completedWorkUnitRefs = (claims: readonly WorkClaim[]): readonly string[] =>
  [...new Set(claims.filter((claim) => claim.state === "closeout").map((claim) => claim.workUnitRef))]

const interruptedClaim = (store: PylonOrchestrationStore, claim: WorkClaim): boolean => {
  const task = store.getTask(fleetRunTaskIdForClaim(claim.runRef, claim.claimRef))
  if (task?.result === null || task?.result === undefined) return false
  try {
    const closeout = S.decodeUnknownSync(FleetRunInterruptedCloseoutSchema)(JSON.parse(task.result), {
      onExcessProperty: "error",
    })
    return closeout.schema === FLEET_RUN_INTERRUPTED_CLOSEOUT_SCHEMA && closeout.claimRef === claim.claimRef
  } catch {
    return false
  }
}

const failedWorkUnitRefs = (
  store: PylonOrchestrationStore,
  claims: readonly WorkClaim[],
): readonly string[] => {
  const completed = new Set(completedWorkUnitRefs(claims))
  return [...new Set(claims
    .filter((claim) => !completed.has(claim.workUnitRef))
    .filter((claim) => claim.state === "released" || claim.state === "expired")
    // A typed interrupted closeout reserves a replacement; it is not evidence
    // that the work unit itself failed and must not poison DAG retry planning.
    .filter((claim) => !interruptedClaim(store, claim))
    .map((claim) => claim.workUnitRef))]
}

/** Resolve plans only from the schema-decoded descriptor persisted with a run. */
export function createPylonDurableFleetRunPlanner(
  input: CreatePylonDurableFleetRunPlannerInput,
): FleetRunSupervisorPlanner {
  return {
    plan: async ({ run, now }) => {
      let stored
      try {
        stored = input.store.getFleetRun(run.runRef)
      } catch {
        throw new FleetRunDurablePlannerError(run.runRef, "corrupt_descriptor")
      }
      if (stored === null) throw new FleetRunDurablePlannerError(run.runRef, "unknown_run")
      const source = stored.workSourceDescriptor
      if (source === undefined) {
        throw new FleetRunDurablePlannerError(run.runRef, "missing_descriptor")
      }
      if (source.kind !== stored.workSource) {
        throw new FleetRunDurablePlannerError(run.runRef, "corrupt_descriptor")
      }

      const claims = input.store.listWorkClaims({ runRef: run.runRef })
      const options = {
        claimRegistry: input.store,
        completedWorkUnitRefs: completedWorkUnitRefs(claims),
        now,
        unitRefMode: stored.authorityBinding?.source === "sarah_authority"
          ? "sarah_authority" as const
          : "local" as const,
      }
      if (source.kind === "fixture") return planFixtureWork(source, options)
      if (source.kind === "issue_list") return planIssueListWork(source, options)
      if (source.kind === "plan_dag") {
        return planDagWork(source, {
          ...options,
          failedWorkUnitRefs: failedWorkUnitRefs(input.store, claims),
        })
      }
      if (input.gh === undefined) {
        throw new FleetRunDurablePlannerError(run.runRef, "github_runner_missing")
      }
      return await planGithubBacklogWork(source, input.gh, options)
    },
  }
}
