import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { validatePlanDagWorkSource, type PlanDagWorkUnit } from "./orchestration/work-planner.js"

export type FleetRunSmokeKind = "live" | "sustained"
export type FleetRunSmokeName = "smoke:fleet-run-live" | "smoke:fleet-run-sustained"
export type FleetRunSmokeWorkSource = "issue_list" | "plan_dag"

export type FleetRunSmokeEnv = Readonly<Record<string, string | undefined>>

type FleetRunSmokeConfig = {
  readonly armEnv: string
  readonly defaultPollIntervalMs: number
  readonly defaultTargetWorkers: number
  readonly defaultTimeoutMs: number
  readonly envPrefix: string
  readonly exactIssueCount?: number
  readonly kind: FleetRunSmokeKind
  readonly minDurationMinutes: number | null
  readonly minRefills: number
  readonly minTargetWorkers: number
  readonly smoke: FleetRunSmokeName
}

export type FleetRunSmokePlan = {
  readonly armed: boolean
  readonly armingEnv: string
  readonly baseUrl: string
  readonly branch: string
  readonly commit: string | null
  readonly expectedCloseout: readonly string[]
  readonly failures: readonly string[]
  readonly issues: readonly number[]
  readonly kind: FleetRunSmokeKind
  readonly managerEnv: FleetRunSmokeEnv
  readonly message: string
  readonly minDurationMs: number | null
  readonly minDurationMinutes: number | null
  readonly minRefills: number
  readonly planNodes: readonly PlanDagWorkUnit[]
  readonly planRef: string | null
  readonly pollIntervalMs: number
  readonly repo: string | null
  readonly requiredCloseouts: number
  readonly runRef: string
  readonly smoke: FleetRunSmokeName
  readonly targetWorkers: number
  readonly timeoutMs: number
  readonly verify: string | null
  readonly workerKind: "codex"
  readonly workSource: FleetRunSmokeWorkSource
}

export type FleetRunSmokeRun = {
  readonly counters: {
    readonly activeAssignments: number
    readonly blockedAssignments: number
    readonly completedAssignments: number
    readonly failedAssignments: number
    readonly workUnitsTotal: number
  }
  readonly runRef: string
  readonly state: string
  readonly targetConcurrency: number
  readonly workerKind: string
  readonly workSource: string
}

export type FleetRunSmokeLifecycleEvent = Readonly<Record<string, unknown>>

export type FleetRunSmokeSnapshot = {
  readonly active: boolean
  readonly lifecycle: readonly FleetRunSmokeLifecycleEvent[]
  readonly pylonRef: string | null
  readonly run: FleetRunSmokeRun
}

export type FleetRunSmokeManager = {
  readonly control?: ((input: { readonly runRef: string; readonly verb: "stop" }) => Promise<unknown>) | undefined
  readonly start: (input: FleetRunSmokeManagerStartInput) => Promise<FleetRunSmokeSnapshot>
  readonly status: (input: { readonly runRef: string }) => Promise<FleetRunSmokeSnapshot | readonly FleetRunSmokeSnapshot[]>
}

export type FleetRunSmokeManagerStartInput = {
  readonly baseUrl: string
  readonly branch: string
  readonly commit: string
  readonly issues: readonly number[]
  readonly objective: string
  readonly planNodes?: readonly PlanDagWorkUnit[] | undefined
  readonly planRef?: string | undefined
  readonly pylonRef: string
  readonly repo: string
  readonly runRef: string
  readonly targetConcurrency: number
  readonly timeoutMs: number
  readonly verify: string
  readonly workerKind: "codex"
  readonly workSource: FleetRunSmokeWorkSource
}

export type FleetRunSmokeCloseoutEvidence = {
  readonly assignmentRef: string
  readonly blockerRefs: readonly string[]
  readonly closeoutChecklistOk: boolean
  readonly demandSource: string | null
  readonly ok: boolean
  readonly proofChecklistOk: boolean
  readonly rawEventCount: number
  readonly statusState: string | null
  readonly tokenRefs: readonly string[]
  readonly tokenRows: number
  readonly totalTokens: number
  readonly traceCount: number
  readonly usageTruth: string | null
}

export type FleetRunSmokeCounterReconciliation =
  | {
      readonly afterTokensServed: number
      readonly beforeTokensServed: number
      readonly delta: number
      readonly expectedMinimumDelta: number
      readonly ok: boolean
      readonly state: "checked"
      readonly url: string
    }
  | {
      readonly ok: false
      readonly reason: string
      readonly state: "unavailable"
      readonly url: string
    }

export type FleetRunSmokeEvidence = {
  readonly assignmentRefs: readonly string[]
  readonly closeouts: readonly FleetRunSmokeCloseoutEvidence[]
  readonly completedAt: string | null
  readonly dispatchFailures: readonly string[]
  readonly duplicateWorkUnitRefs: readonly string[]
  readonly publicCounterReconciliation: FleetRunSmokeCounterReconciliation
  readonly pylonRef: string | null
  readonly refillsObserved: number
  readonly run: FleetRunSmokeRun | null
  readonly runObservedDurationMs: number
  readonly startedAt: string
  readonly tokenRows: number
  readonly totalTokens: number
  readonly workUnitRefs: readonly string[]
}

export type FleetRunSmokeResult = {
  readonly ok: boolean
  readonly skipped: boolean
  readonly smoke: FleetRunSmokeName
  readonly armingEnv: string
  readonly targetWorkers: number
  readonly minDurationMinutes?: number | undefined
  readonly minRefills?: number | undefined
  readonly repo?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly pylonRef?: string | undefined
  readonly issues?: readonly number[] | undefined
  readonly planRef?: string | undefined
  readonly verify?: string | undefined
  readonly runRef?: string | undefined
  readonly workSource?: FleetRunSmokeWorkSource | undefined
  readonly expectedCloseout: readonly string[]
  readonly evidence?: FleetRunSmokeEvidence | undefined
  readonly failures: readonly string[]
  readonly message: string
}

