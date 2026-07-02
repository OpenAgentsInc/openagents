import {
  planFixtureWork,
  planIssueListWork,
  type FixtureWorkSource,
  type IssueListWorkSource,
  type WorkPlannerSkippedUnit,
} from "./work-planner.js"
import type { PylonOrchestrationStore, WorkClaim } from "./store.js"

export type FleetRunAcceptanceClaim = {
  readonly claimRef: string
  readonly workUnitRef: string
  readonly workerAccountRef: string
}

export type FleetRunAcceptanceSkip = {
  readonly workUnitRef: string
  readonly workerAccountRef: string
  readonly skipReason: WorkPlannerSkippedUnit["skipReason"]
  readonly detail?: string
}

export type FleetRunAcceptanceResult = {
  readonly runRef: string
  readonly workerCount: number
  readonly totalUnits: number
  readonly claims: readonly FleetRunAcceptanceClaim[]
  readonly duplicateWorkUnitRefs: readonly string[]
  readonly skipped: readonly FleetRunAcceptanceSkip[]
  readonly allSkipsTyped: boolean
}

export type RunFixtureFleetAcceptanceInput = {
  readonly store: PylonOrchestrationStore
  readonly runRef: string
  readonly workerCount: number
  readonly source?: FixtureWorkSource
  readonly now?: Date
}

export type RunDuplicateTemptationAcceptanceInput = {
  readonly store: PylonOrchestrationStore
  readonly runRef: string
  readonly now?: Date
}

type AcceptancePlanSource =
  | { readonly kind: "fixture"; readonly source: FixtureWorkSource }
  | { readonly kind: "issue_list"; readonly source: IssueListWorkSource }

const DEFAULT_TTL_MS = 10 * 60 * 1000

export function runFixtureFleetAcceptance(input: RunFixtureFleetAcceptanceInput): FleetRunAcceptanceResult {
  const source = input.source ?? { kind: "fixture", count: 10 }
  return runAcceptancePlanner({
    store: input.store,
    runRef: input.runRef,
    workerCount: input.workerCount,
    source: { kind: "fixture", source },
    now: input.now ?? new Date(),
    releaseCompletedClaims: true,
  })
}

export function runDuplicateTemptationAcceptance(
  input: RunDuplicateTemptationAcceptanceInput,
): FleetRunAcceptanceResult {
  return runAcceptancePlanner({
    store: input.store,
    runRef: input.runRef,
    workerCount: 2,
    source: {
      kind: "issue_list",
      source: {
        kind: "issue_list",
        repo: "OpenAgentsInc/openagents",
        issues: [{ number: 7838, title: "Juicy duplicate temptation issue" }],
      },
    },
    now: input.now ?? new Date(),
    releaseCompletedClaims: false,
    maxRounds: 1,
  })
}

function runAcceptancePlanner(input: {
  readonly store: PylonOrchestrationStore
  readonly runRef: string
  readonly workerCount: number
  readonly source: AcceptancePlanSource
  readonly now: Date
  readonly releaseCompletedClaims: boolean
  readonly maxRounds?: number
}): FleetRunAcceptanceResult {
  if (!Number.isInteger(input.workerCount) || input.workerCount < 1) {
    throw new Error("acceptance fixture workerCount must be a positive integer")
  }

  const claims: FleetRunAcceptanceClaim[] = []
  const skipped: FleetRunAcceptanceSkip[] = []
  const completedWorkUnits = new Set<string>()
  const workerRefs = Array.from({ length: input.workerCount }, (_, index) => `fixture-worker-${index + 1}`)
  const maxRounds = input.maxRounds ?? Number.POSITIVE_INFINITY
  let round = 0

  while (round < maxRounds) {
    round += 1
    let claimedInRound = 0

    for (const workerAccountRef of workerRefs) {
      const plan = planForSource(input.source, input.store, input.now)
      for (const unit of plan.skipped) {
        skipped.push({
          workUnitRef: unit.workUnitRef,
          workerAccountRef,
          skipReason: unit.skipReason,
          ...(unit.detail === undefined ? {} : { detail: unit.detail }),
        })
      }

      const next = plan.claimable.find((unit) => !completedWorkUnits.has(unit.workUnitRef))
      if (next === undefined) continue

      const claim = input.store.tryClaimWorkUnit({
        claimRef: `${input.runRef}.claim.${claims.length + 1}`,
        workUnitRef: next.workUnitRef,
        runRef: input.runRef,
        assignmentRef: `${input.runRef}.assignment.${claims.length + 1}`,
        workerAccountRef,
        ttl: DEFAULT_TTL_MS,
        now: input.now,
      })

      if (claim === null) {
        const live = input.store.getLiveWorkClaim(next.workUnitRef, input.now)
        skipped.push({
          workUnitRef: next.workUnitRef,
          workerAccountRef,
          skipReason: "already_claimed",
          ...(live === null ? {} : { detail: live.claimRef }),
        })
        continue
      }

      recordClaim({ claim, claims, completedWorkUnits })
      claimedInRound += 1
      if (input.releaseCompletedClaims) input.store.releaseWorkClaim(claim.claimRef, input.now)
    }

    if (claimedInRound === 0) break
  }

  return {
    runRef: input.runRef,
    workerCount: input.workerCount,
    totalUnits: totalUnitsForSource(input.source),
    claims,
    duplicateWorkUnitRefs: duplicateRefs(claims.map((claim) => claim.workUnitRef)),
    skipped,
    allSkipsTyped: skipped.every((skip) => skip.skipReason !== undefined),
  }
}

function planForSource(source: AcceptancePlanSource, store: PylonOrchestrationStore, now: Date) {
  if (source.kind === "fixture") {
    return planFixtureWork(source.source, { now })
  }
  return planIssueListWork(source.source, { claimRegistry: store, now })
}

function totalUnitsForSource(source: AcceptancePlanSource): number {
  if (source.kind === "fixture") {
    return source.source.units?.length ?? source.source.count ?? 10
  }
  return source.source.issues.length + (source.source.pullRequests?.length ?? 0)
}

function recordClaim(input: {
  readonly claim: WorkClaim
  readonly claims: FleetRunAcceptanceClaim[]
  readonly completedWorkUnits: Set<string>
}) {
  input.claims.push({
    claimRef: input.claim.claimRef,
    workUnitRef: input.claim.workUnitRef,
    workerAccountRef: input.claim.workerAccountRef,
  })
  input.completedWorkUnits.add(input.claim.workUnitRef)
}

function duplicateRefs(values: readonly string[]): string[] {
  const seen = new Set<string>()
  const duplicates = new Set<string>()
  for (const value of values) {
    if (seen.has(value)) duplicates.add(value)
    seen.add(value)
  }
  return [...duplicates].sort()
}
