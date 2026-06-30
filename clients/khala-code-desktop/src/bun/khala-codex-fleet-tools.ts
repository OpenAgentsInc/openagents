import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"
import type { Readable } from "node:stream"
import { Effect, Schema as S, Stream } from "effect"
import {
  KhalaFleetDelegateModuleError,
  khalaFleetDelegationParametersFromEnv,
  khalaFleetDelegationPerAccountConcurrency,
  khalaToolError,
  khalaToolOk,
  khalaToolUnavailable,
  redactKhalaPublicText,
  renderKhalaFleetDelegationObjective,
  resolveKhalaFleetDelegationParameters,
  runKhalaFleetDelegateProgram,
  type KhalaFleetDelegateAccount,
  type KhalaFleetDelegateBlockerCode,
  type KhalaFleetDelegateCapacity,
  type KhalaFleetDelegateProgramResult,
  type KhalaFleetDelegateStep,
  type KhalaFleetDelegateWork,
  type KhalaFleetDelegationParameterSet,
  type KhalaToolDefinition,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "@openagentsinc/khala-tools"

type ChatEnv = Readonly<Record<string, string | undefined>>

export type KhalaCodexFleetCommandInput = {
  readonly cmd: readonly string[]
  readonly cwd?: string | undefined
  readonly detached?: boolean | undefined
  readonly env?: ChatEnv | undefined
  readonly maxOutputBytes?: number | undefined
  readonly onStderrLine?: ((line: string) => void | Promise<void>) | undefined
  readonly timeoutMs: number
}

export type KhalaCodexFleetCommandResult = {
  readonly exitCode: number | null
  readonly signal: NodeJS.Signals | null
  readonly stderr: string
  readonly stdout: string
  readonly timedOut: boolean
}

export type KhalaCodexFleetCommandRunner = (
  input: KhalaCodexFleetCommandInput,
) => Promise<KhalaCodexFleetCommandResult>

export type KhalaCodexFleetToolOptions = {
  readonly delegationParameters?: KhalaFleetDelegationParameterSet | undefined
  readonly env?: ChatEnv | undefined
  readonly onProgress?: KhalaCodexFleetProgressSink | undefined
  readonly runner?: KhalaCodexFleetCommandRunner | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
}

export type KhalaCodexFleetProgressPayload = {
  readonly schema: "openagents.khala_code.codex_spawn_progress.v0.1"
  readonly kind: "codex_spawn_lifecycle"
  readonly toolName: "codex_spawn"
  readonly events: readonly AssignmentLifecycleEvent[]
  readonly lines: readonly string[]
}

export type KhalaCodexFleetProgressSink = (
  payload: KhalaCodexFleetProgressPayload,
) => void | Promise<void>

export type KhalaPylonEnsureResult = {
  readonly ok: boolean
  readonly status: "online" | "started" | "unavailable"
  readonly availableCodexAssignments: number | null
  readonly maxCodexAssignments: number | null
  readonly message: string
  readonly pylonHome: string
  readonly pylonRef: string | null
  readonly started: boolean
  readonly unavailableReason?: string | undefined
}

type PylonPaths = {
  readonly appPath: string
  readonly bunExecutable: string
  readonly pylonHome: string
}

type AccountRow = {
  readonly accountKey: string | null
  readonly accountRef: string
  readonly accountRefHash: string | null
  readonly capacity: AccountCapacityRow | null
  readonly provider: "codex"
  readonly quotaState: string | null
  readonly readiness: string
}

type AccountCapacityRow = {
  readonly available: number | null
  readonly busy: number | null
  readonly queued: number | null
  readonly ready: number | null
}

type FleetStatusResult = {
  readonly accounts: readonly AccountRow[]
  readonly activeAssignments: readonly ActiveAssignmentMarker[]
  readonly availableCodexAssignments: number | null
  readonly ensure: KhalaPylonEnsureResult
  readonly maxCodexAssignments: number | null
  readonly observedAt: string
  readonly processes: readonly ProcessRow[]
  readonly serverAssignments: readonly ServerAssignmentTokenRow[]
  readonly tokenRate: FleetTokenRateProjection
}

type ActiveAssignmentMarker = {
  readonly accountRefHash: string | null
  readonly assignmentRef: string | null
  readonly elapsedMs: number | null
  readonly issueRef: string | null
  readonly refreshedAt: string | null
  readonly runRef: string | null
  readonly service: string | null
  readonly startedAt: string | null
  readonly tokenRate: AssignmentTokenRateProjection
  readonly updatedAt: string | null
}

type TokenMeasurementStatus = "exact" | "estimated" | "not_measured" | "pending"

type AssignmentTokenRateProjection = {
  readonly source: string
  readonly status: TokenMeasurementStatus
  readonly tokenCountKind: string | null
  readonly tokens: number | null
  readonly tokensPerMinute: number | null
}

type ServerAssignmentTokenRow = {
  readonly assignmentRef: string | null
  readonly elapsedMs: number | null
  readonly tokenRate: AssignmentTokenRateProjection
}

type FleetTokenRateProjection = {
  readonly activeAdjustedTokensPerMinute: number | null
  readonly completedStatus: TokenMeasurementStatus
  readonly completedTokenRows: number | null
  readonly completedTokensPerMinute: number | null
  readonly inFlightTokens: number | null
  readonly inFlightTokensPerMinute: number | null
  readonly source: "pylon_khala_apm" | "unavailable"
  readonly unavailableReason: string | null
}

type ProcessRow = {
  readonly elapsed: string
  readonly kind: "codex_exec"
  readonly parentPid: string
  readonly pid: string
}

type SpawnCodexInstancesInput = {
  readonly accountRef?: string | undefined
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly count?: number | undefined
  readonly fixture?: boolean | undefined
  readonly noRun?: boolean | undefined
  readonly prompt: string
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly timeoutMs?: number | undefined
  readonly verify?: string | undefined
}

type NormalizedSpawnInput = {
  readonly accountRef?: string | undefined
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly count: number
  readonly fixture: boolean
  readonly noRun: boolean
  readonly prompt: string
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly timeoutMs: number
  readonly verify?: string | undefined
}

type SpawnCodexInstancesResult = {
  readonly acceptedCount: number
  readonly delegateSignature?: "khala.fleet.delegate" | undefined
  readonly delegateStatus?: "blocked" | "completed" | undefined
  readonly delegateTrace?: readonly KhalaFleetDelegateStep[] | undefined
  readonly pylonRef: string | null
  readonly requestedCount: number
  readonly results: readonly SpawnSlotResult[]
}

type SpawnSlotResult = {
  readonly accountRef: string | null
  readonly assignmentRef: string | null
  readonly autoRunOk: boolean | null
  readonly exitCode: number | null
  readonly slot: number
  readonly status: "accepted" | "failed"
  readonly summary: string
  readonly tokensVerified: number | null
}

type BatchSpawnSlotProjection = {
  readonly accountRef: string | null
  readonly assignmentRef: string | null
  readonly blockerRefs: readonly string[]
  readonly closeoutStatus: string | null
  readonly durableRequestId: string | null
  readonly failure: Record<string, unknown> | null
  readonly lifecycleEvents: readonly Record<string, unknown>[]
  readonly ok: boolean | null
  readonly proof: Record<string, unknown> | null
  readonly runAccepted: boolean | null
  readonly slotIndex: number
  readonly state: string | null
}

type AssignmentLifecycleEvent = {
  readonly assignmentRef?: string | undefined
  readonly event: string
  readonly leaseRef?: string | undefined
  readonly phase?: string | undefined
  readonly status?: string | undefined
}

type PylonLifecycleWireEvent = typeof PylonLifecycleWireEvent.Type

const MAX_SPAWN_COUNT = 5
const DEFAULT_COMMAND_TIMEOUT_MS = 45_000
const DEFAULT_SPAWN_TIMEOUT_MS = 180_000
const DEFAULT_ENSURE_WAIT_MS = 7_500
const DEFAULT_CODEX_ACCOUNT_CONCURRENCY = MAX_SPAWN_COUNT
const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com"
const MAX_MODEL_OUTPUT_BYTES = 4_000
const MAX_STREAMED_LIFECYCLE_EVENTS = 200

const PylonAssignmentLifecycleWireEvent = S.Struct({
  schema: S.Literal("openagents.pylon.assignment_run_lifecycle_event.v0.1"),
  event: S.String,
  observedAt: S.String,
  assignmentRef: S.optional(S.String),
  leaseRef: S.optional(S.String),
  phase: S.optional(S.String),
  status: S.optional(S.String),
})

const PylonSpawnWorkerLifecycleWireEvent = S.Struct({
  schema: S.Literal("openagents.pylon.khala_spawn_worker_event.v0.1"),
  message: S.String,
  observedAt: S.String,
  slotIndex: S.Number,
  state: S.String,
  assignmentEvent: S.optional(S.String),
  assignmentRef: S.optional(S.String),
  leaseRef: S.optional(S.String),
  phase: S.optional(S.String),
  status: S.optional(S.String),
})

const PylonLifecycleWireEvent = S.Union([
  PylonAssignmentLifecycleWireEvent,
  PylonSpawnWorkerLifecycleWireEvent,
])
const decodePylonLifecycleWireEvent = S.decodeUnknownSync(PylonLifecycleWireEvent)

const pylonEnsureToolDefinition: KhalaToolDefinition = {
  authority: "owner_full_access",
  availability: ["owner_local_full", "coding"],
  description:
    "Adopt or start the local OpenAgents Pylon bridge so Khala Code Desktop can use local owner capacity.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      start: {
        default: true,
        description: "Start a local background Pylon node when one is not already online.",
        type: "boolean",
      },
      timeout_ms: {
        description: "Maximum time to spend on individual Pylon probe commands.",
        minimum: 1_000,
        type: "integer",
      },
      wait_ms: {
        description: "Maximum time to wait after starting Pylon before probing again.",
        minimum: 0,
        type: "integer",
      },
    },
    type: "object",
  },
  internalId: "khala.desktop.pylon.ensure",
  label: "Ensure Pylon",
  name: "pylon_ensure",
  outputSchema: {
    additionalProperties: false,
    properties: {
      ok: { type: "boolean" },
      pylonRef: { type: ["string", "null"] },
      started: { type: "boolean" },
      status: { type: "string" },
    },
    required: ["ok", "status", "started", "pylonRef"],
    type: "object",
  },
  permissionMode: "allow",
  prompt:
    "Use when the user asks whether Pylon is online, or before launching Codex fleet work from Desktop.",
  promptGuidelines: [
    "Do not run Codex login or touch the default ~/.codex home.",
    "If the result is unavailable, tell the user the exact missing setup step instead of pretending capacity exists.",
    "Prefer codex_fleet_status for monitoring and codex_spawn for assignments.",
  ],
  renderer: { kind: "pylon_status", rendererRef: "khala.renderer.pylon_status.v1" },
}

const codexFleetStatusToolDefinition: KhalaToolDefinition = {
  authority: "owner_full_access",
  availability: ["owner_local_full", "coding"],
  description:
    "Inspect local Pylon-backed Codex fleet accounts, assignment markers, and local Pylon/Codex processes.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      include_processes: {
        default: true,
        description: "Include a bounded local process snapshot for Pylon and Codex.",
        type: "boolean",
      },
      start_pylon: {
        default: false,
        description: "Start Pylon if it is not online before collecting status.",
        type: "boolean",
      },
    },
    type: "object",
  },
  internalId: "khala.desktop.codex_fleet.status",
  label: "Codex Fleet Status",
  name: "codex_fleet_status",
  outputSchema: {
    additionalProperties: false,
    properties: {
      accounts: { type: "array" },
      activeAssignments: { type: "array" },
      availableCodexAssignments: { type: ["integer", "null"] },
      maxCodexAssignments: { type: ["integer", "null"] },
      pylon: { type: "object" },
      processes: { type: "array" },
    },
    required: ["pylon", "accounts", "activeAssignments", "processes"],
    type: "object",
  },
  permissionMode: "allow",
  prompt:
    "Use when the user asks to monitor Codex instances, fleet capacity, linked accounts, or local Pylon health.",
  promptGuidelines: [
    "Report no ready accounts plainly; do not run device login from this tool.",
    "Summarize account readiness and assignment counts without exposing local credential paths.",
    "Use pylon_ensure only when the user specifically wants to start or adopt the bridge.",
  ],
  renderer: { kind: "codex_fleet_status", rendererRef: "khala.renderer.codex_fleet_status.v1" },
}

const codexSpawnToolDefinition: KhalaToolDefinition = {
  authority: "owner_full_access",
  availability: ["owner_local_full", "coding"],
  description:
    "Dispatch one or more Codex agent assignments through the local Pylon bridge and the hosted Khala system.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      account_ref: {
        description: "Optional linked Pylon Codex account ref to use. If omitted, a ready account is selected.",
        type: "string",
      },
      base_url: {
        description: "Optional OpenAgents base URL override for local testing.",
        type: "string",
      },
      branch: {
        description: "Optional branch name paired with repo/commit/verify workspace pins.",
        type: "string",
      },
      commit: {
        description: "Pinned 40-character commit SHA for real repository work.",
        type: "string",
      },
      count: {
        default: 1,
        description: `Number of Codex assignments to dispatch, capped at ${MAX_SPAWN_COUNT}.`,
        minimum: 1,
        type: "integer",
      },
      fixture: {
        default: true,
        description:
          "Use Pylon's bounded public fixture instead of real repository pins. Defaults on when repo/commit/verify are omitted.",
        type: "boolean",
      },
      no_run: {
        default: false,
        description: "Create the hosted assignment but do not auto-run the local no-spend assignment.",
        type: "boolean",
      },
      prompt: {
        description: "Public-safe assignment objective for Codex.",
        type: "string",
      },
      pylon_ref: {
        description: "Optional target Pylon ref. If omitted, Desktop resolves the local Pylon ref.",
        type: "string",
      },
      repo: {
        description: "Repository pin as owner/repo for real repository work.",
        type: "string",
      },
      timeout_ms: {
        description: "Maximum time for each Pylon request command.",
        minimum: 10_000,
        type: "integer",
      },
      verify: {
        description: "Verification command for real repository work, paired with repo and commit.",
        type: "string",
      },
    },
    required: ["prompt"],
    type: "object",
  },
  internalId: "khala.desktop.codex_fleet.spawn",
  label: "Spawn Codex",
  name: "codex_spawn",
  outputSchema: {
    additionalProperties: false,
    properties: {
      acceptedCount: { type: "integer" },
      delegateSignature: { type: "string" },
      delegateStatus: { type: "string" },
      delegateTrace: { type: "array" },
      pylonRef: { type: ["string", "null"] },
      requestedCount: { type: "integer" },
      results: { type: "array" },
    },
    required: ["requestedCount", "acceptedCount", "results"],
    type: "object",
  },
  permissionMode: "allow",
  prompt:
    "Use when the user asks Khala to spin up, launch, fan out, or assign Codex instances through Desktop.",
  promptGuidelines: [
    "When the user asks for a smoke test or omits repo pins, call this without repo pins; the tool will use the public fixture.",
    "Require complete repo, commit, and verify pins only for real repository work.",
    "Do not run Codex login. If no ready Pylon Codex account exists, tell the user to connect one first.",
    "Omit account_ref unless the user names a specific non-default account; Desktop prefers named ready accounts over the display-only default account.",
    "This MVP exposes only pylon_ensure, codex_fleet_status, and codex_spawn for Codex fleet control. Do not invent codex_terminate or other Codex fleet tools.",
    "After codex_spawn, summarize the returned assignment, auto-run, and closeout status; do not read guessed local output files.",
    "Keep count small; this MVP caps fan-out at five assignments.",
  ],
  renderer: { kind: "codex_spawn", rendererRef: "khala.renderer.codex_spawn.v1" },
}