export type FleetRunSmokeRunOptions = {
  readonly closeoutReader?: ((assignmentRef: string, plan: FleetRunSmokePlan) => Promise<FleetRunSmokeCloseoutEvidence>) | undefined
  readonly createManager?: ((env: FleetRunSmokeEnv, plan: FleetRunSmokePlan) => FleetRunSmokeManager | Promise<FleetRunSmokeManager>) | undefined
  readonly env?: FleetRunSmokeEnv | undefined
  readonly fetch?: typeof fetch | undefined
  readonly manager?: FleetRunSmokeManager | undefined
  readonly now?: (() => Date) | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
}

const DEFAULT_BASE_URL = "https://openagents.com"
const LIVE_TIMEOUT_MS = 2 * 60 * 60 * 1000
const SUSTAINED_TIMEOUT_MS = 4 * 60 * 60 * 1000

const smokeConfigs: Record<FleetRunSmokeKind, FleetRunSmokeConfig> = {
  live: {
    armEnv: "PYLON_FLEET_RUN_LIVE_ARM",
    defaultPollIntervalMs: 10_000,
    defaultTargetWorkers: 2,
    defaultTimeoutMs: LIVE_TIMEOUT_MS,
    envPrefix: "PYLON_FLEET_RUN_LIVE",
    exactIssueCount: 2,
    kind: "live",
    minDurationMinutes: null,
    minRefills: 0,
    minTargetWorkers: 2,
    smoke: "smoke:fleet-run-live",
  },
  sustained: {
    armEnv: "PYLON_FLEET_RUN_SUSTAINED_ARM",
    defaultPollIntervalMs: 30_000,
    defaultTargetWorkers: 5,
    defaultTimeoutMs: SUSTAINED_TIMEOUT_MS,
    envPrefix: "PYLON_FLEET_RUN_SUSTAINED",
    kind: "sustained",
    minDurationMinutes: 30,
    minRefills: 2,
    minTargetWorkers: 5,
    smoke: "smoke:fleet-run-sustained",
  },
}

