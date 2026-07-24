import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import type {
  FullAutoRunClientRunProjection,
  FullAutoRunControlIntentOutcomeReport,
} from "@openagentsinc/khala-sync"
import {
  fetchFullAutoRunControlIntents,
  publishFullAutoRunClientProjection,
  reportFullAutoRunControlIntentOutcome,
  type FullAutoRunControlIntentFetch,
} from "@openagentsinc/khala-sync-client"

import type { FullAutoRunActionContext } from "./full-auto-run-actions.ts"
import {
  applyFullAutoRunControlIntent,
  type FullAutoRunControlIntentOutcome,
} from "./full-auto-run-control-intent.ts"
import type { FullAutoRun } from "./full-auto-run-registry.ts"

export const OMEGA_FULL_AUTO_SYNC_OUTCOMES_SCHEMA =
  "openagents.omega.full_auto_sync_outcomes.v1" as const
const OUTCOME_LIMIT = 256

export type OmegaFullAutoSyncSession = Readonly<{
  baseUrl: string
  accessToken: string
}>

export type OmegaFullAutoSyncStatus = Readonly<{
  available: boolean
  publishBlocksDispatch: false
  reason: string
}>

type OutcomeFile = Readonly<{
  schema: typeof OMEGA_FULL_AUTO_SYNC_OUTCOMES_SCHEMA
  outcomes: ReadonlyArray<FullAutoRunControlIntentOutcomeReport>
}>

const loadOutcomes = (filePath: string): Map<string, FullAutoRunControlIntentOutcomeReport> => {
  try {
    const value = JSON.parse(readFileSync(filePath, "utf8")) as Partial<OutcomeFile>
    if (value.schema !== OMEGA_FULL_AUTO_SYNC_OUTCOMES_SCHEMA || !Array.isArray(value.outcomes)) {
      return new Map()
    }
    return new Map(
      value.outcomes.flatMap((outcome) =>
        typeof outcome?.intentId === "string" &&
        (outcome.status === "applied" || outcome.status === "rejected")
          ? [[outcome.intentId, outcome] as const]
          : [],
      ),
    )
  } catch {
    return new Map()
  }
}

const persistOutcomes = (
  filePath: string,
  outcomes: ReadonlyMap<string, FullAutoRunControlIntentOutcomeReport>,
): void => {
  mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 })
  const bounded = [...outcomes.values()].slice(-OUTCOME_LIMIT)
  const temporaryPath = `${filePath}.tmp`
  writeFileSync(
    temporaryPath,
    `${JSON.stringify({ schema: OMEGA_FULL_AUTO_SYNC_OUTCOMES_SCHEMA, outcomes: bounded } satisfies OutcomeFile, null, 2)}\n`,
    { encoding: "utf8", mode: 0o600 },
  )
  renameSync(temporaryPath, filePath)
}

const workspaceLabel = (workspaceRef: string | undefined): string | null => {
  if (workspaceRef === undefined) return null
  const label = path.basename(workspaceRef).slice(0, 200)
  return label.length > 0 && !/[/\\]/u.test(label) ? label : null
}

/** Public-safe projection. It deliberately names fields one by one. */
export const projectOmegaFullAutoRunForSync = (
  run: FullAutoRun,
): FullAutoRunClientRunProjection | null => {
  const transition = run.transitions.at(-1)
  if (transition === undefined) return null
  return {
    runRef: run.runRef,
    threadRef: run.threadRef ?? null,
    objective: run.objective,
    doneCondition: run.doneCondition,
    lifecycleState: run.state,
    workspaceLabel: workspaceLabel(run.workspaceRef),
    startedAt: run.startedAt ?? null,
    updatedAt: transition.at,
    lastTransition: { actor: transition.actor, at: transition.at },
    laneRef: run.profile?.lane ?? null,
    accountRef: run.profile?.accountRef ?? null,
    turnCap: run.turnCap,
    successfulAttempts: run.successfulAttempts,
    failedAttempts: run.failedAttempts,
    rotationCount: 0,
    receiptSummary: null,
  }
}

const toOutcomeReport = (
  outcome: FullAutoRunControlIntentOutcome,
): FullAutoRunControlIntentOutcomeReport =>
  outcome.status === "applied"
    ? {
        intentId: outcome.intentId,
        status: "applied",
        resultLifecycleState: outcome.resultLifecycleState as FullAutoRunControlIntentOutcomeReport["resultLifecycleState"],
      }
    : {
        intentId: outcome.intentId,
        status: "rejected",
        rejectionReason: outcome.rejectionReason,
      }