export function createKhalaCodexFleetTools(
  options: KhalaCodexFleetToolOptions = {},
): readonly RegisteredKhalaTool[] {
  return [
    {
      definition: pylonEnsureToolDefinition,
      execute: input => executePylonEnsureTool(input, options),
    },
    {
      definition: codexFleetStatusToolDefinition,
      execute: input => executeCodexFleetStatusTool(input, options),
    },
    {
      definition: codexSpawnToolDefinition,
      execute: (input, context) =>
        executeCodexSpawnTool(input, {
          ...options,
          onProgress: async payload => {
            await options.onProgress?.(payload)
            await Effect.runPromise(context.emitProgress(payload))
          },
        }),
    },
  ]
}

export async function ensureLocalPylon(
  input: {
    readonly start?: boolean | undefined
    readonly timeoutMs?: number | undefined
    readonly waitMs?: number | undefined
  },
  options: KhalaCodexFleetToolOptions = {},
): Promise<KhalaPylonEnsureResult> {
  const env = options.env ?? process.env
  const paths = resolvePylonPaths(env)
  const timeoutMs = boundedPositiveInteger(input.timeoutMs, DEFAULT_COMMAND_TIMEOUT_MS, 1_000, 120_000)
  const waitMs = boundedPositiveInteger(input.waitMs, DEFAULT_ENSURE_WAIT_MS, 0, 60_000)
  const start = input.start ?? true

  if (!existsSync(join(paths.appPath, "package.json"))) {
    return {
      ok: false,
      availableCodexAssignments: null,
      maxCodexAssignments: null,
      message: "Pylon source was not found in this checkout.",
      pylonHome: paths.pylonHome,
      pylonRef: null,
      started: false,
      status: "unavailable",
      unavailableReason: "pylon_app_missing",
    }
  }

  const first = await runPylonCommand(["provider", "go-online", "--json"], {
    env,
    paths,
    runner: options.runner,
    timeoutMs,
  })
  const firstJson = parseJsonObject(first.stdout)
  const firstRef = stringField(firstJson, "pylonRef")
  if (first.exitCode === 0 && firstRef !== null) {
    const capacity = capacityFromProviderProjection(firstJson)
    return {
      ok: true,
      availableCodexAssignments: capacity.available,
      maxCodexAssignments: capacity.max,
      message: "Local Pylon is online.",
      pylonHome: paths.pylonHome,
      pylonRef: firstRef,
      started: false,
      status: "online",
    }
  }

  if (!start) {
    return {
      ok: false,
      availableCodexAssignments: null,
      maxCodexAssignments: null,
      message: "Local Pylon is not online.",
      pylonHome: paths.pylonHome,
      pylonRef: null,
      started: false,
      status: "unavailable",
      unavailableReason: safeFailureReason(first),
    }
  }

  await (options.runner ?? defaultCommandRunner)({
    cmd: [paths.bunExecutable, "src/index.ts"],
    cwd: paths.appPath,
    detached: true,
    env: pylonCommandEnv(env, paths.pylonHome),
    timeoutMs: 1_000,
  })

  const sleep = options.sleep ?? delay
  const deadline = Date.now() + waitMs
  let last = first
  while (waitMs > 0 && Date.now() <= deadline) {
    const remainingMs = Math.max(0, deadline - Date.now())
    await sleep(Math.min(750, Math.max(100, remainingMs)))
    last = await runPylonCommand(["provider", "go-online", "--json"], {
      env,
      paths,
      runner: options.runner,
      timeoutMs,
    })
    const json = parseJsonObject(last.stdout)
    const pylonRef = stringField(json, "pylonRef")
    if (last.exitCode === 0 && pylonRef !== null) {
      const capacity = capacityFromProviderProjection(json)
      return {
        ok: true,
        availableCodexAssignments: capacity.available,
        maxCodexAssignments: capacity.max,
        message: "Started local Pylon and confirmed it is online.",
        pylonHome: paths.pylonHome,
        pylonRef,
        started: true,
        status: "started",
      }
    }
  }

  return {
    ok: false,
    availableCodexAssignments: null,
    maxCodexAssignments: null,
    message: "Started local Pylon, but it did not report online before the wait window ended.",
    pylonHome: paths.pylonHome,
    pylonRef: null,
    started: true,
    status: "unavailable",
    unavailableReason: safeFailureReason(last),
  }
}

export async function inspectCodexFleet(
  input: {
    readonly includeProcesses?: boolean | undefined
    readonly startPylon?: boolean | undefined
  } = {},
  options: KhalaCodexFleetToolOptions = {},
): Promise<FleetStatusResult> {
  const env = options.env ?? process.env
  const paths = resolvePylonPaths(env)
  const ensure = await ensureLocalPylon({ start: input.startPylon ?? false }, { ...options, env })
  const baseUrl = resolveOpenAgentsBaseUrl(env)
  const [listResult, statusResult, apmResult, processes, rawActiveAssignments] = await Promise.all([
    runPylonCommand(["codex", "accounts", "list", "--json"], {
      env,
      paths,
      runner: options.runner,
      timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
    }).catch(errorResult),
    runPylonCommand(["accounts", "status", "--provider", "codex", "--json"], {
      env,
      paths,
      runner: options.runner,
      timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
    }).catch(errorResult),
    runPylonCommand(["khala", "apm", "--base-url", baseUrl, "--json"], {
      env,
      paths,
      runner: options.runner,
      timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
    }).catch(errorResult),
    input.includeProcesses === false
      ? Promise.resolve([] as ProcessRow[])
      : collectProcessSnapshot(options.runner ?? defaultCommandRunner, env),
    collectActiveAssignmentMarkers(paths.pylonHome),
  ])

  const listProjection = parseJsonObject(listResult.stdout)
  const statusProjection = parseJsonObject(statusResult.stdout)
  const apm = fleetTokenRateProjectionFromApm(apmResult, rawActiveAssignments.length)
  const accounts = withAccountCapacity(
    mergeAccountRows(listProjection, statusProjection),
    [statusProjection, listProjection],
  )
  const activeAssignments = mergeActiveAssignmentTokenRates(
    rawActiveAssignments,
    apm.serverAssignments,
  )
  const provider = providerCapacity(listProjection, statusProjection, ensure)
  return {
    accounts,
    activeAssignments,
    availableCodexAssignments: provider.available,
    ensure,
    maxCodexAssignments: provider.max,
    observedAt: new Date().toISOString(),
    processes,
    serverAssignments: apm.serverAssignments,
    tokenRate: apm.tokenRate,
  }
}

export async function spawnCodexInstances(
  raw: SpawnCodexInstancesInput,
  options: KhalaCodexFleetToolOptions = {},
): Promise<SpawnCodexInstancesResult> {
  const baseEnv = options.env ?? process.env
  const delegationParameters = options.delegationParameters ??
    khalaFleetDelegationParametersFromEnv(baseEnv)
  const input = decodeSpawnInput(raw, delegationParameters)
  const env = withCodexCapacityAdvertisementEnv(baseEnv, input.count, delegationParameters)
  const paths = resolvePylonPaths(env)
  const baseUrl = resolveOpenAgentsBaseUrl(env, input.baseUrl)
  let fleetStatus: FleetStatusResult | null = null
  let providerProjection: Record<string, unknown> | null = null
  let accountAvailability: ReadonlyMap<string, number> = new Map()
  let heartbeatPylonRef: string | null = null
  let dispatchedResult: SpawnCodexInstancesResult | null = null

  const refreshFleetProjection = async (
    pylonRef: string | undefined,
    reason: "initial" | "no_available_codex_capacity" | "stale_heartbeat",
  ) => {
    const heartbeat = await runPylonCommand(["presence", "heartbeat", "--base-url", baseUrl, "--json"], {
      env,
      paths,
      runner: options.runner,
      timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
    })
    if (heartbeat.exitCode !== 0) {
      throw new KhalaFleetDelegateModuleError({
        blockerCode: reason === "stale_heartbeat" ? "stale_heartbeat" : "capacity_probe_failed",
        message: `Pylon heartbeat failed before Codex spawn: ${safeFailureReason(heartbeat)}`,
        module: "advertise_capacity",
        refs: [`blocker.public.khala_fleet_delegate.${reason === "stale_heartbeat" ? "stale_heartbeat" : "capacity_probe_failed"}`],
      })
    }
    const heartbeatJson = parseJsonObject(heartbeat.stdout)
    const freshHeartbeatPylonRef = stringField(heartbeatJson, "pylonRef")
    if (freshHeartbeatPylonRef !== null) heartbeatPylonRef = freshHeartbeatPylonRef
    else if (pylonRef !== undefined) heartbeatPylonRef = pylonRef
    fleetStatus = await inspectCodexFleet({ includeProcesses: false, startPylon: false }, { ...options, env })
    providerProjection = await readProviderProjection(env, paths, options.runner)
    accountAvailability = codexAccountAvailabilityByRef(fleetStatus.accounts, providerProjection)
    const heartbeatRef = stringField(heartbeatJson, "heartbeatRef")
    return {
      capacity: khalaDelegateCapacityFromFleetStatus(
        fleetStatus,
        providerProjection,
        accountAvailability,
        delegationParameters,
      ),
      ...(heartbeatRef === null ? {} : { heartbeatRef }),
    }
  }

  const delegate = await Effect.runPromise(runKhalaFleetDelegateProgram({
    accountRef: input.accountRef,
    branch: input.branch,
    commit: input.commit,
    fixture: input.fixture,
    objective: input.prompt,
    repo: input.repo,
    verify: input.verify,
  }, {
    ensurePylon: () =>
      Effect.promise(async () => {
        const ensure = await ensureLocalPylon(
          { start: true, timeoutMs: input.timeoutMs, waitMs: DEFAULT_ENSURE_WAIT_MS },
          { ...options, env },
        )
        if (!ensure.ok) {
          throw new KhalaFleetDelegateModuleError({
            blockerCode: "pylon_unavailable",
            message: `Pylon unavailable: ${ensure.message}${ensure.unavailableReason ? ` (${ensure.unavailableReason})` : ""}`,
            module: "ensure_pylon",
            refs: ["blocker.public.khala_fleet_delegate.pylon_unavailable"],
          })
        }
        return {
          pylonRef: input.pylonRef ?? ensure.pylonRef ?? undefined,
          started: ensure.started,
        }
      }),
    advertiseCapacity: ({ pylonRef, reason }) =>
      Effect.promise(() => refreshFleetProjection(pylonRef, reason)),
    dispatch: ({ capacity, work }) =>
      Effect.promise(async () => {
        const planned = planDelegatedSpawnAccounts(
          input,
          capacity,
          fleetStatus,
          accountAvailability,
          delegationParameters,
        )
        if (planned.status === "blocked") return planned.dispatch
        const targetPylonRef = input.pylonRef ?? heartbeatPylonRef ?? ""
        dispatchedResult = input.noRun
          ? await runDelegatedNoRunRequests({
              baseUrl,
              env,
              input,
              paths,
              plannedAccounts: planned.accounts,
              parameters: delegationParameters,
              runner: options.runner,
              targetPylonRef,
            })
          : await runDelegatedBatchSpawn({
              baseUrl,
              env,
              input,
              onProgress: options.onProgress,
              paths,
              plannedAccounts: planned.accounts,
              parameters: delegationParameters,
              runner: options.runner,
              targetPylonRef,
              work,
            })
        const firstAssignmentRef = firstSpawnAssignmentRef(dispatchedResult)
        if (dispatchedResult.acceptedCount === input.count && firstAssignmentRef !== null) {
          return { assignmentRef: firstAssignmentRef, ok: true }
        }
        return {
          blockerCode: classifySpawnDispatchBlocker(dispatchedResult),
          message: renderSpawnResult(dispatchedResult),
          ok: false,
          refs: spawnDispatchBlockerRefs(dispatchedResult),
        }
      }),
    verifyCloseout: () =>
      Effect.succeed({
        ok: dispatchedResult !== null && dispatchedResult.acceptedCount === input.count,
        ...(dispatchedResult === null || dispatchedResult.acceptedCount === input.count
          ? {}
          : { message: renderSpawnResult(dispatchedResult) }),
      }),
  }, {
    parameters: delegationParameters,
  }))

  if (dispatchedResult !== null) {
    return withDelegateTrace(dispatchedResult, delegate)
  }
  if (delegate.status === "blocked") {
    throw new Error(`Khala fleet delegate blocked at ${delegate.trace.at(-1)?.module ?? "unknown"}: ${delegate.message}`)
  }
  throw new Error("Khala fleet delegate completed without a dispatch result.")
}