export function buildFleetRunSmokePlan(
  kind: FleetRunSmokeKind,
  env: FleetRunSmokeEnv = process.env,
  now: Date = new Date(),
): FleetRunSmokePlan {
  const config = smokeConfigs[kind]
  const armed = env[config.armEnv]?.trim() === "1"
  const armingEnv = `${config.armEnv}=1`
  const failures: string[] = []
  const targetWorkers = parseOptionalInteger(
    env[`${config.envPrefix}_TARGET`],
    config.defaultTargetWorkers,
    `${config.envPrefix}_TARGET`,
    failures,
  )
  const minRefills = parseOptionalInteger(
    env[`${config.envPrefix}_MIN_REFILLS`],
    config.minRefills,
    `${config.envPrefix}_MIN_REFILLS`,
    failures,
  )
  const minDurationMinutes = config.minDurationMinutes === null
    ? null
    : parseOptionalInteger(
        env[`${config.envPrefix}_DURATION_MINUTES`],
        config.minDurationMinutes,
        `${config.envPrefix}_DURATION_MINUTES`,
        failures,
      )
  const minDurationMs = minDurationMinutes === null ? null : minDurationMinutes * 60_000
  const requiredCloseouts = Math.max(targetWorkers, config.minTargetWorkers) +
    Math.max(minRefills, config.minRefills)
  const pollIntervalMs = parseOptionalInteger(
    env[`${config.envPrefix}_POLL_MS`],
    config.defaultPollIntervalMs,
    `${config.envPrefix}_POLL_MS`,
    failures,
  )
  const timeoutMs = parseOptionalInteger(
    env[`${config.envPrefix}_TIMEOUT_MS`],
    config.defaultTimeoutMs,
    `${config.envPrefix}_TIMEOUT_MS`,
    failures,
  )
  const baseUrl = firstTrimmed([
    env[`${config.envPrefix}_BASE_URL`],
    env.PYLON_OPENAGENTS_BASE_URL,
    env.OPENAGENTS_BASE_URL,
    DEFAULT_BASE_URL,
  ])
  const branch = firstTrimmed([env[`${config.envPrefix}_BRANCH`], "main"])
  const runRef = firstTrimmed([
    env[`${config.envPrefix}_RUN_REF`],
    `fleet_run.${kind}_smoke.${compactTimestamp(now)}`,
  ])
  const workerKindRaw = firstTrimmed([env[`${config.envPrefix}_WORKER_KIND`], "codex"])
  const workSource = parseWorkSource(
    firstTrimmed([env[`${config.envPrefix}_WORK_SOURCE`], "issue_list"]),
    `${config.envPrefix}_WORK_SOURCE`,
    config,
    failures,
  )

  let pylonRef: string | null = null
  let repo: string | null = null
  let commit: string | null = null
  let verify: string | null = null
  let issues: readonly number[] = []
  let planRef: string | null = null
  let planNodes: readonly PlanDagWorkUnit[] = []

  if (armed) {
    pylonRef = requireTrimmed(env, `${config.envPrefix}_PYLON_REF`, config.armEnv, failures)
    repo = requireTrimmed(env, `${config.envPrefix}_REPO`, config.armEnv, failures)
    commit = requireTrimmed(env, `${config.envPrefix}_COMMIT`, config.armEnv, failures)
    verify = requireTrimmed(env, `${config.envPrefix}_VERIFY`, config.armEnv, failures)
    requireTrimmed(env, "OPENAGENTS_AGENT_TOKEN", config.armEnv, failures)
    if (workSource === "issue_list") {
      issues = parseIssues(
        requireTrimmed(env, `${config.envPrefix}_ISSUES`, config.armEnv, failures),
        `${config.envPrefix}_ISSUES`,
        failures,
      )
    } else {
      planRef = requireTrimmed(env, `${config.envPrefix}_PLAN_REF`, config.armEnv, failures)
      planNodes = parsePlanNodesJson(
        requireTrimmed(env, `${config.envPrefix}_PLAN_NODES_JSON`, config.armEnv, failures),
        `${config.envPrefix}_PLAN_NODES_JSON`,
        failures,
      )
    }
  }

  if (armed && commit !== null && !/^[0-9a-f]{40}$/iu.test(commit)) {
    failures.push(`${config.envPrefix}_COMMIT must be a pinned 40-character commit SHA`)
  }
  if (armed && targetWorkers < config.minTargetWorkers) {
    failures.push(`${config.envPrefix}_TARGET must be at least ${config.minTargetWorkers}`)
  }
  if (armed && config.exactIssueCount !== undefined && targetWorkers !== config.defaultTargetWorkers) {
    failures.push(`${config.envPrefix}_TARGET must be exactly ${config.defaultTargetWorkers}`)
  }
  if (armed && minRefills < config.minRefills) {
    failures.push(`${config.envPrefix}_MIN_REFILLS must be at least ${config.minRefills}`)
  }
  if (armed && minDurationMinutes !== null && minDurationMinutes < config.minDurationMinutes!) {
    failures.push(`${config.envPrefix}_DURATION_MINUTES must be at least ${config.minDurationMinutes}`)
  }
  if (armed && pollIntervalMs < 100) {
    failures.push(`${config.envPrefix}_POLL_MS must be at least 100`)
  }
  if (armed && timeoutMs <= 0) {
    failures.push(`${config.envPrefix}_TIMEOUT_MS must be positive`)
  }
  if (armed && minDurationMs !== null && timeoutMs <= minDurationMs) {
    failures.push(`${config.envPrefix}_TIMEOUT_MS must be greater than ${config.envPrefix}_DURATION_MINUTES`)
  }
  if (armed && workerKindRaw !== "codex") {
    failures.push(`${config.envPrefix}_WORKER_KIND must be codex; this smoke proves exact rows through pylon khala closeout`)
  }
  if (armed && workSource === "issue_list" && config.exactIssueCount !== undefined && issues.length !== config.exactIssueCount) {
    failures.push(`${config.envPrefix}_ISSUES must contain exactly ${config.exactIssueCount} positive issue numbers`)
  }
  if (armed && workSource === "issue_list" && config.exactIssueCount === undefined && issues.length < requiredCloseouts) {
    failures.push(`${config.envPrefix}_ISSUES must contain at least ${requiredCloseouts} distinct issue numbers`)
  }
  if (armed && workSource === "plan_dag" && planNodes.length < requiredCloseouts) {
    failures.push(`${config.envPrefix}_PLAN_NODES_JSON must contain at least ${requiredCloseouts} distinct plan node(s)`)
  }
  if (armed && workSource === "plan_dag" && planRef !== null && repo !== null && commit !== null && verify !== null && planNodes.length > 0) {
    try {
      validatePlanDagWorkSource({
        kind: "plan_dag",
        planRef,
        repo,
        branch,
        baseCommit: commit,
        verify,
        nodes: planNodes,
      })
    } catch (error) {
      failures.push(`${config.envPrefix}_PLAN_NODES_JSON is invalid: ${errorMessage(error)}`)
    }
  }

  return {
    armed,
    armingEnv,
    baseUrl,
    branch,
    commit: commit === null ? null : commit.toLowerCase(),
    expectedCloseout: expectedCloseoutFor(config, targetWorkers, minDurationMinutes, minRefills),
    failures,
    issues,
    kind,
    managerEnv: {
      ...env,
      PYLON_OPENAGENTS_BASE_URL: baseUrl,
    },
    message: armed
      ? failures.length === 0
        ? `${config.smoke} armed; starting a real supervised FleetRun and proving closeout/token evidence.`
        : `${config.smoke} is armed but its inputs are invalid.`
      : `Skipped by default. Arm with ${armingEnv} plus ${requiredEnvList(config).join(", ")}.`,
    minDurationMs,
    minDurationMinutes,
    minRefills,
    planNodes,
    planRef,
    pollIntervalMs,
    repo,
    requiredCloseouts,
    runRef,
    smoke: config.smoke,
    targetWorkers,
    timeoutMs,
    verify,
    workerKind: "codex",
    workSource,
  }
}

export async function runFleetRunSmokeFromEnv(
  kind: FleetRunSmokeKind,
  options: FleetRunSmokeRunOptions = {},
): Promise<FleetRunSmokeResult> {
  const env = options.env ?? process.env
  const plan = buildFleetRunSmokePlan(kind, env, options.now?.() ?? new Date())
  if (!plan.armed) return skippedResult(plan)
  if (plan.failures.length > 0) return failedResult(plan, plan.failures)
  return executeFleetRunSmoke(plan, options)
}

