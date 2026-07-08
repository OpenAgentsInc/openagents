import { randomUUID } from "node:crypto"
import { mkdirSync } from "node:fs"
import { appendFile, mkdir } from "node:fs/promises"
import { Database } from "bun:sqlite"
import { Effect, Exit, Scope } from "effect"
import { dirname, join } from "node:path"

import {
  createPylonOrchestrationStore,
  syncFleetRunsToOwnerLocalState,
  type FleetRun,
  type FleetRunControlVerb,
  type FleetRunState,
  type FleetRunStopCondition,
  type FleetRunWorkSource,
  type WorkClaim,
  type PylonOrchestrationStore,
} from "../../../../apps/pylon/src/orchestration/store.js"
import {
  planDagWork,
  planFixtureWork,
  planGithubBacklogWork,
  planIssueListWork,
  validatePlanDagWorkSource,
  type FixtureWorkSource,
  type GithubBacklogWorkSource,
  type IssueListItem,
  type IssueListWorkSource,
  type PlanDagWorkSource,
  type PlanDagWorkUnit,
  type WorkPlannerOutput,
} from "../../../../apps/pylon/src/orchestration/work-planner.js"
import type { KhalaCodeDesktopFleetRunSupervisorRpc } from "./rpc-handlers.js"
import {
  inspectCodexFleet,
  resolvePylonHome,
  type KhalaCodexFleetToolOptions,
} from "./khala-fleet-tools.js"
import { collectKhalaProcessText, spawnKhalaProcess } from "./khala-process.js"
import { narrowToDelegateWorkerKind } from "@openagentsinc/khala-tools"
import { makePylonService, type PylonServiceShape } from "./pylon-service.js"
import {
  startFleetRunSupervisor,
  type FleetRunSupervisorCapacity,
  type FleetRunSupervisorHandle,
  type FleetRunSupervisorLifecycleEvent,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorPlanner,
  type FleetRunSupervisorRunner,
} from "./fleet-run-supervisor.js"
import type {
  KhalaCodeDesktopFleetRunControlRequest,
  KhalaCodeDesktopFleetRunListRequest,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetRunStartRequest,
  KhalaCodeDesktopFleetWorkerControlRequest,
  KhalaCodeDesktopFleetWorkerControlResult,
} from "../shared/rpc.js"

type ChatEnv = Readonly<Record<string, string | undefined>>

type ActiveSupervisor = {
  readonly handle: FleetRunSupervisorHandle
  readonly scope: Scope.Scope
}

type RpcFleetRunWorkSource = KhalaCodeDesktopFleetRunStartRequest["workSource"]

type RpcIssueListEntry = NonNullable<RpcFleetRunWorkSource["issues"]>[number]
type RpcIssueListItem = Exclude<RpcIssueListEntry, number>

type RpcPlanDagNode = NonNullable<RpcFleetRunWorkSource["nodes"]>[number]

type SupervisorWorkSource = IssueListWorkSource | FixtureWorkSource | GithubBacklogWorkSource | PlanDagWorkSource

const trimRequired = (field: string, value: string): string => {
  const trimmed = value.trim()
  if (trimmed.length === 0) throw new Error(`fleetRunStart plan_dag ${field} is required`)
  return trimmed
}

const trimOptional = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? undefined : trimmed
}

export type KhalaCodeDesktopFleetRunSupervisorRpcAdapterOptions = {
  readonly capacity?: FleetRunSupervisorCapacity
  readonly env?: ChatEnv
  readonly onLifecycleNdjson?: (line: string) => void | Promise<void>
  readonly pylonService?: PylonServiceShape | undefined
  readonly pylonRef?: string | null
  readonly runner?: FleetRunSupervisorRunner
  readonly store?: PylonOrchestrationStore
  readonly tickIntervalMs?: number
  readonly toolOptions?: KhalaCodexFleetToolOptions
}

const DEFAULT_VERIFY = "bun run --cwd clients/khala-code-desktop verify"

const isIssueState = (value: unknown): value is NonNullable<IssueListItem["state"]> =>
  value === "open" ||
  value === "closed" ||
  value === "merged" ||
  value === "OPEN" ||
  value === "CLOSED" ||
  value === "MERGED"