export function resolvePylonHome(env: ChatEnv = process.env): string {
  const explicit = env.PYLON_HOME?.trim()
  if (explicit !== undefined && explicit.length > 0) return resolve(explicit)
  const candidates = pylonHomeCandidates(env)
  const withState = candidates.find(candidate =>
    existsSync(join(candidate, "identity.json")) || existsSync(join(candidate, "config.json")),
  )
  return withState ?? candidates[0] ?? join(homedir(), ".openagents", "pylon")
}

function executePylonEnsureTool(
  input: Readonly<Record<string, unknown>>,
  options: KhalaCodexFleetToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const result = await ensureLocalPylon({
        start: optionalBoolean(input.start) ?? true,
        timeoutMs: optionalInteger(input.timeout_ms),
        waitMs: optionalInteger(input.wait_ms),
      }, options)
      const modelText = renderEnsureResult(result)
      return result.ok
        ? khalaToolOk({
            modelText,
            publicSafety: "private",
            publicSummary: result.message,
            ui: {
              kind: "pylon_status",
              ok: result.ok,
              pylonRef: result.pylonRef,
              started: result.started,
              status: result.status,
            },
          })
        : khalaToolUnavailable({
            modelText,
            publicSafety: "private",
            publicSummary: result.message,
            ui: {
              kind: "pylon_status",
              ok: result.ok,
              pylonRef: result.pylonRef,
              started: result.started,
              status: result.status,
              unavailableReason: result.unavailableReason ?? null,
            },
          })
    } catch (error) {
      return khalaToolError("pylon_ensure_failed", errorMessage(error))
    }
  })
}

function executeCodexFleetStatusTool(
  input: Readonly<Record<string, unknown>>,
  options: KhalaCodexFleetToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const result = await inspectCodexFleet({
        includeProcesses: optionalBoolean(input.include_processes) ?? true,
        startPylon: optionalBoolean(input.start_pylon) ?? false,
      }, options)
      return khalaToolOk({
        modelText: renderFleetStatus(result),
        publicSafety: "private",
        publicSummary: `Codex fleet status: ${result.accounts.length} account(s), ${readyAccountCount(result.accounts)} ready.`,
        ui: {
          accounts: result.accounts,
          activeAssignments: result.activeAssignments,
          availableCodexAssignments: result.availableCodexAssignments,
          kind: "codex_fleet_status",
          maxCodexAssignments: result.maxCodexAssignments,
          observedAt: result.observedAt,
          processes: result.processes,
          pylon: {
            ok: result.ensure.ok,
            pylonRef: result.ensure.pylonRef,
            started: result.ensure.started,
            status: result.ensure.status,
          },
          serverAssignments: result.serverAssignments,
          tokenRate: result.tokenRate,
        },
      })
    } catch (error) {
      return khalaToolError("codex_fleet_status_failed", errorMessage(error))
    }
  })
}

function executeCodexSpawnTool(
  input: Readonly<Record<string, unknown>>,
  options: KhalaCodexFleetToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const result = await spawnCodexInstances({
        accountRef: optionalString(input.account_ref),
        baseUrl: optionalString(input.base_url),
        branch: optionalString(input.branch),
        commit: optionalString(input.commit),
        count: optionalInteger(input.count),
        fixture: optionalBoolean(input.fixture),
        noRun: optionalBoolean(input.no_run),
        prompt: requiredString(input.prompt, "codex_spawn requires prompt"),
        pylonRef: optionalString(input.pylon_ref),
        repo: optionalString(input.repo),
        timeoutMs: optionalInteger(input.timeout_ms),
        verify: optionalString(input.verify),
      }, options)
      const tokensVerified = spawnVerifiedTokenTotal(result)
      const toolResult = khalaToolOk({
        modelText: renderSpawnResult(result),
        publicSafety: "private",
        publicSummary: `Codex spawn accepted ${result.acceptedCount}/${result.requestedCount} request(s); ${tokensVerified} Khala tokens generated.`,
        ui: {
          acceptedCount: result.acceptedCount,
          delegateSignature: result.delegateSignature ?? null,
          delegateStatus: result.delegateStatus ?? null,
          delegateTrace: result.delegateTrace ?? [],
          kind: "codex_spawn",
          pylonRef: result.pylonRef,
          requestedCount: result.requestedCount,
          results: result.results,
          tokensVerified,
        },
      })
      return result.acceptedCount === result.requestedCount
        ? toolResult
        : { ...toolResult, status: "failed" }
    } catch (error) {
      return khalaToolError("codex_spawn_failed", errorMessage(error))
    }
  })
}

function resolvePylonPaths(env: ChatEnv): PylonPaths {
  return {
    appPath: resolvePylonAppPath(env),
    bunExecutable: resolveBunExecutable(env),
    pylonHome: resolvePylonHome(env),
  }
}

function resolvePylonAppPath(env: ChatEnv): string {
  const candidates = dedupe([
    ...(env.OPENAGENTS_PYLON_APP_PATH ? [env.OPENAGENTS_PYLON_APP_PATH] : []),
    ...(env.OPENAGENTS_REPO_ROOT ? [resolve(env.OPENAGENTS_REPO_ROOT, "apps/pylon")] : []),
    ...(env.INIT_CWD ? ancestorPylonCandidates(env.INIT_CWD) : []),
    ...(env.PWD ? ancestorPylonCandidates(env.PWD) : []),
    ...ancestorPylonCandidates(process.cwd()),
    join(homedir(), "work", "openagents", "apps", "pylon"),
    resolve(process.cwd(), "../../apps/pylon"),
    resolve(process.cwd(), "apps/pylon"),
  ])
  return candidates.find(candidate => existsSync(join(candidate, "package.json"))) ??
    candidates[0] ??
    resolve(process.cwd(), "../../apps/pylon")
}

function ancestorPylonCandidates(anchor: string): readonly string[] {
  const candidates: string[] = []
  let current = resolve(anchor)
  for (let index = 0; index < 12; index += 1) {
    candidates.push(resolve(current, "apps/pylon"))
    candidates.push(resolve(current, "../../apps/pylon"))
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return candidates
}

function resolveBunExecutable(env: ChatEnv): string {
  const candidates = [
    ...(env.OPENAGENTS_BUN_PATH ? [env.OPENAGENTS_BUN_PATH] : []),
    process.execPath,
    join(homedir(), ".bun", "bin", "bun"),
    "/opt/homebrew/bin/bun",
    "/usr/local/bin/bun",
    "/usr/bin/bun",
  ]
  return candidates.find(candidate => candidate.length > 0 && existsSync(candidate)) ?? "bun"
}

function pylonHomeCandidates(env: ChatEnv): readonly string[] {
  const home = homedir()
  return dedupe([
    ...(env.PYLON_HOME ? [env.PYLON_HOME] : []),
    join(home, ".openagents", "pylon"),
    join(home, ".pylon"),
    ...(env.PYLON_FABLE_HOME ? [env.PYLON_FABLE_HOME] : []),
    join(home, ".pylon-fable"),
  ])
}

async function runPylonCommand(
  args: readonly string[],
  input: {
    readonly env: ChatEnv
    readonly maxOutputBytes?: number | undefined
    readonly onStderrLine?: ((line: string) => void | Promise<void>) | undefined
    readonly paths: PylonPaths
    readonly runner?: KhalaCodexFleetCommandRunner | undefined
    readonly timeoutMs: number
  },
): Promise<KhalaCodexFleetCommandResult> {
  return (input.runner ?? defaultCommandRunner)({
    cmd: [input.paths.bunExecutable, "src/index.ts", ...args],
    cwd: input.paths.appPath,
    env: pylonCommandEnv(input.env, input.paths.pylonHome),
    maxOutputBytes: input.maxOutputBytes ?? 80_000,
    onStderrLine: input.onStderrLine,
    timeoutMs: input.timeoutMs,
  })
}

async function readProviderProjection(
  env: ChatEnv,
  paths: PylonPaths,
  runner: KhalaCodexFleetCommandRunner | undefined,
): Promise<Record<string, unknown> | null> {
  const result = await runPylonCommand(["provider", "go-online", "--json"], {
    env,
    paths,
    runner,
    timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
  }).catch(errorResult)
  return result.exitCode === 0 ? parseJsonObject(result.stdout) : null
}

function pylonCommandEnv(env: ChatEnv, pylonHome: string): ChatEnv {
  const mergedEnv = { ...process.env, ...env }
  const configuredBaseUrl = configuredOpenAgentsBaseUrl(mergedEnv)
  return {
    ...mergedEnv,
    PATH: pathCandidates(mergedEnv).filter(path => path.length > 0).join(":"),
    ...(configuredBaseUrl === undefined ? {} : { PYLON_OPENAGENTS_BASE_URL: configuredBaseUrl }),
    PYLON_HOME: pylonHome,
  }
}

function configuredOpenAgentsBaseUrl(env: ChatEnv): string | undefined {
  return [env.PYLON_OPENAGENTS_BASE_URL, env.OPENAGENTS_BASE_URL]
    .map(value => value?.trim())
    .find((value): value is string => value !== undefined && value.length > 0)
}

function resolveOpenAgentsBaseUrl(env: ChatEnv, explicit?: string | undefined): string {
  return (
    [explicit, env.PYLON_OPENAGENTS_BASE_URL, env.OPENAGENTS_BASE_URL, DEFAULT_OPENAGENTS_BASE_URL]
      .map(value => value?.trim())
      .find((value): value is string => value !== undefined && value.length > 0) ?? DEFAULT_OPENAGENTS_BASE_URL
  ).replace(/\/+$/u, "")
}

function pathCandidates(env: ChatEnv): readonly string[] {
  return [
    env.PATH ?? "",
    join(homedir(), ".bun", "bin"),
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
  ]
}

async function defaultCommandRunner(
  input: KhalaCodexFleetCommandInput,
): Promise<KhalaCodexFleetCommandResult> {
  const [cmd, ...args] = input.cmd
  if (cmd === undefined) {
    return { exitCode: 127, signal: null, stderr: "empty command", stdout: "", timedOut: false }
  }

  if (input.detached) {
    try {
      const child = spawn(cmd, args, {
        cwd: input.cwd,
        detached: true,
        env: cleanEnv(input.env),
        stdio: "ignore",
      })
      child.unref()
      return { exitCode: null, signal: null, stderr: "", stdout: "", timedOut: false }
    } catch (error) {
      return { exitCode: 127, signal: null, stderr: errorMessage(error), stdout: "", timedOut: false }
    }
  }

  const maxOutputBytes = input.maxOutputBytes ?? 40_000
  let timedOut = false
  let child: ReturnType<typeof spawn>
  try {
    child = spawn(cmd, args, {
      cwd: input.cwd,
      env: cleanEnv(input.env),
      stdio: ["ignore", "pipe", "pipe"],
    })
  } catch (error) {
    return { exitCode: 127, signal: null, stderr: errorMessage(error), stdout: "", timedOut: false }
  }

  const timeout = setTimeout(() => {
    timedOut = true
    child.kill("SIGTERM")
  }, input.timeoutMs)
  timeout.unref?.()
  const forceKill = setTimeout(() => {
    if (timedOut) child.kill("SIGKILL")
  }, input.timeoutMs + 1_500)
  forceKill.unref?.()

  const [stdout, stderr, exited] = await Promise.all([
    collectStream(child.stdout, maxOutputBytes),
    collectStream(child.stderr, maxOutputBytes, { onLine: input.onStderrLine }),
    new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolveExit) => {
      child.on("error", () => resolveExit({ code: 127, signal: null }))
      child.on("close", (code, signal) => resolveExit({ code, signal }))
    }),
  ]).finally(() => {
    clearTimeout(timeout)
    clearTimeout(forceKill)
  })

  return {
    exitCode: exited.code,
    signal: exited.signal,
    stderr,
    stdout,
    timedOut,
  }
}

function collectStream(
  stream: Readable | null,
  maxBytes: number,
  options: { readonly onLine?: ((line: string) => void | Promise<void>) | undefined } = {},
): Promise<string> {
  if (stream === null) return Promise.resolve("")
  if (options.onLine !== undefined) {
    return Effect.runPromise(collectStreamLines(stream, maxBytes, options.onLine))
  }
  return new Promise(resolveText => {
    let text = ""
    stream.setEncoding("utf8")
    stream.on("data", (chunk: string) => {
      text = tailByBytes(`${text}${chunk}`, maxBytes)
    })
    stream.on("end", () => resolveText(text))
    stream.on("error", () => resolveText(text))
  })
}

function collectStreamLines(
  stream: Readable,
  maxBytes: number,
  onLine: (line: string) => void | Promise<void>,
): Effect.Effect<string, never> {
  return Effect.gen(function* () {
    let text = ""
    yield* Stream.fromAsyncIterable(
      stream as AsyncIterable<Uint8Array>,
      error => new Error(String(error)),
    ).pipe(
      Stream.decodeText(),
      Stream.splitLines,
      Stream.runForEach(line =>
        Effect.promise(async () => {
          text = tailByBytes(`${text}${line}\n`, maxBytes)
          try {
            await onLine(line)
          } catch {
            // Tool-card progress is observational; subprocess collection must
            // remain fail-soft if a UI sink disappears mid-run.
          }
        }),
      ),
      Effect.catch(() => Effect.void),
    )
    return text
  })
}

function cleanEnv(env: ChatEnv | undefined): NodeJS.ProcessEnv {
  const clean: NodeJS.ProcessEnv = {}
  for (const [key, value] of Object.entries(env ?? process.env)) {
    if (value !== undefined) clean[key] = value
  }
  return clean
}

function parseJsonObject(raw: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(raw) as unknown
    return isRecord(parsed) ? parsed : null
  } catch {
    return null
  }
}

function mergeAccountRows(
  listProjection: Record<string, unknown> | null,
  statusProjection: Record<string, unknown> | null,
): readonly AccountRow[] {
  const rows = new Map<string, AccountRow>()
  for (const account of arrayField(listProjection, "accounts")) {
    const row = accountRowFrom(account)
    if (row !== null) rows.set(row.accountRef, row)
  }
  for (const account of arrayField(statusProjection, "accounts")) {
    const row = accountRowFrom(account)
    if (row !== null) {
      const previous = rows.get(row.accountRef)
      rows.set(row.accountRef, {
        ...row,
        accountKey: row.accountKey ?? previous?.accountKey ?? null,
        accountRefHash: row.accountRefHash ?? previous?.accountRefHash ?? null,
        capacity: row.capacity ?? previous?.capacity ?? null,
        readiness: row.readiness === "unknown" ? previous?.readiness ?? "unknown" : row.readiness,
      })
    }
  }
  return [...rows.values()].sort((left, right) => left.accountRef.localeCompare(right.accountRef))
}