export async function executeFleetRunSmoke(
  plan: FleetRunSmokePlan,
  options: FleetRunSmokeRunOptions = {},
): Promise<FleetRunSmokeResult> {
  const closeoutReader = options.closeoutReader ?? readPylonKhalaCloseoutEvidence
  const manager = options.manager ?? await options.createManager?.(plan.managerEnv, plan)
  if (manager === undefined) {
    return failedResult(plan, ["a FleetRun smoke manager is required for armed execution"])
  }
  if (plan.repo === null || plan.commit === null || plan.verify === null) {
    return failedResult(plan, ["armed smoke plan is missing repo, commit, or verify"])
  }
  const pylonRef = firstTrimmed([plan.managerEnv[`${smokeConfigs[plan.kind].envPrefix}_PYLON_REF`], ""])
  if (pylonRef === "") return failedResult(plan, ["armed smoke plan is missing pylon ref"])

  const now = options.now ?? (() => new Date())
  const sleep = options.sleep ?? Bun.sleep
  const startedAt = now()
  const counterUrl = publicCounterUrl(plan.baseUrl)
  const beforeCounter = await fetchPublicKhalaTokensServed(counterUrl, options.fetch)
  let snapshot: FleetRunSmokeSnapshot | null = null
  let completedAt: Date | null = null
  let timedOut = false
  let terminalFailure: string | null = null

  try {
    snapshot = await manager.start({
      baseUrl: plan.baseUrl,
      branch: plan.branch,
      commit: plan.commit,
      issues: plan.issues,
      objective: objectiveFor(plan),
      ...(plan.workSource === "plan_dag" && plan.planRef !== null
        ? { planNodes: plan.planNodes, planRef: plan.planRef }
        : {}),
      pylonRef,
      repo: plan.repo,
      runRef: plan.runRef,
      targetConcurrency: plan.targetWorkers,
      timeoutMs: plan.timeoutMs,
      verify: plan.verify,
      workerKind: "codex",
      workSource: plan.workSource,
    })

    while (true) {
      const observedNow = now()
      if (snapshot.run.state === "completed") {
        completedAt = observedNow
        break
      }
      if (snapshot.run.state === "stopped") {
        terminalFailure = "fleet run stopped before completion"
        completedAt = observedNow
        break
      }
      if (observedNow.getTime() - startedAt.getTime() >= plan.timeoutMs) {
        timedOut = true
        terminalFailure = `timed out after ${plan.timeoutMs}ms waiting for ${plan.smoke}`
        break
      }
      await sleep(plan.pollIntervalMs)
      snapshot = singleSnapshot(await manager.status({ runRef: plan.runRef }))
    }
  } catch (error) {
    return failedResult(plan, [errorMessage(error)])
  } finally {
    if (timedOut && manager.control !== undefined) {
      await manager.control({ runRef: plan.runRef, verb: "stop" }).catch(() => undefined)
    }
  }

  const observedAt = completedAt ?? now()
  const preCloseoutEvidence = evidenceFromSnapshot({
    completedAt: completedAt?.toISOString() ?? null,
    plan,
    snapshot,
    startedAt: startedAt.toISOString(),
    observedAt,
  })
  const preCloseoutFailures = [
    ...(terminalFailure === null ? [] : [terminalFailure]),
    ...preCloseoutEvidenceFailures(plan, preCloseoutEvidence),
  ]

  const closeoutRefs = preCloseoutEvidence.assignmentRefs
  const closeouts = await Promise.all(
    closeoutRefs.map(assignmentRef =>
      closeoutReader(assignmentRef, plan).catch(error => ({
        assignmentRef,
        blockerRefs: [`blocker.${plan.kind}_smoke.closeout_read_failed`],
        closeoutChecklistOk: false,
        demandSource: null,
        ok: false,
        proofChecklistOk: false,
        rawEventCount: 0,
        statusState: null,
        tokenRefs: [],
        tokenRows: 0,
        totalTokens: 0,
        traceCount: 0,
        usageTruth: null,
        failure: redactSmokeText(errorMessage(error)),
      }) as FleetRunSmokeCloseoutEvidence),
    ),
  )
  const tokenRows = closeouts.reduce((sum, closeout) => sum + closeout.tokenRows, 0)
  const totalTokens = closeouts.reduce((sum, closeout) => sum + closeout.totalTokens, 0)
  const afterCounter = await fetchPublicKhalaTokensServed(counterUrl, options.fetch)
  const evidence: FleetRunSmokeEvidence = {
    ...preCloseoutEvidence,
    closeouts,
    publicCounterReconciliation: reconcilePublicCounter(counterUrl, beforeCounter, afterCounter, totalTokens),
    tokenRows,
    totalTokens,
  }
  const failures = [
    ...preCloseoutFailures,
    ...closeoutEvidenceFailures(plan, evidence),
  ]

  return {
    ok: failures.length === 0,
    skipped: false,
    smoke: plan.smoke,
    armingEnv: plan.armingEnv,
    targetWorkers: plan.targetWorkers,
    ...(plan.minDurationMinutes === null ? {} : { minDurationMinutes: plan.minDurationMinutes }),
    ...(plan.minRefills === 0 ? {} : { minRefills: plan.minRefills }),
    repo: plan.repo ?? undefined,
    branch: plan.branch,
    commit: plan.commit ?? undefined,
    pylonRef,
    issues: plan.issues,
    ...(plan.planRef === null ? {} : { planRef: plan.planRef }),
    verify: plan.verify ?? undefined,
    runRef: plan.runRef,
    workSource: plan.workSource,
    expectedCloseout: plan.expectedCloseout,
    evidence,
    failures,
    message: failures.length === 0
      ? `${plan.smoke} passed with ${evidence.closeouts.length} closeout(s), ${evidence.tokenRows} exact token row(s), and ${evidence.refillsObserved} refill(s).`
      : `${plan.smoke} failed: ${failures.join("; ")}`,
  }
}