const normalizeIssueState = (value: unknown): IssueListItem["state"] | undefined => {
  if (!isIssueState(value)) return undefined
  const normalized = value.toLowerCase()
  return normalized === "closed" || normalized === "merged" ? normalized : "open"
}

const normalizeIssueItem = (item: number | RpcIssueListItem | IssueListItem): number | IssueListItem => {
  if (typeof item === "number") return item
  const normalized: IssueListItem = {
    number: item.number,
    ...(item.kind === undefined ? {} : { kind: item.kind }),
    ...(item.labels === undefined ? {} : { labels: [...item.labels] }),
    ...(item.title === undefined ? {} : { title: item.title }),
    ...(item.url === undefined ? {} : { url: item.url }),
  }
  const state = normalizeIssueState(item.state)
  return state === undefined ? normalized : { ...normalized, state }
}

const normalizeRpcIssueItem = (
  item: RpcIssueListEntry,
): number | IssueListItem => normalizeIssueItem(item as number | RpcIssueListItem)

const normalizePlanDagNode = (node: RpcPlanDagNode): PlanDagWorkUnit => ({
  ref: trimRequired("node ref", node.ref),
  title: trimRequired("node title", node.title),
  objective: trimRequired("node objective", node.objective),
  ...(node.dependsOn === undefined ? {} : { dependsOn: node.dependsOn.map(ref => ref.trim()) }),
  ...(trimOptional(node.repo) === undefined ? {} : { repo: trimOptional(node.repo)! }),
  ...(trimOptional(node.branch) === undefined ? {} : { branch: trimOptional(node.branch)! }),
  ...(trimOptional(node.baseCommit) === undefined ? {} : { baseCommit: trimOptional(node.baseCommit)! }),
  ...(trimOptional(node.verify) === undefined ? {} : { verify: trimOptional(node.verify)! }),
  ...(node.issue === undefined ? {} : { issue: node.issue }),
  ...(node.labels === undefined ? {} : { labels: [...node.labels] }),
  ...(trimOptional(node.url) === undefined ? {} : { url: trimOptional(node.url)! }),
})

const normalizeWorkSource = (
  source: KhalaCodeDesktopFleetRunStartRequest["workSource"],
): SupervisorWorkSource => {
  if (source.kind === "issue_list") {
    if (source.repo === undefined || source.repo.trim().length === 0) {
      throw new Error("fleetRunStart issue_list workSource requires repo")
    }
    return {
      kind: "issue_list",
      repo: source.repo,
      issues: [...(source.issues ?? [])].map(issue => normalizeRpcIssueItem(issue as Parameters<typeof normalizeRpcIssueItem>[0])),
    }
  }
  if (source.kind === "github_backlog") {
    if (source.repo === undefined || source.repo.trim().length === 0) {
      throw new Error("fleetRunStart github_backlog workSource requires repo")
    }
    return {
      kind: "github_backlog",
      repo: source.repo,
      ...(source.limit === undefined ? {} : { limit: Math.max(1, Math.trunc(source.limit)) }),
    }
  }
  if (source.kind === "plan_dag") {
    if (source.planRef === undefined || source.planRef.trim().length === 0) {
      throw new Error("fleetRunStart plan_dag workSource requires planRef")
    }
    if (source.nodes === undefined || source.nodes.length === 0) {
      throw new Error("fleetRunStart plan_dag workSource requires at least one node")
    }
    return validatePlanDagWorkSource({
      kind: "plan_dag",
      planRef: source.planRef.trim(),
      nodes: source.nodes.map(normalizePlanDagNode),
      ...(trimOptional(source.repo) === undefined ? {} : { repo: trimOptional(source.repo)! }),
      ...(trimOptional(source.branch) === undefined ? {} : { branch: trimOptional(source.branch)! }),
      ...(trimOptional(source.baseCommit) === undefined ? {} : { baseCommit: trimOptional(source.baseCommit)! }),
      ...(trimOptional(source.verify) === undefined ? {} : { verify: trimOptional(source.verify)! }),
    })
  }
  return {
    kind: "fixture",
    ...(source.count === undefined ? {} : { count: Math.max(1, Math.trunc(source.count)) }),
  }
}

