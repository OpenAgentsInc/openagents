import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { readFile, readdir, stat } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"
import type { Readable } from "node:stream"
import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  khalaToolUnavailable,
  redactKhalaPublicText,
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
  readonly env?: ChatEnv | undefined
  readonly runner?: KhalaCodexFleetCommandRunner | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
}

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
  readonly accountRef: string
  readonly provider: "codex"
  readonly quotaState: string | null
  readonly readiness: string
}

type FleetStatusResult = {
  readonly accounts: readonly AccountRow[]
  readonly activeAssignments: readonly ActiveAssignmentMarker[]
  readonly availableCodexAssignments: number | null
  readonly ensure: KhalaPylonEnsureResult
  readonly maxCodexAssignments: number | null
  readonly observedAt: string
  readonly processes: readonly ProcessRow[]
}

type ActiveAssignmentMarker = {
  readonly assignmentRef: string | null
  readonly issueRef: string | null
  readonly updatedAt: string | null
}

type ProcessRow = {
  readonly elapsed: string
  readonly kind: "codex" | "pylon"
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
}

const MAX_SPAWN_COUNT = 4
const DEFAULT_COMMAND_TIMEOUT_MS = 45_000
const DEFAULT_SPAWN_TIMEOUT_MS = 180_000
const DEFAULT_ENSURE_WAIT_MS = 7_500
const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com"
const MAX_MODEL_OUTPUT_BYTES = 4_000

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
    "This MVP exposes only pylon_ensure, codex_fleet_status, and codex_spawn for Codex fleet control. Do not invent codex_terminate or other Codex fleet tools.",
    "After codex_spawn, summarize the returned assignment, auto-run, and closeout status; do not read guessed local output files.",
    "Keep count small; this MVP caps fan-out at four assignments.",
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
      execute: input => executeCodexSpawnTool(input, options),
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
  const [listResult, statusResult, processes, activeAssignments] = await Promise.all([
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
    input.includeProcesses === false
      ? Promise.resolve([] as ProcessRow[])
      : collectProcessSnapshot(options.runner ?? defaultCommandRunner, env),
    collectActiveAssignmentMarkers(paths.pylonHome),
  ])

  const accounts = mergeAccountRows(
    parseJsonObject(listResult.stdout),
    parseJsonObject(statusResult.stdout),
  )
  const provider = providerCapacity(parseJsonObject(listResult.stdout), parseJsonObject(statusResult.stdout), ensure)
  return {
    accounts,
    activeAssignments,
    availableCodexAssignments: provider.available,
    ensure,
    maxCodexAssignments: provider.max,
    observedAt: new Date().toISOString(),
    processes,
  }
}

export async function spawnCodexInstances(
  raw: SpawnCodexInstancesInput,
  options: KhalaCodexFleetToolOptions = {},
): Promise<SpawnCodexInstancesResult> {
  const input = decodeSpawnInput(raw)
  const env = options.env ?? process.env
  const ensure = await ensureLocalPylon(
    { start: true, timeoutMs: input.timeoutMs, waitMs: DEFAULT_ENSURE_WAIT_MS },
    { ...options, env },
  )
  if (!ensure.ok) {
    throw new Error(`Pylon unavailable: ${ensure.message}${ensure.unavailableReason ? ` (${ensure.unavailableReason})` : ""}`)
  }

  const status = await inspectCodexFleet({ includeProcesses: false, startPylon: false }, { ...options, env })
  const readyAccounts = preferredSpawnAccounts(status.accounts.filter(account => isReadyAccount(account)))
  if (input.accountRef === undefined && readyAccounts.length === 0) {
    throw new Error(
      "No ready Pylon Codex account is connected. Connect one first with `khala fleet connect` or `pylon auth codex --account codex`; this Desktop tool will not run Codex device login.",
    )
  }
  if (input.accountRef !== undefined) {
    const selected = status.accounts.find(account => account.accountRef === input.accountRef)
    if (selected !== undefined && !isReadyAccount(selected)) {
      throw new Error(`Pylon Codex account ${input.accountRef} is not ready (${selected.readiness}).`)
    }
  }
  if (status.availableCodexAssignments !== null && status.availableCodexAssignments <= 0) {
    throw new Error(
      `No Pylon Codex assignment capacity is available right now (${capacityLabel(status.availableCodexAssignments, status.maxCodexAssignments)}). Wait for the running assignment to finish or retry after Pylon refreshes.`,
    )
  }

  const paths = resolvePylonPaths(env)
  const results: SpawnSlotResult[] = []
  const baseUrl = resolveOpenAgentsBaseUrl(env, input.baseUrl)
  const heartbeat = await runPylonCommand(["presence", "heartbeat", "--base-url", baseUrl, "--json"], {
    env,
    paths,
    runner: options.runner,
    timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
  })
  if (heartbeat.exitCode !== 0) {
    throw new Error(`Pylon heartbeat failed before Codex spawn: ${safeFailureReason(heartbeat)}`)
  }
  const heartbeatPylonRef = stringField(parseJsonObject(heartbeat.stdout), "pylonRef")
  const targetPylonRef = input.pylonRef ?? heartbeatPylonRef ?? ensure.pylonRef ?? ""
  for (let index = 0; index < input.count; index += 1) {
    const selectedAccount = input.accountRef ?? readyAccounts[index % readyAccounts.length]?.accountRef
    const selectedCommandAccount = commandAccountRef(selectedAccount)
    const args = [
      "khala",
      "request",
      "--workflow",
      "codex_agent_task",
      "--prompt",
      input.prompt,
      "--pylon-ref",
      targetPylonRef,
      ...(selectedCommandAccount === undefined ? [] : ["--account-ref", selectedCommandAccount]),
      ...(input.fixture ? ["--fixture"] : workspacePinArgs(input)),
      "--base-url",
      baseUrl,
      ...(input.noRun ? ["--no-run"] : []),
      "--json",
    ]
    const result = await runPylonCommand(args, {
      env,
      paths,
      runner: options.runner,
      timeoutMs: input.timeoutMs,
    })
    const json = parseJsonObject(result.stdout)
    const assignmentRef = stringField(json, "assignmentRef")
    const autoRunOk = booleanField(recordField(json, "autoRun"), "ok")
    const accepted = result.exitCode === 0 && assignmentRef !== null
    results.push({
      accountRef: selectedAccount ?? null,
      assignmentRef,
      autoRunOk,
      exitCode: result.exitCode,
      slot: index + 1,
      status: accepted ? "accepted" : "failed",
      summary: accepted
        ? acceptedSpawnSummary(assignmentRef, json)
        : safeFailureReason(result),
    })
  }

  return {
    acceptedCount: results.filter(result => result.status === "accepted").length,
    pylonRef: targetPylonRef || null,
    requestedCount: input.count,
    results,
  }
}