export async function readPylonKhalaCloseoutEvidence(
  assignmentRef: string,
  plan: FleetRunSmokePlan,
): Promise<FleetRunSmokeCloseoutEvidence> {
  const env = cleanEnv(plan.managerEnv)
  const proc = Bun.spawn([
    resolveBunExecutable(plan.managerEnv),
    "src/index.ts",
    "khala",
    "closeout",
    assignmentRef,
    "--base-url",
    plan.baseUrl,
    "--json",
  ], {
    cwd: pylonAppDir(),
    env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const closeoutTimeoutMs = Math.min(Math.max(plan.pollIntervalMs * 4, 30_000), 120_000)
  const timeout = setTimeout(() => proc.kill("SIGTERM"), closeoutTimeoutMs)
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]).finally(() => clearTimeout(timeout))
  if (exitCode !== 0) {
    throw new Error(redactSmokeText(stderr.trim() || stdout.trim() || `pylon khala closeout exited ${exitCode}`))
  }
  const parsed = JSON.parse(stdout) as unknown
  return closeoutEvidenceFromPayload(assignmentRef, parsed)
}

export function closeoutEvidenceFromPayload(
  assignmentRef: string,
  payload: unknown,
): FleetRunSmokeCloseoutEvidence {
  const root = record(payload)
  const closeoutChecklist = recordField(root, "closeoutChecklist")
  const proof = recordField(root, "proof")
  const proofChecklist = recordField(proof, "proofChecklist")
  const status = recordField(root, "status")
  const progress = recordField(status, "progress")
  const tokenUsage = recordField(proof, "tokenUsage")
  const rawEvents = recordField(proof, "rawEvents")
  const traces = recordField(proof, "traces")
  const blockerRefs = dedupe([
    ...stringArrayField(closeoutChecklist, "blockerRefs"),
    ...stringArrayField(proofChecklist, "blockerRefs"),
  ])
  return {
    assignmentRef,
    blockerRefs,
    closeoutChecklistOk: booleanField(closeoutChecklist, "ok") === true,
    demandSource: stringField(tokenUsage, "demandSource"),
    ok: booleanField(root, "ok") === true,
    proofChecklistOk: booleanField(proofChecklist, "ok") === true,
    rawEventCount: numberField(rawEvents, "eventCount") ?? numberField(rawEvents, "count") ?? 0,
    statusState: stringField(progress, "state"),
    tokenRefs: stringArrayField(tokenUsage, "refs"),
    tokenRows: numberField(tokenUsage, "rowCount") ?? 0,
    totalTokens: numberField(tokenUsage, "totalTokens") ?? 0,
    traceCount: numberField(traces, "count") ?? 0,
    usageTruth: stringField(tokenUsage, "usageTruth"),
  }
}

export function writeFleetRunSmokeResult(result: FleetRunSmokeResult): void {
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
}

function skippedResult(plan: FleetRunSmokePlan): FleetRunSmokeResult {
  return {
    ok: true,
    skipped: true,
    smoke: plan.smoke,
    armingEnv: plan.armingEnv,
    targetWorkers: plan.targetWorkers,
    ...(plan.minDurationMinutes === null ? {} : { minDurationMinutes: plan.minDurationMinutes }),
    ...(plan.minRefills === 0 ? {} : { minRefills: plan.minRefills }),
    expectedCloseout: plan.expectedCloseout,
    failures: [],
    message: plan.message,
  }
}

function failedResult(plan: FleetRunSmokePlan, failures: readonly string[]): FleetRunSmokeResult {
  return {
    ok: false,
    skipped: false,
    smoke: plan.smoke,
    armingEnv: plan.armingEnv,
    targetWorkers: plan.targetWorkers,
    ...(plan.minDurationMinutes === null ? {} : { minDurationMinutes: plan.minDurationMinutes }),
    ...(plan.minRefills === 0 ? {} : { minRefills: plan.minRefills }),
    repo: plan.repo ?? undefined,
    branch: plan.branch,
    commit: plan.commit ?? undefined,
    pylonRef: plan.managerEnv[`${smokeConfigs[plan.kind].envPrefix}_PYLON_REF`],
    issues: plan.issues.length === 0 ? undefined : plan.issues,
    ...(plan.planRef === null ? {} : { planRef: plan.planRef }),
    verify: plan.verify ?? undefined,
    runRef: plan.runRef,
    workSource: plan.workSource,
    expectedCloseout: plan.expectedCloseout,
    failures,
    message: failures.join("; "),
  }
}

function evidenceFromSnapshot(input: {
  readonly completedAt: string | null
  readonly observedAt: Date
  readonly plan: FleetRunSmokePlan
  readonly snapshot: FleetRunSmokeSnapshot | null
  readonly startedAt: string
}): Omit<FleetRunSmokeEvidence, "closeouts" | "publicCounterReconciliation" | "tokenRows" | "totalTokens"> {
  const lifecycle = input.snapshot?.lifecycle ?? []
  const assignmentRefs = dedupe(lifecycle.flatMap(assignmentRefsFromLifecycleEvent))
  const dispatchFailures = dedupe(lifecycle.flatMap(dispatchFailureFromLifecycleEvent))
  const workUnitRefs = lifecycle.flatMap(workUnitRefsFromLifecycleEvent)
  const duplicateWorkUnitRefs = duplicateRefs(workUnitRefs)
  return {
    assignmentRefs,
    completedAt: input.completedAt,
    dispatchFailures,
    duplicateWorkUnitRefs,
    pylonRef: input.snapshot?.pylonRef ?? null,
    refillsObserved: Math.max(0, assignmentRefs.length - input.plan.targetWorkers),
    run: input.snapshot?.run ?? null,
    runObservedDurationMs: Math.max(0, input.observedAt.getTime() - Date.parse(input.startedAt)),
    startedAt: input.startedAt,
    workUnitRefs: dedupe(workUnitRefs),
  }
}