function accountRowFrom(value: unknown): AccountRow | null {
  const account = record(value)
  if (account === null) return null
  const provider = stringField(account, "provider")
  if (provider !== "codex") return null
  const accountRef =
    stringField(account, "accountRef") ??
    stringField(account, "ref") ??
    stringField(account, "id") ??
    "(default)"
  const accountRefHash = stringField(account, "accountRefHash")
  const readinessObject = recordField(account, "readiness")
  const readiness =
    stringField(readinessObject, "state") ??
    stringField(account, "readiness") ??
    (stringField(account, "homeState") === "present" ? "ready" : "unknown")
  const quota = recordField(account, "quota")
  return {
    accountKey: accountKeyFromHash(accountRefHash),
    accountRef,
    accountRefHash,
    capacity: null,
    provider: "codex",
    quotaState: stringField(quota, "state"),
    readiness,
  }
}

function accountKeyFromHash(accountRefHash: string | null): string | null {
  const key = accountRefHash?.split(".").at(-1)?.trim().toLowerCase()
  return key !== undefined && /^[a-f0-9]{16,64}$/u.test(key) ? key : null
}

function providerCapacity(
  listProjection: Record<string, unknown> | null,
  statusProjection: Record<string, unknown> | null,
  ensure: KhalaPylonEnsureResult,
): { available: number | null; max: number | null } {
  for (const projection of [statusProjection, listProjection]) {
    const dispatch = recordField(projection, "ownCapacityDispatch")
    const available = numberField(dispatch, "availableCodexAssignments") ??
      numberField(dispatch, "totalAvailableCodexAssignments")
    const max = numberField(dispatch, "maxCodexAssignments") ??
      numberField(dispatch, "totalMaxCodexAssignments")
    if (available !== null || max !== null) return { available, max }
  }
  if (ensure.availableCodexAssignments !== null || ensure.maxCodexAssignments !== null) {
    return {
      available: ensure.availableCodexAssignments,
      max: ensure.maxCodexAssignments,
    }
  }
  return { available: ensure.ok ? null : 0, max: ensure.ok ? null : 0 }
}

function capacityFromProviderProjection(
  projection: Record<string, unknown> | null,
): { available: number | null; max: number | null } {
  const dispatch = recordField(projection, "ownCapacityDispatch")
  return {
    available: numberField(dispatch, "availableCodexAssignments") ??
      numberField(dispatch, "totalAvailableCodexAssignments"),
    max: numberField(dispatch, "maxCodexAssignments") ??
      numberField(dispatch, "totalMaxCodexAssignments"),
  }
}

function codexAccountAvailabilityByRef(
  accounts: readonly AccountRow[],
  projection: Record<string, unknown> | null,
): ReadonlyMap<string, number> {
  const ownCapacity = recordField(projection, "ownCapacityDispatch")
  const accountRefByKey = new Map<string, string>()
  for (const account of accounts) {
    if (account.accountKey !== null) accountRefByKey.set(account.accountKey, account.accountRef)
  }
  const out = new Map<string, number>()
  for (const capacity of arrayField(ownCapacity, "codexAccounts")) {
    const row = record(capacity)
    if (row === null) continue
    const accountKey = stringField(row, "accountKey")
    const accountRef = accountKey === null ? null : accountRefByKey.get(accountKey) ?? null
    const available = numberField(row, "available")
    if (accountRef !== null && available !== null) out.set(accountRef, available)
  }
  return out
}

function withAccountCapacity(
  accounts: readonly AccountRow[],
  projections: readonly (Record<string, unknown> | null)[],
): readonly AccountRow[] {
  const capacityByKey = new Map<string, AccountCapacityRow>()
  const capacityByRef = new Map<string, AccountCapacityRow>()
  for (const projection of projections) {
    const ownCapacity = recordField(projection, "ownCapacityDispatch")
    for (const item of arrayField(ownCapacity, "codexAccounts")) {
      const row = record(item)
      if (row === null) continue
      const capacity = accountCapacityRowFrom(row)
      const accountKey = stringField(row, "accountKey")
      const accountRef = stringField(row, "accountRef")
      if (accountKey !== null) capacityByKey.set(accountKey, capacity)
      if (accountRef !== null) capacityByRef.set(accountRef, capacity)
    }
  }
  if (capacityByKey.size === 0 && capacityByRef.size === 0) return accounts
  return accounts.map(account => ({
    ...account,
    capacity:
      (account.accountKey === null ? undefined : capacityByKey.get(account.accountKey)) ??
      capacityByRef.get(account.accountRef) ??
      account.capacity,
  }))
}

function accountCapacityRowFrom(row: Record<string, unknown>): AccountCapacityRow {
  return {
    available: numberField(row, "available"),
    busy: numberField(row, "busy"),
    queued: numberField(row, "queued"),
    ready: numberField(row, "ready"),
  }
}

async function collectActiveAssignmentMarkers(pylonHome: string): Promise<readonly ActiveAssignmentMarker[]> {
  const root = join(pylonHome, "active-assignment-runs")
  const nowMs = Date.now()
  try {
    const entries = await readdir(root, { withFileTypes: true })
    const markers = await Promise.all(entries
      .filter(entry => entry.isFile() && entry.name.endsWith(".json"))
      .slice(0, 25)
      .map(async entry => {
        const path = join(root, entry.name)
        const parsed = parseJsonObject(await readFile(path, "utf8").catch(() => ""))
        const fileStat = await stat(path).catch(() => null)
        const startedAt = stringField(parsed, "startedAt")
        const refreshedAt =
          stringField(parsed, "refreshedAt") ??
          stringField(parsed, "updatedAt") ??
          stringField(parsed, "observedAt") ??
          (fileStat === null ? null : new Date(fileStat.mtimeMs).toISOString())
        return {
          accountRefHash: stringField(parsed, "accountRefHash"),
          assignmentRef: stringField(parsed, "assignmentRef"),
          elapsedMs: elapsedMsSince(startedAt, nowMs),
          issueRef: issueRefFromMarker(parsed, entry.name),
          refreshedAt,
          runRef: stringField(parsed, "runRef"),
          service: stringField(parsed, "service"),
          startedAt,
          tokenRate: assignmentTokenRateProjectionFromRecord(parsed, elapsedMsSince(startedAt, nowMs), true),
          updatedAt: refreshedAt,
        }
      }))
    return markers.sort((left, right) =>
      nullableTime(right.updatedAt) - nullableTime(left.updatedAt),
    )
  } catch {
    return []
  }
}

function fleetTokenRateProjectionFromApm(
  result: KhalaCodexFleetCommandResult,
  localActiveAssignmentCount: number,
): { serverAssignments: readonly ServerAssignmentTokenRow[]; tokenRate: FleetTokenRateProjection } {
  if (result.exitCode !== 0) {
    return {
      serverAssignments: [],
      tokenRate: unavailableFleetTokenRate(safeFailureReason(result)),
    }
  }
  const payload = parseJsonObject(result.stdout)
  if (payload === null) {
    return {
      serverAssignments: [],
      tokenRate: unavailableFleetTokenRate("khala apm returned a non-JSON response"),
    }
  }
  const counted = recordField(payload, "counted")
  const active = recordField(payload, "active")
  const rawServerAssignments = arrayField(active, "serverAssignments")
  const serverAssignments = rawServerAssignments
    .flatMap((item): ServerAssignmentTokenRow[] => {
      const assignment = record(item)
      if (assignment === null) return []
      const elapsedMs = numberField(assignment, "elapsedMs")
      return [{
        assignmentRef: stringField(assignment, "assignmentRef"),
        elapsedMs,
        tokenRate: assignmentTokenRateProjectionFromRecord(assignment, elapsedMs, true),
      }]
    })
    .slice(0, 25)
  const completedTokensPerMinute = numberField(counted, "completedTokensPerMinute")
  const completedTokenRows = firstNumberField(counted, [
    "completedTokenRows",
    "completedTokenRowCount",
    "rowCount",
    "tokenRows",
  ])
  const serverAssignmentCount =
    numberField(active, "serverAssignmentCount") ??
    serverAssignments.length
  const activeAssignmentCount = Math.max(serverAssignmentCount, localActiveAssignmentCount)
  return {
    serverAssignments,
    tokenRate: {
      activeAdjustedTokensPerMinute: numberField(active, "adjustedTokensPerMinute"),
      completedStatus: completedTokenRateStatus({
        activeAssignmentCount,
        completedTokenRows,
        completedTokensPerMinute,
        sourceRefs: stringArrayField(counted, "sourceRefs"),
      }),
      completedTokenRows,
      completedTokensPerMinute,
      inFlightTokens: numberField(active, "inFlightTokens"),
      inFlightTokensPerMinute: numberField(active, "inFlightTokensPerMinute"),
      source: "pylon_khala_apm",
      unavailableReason: null,
    },
  }
}

function unavailableFleetTokenRate(reason: string): FleetTokenRateProjection {
  return {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: reason,
  }
}

function completedTokenRateStatus(input: {
  readonly activeAssignmentCount: number
  readonly completedTokenRows: number | null
  readonly completedTokensPerMinute: number | null
  readonly sourceRefs: readonly string[]
}): TokenMeasurementStatus {
  if (input.completedTokenRows !== null) return "exact"
  if (
    input.completedTokensPerMinute !== null &&
    input.completedTokensPerMinute > 0 &&
    input.sourceRefs.some(ref => ref.includes("token_usage_events"))
  ) {
    return "exact"
  }
  return input.activeAssignmentCount > 0 ? "pending" : "not_measured"
}

function mergeActiveAssignmentTokenRates(
  markers: readonly ActiveAssignmentMarker[],
  serverAssignments: readonly ServerAssignmentTokenRow[],
): readonly ActiveAssignmentMarker[] {
  const serverByAssignmentRef = new Map<string, ServerAssignmentTokenRow>()
  for (const assignment of serverAssignments) {
    if (assignment.assignmentRef !== null) serverByAssignmentRef.set(assignment.assignmentRef, assignment)
  }
  return markers.map(marker => {
    const server = marker.assignmentRef === null ? undefined : serverByAssignmentRef.get(marker.assignmentRef)
    if (server === undefined) return marker
    return {
      ...marker,
      elapsedMs: marker.elapsedMs ?? server.elapsedMs,
      tokenRate: server.tokenRate.status === "not_measured" ? marker.tokenRate : server.tokenRate,
    }
  })
}

function assignmentTokenRateProjectionFromRecord(
  source: Record<string, unknown> | null,
  elapsedMs: number | null,
  active: boolean,
): AssignmentTokenRateProjection {
  const proof = recordField(source, "proof")
  const tokenUsage = recordField(source, "tokenUsage")
  const tokens =
    numberField(source, "tokens") ??
    numberField(source, "tokensSoFar") ??
    numberField(source, "totalTokens") ??
    numberField(proof, "totalTokens") ??
    numberField(tokenUsage, "totalTokens")
  const tokenRows =
    firstNumberField(source, ["tokenRows", "rowCount"]) ??
    firstNumberField(proof, ["tokenRows", "rowCount"]) ??
    firstNumberField(tokenUsage, ["rowCount", "tokenRows"])
  const tokenCountKind =
    stringField(source, "tokenCountKind") ??
    stringField(proof, "usageTruth") ??
    stringField(tokenUsage, "usageTruth")
  const rawSource = stringField(source, "source") ?? "not_measured"
  const tokensPerMinute =
    numberField(source, "tokensPerMinute") ??
    tokensPerMinuteFromElapsed(tokens, elapsedMs)
  const exact =
    tokenRows !== null ||
    tokenCountKind === "exact" ||
    stringField(source, "usageTruth") === "exact" ||
    stringField(proof, "usageTruth") === "exact" ||
    stringField(tokenUsage, "usageTruth") === "exact"
  const status: TokenMeasurementStatus = exact
    ? "exact"
    : tokens !== null && tokens > 0
      ? "estimated"
      : active
        ? "pending"
        : "not_measured"
  return {
    source: exact ? "token_usage_events" : rawSource,
    status,
    tokenCountKind,
    tokens,
    tokensPerMinute,
  }
}

function tokensPerMinuteFromElapsed(tokens: number | null, elapsedMs: number | null): number | null {
  if (tokens === null || elapsedMs === null) return null
  const elapsedMinutes = Math.max(elapsedMs / 60_000, 1 / 6)
  return Math.round(tokens / elapsedMinutes)
}

function elapsedMsSince(startedAt: string | null, nowMs: number): number | null {
  if (startedAt === null) return null
  const startedMs = Date.parse(startedAt)
  return Number.isFinite(startedMs) ? Math.max(0, nowMs - startedMs) : null
}

async function collectProcessSnapshot(
  runner: KhalaCodexFleetCommandRunner,
  env: ChatEnv,
): Promise<readonly ProcessRow[]> {
  const result = await runner({
    cmd: ["ps", "-axo", "pid,ppid,etime,command"],
    env: pylonCommandEnv(env, resolvePylonHome(env)),
    maxOutputBytes: 80_000,
    timeoutMs: 5_000,
  }).catch(errorResult)
  if (result.exitCode !== 0) return []
  return result.stdout
    .split(/\r?\n/u)
    .slice(1)
    .map(parsePsRow)
    .filter((row): row is NonNullable<ReturnType<typeof parsePsRow>> => row !== null)
    .filter(row => isCodexExecAgentProcess(row.command))
    .slice(0, 25)
    .map(row => ({
      elapsed: row.elapsed,
      kind: "codex_exec",
      parentPid: row.parentPid,
      pid: row.pid,
    }))
}

function parsePsRow(line: string): { command: string; elapsed: string; parentPid: string; pid: string } | null {
  const trimmed = line.trim()
  if (trimmed.length === 0) return null
  const match = /^(?<pid>\d+)\s+(?<parentPid>\d+)\s+(?<elapsed>\S+)\s+(?<command>.+)$/u.exec(trimmed)
  if (match?.groups === undefined) return null
  return {
    command: match.groups.command ?? "",
    elapsed: match.groups.elapsed ?? "",
    parentPid: match.groups.parentPid ?? "",
    pid: match.groups.pid ?? "",
  }
}