const workSourceKind = (
  source: SupervisorWorkSource,
): FleetRunWorkSource => source.kind

const runRefFor = (): string => `fleet_run.${randomUUID()}`

const projectWorkSource = (
  source: SupervisorWorkSource | undefined,
  fallback: FleetRunWorkSource,
): KhalaCodeDesktopFleetRunProjection["workSource"] => {
  if (source === undefined) return { kind: fallback }
  if (source.kind === "issue_list") {
    return {
      kind: "issue_list",
      repo: source.repo,
      issues: source.issues.map(issue => typeof issue === "number" ? issue : normalizeIssueItem(issue)),
    }
  }
  if (source.kind === "github_backlog") {
    return {
      kind: "github_backlog",
      repo: source.repo,
      ...(source.limit === undefined ? {} : { limit: source.limit }),
    }
  }
  if (source.kind === "plan_dag") {
    return {
      kind: "plan_dag",
      planRef: source.planRef,
      nodes: source.nodes.map(node => ({
        ref: node.ref,
        title: node.title,
        objective: node.objective,
        ...(node.dependsOn === undefined ? {} : { dependsOn: [...node.dependsOn] }),
        ...(node.repo === undefined ? {} : { repo: node.repo }),
        ...(node.branch === undefined ? {} : { branch: node.branch }),
        ...(node.baseCommit === undefined ? {} : { baseCommit: node.baseCommit }),
        ...(node.verify === undefined ? {} : { verify: node.verify }),
        ...(node.issue === undefined ? {} : { issue: node.issue }),
        ...(node.labels === undefined ? {} : { labels: [...node.labels] }),
        ...(node.url === undefined ? {} : { url: node.url }),
      })),
      ...(source.repo === undefined ? {} : { repo: source.repo }),
      ...(source.branch === undefined ? {} : { branch: source.branch }),
      ...(source.baseCommit === undefined ? {} : { baseCommit: source.baseCommit }),
      ...(source.verify === undefined ? {} : { verify: source.verify }),
    }
  }
  return {
    kind: "fixture",
    ...(source.count === undefined ? {} : { count: source.count }),
  }
}

const projectRun = (
  run: FleetRun,
  input: {
    readonly pylonRef: string | null
    readonly workSource?: SupervisorWorkSource
  },
): KhalaCodeDesktopFleetRunProjection => ({
  counters: run.counters,
  createdAt: run.createdAt,
  dispatchKind: "supervised_dispatch",
  objectiveProjected: false,
  pylonRef: input.pylonRef,
  refillPolicy: run.refillPolicy,
  runRef: run.runRef,
  startedAt: run.startedAt,
  state: run.state,
  targetConcurrency: run.targetConcurrency,
  updatedAt: run.updatedAt,
  workerKind: run.workerKind,
  workSource: projectWorkSource(input.workSource, run.workSource),
})

const ghJson = async (args: readonly string[]): Promise<string> => {
  const result = await collectKhalaProcessText(
    spawnKhalaProcess("gh", args, { forceKillAfter: "1500 millis" }),
  )
  if (result.exitCode === 0) return result.stdout
  throw new Error(`gh ${args.join(" ")} failed: ${result.stderr.trim()}`)
}

const completedWorkUnitRefsForRun = (claims: readonly WorkClaim[]): readonly string[] =>
  [...new Set(claims.filter(claim => claim.state === "closeout").map(claim => claim.workUnitRef))]

const failedWorkUnitRefsForRun = (claims: readonly WorkClaim[]): readonly string[] => {
  const completed = new Set(completedWorkUnitRefsForRun(claims))
  return [...new Set(claims
    .filter(claim => !completed.has(claim.workUnitRef))
    .filter(claim => claim.state === "released" || claim.state === "expired")
    .map(claim => claim.workUnitRef))]
}