function preCloseoutEvidenceFailures(
  plan: FleetRunSmokePlan,
  evidence: Omit<FleetRunSmokeEvidence, "closeouts" | "publicCounterReconciliation" | "tokenRows" | "totalTokens">,
): readonly string[] {
  const failures: string[] = []
  const run = evidence.run
  if (run === null) {
    failures.push("fleet run snapshot was not observed")
    return failures
  }
  if (run.state !== "completed") failures.push(`expected completed fleet run, got ${run.state}`)
  if (run.targetConcurrency < plan.targetWorkers) {
    failures.push(`expected target concurrency ${plan.targetWorkers}, got ${run.targetConcurrency}`)
  }
  if (run.counters.completedAssignments < plan.requiredCloseouts) {
    failures.push(`expected at least ${plan.requiredCloseouts} completed assignments, got ${run.counters.completedAssignments}`)
  }
  if (run.counters.failedAssignments !== 0) {
    failures.push(`expected zero failed assignments, got ${run.counters.failedAssignments}`)
  }
  if (run.counters.blockedAssignments !== 0) {
    failures.push(`expected zero blocked assignments, got ${run.counters.blockedAssignments}`)
  }
  for (const failure of evidence.dispatchFailures) {
    failures.push(`dispatch failure: ${failure}`)
  }
  if (evidence.assignmentRefs.length < plan.requiredCloseouts) {
    failures.push(`expected at least ${plan.requiredCloseouts} assignment refs, got ${evidence.assignmentRefs.length}`)
  }
  if (evidence.duplicateWorkUnitRefs.length > 0) {
    failures.push(`expected zero duplicate work-unit claims, saw ${evidence.duplicateWorkUnitRefs.join(", ")}`)
  }
  if (plan.minDurationMs !== null && evidence.runObservedDurationMs < plan.minDurationMs) {
    failures.push(`expected sustained run duration >= ${plan.minDurationMs}ms, got ${evidence.runObservedDurationMs}ms`)
  }
  if (plan.minRefills > 0 && evidence.refillsObserved < plan.minRefills) {
    failures.push(`expected at least ${plan.minRefills} refill(s), got ${evidence.refillsObserved}`)
  }
  return failures
}

function closeoutEvidenceFailures(plan: FleetRunSmokePlan, evidence: FleetRunSmokeEvidence): readonly string[] {
  const failures: string[] = []
  if (evidence.closeouts.length < plan.requiredCloseouts) {
    failures.push(`expected at least ${plan.requiredCloseouts} closeout(s), got ${evidence.closeouts.length}`)
  }
  for (const closeout of evidence.closeouts) {
    if (!closeout.ok || !closeout.closeoutChecklistOk || !closeout.proofChecklistOk) {
      failures.push(`closeout checklist failed for ${closeout.assignmentRef}`)
    }
    if (closeout.statusState !== "closed_out") {
      failures.push(`expected ${closeout.assignmentRef} to be closed_out, got ${closeout.statusState ?? "unknown"}`)
    }
    if (closeout.usageTruth !== "exact") {
      failures.push(`expected exact token rows for ${closeout.assignmentRef}, got ${closeout.usageTruth ?? "unknown"}`)
    }
    if (closeout.demandSource !== "khala_coding_delegation") {
      failures.push(`expected khala_coding_delegation demand source for ${closeout.assignmentRef}`)
    }
    if (closeout.tokenRows <= 0 || closeout.totalTokens <= 0 || closeout.tokenRefs.length < Math.min(closeout.tokenRows, 100)) {
      failures.push(`expected positive exact token row evidence for ${closeout.assignmentRef}`)
    }
    if (closeout.traceCount <= 0 || closeout.rawEventCount <= 0) {
      failures.push(`expected owner trace and raw-event evidence for ${closeout.assignmentRef}`)
    }
    for (const blocker of closeout.blockerRefs) failures.push(`${closeout.assignmentRef}: ${blocker}`)
  }
  if (evidence.tokenRows <= 0 || evidence.totalTokens <= 0) {
    failures.push("expected aggregate exact token rows and verified tokens to be positive")
  }
  if (evidence.publicCounterReconciliation.state !== "checked") {
    failures.push(`public counter reconciliation unavailable: ${evidence.publicCounterReconciliation.reason}`)
  } else if (!evidence.publicCounterReconciliation.ok) {
    failures.push(
      `expected public counter delta >= ${evidence.publicCounterReconciliation.expectedMinimumDelta}, got ${evidence.publicCounterReconciliation.delta}`,
    )
  }
  return failures
}

function assignmentRefsFromLifecycleEvent(event: FleetRunSmokeLifecycleEvent): readonly string[] {
  if (event.kind === "dispatch" && typeof event.assignmentRef === "string") return [event.assignmentRef]
  const nested = recordField(event, "event")
  const assignmentRef = stringField(nested, "assignmentRef")
  return assignmentRef === null ? [] : [assignmentRef]
}

function workUnitRefsFromLifecycleEvent(event: FleetRunSmokeLifecycleEvent): readonly string[] {
  if (event.kind !== "dispatch") return []
  const workUnitRef = stringField(event, "workUnitRef")
  const status = stringField(event, "status")
  return workUnitRef === null || status === "failed" ? [] : [workUnitRef]
}