function isCodexExecAgentProcess(command: string): boolean {
  if (!/(^|[\s/])codex\s+exec(?:\s|$)/iu.test(command)) return false
  if (/\/Applications\/Codex\.app\//iu.test(command)) return false
  if (/\bdurable-runner-pool\.sh\b/iu.test(command)) return false
  if (/\b(?:grep|rg|ripgrep)\b.*\bcodex\s+exec\b/iu.test(command)) return false
  if (/\bps\s+-axo\b/iu.test(command)) return false
  if (/\bkhala-codex-fleet-tools\b/iu.test(command)) return false
  return true
}

function decodeSpawnInput(
  raw: SpawnCodexInstancesInput,
  parameters: KhalaFleetDelegationParameterSet,
): NormalizedSpawnInput {
  const delegationParameters = resolveKhalaFleetDelegationParameters(parameters)
  const prompt = raw.prompt.trim()
  if (prompt.length === 0) throw new Error("codex_spawn requires a non-empty prompt")
  const count = boundedPositiveInteger(raw.count, 1, 1, MAX_SPAWN_COUNT)
  const timeoutMs = boundedPositiveInteger(raw.timeoutMs, DEFAULT_SPAWN_TIMEOUT_MS, 10_000, 600_000)
  const hasAnyPin = raw.repo !== undefined || raw.commit !== undefined || raw.verify !== undefined
  const fixture = raw.fixture ?? !hasAnyPin
  const verify = raw.verify ?? (!fixture && (raw.repo !== undefined || raw.commit !== undefined)
    ? delegationParameters.verifyCriteria?.defaultVerify
    : undefined)
  const missingPins = [
    raw.repo === undefined ? "repo" : null,
    raw.commit === undefined ? "commit" : null,
    verify === undefined ? "verify" : null,
  ].filter((value): value is string => value !== null)
  if (fixture && hasAnyPin) {
    throw new Error("codex_spawn fixture cannot be combined with repo, commit, or verify pins")
  }
  if (!fixture && missingPins.length > 0) {
    throw new Error(`codex_spawn requires fixture: true or complete real-work pins; missing ${missingPins.join(", ")}`)
  }
  return {
    ...raw,
    accountRef: normalizeRequestedAccountRef(raw.accountRef),
    count,
    fixture,
    noRun: raw.noRun ?? false,
    prompt,
    timeoutMs,
    verify,
  }
}

function workspacePinArgs(input: NormalizedSpawnInput): readonly string[] {
  return [
    "--repo",
    input.repo ?? "",
    ...(input.branch === undefined ? [] : ["--branch", input.branch]),
    "--commit",
    input.commit ?? "",
    "--verify",
    input.verify ?? "",
  ]
}

function renderEnsureResult(result: KhalaPylonEnsureResult): string {
  return [
    `Pylon: ${result.status}${result.pylonRef ? ` (${result.pylonRef})` : ""}`,
    result.message,
    result.unavailableReason ? `Reason: ${result.unavailableReason}` : null,
  ].filter((line): line is string => line !== null).join("\n")
}

function renderFleetStatus(result: FleetStatusResult): string {
  const lines = [
    `Pylon: ${result.ensure.ok ? result.ensure.status : "unavailable"}${result.ensure.pylonRef ? ` (${result.ensure.pylonRef})` : ""}`,
    `Codex capacity: ${capacityLabel(result.availableCodexAssignments, result.maxCodexAssignments)}`,
    `Codex accounts: ${result.accounts.length} total, ${readyAccountCount(result.accounts)} ready`,
    renderFleetTokenRate(result.tokenRate),
  ]
  if (result.accounts.length > 0) {
    lines.push(...result.accounts.map(renderAccountStatusLine))
  } else {
    lines.push("- no Pylon Codex accounts found")
  }
  lines.push(`Active assignment markers: ${result.activeAssignments.length}`)
  if (result.activeAssignments.length > 0) {
    lines.push(...result.activeAssignments.slice(0, 8).map(renderActiveAssignmentLine))
  }
  if (result.serverAssignments.length > 0) {
    lines.push(`Server assignment token rows: ${result.serverAssignments.length}`)
    lines.push(...result.serverAssignments.slice(0, 8).map(renderServerAssignmentTokenLine))
  }
  lines.push(`Active Codex exec processes: ${result.processes.length}`)
  if (result.activeAssignments.length !== result.processes.length) {
    lines.push(
      `Assignment/process reconciliation: ${result.activeAssignments.length} marker(s), ${result.processes.length} codex exec process(es)`,
    )
  }
  if (result.processes.length > 0) {
    lines.push(...result.processes.slice(0, 5).map(process =>
      `- ${process.pid} parent=${process.parentPid} elapsed=${process.elapsed} ${process.kind}`,
    ))
  }
  return lines.join("\n")
}

function renderFleetTokenRate(tokenRate: FleetTokenRateProjection): string {
  if (tokenRate.source === "unavailable") {
    return `Token rate: not_measured (khala apm unavailable: ${tokenRate.unavailableReason ?? "unknown"})`
  }
  const completed = tokenRate.completedTokensPerMinute === null ||
      (tokenRate.completedStatus !== "exact" && tokenRate.completedTokensPerMinute === 0)
    ? `${tokenRate.completedStatus} exact token rows`
    : `${tokenRate.completedStatus} ${tokenRate.completedTokensPerMinute} tokens/min completed window`
  const rows = tokenRate.completedTokenRows === null ? "" : ` across ${tokenRate.completedTokenRows} exact row(s)`
  const active = tokenRate.activeAdjustedTokensPerMinute === null
    ? ""
    : `; active-adjusted ${tokenRate.activeAdjustedTokensPerMinute} tokens/min`
  const inFlight = tokenRate.inFlightTokens === null
    ? ""
    : `; in-flight ${tokenRate.inFlightTokens} token(s)`
  return `Token rate: ${completed}${rows}${active}${inFlight}`
}

function renderAccountStatusLine(account: AccountRow): string {
  const capacity = account.capacity
  const capacityText = capacity === null
    ? ""
    : `, slots ${capacityLabel(capacity.available, capacity.ready)}${capacity.busy === null ? "" : `, busy ${capacity.busy}`}${capacity.queued === null ? "" : `, queued ${capacity.queued}`}`
  return `- ${account.accountRef}: ${account.readiness}${capacityText}${account.quotaState ? `, quota ${account.quotaState}` : ""}`
}

function renderActiveAssignmentLine(marker: ActiveAssignmentMarker): string {
  const ref = marker.assignmentRef ?? "(unknown assignment)"
  const issue = marker.issueRef === null ? "" : ` issue=${marker.issueRef}`
  const elapsed = marker.elapsedMs === null ? "" : ` elapsed=${formatElapsedMs(marker.elapsedMs)}`
  const account = marker.accountRefHash === null ? "" : ` account=${marker.accountRefHash}`
  return `- ${ref}${issue}${elapsed}${account} ${renderAssignmentTokenRate(marker.tokenRate)}`
}

function renderServerAssignmentTokenLine(row: ServerAssignmentTokenRow): string {
  const ref = row.assignmentRef ?? "(unknown assignment)"
  const elapsed = row.elapsedMs === null ? "" : ` elapsed=${formatElapsedMs(row.elapsedMs)}`
  return `- ${ref}${elapsed} ${renderAssignmentTokenRate(row.tokenRate)}`
}

function renderAssignmentTokenRate(tokenRate: AssignmentTokenRateProjection): string {
  if (tokenRate.status === "pending") return "tokens=pending exact rows"
  if (tokenRate.status === "not_measured") return "tokens=not_measured"
  const tokens = tokenRate.tokens === null ? "unknown" : String(tokenRate.tokens)
  const rate = tokenRate.tokensPerMinute === null ? "" : `, ${tokenRate.tokensPerMinute} tokens/min`
  const kind = tokenRate.tokenCountKind === null ? "" : `, kind=${tokenRate.tokenCountKind}`
  return `tokens=${tokenRate.status} ${tokens}${rate}${kind}`
}

function spawnResultFromBatchCommand(input: {
  readonly accountHints: readonly AccountRow[]
  readonly command: KhalaCodexFleetCommandResult
  readonly fallbackPylonRef: string | null
  readonly requestedCount: number
}): SpawnCodexInstancesResult {
  const payload = parseJsonObject(input.command.stdout)
  if (input.command.exitCode !== 0 || payload === null) {
    return {
      acceptedCount: 0,
      pylonRef: input.fallbackPylonRef,
      requestedCount: input.requestedCount,
      results: [{
        accountRef: input.accountHints[0]?.accountRef ?? null,
        assignmentRef: null,
        autoRunOk: null,
        exitCode: input.command.exitCode,
        slot: 1,
        status: "failed",
        summary: safeFailureReason(input.command),
        tokensVerified: null,
      }],
    }
  }

  const aggregate = recordField(payload, "aggregate")
  const plan = recordField(payload, "plan")
  const planSlots = arrayField(plan, "slots")
  const rawResults = arrayField(payload, "results")
  const acceptedCount = numberField(aggregate, "acceptedCount") ??
    rawResults.filter(slot => booleanField(record(slot), "ok") === true).length
  const requestedCount = numberField(plan, "requestedCount") ?? input.requestedCount
  const pylonRef = stringField(plan, "targetPylonRef") ?? input.fallbackPylonRef
  const results = rawResults.map((slotValue, index): SpawnSlotResult => {
    const slot = batchSpawnSlotProjection(slotValue, index)
    const accountRef =
      batchPlanSlotAccountRef(planSlots[slot.slotIndex]) ??
      input.accountHints[index]?.accountRef ??
      null
    const accepted = slot.ok === true && slot.runAccepted !== false
    return {
      accountRef,
      assignmentRef: slot.assignmentRef,
      autoRunOk: slot.runAccepted,
      exitCode: input.command.exitCode,
      slot: slot.slotIndex + 1,
      status: accepted ? "accepted" : "failed",
      summary: acceptedBatchSpawnSummary(slot, payload),
      tokensVerified: numberField(slot.proof, "totalTokens"),
    }
  })

  return {
    acceptedCount,
    pylonRef,
    requestedCount,
    results,
  }
}

function withCodexCapacityAdvertisementEnv(
  env: ChatEnv,
  requestedCount: number,
  parameters: KhalaFleetDelegationParameterSet,
): ChatEnv {
  const fallbackTarget = Math.max(
    DEFAULT_CODEX_ACCOUNT_CONCURRENCY,
    requestedCount,
    positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY) ?? 0,
    positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_CONCURRENCY) ?? 0,
  )
  const admittedTarget = khalaFleetDelegationPerAccountConcurrency(parameters, fallbackTarget)
  const target = Math.max(
    admittedTarget,
    requestedCount,
    positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY) ?? 0,
    positiveIntegerEnv(env.OPENAGENTS_PYLON_CODEX_CONCURRENCY) ?? 0,
  )
  return {
    ...env,
    OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY: String(target),
    OPENAGENTS_PYLON_CODEX_BUSY: "0",
    OPENAGENTS_PYLON_CODEX_CONCURRENCY: String(target),
    OPENAGENTS_PYLON_CODEX_QUEUED: "0",
  }
}

function khalaDelegateCapacityFromFleetStatus(
  status: FleetStatusResult,
  projection: Record<string, unknown> | null,
  availability: ReadonlyMap<string, number>,
  parameters: KhalaFleetDelegationParameterSet = resolveKhalaFleetDelegationParameters(undefined),
): KhalaFleetDelegateCapacity {
  const capacity = capacityFromProviderProjection(projection)
  const accounts = preferredSpawnAccounts(status.accounts, availability, parameters)
    .map(account => khalaDelegateAccountFromRow(account, availability))
  const readyCount = accounts.filter(account => account.readiness === "ready" || account.readiness === "available").length
  const accountAvailable = availability.size === 0
    ? readyCount
    : accounts.reduce((sum, account) => sum + Math.max(0, account.availableSlots ?? 0), 0)
  const available = capacity.available ?? status.availableCodexAssignments ?? accountAvailable
  const max = capacity.max ?? status.maxCodexAssignments ?? Math.max(available, readyCount)
  return {
    accounts,
    available: Math.max(0, Math.floor(available)),
    max: Math.max(0, Math.floor(max)),
  }
}

function khalaDelegateAccountFromRow(
  account: AccountRow,
  availability: ReadonlyMap<string, number>,
): KhalaFleetDelegateAccount {
  const availableSlots = availability.get(account.accountRef)
  return {
    accountRef: account.accountRef,
    ...(availableSlots === undefined ? {} : { availableSlots }),
    isDefault: isDefaultAccountRef(account.accountRef),
    readiness: khalaDelegateReadiness(account),
  }
}

function khalaDelegateReadiness(account: AccountRow): KhalaFleetDelegateAccount["readiness"] {
  if (account.readiness === "ready" || account.readiness === "available") return account.readiness
  if (/credential|login|auth|missing/iu.test(account.readiness)) return "credentials_missing"
  if (/revok|disabled/iu.test(account.readiness)) return "revoked"
  return "unknown"
}