const plannerFor = (
  store: PylonOrchestrationStore,
  sources: ReadonlyMap<string, SupervisorWorkSource>,
): FleetRunSupervisorPlanner => ({
  plan: async ({ run, now }): Promise<WorkPlannerOutput> => {
    const source = sources.get(run.runRef)
    if (source === undefined) return planFixtureWork({ kind: "fixture", count: 0 }, { now })
    if (source.kind === "fixture") return planFixtureWork(source, { now })
    if (source.kind === "issue_list") return planIssueListWork(source, { claimRegistry: store, now })
    if (source.kind === "plan_dag") {
      const claims = store.listWorkClaims({ runRef: run.runRef })
      return planDagWork(source, {
        claimRegistry: store,
        completedWorkUnitRefs: completedWorkUnitRefsForRun(claims),
        failedWorkUnitRefs: failedWorkUnitRefsForRun(claims),
        now,
      })
    }
    return planGithubBacklogWork(source, ghJson, { claimRegistry: store, now })
  },
})

const capacityFor = (options: KhalaCodexFleetToolOptions | undefined): FleetRunSupervisorCapacity => ({
  accounts: async ({ run }) => {
    const status = await inspectCodexFleet({
      includeProcesses: false,
      includeRateLimits: false,
      startPylon: true,
      workerKind: narrowToDelegateWorkerKind(run.workerKind),
    }, options)
    return status.accounts
      .filter(account => !account.paused && (
        run.workerKind === "auto" ||
        account.provider === (run.workerKind === "claude" ? "claude_agent" : "codex")
      ))
      .map(account => ({
        accountRef: account.accountRef,
        advertisedCapacity: Math.max(0, Math.trunc(
          account.capacity?.available ??
          account.capacity?.ready ??
          (account.readiness === "ready" || account.readiness === "available" ? 1 : 0),
        )),
        // Carry the account's concrete kind so a mixed `auto` run labels each
        // dispatch by the harness that actually claimed the unit.
        workerKind: account.provider === "claude_agent" ? "claude" as const : "codex" as const,
      }))
  },
})

const runnerFor = (input: {
  readonly env?: ChatEnv | undefined
  readonly pylonRef: string | null
  readonly pylonService?: PylonServiceShape | undefined
  readonly toolOptions?: KhalaCodexFleetToolOptions
}): FleetRunSupervisorRunner => ({
  dispatch: async dispatch => {
    const fixture = dispatch.workUnit.kind === "fixture"
    const service = input.pylonService ?? input.toolOptions?.pylonService ?? makePylonService({
      env: input.env ?? input.toolOptions?.env,
      runner: input.toolOptions?.runner,
    })
    return await Effect.runPromise(service.runAssignment({
      accountRef: dispatch.accountRef,
      branch: dispatch.workUnit.branch,
      commit: dispatch.workUnit.baseCommit,
      fixture,
      objective: dispatch.workUnit.body ?? dispatch.run.objective,
      pylonRef: input.pylonRef ?? undefined,
      repo: dispatch.workUnit.repo,
      verify: fixture ? undefined : dispatch.workUnit.verify ?? DEFAULT_VERIFY,
      // Use the per-account kind the supervisor resolved for THIS claim (mixed
      // pools dispatch codex and claude side by side); grok never reaches here.
      workerKind: dispatch.workerKind,
    }))
  },
})

const defaultStore = (env: ChatEnv): PylonOrchestrationStore => {
  const home = resolvePylonHome(env)
  mkdirSync(home, { recursive: true })
  return createPylonOrchestrationStore(new Database(join(home, "orchestration.sqlite")))
}

const manualInboxLedgerPath = (home: string): string =>
  join(home, "fleet-worker-inbox.jsonl")

const inboxRefFor = (request: KhalaCodeDesktopFleetWorkerControlRequest, observedAt: string): string =>
  `inbox.assignment.${request.workerRefHash}.${request.verb}.${observedAt.replace(/[^0-9TZ]/g, "")}`

const lifecycleEventLine = (event: FleetRunSupervisorLifecycleEvent): string =>
  `${JSON.stringify(event)}\n`