function dispatchFailureFromLifecycleEvent(event: FleetRunSmokeLifecycleEvent): readonly string[] {
  if (event.kind !== "dispatch") return []
  const status = stringField(event, "status")
  if (status !== "failed" && status !== "blocked") return []
  const workUnitRef = stringField(event, "workUnitRef")
  const summary = stringField(event, "summary")
  const detail = summary === null ? status : redactSmokeText(summary)
  return [workUnitRef === null ? detail : `${workUnitRef}: ${detail}`]
}

function singleSnapshot(value: FleetRunSmokeSnapshot | readonly FleetRunSmokeSnapshot[]): FleetRunSmokeSnapshot {
  if (Array.isArray(value)) throw new Error("expected one fleet run snapshot")
  return value as FleetRunSmokeSnapshot
}

function objectiveFor(plan: FleetRunSmokePlan): string {
  return [
    `${plan.smoke}: supervised live FleetRun evidence run.`,
    `Work source: ${plan.repo ?? "unknown repo"} ${plan.workSource}.`,
    `Pinned base: ${plan.branch}@${plan.commit ?? "unknown commit"}.`,
    `Verify command: ${plan.verify ?? "unknown verify"}.`,
    plan.workSource === "issue_list"
      ? "Each worker must open exactly one PR for its assigned issue and include verify-green evidence."
      : "Each plan node is a bounded live-fleet evidence unit; open a PR only when that node explicitly asks for one.",
    "Do not claim work outside the assigned work unit; respect the FleetRun claim ref and close out with exact token accounting.",
  ].join("\n")
}

function expectedCloseoutFor(
  config: FleetRunSmokeConfig,
  targetWorkers: number,
  minDurationMinutes: number | null,
  minRefills: number,
): readonly string[] {
  if (config.kind === "live") {
    return [
      "two real workers accepted by the supervised FleetRun",
      "two distinct assignment refs close out successfully",
      "each closeout checklist proves exact token_usage_events rows",
      "public /api/public/khala-tokens-served delta reconciles to the exact closeout total",
      "zero duplicate live work-unit claims",
    ]
  }
  return [
    `${targetWorkers} or more real workers accepted by the supervised FleetRun`,
    `run remains active long enough to prove at least ${minDurationMinutes} minutes of sustained execution`,
    `${minRefills} or more refill assignments close out after the first target wave`,
    "each closeout checklist proves exact token_usage_events rows",
    "public /api/public/khala-tokens-served delta reconciles to the exact closeout total",
    "zero duplicate live work-unit claims",
  ]
}

function requiredEnvList(config: FleetRunSmokeConfig): readonly string[] {
  return [
    `${config.envPrefix}_WORK_SOURCE`,
    `${config.envPrefix}_PYLON_REF`,
    `${config.envPrefix}_ISSUES or ${config.envPrefix}_PLAN_REF + ${config.envPrefix}_PLAN_NODES_JSON`,
    `${config.envPrefix}_REPO`,
    `${config.envPrefix}_COMMIT`,
    `${config.envPrefix}_VERIFY`,
    "OPENAGENTS_AGENT_TOKEN",
  ]
}

function parseWorkSource(
  value: string,
  key: string,
  config: FleetRunSmokeConfig,
  failures: string[],
): FleetRunSmokeWorkSource {
  if (value === "issue_list") return "issue_list"
  if (value === "plan_dag" && config.kind === "sustained") return "plan_dag"
  if (value === "plan_dag") {
    failures.push(`${key}=plan_dag is only supported for smoke:fleet-run-sustained`)
    return "issue_list"
  }
  failures.push(`${key} must be issue_list or plan_dag`)
  return "issue_list"
}

function parseIssues(value: string | null, key: string, failures: string[]): readonly number[] {
  if (value === null) return []
  const issues = value.split(",").map(part => Number(part.trim()))
  if (issues.length === 0 || issues.some(issue => !Number.isInteger(issue) || issue <= 0)) {
    failures.push(`${key} must contain positive issue numbers`)
    return []
  }
  if (new Set(issues).size !== issues.length) {
    failures.push(`${key} must name distinct issues`)
  }
  return issues
}

function requireTrimmed(env: FleetRunSmokeEnv, key: string, armEnv: string, failures: string[]): string | null {
  const value = env[key]?.trim()
  if (value === undefined || value === "") {
    failures.push(`${key} is required when ${armEnv}=1`)
    return null
  }
  return value
}

function parsePlanNodesJson(value: string | null, key: string, failures: string[]): readonly PlanDagWorkUnit[] {
  if (value === null) return []
  let parsed: unknown
  try {
    parsed = JSON.parse(value)
  } catch (error) {
    failures.push(`${key} must be a JSON array of plan nodes: ${errorMessage(error)}`)
    return []
  }
  if (!Array.isArray(parsed)) {
    failures.push(`${key} must be a JSON array of plan nodes`)
    return []
  }
  const nodes: PlanDagWorkUnit[] = []
  const seenRefs = new Set<string>()
  for (const [index, item] of parsed.entries()) {
    const node = record(item)
    if (node === null) {
      failures.push(`${key}[${index}] must be an object`)
      continue
    }
    const ref = stringField(node, "ref")?.trim()
    const title = stringField(node, "title")?.trim()
    const objective = stringField(node, "objective")?.trim()
    if (ref === undefined || ref.length === 0) failures.push(`${key}[${index}].ref is required`)
    if (title === undefined || title.length === 0) failures.push(`${key}[${index}].title is required`)
    if (objective === undefined || objective.length === 0) failures.push(`${key}[${index}].objective is required`)
    if (ref === undefined || title === undefined || objective === undefined || ref.length === 0 || title.length === 0 || objective.length === 0) {
      continue
    }
    if (seenRefs.has(ref)) failures.push(`${key} must name distinct plan node refs`)
    seenRefs.add(ref)
    nodes.push({
      ref,
      title,
      objective,
      ...optionalStringPlanField(node, "repo"),
      ...optionalStringPlanField(node, "branch"),
      ...optionalStringPlanField(node, "baseCommit"),
      ...optionalStringPlanField(node, "verify"),
      ...optionalIntegerPlanField(node, "issue", key, index, failures),
      ...optionalStringArrayPlanField(node, "dependsOn", key, index, failures),
      ...optionalStringArrayPlanField(node, "labels", key, index, failures),
      ...optionalStringPlanField(node, "url"),
    })
  }
  return nodes
}