export function resolvePylonHome(env: ChatEnv = process.env): string {
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
      const toolResult = khalaToolOk({
        modelText: renderSpawnResult(result),
        publicSafety: "private",
        publicSummary: `Codex spawn accepted ${result.acceptedCount}/${result.requestedCount} request(s).`,
        ui: {
          acceptedCount: result.acceptedCount,
          kind: "codex_spawn",
          pylonRef: result.pylonRef,
          requestedCount: result.requestedCount,
          results: result.results,
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
    readonly paths: PylonPaths
    readonly runner?: KhalaCodexFleetCommandRunner | undefined
    readonly timeoutMs: number
  },
): Promise<KhalaCodexFleetCommandResult> {
  return (input.runner ?? defaultCommandRunner)({
    cmd: [input.paths.bunExecutable, "src/index.ts", ...args],
    cwd: input.paths.appPath,
    env: pylonCommandEnv(input.env, input.paths.pylonHome),
    maxOutputBytes: 80_000,
    timeoutMs: input.timeoutMs,
  })
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
    collectStream(child.stderr, maxOutputBytes),
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

function collectStream(stream: Readable | null, maxBytes: number): Promise<string> {
  if (stream === null) return Promise.resolve("")
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
  const readinessObject = recordField(account, "readiness")
  const readiness =
    stringField(readinessObject, "state") ??
    stringField(account, "readiness") ??
    (stringField(account, "homeState") === "present" ? "ready" : "unknown")
  const quota = recordField(account, "quota")
  return {
    accountRef,
    provider: "codex",
    quotaState: stringField(quota, "state"),
    readiness,
  }
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

async function collectActiveAssignmentMarkers(pylonHome: string): Promise<readonly ActiveAssignmentMarker[]> {
  const root = join(pylonHome, "active-assignment-runs")
  try {
    const entries = await readdir(root, { withFileTypes: true })
    const markers = await Promise.all(entries
      .filter(entry => entry.isFile() && entry.name.endsWith(".json"))
      .slice(0, 25)
      .map(async entry => {
        const path = join(root, entry.name)
        const parsed = parseJsonObject(await readFile(path, "utf8").catch(() => ""))
        const fileStat = await stat(path).catch(() => null)
        return {
          assignmentRef: stringField(parsed, "assignmentRef"),
          issueRef: issueRefFromMarker(parsed, entry.name),
          updatedAt:
            stringField(parsed, "updatedAt") ??
            stringField(parsed, "observedAt") ??
            (fileStat === null ? null : new Date(fileStat.mtimeMs).toISOString()),
        }
      }))
    return markers.sort((left, right) =>
      nullableTime(right.updatedAt) - nullableTime(left.updatedAt),
    )
  } catch {
    return []
  }
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
    .map(line => line.trim())
    .filter(line => /(pylon|codex)/iu.test(line))
    .filter(line => !/(rg -n|khala-codex-fleet-tools|ps -axo)/iu.test(line))
    .slice(0, 25)
    .map(line => {
      const parts = line.split(/\s+/u)
      const pid = parts[0] ?? ""
      const elapsed = parts[2] ?? ""
      const command = parts.slice(3).join(" ")
      return {
        elapsed,
        kind: /pylon/iu.test(command) ? "pylon" as const : "codex" as const,
        pid,
      }
    })
}

function decodeSpawnInput(raw: SpawnCodexInstancesInput): NormalizedSpawnInput {
  const prompt = raw.prompt.trim()
  if (prompt.length === 0) throw new Error("codex_spawn requires a non-empty prompt")
  const count = boundedPositiveInteger(raw.count, 1, 1, MAX_SPAWN_COUNT)
  const timeoutMs = boundedPositiveInteger(raw.timeoutMs, DEFAULT_SPAWN_TIMEOUT_MS, 10_000, 600_000)
  const hasAnyPin = raw.repo !== undefined || raw.commit !== undefined || raw.verify !== undefined
  const fixture = raw.fixture ?? !hasAnyPin
  const missingPins = [
    raw.repo === undefined ? "repo" : null,
    raw.commit === undefined ? "commit" : null,
    raw.verify === undefined ? "verify" : null,
  ].filter((value): value is string => value !== null)
  if (fixture && hasAnyPin) {
    throw new Error("codex_spawn fixture cannot be combined with repo, commit, or verify pins")
  }
  if (!fixture && missingPins.length > 0) {
    throw new Error(`codex_spawn requires fixture: true or complete real-work pins; missing ${missingPins.join(", ")}`)
  }
  return {
    ...raw,
    count,
    fixture,
    noRun: raw.noRun ?? false,
    prompt,
    timeoutMs,
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
  ]
  if (result.accounts.length > 0) {
    lines.push(...result.accounts.map(account =>
      `- ${account.accountRef}: ${account.readiness}${account.quotaState ? `, quota ${account.quotaState}` : ""}`,
    ))
  } else {
    lines.push("- no Pylon Codex accounts found")
  }
  lines.push(`Active assignment markers: ${result.activeAssignments.length}`)
  if (result.processes.length > 0) {
    lines.push(`Local Pylon/Codex processes: ${result.processes.length}`)
    lines.push(...result.processes.slice(0, 5).map(process => `- ${process.pid} ${process.elapsed} ${process.kind}`))
  }
  return lines.join("\n")
}

function renderSpawnResult(result: SpawnCodexInstancesResult): string {
  return [
    `Codex spawn: accepted ${result.acceptedCount}/${result.requestedCount}${result.pylonRef ? ` via ${result.pylonRef}` : ""}`,
    ...result.results.map(slot => renderSpawnSlotResult(slot)),
  ].join("\n")
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

function acceptedSpawnSummary(
  assignmentRef: string,
  payload: Record<string, unknown> | null,
): string {
  const autoRun = recordField(payload, "autoRun")
  const assignmentRun = recordField(payload, "assignmentRun")
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

  lines.push("next: summarize this status; no local output path was returned")
  return lines.join("\n")
}

function readyAccountCount(accounts: readonly AccountRow[]): number {
  return accounts.filter(isReadyAccount).length
}

function isReadyAccount(account: AccountRow): boolean {
  return account.readiness === "ready" || account.readiness === "available"
}

function preferredSpawnAccounts(accounts: readonly AccountRow[]): readonly AccountRow[] {
  return [...accounts].sort((left, right) => {
    const leftDefault = left.accountRef === "(default)"
    const rightDefault = right.accountRef === "(default)"
    if (leftDefault !== rightDefault) return leftDefault ? 1 : -1
    return left.accountRef.localeCompare(right.accountRef)
  })
}

function commandAccountRef(accountRef: string | undefined): string | undefined {
  if (accountRef === undefined || accountRef === "(default)") return undefined
  return /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/u.test(accountRef) ? accountRef : undefined
}

function capacityLabel(available: number | null, max: number | null): string {
  if (available === null && max === null) return "unknown"
  if (available !== null && max !== null) return `${available}/${max} available`
  if (available !== null) return `${available} available`
  return `${max} max`
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
  if (result.timedOut) return "command timed out"
  const combined = `${result.stdout}\n${result.stderr}`.trim()
  if (combined.length === 0) return `command exited ${result.exitCode ?? "without status"}`
  return truncateForModel(safeOutputLine(combined))
}

function safeOutputLine(value: string): string {
  return redactKhalaPublicText(value)
    .replaceAll(homedir(), "~")
    .replace(/\s+/gu, " ")
    .trim()
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

function stringField(source: Record<string, unknown> | null, field: string): string | null {
  const value = source?.[field]
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

function numberField(source: Record<string, unknown> | null, field: string): number | null {
  const value = source?.[field]
  return typeof value === "number" && Number.isFinite(value) ? value : null
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