function planDelegatedSpawnAccounts(
  input: NormalizedSpawnInput,
  capacity: KhalaFleetDelegateCapacity,
  status: FleetStatusResult | null,
  availability: ReadonlyMap<string, number>,
  parameters: KhalaFleetDelegationParameterSet,
):
  | Readonly<{ accounts: readonly AccountRow[]; status: "planned" }>
  | Readonly<{ dispatch: { blockerCode: KhalaFleetDelegateBlockerCode; message: string; ok: false; refs: readonly string[] }; status: "blocked" }> {
  if (status === null) {
    return {
      dispatch: {
        blockerCode: "capacity_probe_failed",
        message: "Codex fleet projection was not available after capacity advertisement.",
        ok: false,
        refs: ["blocker.public.khala_fleet_delegate.capacity_probe_failed"],
      },
      status: "blocked",
    }
  }
  const readyAccounts = preferredSpawnAccounts(
    status.accounts.filter(account => isReadyAccount(account)),
    availability,
    parameters,
  )
  if (readyAccounts.length === 0) {
    return {
      dispatch: {
        blockerCode: "connect_account_required",
        message:
          "No ready Pylon Codex account is connected. Connect one first with `khala fleet connect` or `pylon auth codex --account codex`; this Desktop tool will not run Codex device login.",
        ok: false,
        refs: ["blocker.public.khala_fleet_delegate.connect_account_required"],
      },
      status: "blocked",
    }
  }
  let selectedAccounts: readonly AccountRow[]
  try {
    selectedAccounts = resolveSpawnAccounts(input.accountRef, readyAccounts, status.accounts, availability, parameters)
  } catch (error) {
    const message = errorMessage(error)
    const blockerCode: KhalaFleetDelegateBlockerCode = /not found|not connected/iu.test(message)
      ? "connect_account_required"
      : /revoked/iu.test(message)
        ? "revoked"
        : "credentials_missing"
    return {
      dispatch: {
        blockerCode,
        message,
        ok: false,
        refs: [`blocker.public.khala_fleet_delegate.${blockerCode}`],
      },
      status: "blocked",
    }
  }
  if (selectedAccounts.length === 0 || capacity.available <= 0) {
    return {
      dispatch: {
        blockerCode: "no_available_codex_capacity",
        message: `No Pylon Codex assignment capacity is available right now (${capacityLabel(capacity.available, capacity.max)}).`,
        ok: false,
        refs: ["blocker.public.pylon_dispatch.no_available_codex_capacity"],
      },
      status: "blocked",
    }
  }
  const plannedAccounts = planSpawnAccounts(input.count, selectedAccounts, availability)
  if (plannedAccounts.length < input.count) {
    return {
      dispatch: {
        blockerCode: "no_available_codex_capacity",
        message: `Only ${plannedAccounts.length}/${input.count} advertised Pylon Codex account slot(s) are free right now.`,
        ok: false,
        refs: ["blocker.public.pylon_dispatch.no_available_codex_capacity"],
      },
      status: "blocked",
    }
  }
  return { accounts: plannedAccounts, status: "planned" }
}

async function runDelegatedBatchSpawn(input: {
  readonly baseUrl: string
  readonly env: ChatEnv
  readonly input: NormalizedSpawnInput
  readonly onProgress?: KhalaCodexFleetProgressSink | undefined
  readonly parameters: KhalaFleetDelegationParameterSet
  readonly paths: PylonPaths
  readonly plannedAccounts: readonly AccountRow[]
  readonly runner?: KhalaCodexFleetCommandRunner | undefined
  readonly targetPylonRef: string
  readonly work: KhalaFleetDelegateWork
}): Promise<SpawnCodexInstancesResult> {
  const selectedCommandAccount =
    input.input.accountRef === undefined
      ? undefined
      : commandAccountRef(input.plannedAccounts[0]?.accountRef)
  const workArgs = input.work.kind === "fixture"
    ? ["--fixture"]
    : [
        "--repo",
        input.work.repo,
        "--branch",
        input.work.branch,
        "--commit",
        input.work.commit,
        "--verify",
        input.work.verify,
      ]
  const objective = renderKhalaFleetDelegationObjective({
    objective: input.input.prompt,
    repo: input.work.kind === "fixture" ? undefined : input.work.repo,
    verify: input.work.kind === "fixture" ? undefined : input.work.verify,
  }, input.parameters)
  const streamedLifecycleEvents: Record<string, unknown>[] = []
  const result = await runPylonCommand([
    "khala",
    "spawn",
    "--count",
    String(input.input.count),
    "--max-parallel",
    String(input.input.count),
    "--objective",
    objective,
    "--pylon-ref",
    input.targetPylonRef,
    ...(selectedCommandAccount === undefined ? [] : ["--account-ref", selectedCommandAccount]),
    ...workArgs,
    "--base-url",
    input.baseUrl,
    "--execute",
    "--lifecycle-ndjson",
    "--json",
  ], {
    env: input.env,
    maxOutputBytes: 5_000_000,
    onStderrLine: line =>
      handlePylonLifecycleStderrLine({
        events: streamedLifecycleEvents,
        line,
        onProgress: input.onProgress,
      }),
    paths: input.paths,
    runner: input.runner,
    timeoutMs: input.input.timeoutMs,
  })
  return spawnResultFromBatchCommand({
    accountHints: input.plannedAccounts,
    command: result,
    fallbackPylonRef: input.targetPylonRef || null,
    requestedCount: input.input.count,
  })
}

async function runDelegatedNoRunRequests(input: {
  readonly baseUrl: string
  readonly env: ChatEnv
  readonly input: NormalizedSpawnInput
  readonly parameters: KhalaFleetDelegationParameterSet
  readonly paths: PylonPaths
  readonly plannedAccounts: readonly AccountRow[]
  readonly runner?: KhalaCodexFleetCommandRunner | undefined
  readonly targetPylonRef: string
}): Promise<SpawnCodexInstancesResult> {
  const objective = renderKhalaFleetDelegationObjective({
    objective: input.input.prompt,
    repo: input.input.repo,
    verify: input.input.verify,
  }, input.parameters)
  const results = await Promise.all(input.plannedAccounts.map(async (selectedAccount, index): Promise<SpawnSlotResult> => {
    const selectedCommandAccount = commandAccountRef(selectedAccount.accountRef)
    const result = await runPylonCommand([
      "khala",
      "request",
      "--workflow",
      "codex_agent_task",
      "--prompt",
      objective,
      "--pylon-ref",
      input.targetPylonRef,
      ...(selectedCommandAccount === undefined ? [] : ["--account-ref", selectedCommandAccount]),
      ...(input.input.fixture ? ["--fixture"] : workspacePinArgs(input.input)),
      "--base-url",
      input.baseUrl,
      "--no-run",
      "--json",
    ], {
      env: input.env,
      paths: input.paths,
      runner: input.runner,
      timeoutMs: input.input.timeoutMs,
    })
    const json = parseJsonObject(result.stdout)
    const assignmentRef = stringField(json, "assignmentRef")
    const autoRunOk = booleanField(recordField(json, "autoRun"), "ok")
    const accepted = result.exitCode === 0 && assignmentRef !== null && autoRunOk !== false
    const hasAssignmentProjection = result.exitCode === 0 && assignmentRef !== null
    return {
      accountRef: selectedAccount.accountRef,
      assignmentRef,
      autoRunOk,
      exitCode: result.exitCode,
      slot: index + 1,
      status: accepted ? "accepted" : "failed",
      summary: hasAssignmentProjection
        ? acceptedSpawnSummary(assignmentRef, json)
        : safeFailureReason(result),
      tokensVerified: hasAssignmentProjection
        ? numberField(recordField(json, "proof"), "totalTokens")
        : null,
    }
  }))

  return {
    acceptedCount: results.filter(result => result.status === "accepted").length,
    pylonRef: input.targetPylonRef || null,
    requestedCount: input.input.count,
    results,
  }
}

function firstSpawnAssignmentRef(result: SpawnCodexInstancesResult): string | null {
  return result.results.find(slot => slot.assignmentRef !== null)?.assignmentRef ?? null
}

function classifySpawnDispatchBlocker(result: SpawnCodexInstancesResult): KhalaFleetDelegateBlockerCode {
  const text = renderSpawnResult(result)
  if (/duplicate_active_assignment|duplicate active assignment/iu.test(text)) return "duplicate_active_assignment"
  if (/stale_heartbeat|stale heartbeat|presence\.stale_heartbeat/iu.test(text)) return "stale_heartbeat"
  if (/no_available_codex_capacity|no .*capacity|0\/\d+ available|rate.?limit|429|409/iu.test(text)) {
    return "no_available_codex_capacity"
  }
  return "dispatch_failed"
}

function spawnDispatchBlockerRefs(result: SpawnCodexInstancesResult): readonly string[] {
  const refs = result.results
    .flatMap(slot => slot.summary.match(/blocker(?: refs)?: ([^\n]+)/iu)?.[1]?.split(",") ?? [])
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0 && ref !== "none")
  if (refs.length > 0) return [...new Set(refs)]
  const blocker = classifySpawnDispatchBlocker(result)
  if (blocker === "duplicate_active_assignment") return ["blocker.public.pylon_dispatch.duplicate_active_assignment"]
  if (blocker === "stale_heartbeat") return ["blocker.public.pylon_dispatch.stale_heartbeat"]
  if (blocker === "no_available_codex_capacity") return ["blocker.public.pylon_dispatch.no_available_codex_capacity"]
  return ["blocker.public.khala_fleet_delegate.dispatch_failed"]
}

function batchSpawnSlotProjection(value: unknown, fallbackIndex: number): BatchSpawnSlotProjection {
  const slot = record(value)
  const rawIndex = numberField(slot, "slotIndex")
  return {
    accountRef: null,
    assignmentRef: stringField(slot, "assignmentRef"),
    blockerRefs: stringArrayField(slot, "blockerRefs"),
    closeoutStatus: stringField(slot, "closeoutStatus"),
    durableRequestId: stringField(slot, "durableRequestId"),
    failure: recordField(slot, "failure"),
    lifecycleEvents: arrayField(slot, "lifecycleEvents").flatMap(event => {
      const item = record(event)
      return item === null ? [] : [item]
    }),
    ok: booleanField(slot, "ok"),
    proof: recordField(slot, "proof"),
    runAccepted: booleanField(slot, "runAccepted"),
    slotIndex: rawIndex === null ? fallbackIndex : Math.max(0, Math.floor(rawIndex)),
    state: stringField(slot, "state"),
  }
}

function batchPlanSlotAccountRef(value: unknown): string | null {
  const slot = record(value)
  const account = recordField(slot, "account")
  return stringField(account, "accountRef")
}

export function spawnVerifiedTokenTotal(result: SpawnCodexInstancesResult): number {
  return result.results.reduce(
    (sum, slot) => sum + (slot.tokensVerified ?? 0),
    0,
  )
}

function renderSpawnResult(result: SpawnCodexInstancesResult): string {
  const tokensVerified = spawnVerifiedTokenTotal(result)
  return [
    ...renderDelegateTraceLines(result),
    `Codex spawn: accepted ${result.acceptedCount}/${result.requestedCount}${result.pylonRef ? ` via ${result.pylonRef}` : ""}`,
    ...(tokensVerified > 0
      ? [`Khala tokens generated: ${tokensVerified} verified (exact)`]
      : []),
    ...result.results.map(slot => renderSpawnSlotResult(slot)),
  ].join("\n")
}

function renderDelegateTraceLines(result: SpawnCodexInstancesResult): string[] {
  if (result.delegateTrace === undefined || result.delegateTrace.length === 0) {
    return []
  }
  const status = result.delegateStatus ?? "completed"
  return [
    `Khala fleet delegate: ${result.delegateSignature ?? "khala.fleet.delegate"} (${status})`,
    ...result.delegateTrace.map(step =>
      `- ${step.module}: ${step.status}${step.fallbackModule === undefined ? "" : ` -> ${step.fallbackModule}`}${step.blockerCode === undefined ? "" : ` [${step.blockerCode}]`} :: ${step.message}`,
    ),
  ]
}

function withDelegateTrace(
  result: SpawnCodexInstancesResult,
  delegate: KhalaFleetDelegateProgramResult,
): SpawnCodexInstancesResult {
  return {
    ...result,
    delegateSignature: delegate.signature,
    delegateStatus: delegate.status,
    delegateTrace: delegate.trace,
  }
}

function renderSpawnSlotResult(slot: SpawnSlotResult): string {
  const headline = `- slot ${slot.slot}${slot.accountRef ? ` ${slot.accountRef}` : ""}: ${slot.status}`
  const details = slot.summary
    .split("\n")
    .map(line => line.trimEnd())
    .filter(line => line.length > 0)
  return details.length === 0
    ? headline
    : `${headline}\n  ${details.join("\n  ")}`
}

function acceptedBatchSpawnSummary(
  slot: BatchSpawnSlotProjection,
  payload: Record<string, unknown>,
): string {
  const counter = recordField(payload, "counter")
  const lines = [
    slot.assignmentRef === null ? null : `assignment: ${slot.assignmentRef}`,
    slot.durableRequestId === null ? null : `durable request: ${slot.durableRequestId}`,
    slot.state === null ? null : `state: ${slot.state}`,
    slot.runAccepted === null ? null : `assignment run: ${slot.runAccepted ? "completed" : "failed"}`,
    slot.closeoutStatus === null ? null : `closeout: ${slot.closeoutStatus}`,
    `blocker refs: ${slot.blockerRefs.length === 0 ? "none" : slot.blockerRefs.slice(0, 3).join(", ")}`,
    ...batchProofSummaryLines(slot.proof),
    ...batchFailureSummaryLines(slot.failure),
    ...batchCounterSummaryLines(counter),
    ...batchLifecycleSummaryLines(slot.lifecycleEvents),
    slot.ok === true ? "next: summarize this status; no local output path was returned" : null,
  ].filter((line): line is string => line !== null && line.length > 0)
  return lines.join("\n")
}

function batchProofSummaryLines(proof: Record<string, unknown> | null): string[] {
  if (proof === null) return []
  const totalTokens = numberField(proof, "totalTokens")
  const tokenRows = numberField(proof, "tokenRows")
  const traceCount = numberField(proof, "traceCount")
  const rawEventCount = numberField(proof, "rawEventCount")
  return [
    `proof: ${totalTokens ?? 0} verified tokens across ${tokenRows ?? 0} row(s)`,
    `owner evidence: ${traceCount ?? 0} trace(s), ${rawEventCount ?? 0} raw event(s)`,
  ]
}

function batchFailureSummaryLines(failure: Record<string, unknown> | null): string[] {
  if (failure === null) return []
  const message = stringField(failure, "message") ?? "worker failed"
  const ref = stringField(failure, "ref")
  return [`failure: ${ref === null ? message : `${message} (${ref})`}`]
}

function batchCounterSummaryLines(counter: Record<string, unknown> | null): string[] {
  if (counter === null) return []
  const state = stringField(counter, "state")
  const delta = numberField(counter, "delta")
  const expected = numberField(counter, "expectedMinimumDelta")
  if (state === null && delta === null && expected === null) return []
  return [`counter: ${state ?? "unknown"}${delta === null ? "" : `, delta ${delta}`}${expected === null ? "" : `, expected ${expected}`}`]
}