function optionalStringPlanField(
  node: Record<string, unknown>,
  field: "baseCommit" | "branch" | "repo" | "url" | "verify",
): Partial<PlanDagWorkUnit> {
  const value = stringField(node, field)?.trim()
  return value === undefined || value.length === 0 ? {} : { [field]: value }
}

function optionalIntegerPlanField(
  node: Record<string, unknown>,
  field: "issue",
  key: string,
  index: number,
  failures: string[],
): Partial<PlanDagWorkUnit> {
  const value = node[field]
  if (value === undefined) return {}
  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    failures.push(`${key}[${index}].${field} must be a positive integer`)
    return {}
  }
  return { issue: value }
}

function optionalStringArrayPlanField(
  node: Record<string, unknown>,
  field: "dependsOn" | "labels",
  key: string,
  index: number,
  failures: string[],
): Partial<PlanDagWorkUnit> {
  const value = node[field]
  if (value === undefined) return {}
  if (!Array.isArray(value) || value.some(item => typeof item !== "string" || item.trim().length === 0)) {
    failures.push(`${key}[${index}].${field} must be an array of non-empty strings`)
    return {}
  }
  return { [field]: value.map(item => item.trim()) }
}

function parseOptionalInteger(value: string | undefined, fallback: number, key: string, failures: string[]): number {
  if (value === undefined || value.trim() === "") return fallback
  const parsed = Number(value.trim())
  if (!Number.isInteger(parsed)) {
    failures.push(`${key} must be an integer`)
    return fallback
  }
  return parsed
}

function firstTrimmed(values: readonly (string | undefined)[]): string {
  for (const value of values) {
    const trimmed = value?.trim()
    if (trimmed !== undefined && trimmed !== "") return trimmed
  }
  return ""
}

function compactTimestamp(now: Date): string {
  return now.toISOString().replace(/[^0-9]/gu, "").slice(0, 14)
}

function publicCounterUrl(baseUrl: string): string {
  return new URL("/api/public/khala-tokens-served", baseUrl).toString()
}

async function fetchPublicKhalaTokensServed(
  url: string,
  fetchImpl: typeof fetch = fetch,
): Promise<number | Error> {
  try {
    const response = await fetchImpl(url, { headers: { accept: "application/json" } })
    if (!response.ok) return new Error(`counter query failed (${response.status})`)
    const payload = await response.json() as Record<string, unknown>
    const value = payload.tokensServed
    return typeof value === "number" && Number.isFinite(value)
      ? Math.max(0, Math.trunc(value))
      : new Error("counter payload missing numeric tokensServed")
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error))
  }
}

function reconcilePublicCounter(
  url: string,
  before: number | Error,
  after: number | Error,
  expectedMinimumDelta: number,
): FleetRunSmokeCounterReconciliation {
  if (before instanceof Error) return { ok: false, reason: before.message, state: "unavailable", url }
  if (after instanceof Error) return { ok: false, reason: after.message, state: "unavailable", url }
  const delta = Math.max(0, after - before)
  return {
    afterTokensServed: after,
    beforeTokensServed: before,
    delta,
    expectedMinimumDelta,
    ok: delta >= expectedMinimumDelta,
    state: "checked",
    url,
  }
}

function resolveBunExecutable(env: FleetRunSmokeEnv): string {
  return firstTrimmed([env.OPENAGENTS_BUN_PATH, process.execPath, "bun"])
}

function pylonAppDir(): string {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..")
}

function cleanEnv(env: FleetRunSmokeEnv): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [key, value] of Object.entries({ ...process.env, ...env })) {
    if (value !== undefined) out[key] = value
  }
  return out
}

export function redactSmokeText(text: string): string {
  return text
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/gu, "Bearer <redacted>")
    .replace(/oa_agent_[A-Za-z0-9._~+/-]+/gu, "oa_agent_<redacted>")
    .replace(/(OPENAGENTS_AGENT_TOKEN=)[^\s]+/gu, "$1<redacted>")
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function duplicateRefs(values: readonly string[]): readonly string[] {
  const counts = new Map<string, number>()
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1)
  return [...counts.entries()].flatMap(([value, count]) => count > 1 ? [value] : [])
}

function dedupe(values: readonly string[]): readonly string[] {
  return [...new Set(values.filter(value => value.trim() !== ""))]
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null ? value as Record<string, unknown> : null
}

function recordField(source: Record<string, unknown> | null, field: string): Record<string, unknown> | null {
  return record(source?.[field])
}

function stringField(source: Record<string, unknown> | null, field: string): string | null {
  const value = source?.[field]
  return typeof value === "string" ? value : null
}

function numberField(source: Record<string, unknown> | null, field: string): number | null {
  const value = source?.[field]
  return typeof value === "number" && Number.isFinite(value) ? value : null
}

function booleanField(source: Record<string, unknown> | null, field: string): boolean | null {
  const value = source?.[field]
  return typeof value === "boolean" ? value : null
}

function stringArrayField(source: Record<string, unknown> | null, field: string): readonly string[] {
  const value = source?.[field]
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []
}