export type OmegaFullAutoSync = Readonly<{
  status: () => OmegaFullAutoSyncStatus
  refreshStatus: () => Promise<OmegaFullAutoSyncStatus>
  publish: (run: FullAutoRun) => Promise<"published" | "sync_unavailable">
  tick: (runs: ReadonlyArray<FullAutoRun>) => Promise<void>
}>

export const createOmegaFullAutoSync = (input: Readonly<{
  resolveSession: () => Promise<OmegaFullAutoSyncSession | null>
  actionContext: () => FullAutoRunActionContext
  outcomesPath: string
  fetchImpl?: FullAutoRunControlIntentFetch
}>): OmegaFullAutoSync => {
  const outcomes = loadOutcomes(input.outcomesPath)
  let currentStatus: OmegaFullAutoSyncStatus = {
    available: false,
    publishBlocksDispatch: false,
    reason: "omega_khala_sync_session_unavailable",
  }
  let activeTick: Promise<void> | null = null

  const resolveSession = async (): Promise<OmegaFullAutoSyncSession | null> => {
    try {
      const session = await input.resolveSession()
      if (
        session === null ||
        session.accessToken.trim() === "" ||
        !/^https:\/\//u.test(session.baseUrl)
      ) {
        currentStatus = {
          available: false,
          publishBlocksDispatch: false,
          reason: "omega_khala_sync_session_unavailable",
        }
        return null
      }
      currentStatus = {
        available: true,
        publishBlocksDispatch: false,
        reason: "omega_khala_sync_session_ready",
      }
      return session
    } catch {
      currentStatus = {
        available: false,
        publishBlocksDispatch: false,
        reason: "omega_khala_sync_session_unavailable",
      }
      return null
    }
  }

  const publish = async (run: FullAutoRun): Promise<"published" | "sync_unavailable"> => {
    const session = await resolveSession()
    if (session === null) return "sync_unavailable"
    const projection = projectOmegaFullAutoRunForSync(run)
    if (projection === null) return "sync_unavailable"
    const result = await publishFullAutoRunClientProjection({
      baseUrl: session.baseUrl,
      accessToken: session.accessToken,
      run: projection,
      ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
    })
    if (result.state !== "published") {
      currentStatus = {
        available: false,
        publishBlocksDispatch: false,
        reason:
          result.state === "unauthorized"
            ? "omega_khala_sync_session_unauthorized"
            : "omega_khala_sync_transport_unavailable",
      }
      return "sync_unavailable"
    }
    return "published"
  }

  const runTick = async (runs: ReadonlyArray<FullAutoRun>): Promise<void> => {
    const session = await resolveSession()
    if (session === null) return

    const activeRun = runs.find((run) =>
      run.state === "running" ||
      run.state === "pausing" ||
      run.state === "paused" ||
      run.state === "retrying" ||
      run.state === "stalled",
    )
    const latestRun = runs.toSorted((left, right) => {
      const leftAt = left.transitions.at(-1)?.at ?? left.createdAt
      const rightAt = right.transitions.at(-1)?.at ?? right.createdAt
      return rightAt.localeCompare(leftAt)
    })[0]
    const projectedRun = activeRun ?? latestRun
    if (projectedRun !== undefined) await publish(projectedRun)

    const listed = await fetchFullAutoRunControlIntents({
      baseUrl: session.baseUrl,
      accessToken: session.accessToken,
      ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
    })
    if (listed.state !== "available") return

    for (const intent of listed.intents) {
      if (intent.status !== "pending") continue
      let outcome = outcomes.get(intent.intentId)
      if (outcome === undefined) {
        outcome = toOutcomeReport(
          applyFullAutoRunControlIntent(input.actionContext(), {
            intentId: intent.intentId,
            runRef: intent.runRef,
            action: intent.action,
          }),
        )
        outcomes.set(intent.intentId, outcome)
        persistOutcomes(input.outcomesPath, outcomes)
      }
      await reportFullAutoRunControlIntentOutcome({
        baseUrl: session.baseUrl,
        accessToken: session.accessToken,
        outcome,
        ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
      })
    }
  }

  return {
    status: () => currentStatus,
    refreshStatus: async () => {
      await resolveSession()
      return currentStatus
    },
    publish,
    tick: (runs) => {
      if (activeTick !== null) return activeTick
      activeTick = runTick(runs).finally(() => {
        activeTick = null
      })
      return activeTick
    },
  }
}