function batchLifecycleSummaryLines(events: readonly Record<string, unknown>[]): string[] {
  if (events.length === 0) return []
  return [
    "lifecycle:",
    ...events.slice(-8).map(event => {
      const assignmentEvent = stringField(event, "assignmentEvent")
      const state = stringField(event, "state")
      const status = stringField(event, "status")
      const message = stringField(event, "message")
      const parts = [
        assignmentEvent ?? state,
        status === null ? null : `status=${status}`,
        message === null ? null : message,
      ].filter((part): part is string => part !== null && part.length > 0)
      return `  - ${parts.length === 0 ? "event" : parts.join(" · ")}`
    }),
  ]
}

function acceptedSpawnSummary(
  assignmentRef: string,
  payload: Record<string, unknown> | null,
): string {
  const autoRun = recordField(payload, "autoRun")
  const assignmentRun = recordField(payload, "assignmentRun")
  const lifecycleEvents = lifecycleEventsFromUnknown(payload?.assignmentLifecycleEvents)
  const lines = [`assignment: ${assignmentRef}`]

  const attempted = booleanField(autoRun, "attempted")
  const autoRunOk = booleanField(autoRun, "ok")
  const autoRunReason = stringField(autoRun, "reason")
  if (attempted === false) {
    lines.push(`auto-run: not attempted${autoRunReason === null ? "" : ` (${autoRunReason})`}`)
  } else if (autoRunOk !== null) {
    lines.push(`auto-run: ${autoRunOk ? "completed" : "failed"}`)
  } else {
    lines.push("auto-run: unknown")
  }

  if (assignmentRun === null) {
    lines.push("assignment run: no result returned")
  } else {
    const runOk = booleanField(assignmentRun, "ok")
    if (runOk !== null) {
      lines.push(`assignment run: ${runOk ? "completed" : "failed"}`)
    }
    const closeout = recordField(assignmentRun, "closeout")
    if (closeout !== null) {
      const closeoutParts = [
        stringField(closeout, "status"),
        stringField(closeout, "paymentMode"),
        stringField(closeout, "settlementState"),
      ].filter((value): value is string => value !== null)
      if (closeoutParts.length > 0) lines.push(`closeout: ${closeoutParts.join(", ")}`)
      const resultRefs = stringArrayField(closeout, "resultRefs")
      if (resultRefs.length > 0) lines.push(`result refs: ${resultRefs.slice(0, 3).join(", ")}`)
      const blockerRefs = stringArrayField(closeout, "blockerRefs")
      lines.push(`blocker refs: ${blockerRefs.length === 0 ? "none" : blockerRefs.slice(0, 3).join(", ")}`)
    }
    const closeoutReceipt = recordField(assignmentRun, "closeoutReceipt")
    const closeoutRef = stringField(closeoutReceipt, "closeoutRef")
    if (closeoutRef !== null) lines.push(`closeout ref: ${closeoutRef}`)
  }

  const lifecycleLines = renderLifecycleSummaryLines(lifecycleEvents)
  if (lifecycleLines.length > 0) lines.push(...lifecycleLines)
  lines.push("next: summarize this status; no local output path was returned")
  return lines.join("\n")
}

function readyAccountCount(accounts: readonly AccountRow[]): number {
  return accounts.filter(isReadyAccount).length
}

function isReadyAccount(account: AccountRow): boolean {
  return account.readiness === "ready" || account.readiness === "available"
}

function preferredSpawnAccounts(
  accounts: readonly AccountRow[],
  availability: ReadonlyMap<string, number> = new Map(),
  parameters: KhalaFleetDelegationParameterSet = resolveKhalaFleetDelegationParameters(undefined),
): readonly AccountRow[] {
  const heuristic =
    resolveKhalaFleetDelegationParameters(parameters).accountRanking?.heuristic ??
    "named_ready_highest_slots"
  return [...accounts].sort((left, right) => {
    const leftReady = isReadyAccount(left)
    const rightReady = isReadyAccount(right)
    if (leftReady !== rightReady) return leftReady ? -1 : 1
    if (heuristic === "lexicographic_ready") return left.accountRef.localeCompare(right.accountRef)
    const leftSlots = availability.get(left.accountRef) ?? 1
    const rightSlots = availability.get(right.accountRef) ?? 1
    if (leftSlots !== rightSlots) return rightSlots - leftSlots
    const leftDefault = isDefaultAccountRef(left.accountRef)
    const rightDefault = isDefaultAccountRef(right.accountRef)
    if (leftDefault !== rightDefault) {
      return heuristic === "default_ready_highest_slots"
        ? leftDefault ? -1 : 1
        : leftDefault ? 1 : -1
    }
    return left.accountRef.localeCompare(right.accountRef)
  })
}

function resolveSpawnAccounts(
  requestedAccountRef: string | undefined,
  readyAccounts: readonly AccountRow[],
  allAccounts: readonly AccountRow[],
  availability: ReadonlyMap<string, number>,
  parameters: KhalaFleetDelegationParameterSet,
): readonly AccountRow[] {
  if (requestedAccountRef === undefined) return dispatchableSpawnAccounts(readyAccounts, availability)
  if (isDefaultAccountRef(requestedAccountRef)) {
    const candidates = dispatchableSpawnAccounts(readyAccounts, availability)
    const heuristic =
      resolveKhalaFleetDelegationParameters(parameters).accountRanking?.heuristic ??
      "named_ready_highest_slots"
    if (heuristic === "default_ready_highest_slots") {
      return preferredSpawnAccounts(candidates, availability, parameters)
    }
    const namedReadyAccounts = candidates.filter(account => !isDefaultAccountRef(account.accountRef))
    return namedReadyAccounts.length > 0 ? namedReadyAccounts : candidates
  }
  const selected = allAccounts.find(account => account.accountRef === requestedAccountRef)
  if (selected === undefined) {
    throw new Error(`Pylon Codex account ${requestedAccountRef} was not found.`)
  }
  if (!isReadyAccount(selected)) {
    throw new Error(`Pylon Codex account ${requestedAccountRef} is not ready (${selected.readiness}).`)
  }
  const slots = availability.get(selected.accountRef)
  if (slots !== undefined && slots <= 0) {
    throw new Error(`Pylon Codex account ${requestedAccountRef} has no advertised available slots right now.`)
  }
  return [selected]
}

function dispatchableSpawnAccounts(
  accounts: readonly AccountRow[],
  availability: ReadonlyMap<string, number>,
): readonly AccountRow[] {
  if (availability.size === 0) return accounts
  const positive = accounts.filter(account => (availability.get(account.accountRef) ?? 0) > 0)
  if (positive.length > 0) return positive
  return accounts.filter(account => availability.get(account.accountRef) === undefined)
}

function planSpawnAccounts(
  count: number,
  accounts: readonly AccountRow[],
  availability: ReadonlyMap<string, number>,
): readonly AccountRow[] {
  if (count <= 0 || accounts.length === 0) return []
  if (availability.size === 0) {
    return Array.from({ length: count }, (_, index) => accounts[index % accounts.length]!)
  }
  const planned: AccountRow[] = []
  for (const account of accounts) {
    const available = availability.get(account.accountRef)
    if (available === undefined) {
      planned.push(account)
      continue
    }
    const slots = Math.max(0, Math.floor(available))
    for (let index = 0; index < slots && planned.length < count; index += 1) {
      planned.push(account)
    }
    if (planned.length >= count) break
  }
  return planned
}

function normalizeRequestedAccountRef(accountRef: string | undefined): string | undefined {
  const trimmed = accountRef?.trim()
  if (trimmed === undefined || trimmed.length === 0) return undefined
  return isDefaultAccountRef(trimmed) ? "(default)" : trimmed
}

function isDefaultAccountRef(accountRef: string | undefined): boolean {
  if (accountRef === undefined) return false
  return /^(?:\(default\)|default)$/iu.test(accountRef.trim())
}

function commandAccountRef(accountRef: string | undefined): string | undefined {
  if (accountRef === undefined || isDefaultAccountRef(accountRef)) return undefined
  return /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/u.test(accountRef) ? accountRef : undefined
}

function capacityLabel(available: number | null, max: number | null): string {
  if (available === null && max === null) return "unknown"
  if (available !== null && max !== null) return `${available}/${max} available`
  if (available !== null) return `${available} available`
  return `${max} max`
}

function formatElapsedMs(value: number): string {
  const totalSeconds = Math.max(0, Math.floor(value / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) return `${hours}h${minutes.toString().padStart(2, "0")}m`
  if (minutes > 0) return `${minutes}m${seconds.toString().padStart(2, "0")}s`
  return `${seconds}s`
}

function issueRefFromMarker(marker: Record<string, unknown> | null, fileName: string): string | null {
  const direct =
    stringField(marker, "issueRef") ??
    stringField(marker, "prRef") ??
    stringField(marker, "pullRequestRef")
  if (direct !== null) return direct.match(/\d+/u)?.[0] ?? direct
  return fileName.match(/(\d{2,})/u)?.[1] ?? null
}

function safeFailureReason(result: KhalaCodexFleetCommandResult): string {
  const combined = `${result.stdout}\n${result.stderr}`
  const lifecycleEvents = lifecycleEventsFromText(combined)
  const nonLifecycle = stripLifecycleJsonLines(combined).trim()
  const lines = [
    result.timedOut
      ? "command timed out"
      : nonLifecycle.length === 0
        ? `command exited ${result.exitCode ?? "without status"}`
        : safeOutputBlock(nonLifecycle),
    ...renderLifecycleSummaryLines(lifecycleEvents),
  ].filter((line): line is string => line.length > 0)
  return truncateForModel(lines.join("\n"))
}

function safeOutputBlock(value: string): string {
  return redactKhalaPublicText(value)
    .replaceAll(homedir(), "~")
    .split(/\r?\n/u)
    .map(line => line.replace(/\s+/gu, " ").trimEnd())
    .filter(line => line.trim().length > 0)
    .join("\n")
    .trim()
}

function lifecycleEventsFromText(value: string): AssignmentLifecycleEvent[] {
  return value
    .split(/\r?\n/u)
    .map(line => lifecycleEventFromJsonLine(line))
    .filter((event): event is AssignmentLifecycleEvent => event !== null)
}

function stripLifecycleJsonLines(value: string): string {
  return value
    .split(/\r?\n/u)
    .filter(line => parsePylonLifecycleNdjsonLine(line) === null)
    .join("\n")
}

async function handlePylonLifecycleStderrLine(input: {
  readonly events: Record<string, unknown>[]
  readonly line: string
  readonly onProgress?: KhalaCodexFleetProgressSink | undefined
}): Promise<void> {
  const event = parsePylonLifecycleNdjsonLine(input.line)
  if (event === null) return
  input.events.push(event)
  while (input.events.length > MAX_STREAMED_LIFECYCLE_EVENTS) input.events.shift()
  const lifecycleEvents = input.events
    .map(lifecycleEventFromUnknown)
    .filter((item): item is AssignmentLifecycleEvent => item !== null)
  if (lifecycleEvents.length === 0) return
  await input.onProgress?.({
    schema: "openagents.khala_code.codex_spawn_progress.v0.1",
    events: lifecycleEvents,
    kind: "codex_spawn_lifecycle",
    lines: liveLifecycleSummaryLines(input.events),
    toolName: "codex_spawn",
  })
}

function liveLifecycleSummaryLines(events: readonly Record<string, unknown>[]): string[] {
  return events.some(event => stringField(event, "schema") === "openagents.pylon.khala_spawn_worker_event.v0.1")
    ? batchLifecycleSummaryLines(events)
    : renderLifecycleSummaryLines(lifecycleEventsFromUnknown(events))
}

function lifecycleEventsFromUnknown(value: unknown): AssignmentLifecycleEvent[] {
  return Array.isArray(value)
    ? value
        .map(lifecycleEventFromUnknown)
        .filter((event): event is AssignmentLifecycleEvent => event !== null)
    : []
}

function lifecycleEventFromJsonLine(line: string): AssignmentLifecycleEvent | null {
  const event = parsePylonLifecycleNdjsonLine(line)
  return event === null ? null : lifecycleEventFromUnknown(event)
}

export function parsePylonLifecycleNdjsonLine(line: string): Record<string, unknown> | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) return null
  try {
    const decoded: PylonLifecycleWireEvent = decodePylonLifecycleWireEvent(JSON.parse(trimmed) as unknown)
    return record(decoded)
  } catch {
    return null
  }
}

function lifecycleEventFromUnknown(value: unknown): AssignmentLifecycleEvent | null {
  let event = record(value)
  if (event === null) return null
  try {
    const decoded: PylonLifecycleWireEvent = decodePylonLifecycleWireEvent(event)
    event = record(decoded)
    if (event === null) return null
  } catch {
    return null
  }
  const schema = stringField(event, "schema")
  if (
    schema !== "openagents.pylon.assignment_run_lifecycle_event.v0.1" &&
    schema !== "openagents.pylon.khala_spawn_worker_event.v0.1"
  ) return null
  const eventName =
    stringField(event, "event") ??
    stringField(event, "assignmentEvent") ??
    stringField(event, "state") ??
    stringField(event, "message")
  if (eventName === null) return null
  const assignmentRef = stringField(event, "assignmentRef")
  const leaseRef = stringField(event, "leaseRef")
  const phase = stringField(event, "phase")
  const status = stringField(event, "status")
  return {
    ...(assignmentRef === null ? {} : { assignmentRef }),
    event: eventName,
    ...(leaseRef === null ? {} : { leaseRef }),
    ...(phase === null ? {} : { phase }),
    ...(status === null ? {} : { status }),
  }
}

function renderLifecycleSummaryLines(events: readonly AssignmentLifecycleEvent[]): string[] {
  const compact = compactLifecycleEvents(events)
  if (compact.length === 0) return []
  return [
    "lifecycle:",
    ...compact.slice(-8).map(event => `  - ${formatLifecycleEvent(event)}`),
  ]
}