export function createKhalaCodeDesktopFleetRunSupervisorRpcAdapter(
  options: KhalaCodeDesktopFleetRunSupervisorRpcAdapterOptions = {},
): KhalaCodeDesktopFleetRunSupervisorRpc {
  const env = options.env ?? {}
  const store = options.store ?? defaultStore(env)
  const pylonRef = options.pylonRef ?? null
  const sources = new Map<string, SupervisorWorkSource>()
  const active = new Map<string, ActiveSupervisor>()
  const paths = { home: resolvePylonHome(env) }
  const toolOptions = { ...options.toolOptions, env: { ...env, ...options.toolOptions?.env } }

  const sync = async (): Promise<void> => {
    await syncFleetRunsToOwnerLocalState(store, paths)
  }

  const publishLifecycle = async (event: FleetRunSupervisorObservedEvent): Promise<void> => {
    if (event.kind !== "lifecycle") return
    await options.onLifecycleNdjson?.(lifecycleEventLine(event.event))
  }

  const projectionInput = (
    runRef: string,
  ): { readonly pylonRef: string | null; readonly workSource?: SupervisorWorkSource } => {
    const workSource = sources.get(runRef)
    return workSource === undefined ? { pylonRef } : { pylonRef, workSource }
  }

  const closeSupervisor = async (runRef: string): Promise<void> => {
    const supervisor = active.get(runRef)
    if (supervisor === undefined) return
    await Effect.runPromise(Scope.close(supervisor.scope, Exit.void))
    active.delete(runRef)
  }

  const startSupervisor = async (runRef: string): Promise<boolean> => {
    if (active.has(runRef)) return false
    const scope = Effect.runSync(Scope.make())
    try {
      const handle = await Effect.runPromise(Effect.provideService(
        startFleetRunSupervisor({
          capacity: options.capacity ?? capacityFor(toolOptions),
          planner: plannerFor(store, sources),
          pylonRef: pylonRef ?? "local-pylon",
          runRef,
          runner: options.runner ?? runnerFor({ env, pylonRef, pylonService: options.pylonService, toolOptions }),
          store,
          onLifecycle: publishLifecycle,
          ...(options.tickIntervalMs === undefined ? {} : { tickIntervalMs: options.tickIntervalMs }),
        }),
        Scope.Scope,
        scope,
      ))
      active.set(runRef, { handle, scope })
      return true
    } catch (error) {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      throw error
    }
  }

  const claimForWorkerControl = (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ) => {
    const claims = store.listWorkClaims({
      ...(request.runRef === null ? {} : { runRef: request.runRef }),
    })
    if (request.assignmentRef !== null) {
      return claims.find(claim => claim.assignmentRef === request.assignmentRef) ?? null
    }
    if (request.issueRef !== null) {
      return claims.find(claim => claim.workUnitRef.includes(request.issueRef ?? "")) ?? null
    }
    return null
  }

  const appendManualFlag = async (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ): Promise<string> => {
    const observedAt = new Date().toISOString()
    const ref = inboxRefFor(request, observedAt)
    const row = {
      schemaVersion: "khala-code-desktop.fleet-worker-inbox.v1",
      ref,
      assignmentRef: request.assignmentRef,
      issueRef: request.issueRef,
      note: request.note ?? null,
      observedAt,
      runRef: request.runRef,
      verb: request.verb,
      workerRefHash: request.workerRefHash,
    }
    const path = manualInboxLedgerPath(paths.home)
    await mkdir(dirname(path), { recursive: true })
    await appendFile(path, `${JSON.stringify(row)}\n`, "utf8")
    return ref
  }

  const closeClaimTask = (
    claimRef: string,
    status: "blocked" | "failed",
    summary: string,
  ): void => {
    const task = store.listTasks("dispatched").find(candidate =>
      candidate.id.includes(claimRef.replace(/[^a-zA-Z0-9_.-]/g, "_"))
    )
    if (task === undefined) return
    const context = store.listDispatchContexts("dispatched").find(candidate =>
      candidate.currentTaskId === task.id
    )
    if (context === undefined) return
    store.recordWorkerDone({
      contextId: context.id,
      taskId: task.id,
      status,
      result: JSON.stringify({ assignmentRef: null, summary }),
      maxFailures: Number.MAX_SAFE_INTEGER,
    })
  }

  const workerControl = async (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ): Promise<KhalaCodeDesktopFleetWorkerControlResult> => {
    const claim = claimForWorkerControl(request)
    if (request.verb === "flag") {
      return {
        accepted: true,
        assignmentRef: request.assignmentRef,
        inboxItemRef: await appendManualFlag(request),
        ok: true,
        runRef: request.runRef,
        verb: request.verb,
        workerRefHash: request.workerRefHash,
      }
    }
    if (claim === null) {
      throw new Error(`fleetWorkerControl could not find active claim for ${request.assignmentRef ?? request.issueRef ?? request.workerRefHash}`)
    }
    if (request.verb === "interrupt") {
      if (request.runRef !== null) await closeSupervisor(request.runRef)
      closeClaimTask(claim.claimRef, "blocked", "manual interrupt requested from fleet worker card")
      store.releaseWorkClaim(claim.claimRef)
    } else {
      closeClaimTask(claim.claimRef, "failed", "manual retry requested from fleet worker card")
      store.releaseLiveWorkClaim(claim.workUnitRef)
      const runRef = request.runRef ?? claim.runRef
      const run = store.updateFleetRunState(runRef, "running")
      store.upsertFleetRun({
        ...run,
        counters: {
          ...run.counters,
          workUnitsTotal: run.counters.workUnitsTotal + 1,
        },
      })
      await startSupervisor(runRef)
      await Effect.runPromise(active.get(runRef)?.handle.tick() ?? Effect.void)
    }
    await sync()
    return {
      accepted: true,
      assignmentRef: request.assignmentRef,
      inboxItemRef: null,
      ok: true,
      runRef: request.runRef ?? claim.runRef,
      verb: request.verb,
      workerRefHash: request.workerRefHash,
    }
  }

  return {
    async control(request: KhalaCodeDesktopFleetRunControlRequest) {
      const result = store.controlFleetRun(request.runRef, request.verb as FleetRunControlVerb)
      if (request.verb === "resume") {
        await startSupervisor(request.runRef)
      } else if (request.verb === "pause" || request.verb === "stop") {
        await closeSupervisor(request.runRef)
      }
      await sync()
      return {
        previousState: result.previousState as FleetRunState,
        run: projectRun(result.run, projectionInput(result.run.runRef)),
        supervisorActive: active.has(result.run.runRef),
      }
    },
    async list(request?: KhalaCodeDesktopFleetRunListRequest) {
      return store
        .listFleetRuns(request?.state as FleetRunState | undefined)
        .map(run => projectRun(run, projectionInput(run.runRef)))
    },
    async start(request: KhalaCodeDesktopFleetRunStartRequest) {
      const workSource = normalizeWorkSource(request.workSource)
      const refillPolicy = request.refillPolicy === undefined
        ? undefined
        : {
            ...(request.refillPolicy.cooldownAware === undefined ? {} : { cooldownAware: request.refillPolicy.cooldownAware }),
            ...(request.refillPolicy.maxPerAccount === undefined ? {} : { maxPerAccount: request.refillPolicy.maxPerAccount }),
            ...(request.refillPolicy.stopCondition === undefined
              ? {}
              : { stopCondition: request.refillPolicy.stopCondition as FleetRunStopCondition }),
          }
      const run = store.createFleetRun({
        runRef: request.runRef ?? runRefFor(),
        objective: request.objective,
        ...(refillPolicy === undefined ? {} : { refillPolicy }),
        state: "running",
        targetConcurrency: request.targetConcurrency,
        workerKind: request.workerKind ?? "codex",
        workSource: workSourceKind(workSource),
      })
      sources.set(run.runRef, workSource)
      const supervisorStarted = await startSupervisor(run.runRef)
      if (request.tickImmediately === true) {
        await Effect.runPromise(active.get(run.runRef)?.handle.tick() ?? Effect.void)
      }
      await sync()
      const projected = projectRun(store.getFleetRun(run.runRef) ?? run, { pylonRef, workSource })
      return { run: projected, supervisorStarted }
    },
    async status(request) {
      const run = store.getFleetRun(request.runRef)
      return {
        run: run === null ? null : projectRun(run, projectionInput(run.runRef)),
        supervisorActive: active.has(request.runRef),
      }
    },
    workerControl,
  }
}