function compactLifecycleEvents(events: readonly AssignmentLifecycleEvent[]): readonly AssignmentLifecycleEvent[] {
  const compact: AssignmentLifecycleEvent[] = []
  for (const event of events) {
    const previous = compact.at(-1)
    if (
      previous !== undefined &&
      previous.event === event.event &&
      previous.phase === event.phase &&
      previous.status === event.status
    ) {
      compact[compact.length - 1] = event
      continue
    }
    compact.push(event)
  }
  return compact
}

function formatLifecycleEvent(event: AssignmentLifecycleEvent): string {
  const details = [
    event.phase === undefined ? null : `phase=${event.phase}`,
    event.status === undefined ? null : `status=${event.status}`,
  ].filter((value): value is string => value !== null)
  return details.length === 0
    ? event.event
    : `${event.event} (${details.join(", ")})`
}

function truncateForModel(value: string): string {
  return tailByBytes(value, MAX_MODEL_OUTPUT_BYTES)
}

function tailByBytes(text: string, maxBytes: number): string {
  const bytes = Buffer.from(text, "utf8")
  if (bytes.byteLength <= maxBytes) return text
  return bytes.subarray(Math.max(0, bytes.byteLength - maxBytes)).toString("utf8")
}

function optionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined
}

function requiredString(value: unknown, message: string): string {
  const string = optionalString(value)
  if (string === undefined) throw new Error(message)
  return string
}

function optionalBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined
}

function optionalInteger(value: unknown): number | undefined {
  return Number.isInteger(value) ? Number(value) : undefined
}

function boundedPositiveInteger(value: number | undefined, fallback: number, min: number, max: number): number {
  const resolved = value ?? fallback
  if (!Number.isInteger(resolved) || resolved < min) return fallback
  return Math.min(resolved, max)
}

function positiveIntegerEnv(value: string | undefined): number | null {
  const parsed = Number.parseInt(value?.trim() ?? "", 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null
}

function stringField(source: Record<string, unknown> | null, field: string): string | null {
  const value = source?.[field]
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

function numberField(source: Record<string, unknown> | null, field: string): number | null {
  const value = source?.[field]
  return typeof value === "number" && Number.isFinite(value) ? value : null
}

function firstNumberField(source: Record<string, unknown> | null, fields: readonly string[]): number | null {
  for (const field of fields) {
    const value = numberField(source, field)
    if (value !== null) return value
  }
  return null
}

function booleanField(source: Record<string, unknown> | null, field: string): boolean | null {
  const value = source?.[field]
  return typeof value === "boolean" ? value : null
}

function recordField(source: Record<string, unknown> | null, field: string): Record<string, unknown> | null {
  return record(source?.[field])
}

function arrayField(source: Record<string, unknown> | null, field: string): readonly unknown[] {
  const value = source?.[field]
  return Array.isArray(value) ? value : []
}

function stringArrayField(source: Record<string, unknown> | null, field: string): readonly string[] {
  return arrayField(source, field)
    .filter((value): value is string => typeof value === "string" && value.trim().length > 0)
    .map(value => value.trim())
}

function record(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function nullableTime(value: string | null): number {
  if (value === null) return 0
  const millis = Date.parse(value)
  return Number.isFinite(millis) ? millis : 0
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function errorResult(error: unknown): KhalaCodexFleetCommandResult {
  return {
    exitCode: 127,
    signal: null,
    stderr: errorMessage(error),
    stdout: "",
    timedOut: false,
  }
}

function dedupe(values: readonly string[]): readonly string[] {
  return [...new Set(values.map(value => resolve(value)))]
}

function delay(ms: number): Promise<void> {
  return new Promise(resolveDelay => setTimeout(resolveDelay, ms))
}

export type RemoveCodexAccountResult = {
  readonly ok: boolean
  readonly removed: boolean
  readonly accountRef: string
  readonly error?: string
}

// Remove a Codex account from the local Pylon registry (config.dev.accounts) and
// delete its isolated per-account home. Safety: only ever deletes homes under
// <pylon home>/accounts/codex/<ref>, never the default ~/.codex.
export async function removeCodexAccount(
  accountRef: string,
  options: KhalaCodexFleetToolOptions = {},
): Promise<RemoveCodexAccountResult> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const configPath = join(pylonHome, "config.json")

  if (
    accountRef.length === 0 ||
    accountRef.includes("/") ||
    accountRef.includes("..") ||
    accountRef === "(default)"
  ) {
    return {
      ok: false,
      removed: false,
      accountRef,
      error: "invalid or non-removable account ref",
    }
  }

  // Accounts are surfaced from THREE sources: the Pylon registry
  // (config.dev.accounts), isolated homes under <pylon home>/accounts/codex/<ref>,
  // and sibling dotfile homes in $HOME (~/.codex-<x> / ~/.claude-<x>, discovered by
  // discoverPylonSiblingAccountHomes). Remove the account from all of them.
  const siblingRoot = (env.PYLON_ACCOUNT_HOME_ROOT ?? "").trim() || homedir()
  let removedSomething = false

  try {
    // 1) drop the registry entry if present
    try {
      const raw = await readFile(configPath, "utf8")
      const config = JSON.parse(raw) as {
        dev?: { accounts?: ReadonlyArray<Record<string, unknown>> }
      }
      const accounts = Array.isArray(config.dev?.accounts) ? config.dev.accounts : []
      const remaining = accounts.filter(
        account => account?.ref !== accountRef,
      )
      if (remaining.length !== accounts.length) {
        const nextConfig = {
          ...config,
          dev: { ...(config.dev ?? {}), accounts: remaining },
        }
        const tempPath = `${configPath}.tmp`
        await writeFile(tempPath, `${JSON.stringify(nextConfig, null, 2)}\n`)
        await rename(tempPath, configPath)
        removedSomething = true
      }
    } catch {
      // no/unreadable config — other removals below still apply
    }

    // 2) isolated per-account home under the pylon home (safe path)
    const isolatedHome = join(pylonHome, "accounts", "codex", accountRef)
    if (existsSync(isolatedHome)) {
      await rm(isolatedHome, { recursive: true, force: true })
      removedSomething = true
    }

    // 3) sibling dotfile home in $HOME — but NEVER the bare defaults ~/.codex / ~/.claude
    if (accountRef !== "codex" && accountRef !== "claude") {
      const siblingHome = join(siblingRoot, `.${accountRef}`)
      if (existsSync(siblingHome)) {
        await rm(siblingHome, { recursive: true, force: true })
        removedSomething = true
      }
    }

    return { ok: true, removed: removedSomething, accountRef }
  } catch (error) {
    return {
      ok: false,
      removed: false,
      accountRef,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

const decodeCodexIdTokenEmail = (idToken: string): string | null => {
  try {
    const payload = idToken.split(".")[1]
    if (payload === undefined) return null
    const claims = JSON.parse(
      Buffer.from(payload, "base64url").toString("utf8"),
    ) as Record<string, unknown>
    const profile = claims["https://api.openai.com/profile"]
    const auth = claims["https://api.openai.com/auth"]
    const candidates: unknown[] = [
      claims.email,
      typeof profile === "object" && profile !== null
        ? (profile as Record<string, unknown>).email
        : undefined,
      typeof auth === "object" && auth !== null
        ? (auth as Record<string, unknown>).email
        : undefined,
    ]
    const email = candidates.find(
      value => typeof value === "string" && value.includes("@"),
    )
    return typeof email === "string" ? email : null
  } catch {
    return null
  }
}

const codexEmailFromHome = async (home: string): Promise<string | null> => {
  try {
    const raw = JSON.parse(
      await readFile(join(home, "auth.json"), "utf8"),
    ) as Record<string, unknown>
    const tokens =
      typeof raw.tokens === "object" && raw.tokens !== null
        ? (raw.tokens as Record<string, unknown>)
        : {}
    const idToken = tokens.id_token ?? raw.id_token
    return typeof idToken === "string" ? decodeCodexIdTokenEmail(idToken) : null
  } catch {
    return null
  }
}

// Resolve the on-disk home for a fleet account ref and decode the signed-in
// email from its auth.json. Mirrors how the fleet list discovers accounts:
// (default) -> ~/.codex, registry refs -> config home, otherwise the sibling
// dotfile (~/.<ref>) or the isolated home (<pylon home>/accounts/codex/<ref>).
export async function collectCodexAccountEmails(
  accountRefs: ReadonlyArray<string>,
  options: KhalaCodexFleetToolOptions = {},
): Promise<Record<string, string | null>> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const siblingRoot = (env.PYLON_ACCOUNT_HOME_ROOT ?? "").trim() || homedir()
  const defaultHome = (env.CODEX_HOME ?? "").trim() || join(homedir(), ".codex")

  const configHomes: Record<string, string> = {}
  try {
    const config = JSON.parse(
      await readFile(join(pylonHome, "config.json"), "utf8"),
    ) as { dev?: { accounts?: ReadonlyArray<Record<string, unknown>> } }
    for (const account of config.dev?.accounts ?? []) {
      if (
        account?.provider === "codex" &&
        typeof account.ref === "string" &&
        typeof account.home === "string"
      ) {
        configHomes[account.ref] = account.home
      }
    }
  } catch {
    // no config — fall back to default/sibling/isolated resolution
  }

  const homeForRef = (accountRef: string): string | null => {
    if (accountRef === "(default)") return defaultHome
    if (configHomes[accountRef] !== undefined) return configHomes[accountRef]
    const sibling = join(siblingRoot, `.${accountRef}`)
    if (existsSync(sibling)) return sibling
    const isolated = join(pylonHome, "accounts", "codex", accountRef)
    if (existsSync(isolated)) return isolated
    return null
  }

  const out: Record<string, string | null> = {}
  for (const accountRef of accountRefs) {
    const home = homeForRef(accountRef)
    out[accountRef] = home === null ? null : await codexEmailFromHome(home)
  }
  return out
}

export type CodexConnectStart = {
  readonly ok: boolean
  readonly accountRef: string
  readonly verificationUrl: string | null
  readonly userCode: string | null
  readonly output: string
  readonly error?: string
}

// Begin (or re-begin) a Codex device-auth login for an account ref through the
// UI. Spawns `pylon accounts connect codex --account <ref> --force-device-login`,
// which registers the account and runs `codex login --device-auth` (auto-opening
// the browser). We read its streamed output until the verification URL + user
// code appear, return them, and leave the process running — it finishes when the
// user authorizes in the browser, after which the account reads as ready on the
// next fleet refresh.
export async function beginCodexConnect(
  accountRef: string,
  options: KhalaCodexFleetToolOptions = {},
): Promise<CodexConnectStart> {
  if (!/^[A-Za-z0-9._-]+$/.test(accountRef) || accountRef === "(default)") {
    return {
      ok: false,
      accountRef,
      verificationUrl: null,
      userCode: null,
      output: "",
      error: "invalid account ref",
    }
  }
  const env = options.env ?? process.env
  const paths = resolvePylonPaths(env)
  let child: ReturnType<typeof Bun.spawn>
  try {
    child = Bun.spawn(
      [
        paths.bunExecutable,
        "src/index.ts",
        "accounts",
        "connect",
        "codex",
        "--account",
        accountRef,
        "--force-device-login",
        "--json",
      ],
      {
        cwd: paths.appPath,
        env: pylonCommandEnv(env, paths.pylonHome),
        stdout: "pipe",
        stderr: "pipe",
        stdin: "ignore",
      },
    )
  } catch (error) {
    return {
      ok: false,
      accountRef,
      verificationUrl: null,
      userCode: null,
      output: "",
      error: error instanceof Error ? error.message : String(error),
    }
  }

  const decoder = new TextDecoder()
  const stripAnsi = (value: string): string =>
    // eslint-disable-next-line no-control-regex
    value.replace(/\x1b\[[0-9;]*m/g, "")
  let buffer = ""
  let verificationUrl: string | null = null
  let userCode: string | null = null
  let done = false

  const tryParse = (): void => {
    const clean = stripAnsi(buffer)
    if (verificationUrl === null) {
      verificationUrl = clean.match(/https?:\/\/[^\s'"]+/)?.[0] ?? null
    }
    if (userCode === null) {
      userCode = clean.match(/\b[A-Z0-9]{3,}-[A-Z0-9]{3,}\b/)?.[0] ?? null
    }
    if (verificationUrl !== null && userCode !== null) done = true
  }

  const readStream = async (
    stream: ReadableStream<Uint8Array> | undefined,
  ): Promise<void> => {
    if (stream === undefined) return
    const reader = stream.getReader()
    try {
      // Drain to end-of-stream, NOT until the code is captured. If we stop
      // reading while the child keeps writing (codex login + pylon connect
      // continue after the device prompt), the pipe fills and the child blocks
      // on write — so it never finishes registering the account and the panel
      // poll never sees it ready. The `done` flag only gates when we return.
      while (true) {
        const { value, done: streamDone } = await reader.read()
        if (streamDone) break
        if (value !== undefined) {
          buffer += decoder.decode(value, { stream: true })
          if (buffer.length < 65_536) tryParse()
        }
      }
    } catch {
      // stream interrupted — return whatever we captured
    } finally {
      reader.releaseLock()
    }
  }

  // Fire the readers in the background (they keep draining until the child
  // exits, so it never blocks on backpressure) and return as soon as the URL +
  // code are captured (~2-3s). The child keeps running until the user authorizes.
  void readStream(child.stdout as ReadableStream<Uint8Array>)
  void readStream(child.stderr as ReadableStream<Uint8Array>)
  const deadline = Date.now() + 25_000
  while (!done && Date.now() < deadline) {
    await new Promise<void>(resolve => setTimeout(resolve, 150))
  }

  // Leave the process running; it completes when the user authorizes.
  return {
    ok: true,
    accountRef,
    verificationUrl,
    userCode,
    output: stripAnsi(buffer).slice(-1500),
  }
}

// Open a URL in the user's default browser. The Electrobun webview (WKWebView)
// does not open external links itself, so the host process does it.
export function openExternalUrl(url: string): boolean {
  if (!/^https?:\/\//.test(url)) return false
  const platform = process.platform
  const command =
    platform === "darwin"
      ? ["open", url]
      : platform === "win32"
        ? ["cmd", "/c", "start", "", url]
        : ["xdg-open", url]
  try {
    Bun.spawn(command, { stdout: "ignore", stderr: "ignore", stdin: "ignore" })
    return true
  } catch {
    return false
  }
}
