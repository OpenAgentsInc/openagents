import { access, mkdir, writeFile } from "node:fs/promises"
import { isAbsolute, join, resolve } from "node:path"
import { createHash } from "node:crypto"
import {
  CODEX_AGENT_SDK_PACKAGE,
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  type CodexAgentProbeOptions,
  type CodexAgentSandboxMode,
} from "./codex-agent.js"
import {
  gitCheckoutWorkspaceFrom,
  materializeGitCheckoutWorkspaceWithLease,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
} from "./workspace-materializer.js"
import {
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import {
  createPylonCodexTurnReporter,
  type CodexTurnReportItem,
  type CodexTurnReporter,
  type CodexTurnUsage,
} from "./codex-turn-reporter.js"
import type { PylonLocalState } from "./state.js"

/**
 * The local Codex executor gate (issue #4789, epic #4793, promise
 * autopilot.codex_probe_pylon_successor.v1).
 *
 * Recognizes the codex_sdk coding work class on a Pylon assignment,
 * materializes a bounded fixture workspace, drives one Codex SDK thread
 * inside it, verifies the result with the fixture's real test command, and
 * digests everything into public-safe closeout refs. The complete raw SDK
 * event stream is posted only to the owner-scoped private Codex turn ingest
 * store; public closeouts and ATIF traces remain redacted/ref-only.
 *
 * Boundary law (the design delta from the Claude Agent gate): the Codex
 * SDK has no PreToolUse hook. For the caller-owned Khala->Pylon->Codex lane,
 * the executor intentionally uses the SDK equivalent of
 * `--dangerously-bypass-approvals-and-sandbox` so Codex can perform real local
 * GitHub/worktree operations. Assignment payloads cannot request that mode:
 * it is an owner-local executor invariant, paired with post-hoc validation that
 * every reported file change stayed inside the materialized workspace.
 */

export const CODEX_AGENT_TASK_SCHEMA = "openagents.pylon.codex_agent_task.v0.3"
export const CODEX_AGENT_TASK_AGENT_KIND = "codex_sdk"
export const CODEX_AGENT_SUM_REPAIR_FIXTURE_REF = "fixture.public.pylon.codex_agent.sum_repair.v1"

// The git_checkout workspace contract is shared with the Claude Agent
// lane (B2 #4756) through the adapter-neutral materializer module
// (#4798) — same validator, same checkout runner, never forked.
export type CodexAgentGitCheckoutWorkspace = GitCheckoutWorkspace
export type CodexAgentCheckoutRunner = WorkspaceCheckoutRunner
export type CodexAgentRuntimeSandboxMode = CodexAgentSandboxMode | "danger-full-access"

export const CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE = "danger-full-access" as const
export const CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY = "never" as const

export type CodexAgentTaskPayload = {
  schema: typeof CODEX_AGENT_TASK_SCHEMA
  agentKind: typeof CODEX_AGENT_TASK_AGENT_KIND
  fixtureRef?: string
  objectiveSummary?: string
  sandboxMode?: CodexAgentSandboxMode
  timeoutSeconds?: number
  workspace?: CodexAgentGitCheckoutWorkspace
}

export type CodexAgentRunInput = {
  assignmentRef?: string
  cwd: string
  instructions: string
  leaseRef?: string
  pylonRef?: string
  runRef?: string
  workspaceRef?: string
  account?: ResolvedPylonAccountSelection | null
  env?: Record<string, string | undefined>
  eventReporter?: CodexTurnReporter
  networkAccessEnabled: boolean
  sandboxMode: CodexAgentRuntimeSandboxMode
  timeoutMs: number
  model?: string
}

export type CodexAgentRunOutcome =
  | "completed"
  | "budget_exceeded"
  | "workspace_escape_blocked"
  | "refused"

export type CodexAgentRunResult = {
  outcome: CodexAgentRunOutcome
  turnCount: number
  editedFileCount: number
  commandCount: number
  sessionRef: string | null
}

export type CodexAgentRunner = (input: CodexAgentRunInput) => Promise<CodexAgentRunResult>

type LocalCommandResult = {
  exitCode: number
  stderrBytes: number
  stdoutBytes: number
  timedOut: boolean
}

type LocalCommandRunner = (input: { args: string[]; cwd: string; timeoutMs?: number }) => Promise<LocalCommandResult>

export type CodexAgentExecutionOptions = {
  account?: ResolvedPylonAccountSelection | null
  agentToken?: string
  baseUrl?: string
  checkoutRunner?: CodexAgentCheckoutRunner
  codexAgentRunner?: CodexAgentRunner
  codexAgentProbe?: CodexAgentProbeOptions
  codexTurnReporter?: CodexTurnReporter
  dependencyInstaller?: LocalCommandRunner
  fetch?: typeof fetch
}

type CodexAgentFixture = {
  files: Record<string, string>
  instructions: string
  verificationArgs: string[]
}

const DEFAULT_TIMEOUT_SECONDS = 300
const MAX_TIMEOUT_SECONDS = 1200

const CODEX_AGENT_FIXTURES: Record<string, CodexAgentFixture> = {
  [CODEX_AGENT_SUM_REPAIR_FIXTURE_REF]: {
    files: {
      "package.json": `${JSON.stringify(
        {
          private: true,
          scripts: { test: "bun test sum.test.ts" },
          type: "module",
        },
        null,
        2,
      )}\n`,
      "sum.ts": "export const sum = (left: number, right: number) => left - right\n",
      "sum.test.ts": [
        'import { describe, expect, test } from "bun:test"',
        'import { sum } from "./sum"',
        "",
        'describe("sum fixture", () => {',
        '  test("adds two numbers", () => {',
        "    expect(sum(2, 3)).toBe(5)",
        "  })",
        "})",
        "",
      ].join("\n"),
    },
    instructions: [
      "You are working in a bounded fixture workspace.",
      "The test in sum.test.ts fails because sum.ts has a bug.",
      "Fix the implementation in sum.ts so the test passes, then run",
      "`bun test sum.test.ts` to confirm. Only modify files inside this",
      "working directory.",
    ].join(" "),
    verificationArgs: ["bun", "test", "sum.test.ts"],
  },
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

export function codexAgentTaskFrom(codingAssignment: unknown): CodexAgentTaskPayload | null {
  const codex = (codingAssignment as { codex?: unknown } | null)?.codex
  if (codex === null || typeof codex !== "object") return null
  const payload = codex as CodexAgentTaskPayload
  if (payload.schema !== CODEX_AGENT_TASK_SCHEMA) return null
  if (payload.agentKind !== CODEX_AGENT_TASK_AGENT_KIND) return null
  const workspace = gitCheckoutWorkspaceFrom(codingAssignment)
  const objective = (codingAssignment as { objective?: { publicSummary?: unknown } } | null)?.objective
  const hasFixture =
    typeof payload.fixtureRef === "string" &&
    CODEX_AGENT_FIXTURES[payload.fixtureRef] !== undefined
  if (!hasFixture && workspace === null) return null
  return {
    ...payload,
    ...(typeof objective?.publicSummary === "string" ? { objectiveSummary: objective.publicSummary } : {}),
    ...(workspace === null ? {} : { workspace }),
  }
}

function boundedNumber(value: number | undefined, fallback: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return fallback
  return Math.min(Math.floor(value), max)
}

/**
 * Resolves the effective sandbox mode for caller-owned Khala coding
 * assignments. The public assignment wire format and assignment-safe config
 * still accept only bounded modes, but this local executor always maps to the
 * Codex SDK equivalent of `--dangerously-bypass-approvals-and-sandbox`.
 */
export function effectiveSandboxMode(
  _taskMode: CodexAgentSandboxMode | undefined,
  _configMode: CodexAgentSandboxMode | undefined,
): CodexAgentRuntimeSandboxMode {
  return CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE
}

/**
 * Post-hoc boundary check: returns true when a file change reported by the
 * Codex thread resolves outside the bounded workspace. The SDK sandbox is
 * the enforcement layer; this is the independent verification of it.
 */
export function fileChangeEscapesWorkspace(path: string, workspace: string): boolean {
  if (typeof path !== "string" || path.length === 0) return false
  const workspaceRoot = resolve(workspace)
  const resolved = isAbsolute(path) ? resolve(path) : resolve(workspaceRoot, path)
  return resolved !== workspaceRoot && !resolved.startsWith(`${workspaceRoot}/`)
}

async function runCommand(input: { args: string[]; cwd: string; timeoutMs?: number }): Promise<LocalCommandResult> {
  const proc = Bun.spawn(input.args, { cwd: input.cwd, stderr: "pipe", stdout: "pipe" })
  let timedOut = false
  const timer =
    input.timeoutMs === undefined
      ? undefined
      : setTimeout(() => {
          timedOut = true
          proc.kill()
        }, input.timeoutMs)
  try {
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).arrayBuffer(),
      new Response(proc.stderr).arrayBuffer(),
      proc.exited,
    ])
    return { exitCode, stderrBytes: stderr.byteLength, stdoutBytes: stdout.byteLength, timedOut }
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

type DependencyPreparation =
  | { ok: true; prepared: boolean; receiptRef?: string }
  | { ok: false; receiptRef: string }

async function prepareWorkspaceDependencies(input: {
  installer?: LocalCommandRunner
  workspace: string
}): Promise<DependencyPreparation> {
  const hasPackageJson = await pathExists(join(input.workspace, "package.json"))
  const hasBunLock =
    (await pathExists(join(input.workspace, "bun.lock"))) ||
    (await pathExists(join(input.workspace, "bun.lockb")))
  if (!hasPackageJson || !hasBunLock) {
    return { ok: true, prepared: false }
  }

  const nodeModulesReady = await pathExists(join(input.workspace, "node_modules"))
  if (nodeModulesReady) {
    return {
      ok: true,
      prepared: false,
      receiptRef: "dependency.pylon.codex_agent_task.node_modules_present",
    }
  }

  const installer = input.installer ?? runCommand
  const install = await installer({
    args: ["bun", "install", "--frozen-lockfile"],
    cwd: input.workspace,
    timeoutMs: 5 * 60 * 1000,
  })
  const receiptRef = stableRef(
    "dependency.pylon.codex_agent_task.bun_install",
    `${install.exitCode}:${install.timedOut}:${install.stdoutBytes}:${install.stderrBytes}`,
  )
  if (install.exitCode !== 0 || install.timedOut) {
    return { ok: false, receiptRef }
  }
  return { ok: true, prepared: true, receiptRef }
}

type CodexThreadEvent = {
  type?: string
  thread_id?: string
  usage?: {
    input_tokens?: number
    cached_input_tokens?: number
    output_tokens?: number
    reasoning_output_tokens?: number
  }
  error?: { message?: string }
  item?: {
    type?: string
    status?: string
    text?: string
    command?: string
    aggregated_output?: string
    exit_code?: number
    name?: string
    tool_name?: string
    server_name?: string
    changes?: Array<{ path?: string; kind?: string }>
  }
}

type RawCodexThreadEvent = Record<string, unknown>

function finiteToken(value: number | undefined): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0
}

function usageFromTurnCompleted(event: CodexThreadEvent): CodexTurnUsage | undefined {
  if (event.usage === undefined) return undefined
  return {
    inputTokens: finiteToken(event.usage.input_tokens),
    cachedInputTokens: finiteToken(event.usage.cached_input_tokens),
    outputTokens: finiteToken(event.usage.output_tokens),
    reasoningOutputTokens: finiteToken(event.usage.reasoning_output_tokens),
  }
}

function outputBytes(item: CodexThreadEvent["item"]): number | undefined {
  if (typeof item?.aggregated_output !== "string") return undefined
  return new TextEncoder().encode(item.aggregated_output).byteLength
}

function projectCodexItem(
  item: CodexThreadEvent["item"],
  ordinal: number,
): CodexTurnReportItem | undefined {
  if (item === undefined) return undefined
  if (item.type === "agent_message") {
    return {
      ordinal,
      itemType: "agent_message",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
      ...(typeof item.text === "string" ? { message: item.text } : {}),
    }
  }
  if (item.type === "reasoning") {
    return {
      ordinal,
      itemType: "reasoning",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
      ...(typeof item.text === "string" ? { reasoningSummary: item.text } : {}),
    }
  }
  if (item.type === "command_execution") {
    return {
      ordinal,
      itemType: "command_execution",
      commandLabel: "shell_command",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
      ...(typeof item.exit_code === "number" ? { exitCode: item.exit_code } : {}),
      ...(outputBytes(item) === undefined ? {} : { outputBytes: outputBytes(item) }),
    }
  }
  if (item.type === "file_change") {
    const changes = Array.isArray(item.changes) ? item.changes : []
    return {
      ordinal,
      itemType: "file_change",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
      changeCount: changes.length,
    }
  }
  if (item.type === "mcp_tool_call") {
    return {
      ordinal,
      itemType: "mcp_tool_call",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
      ...(typeof item.tool_name === "string"
        ? { toolName: item.tool_name }
        : typeof item.name === "string"
          ? { toolName: item.name }
          : {}),
    }
  }
  if (item.type === "web_search") {
    return {
      ordinal,
      itemType: "web_search",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
    }
  }
  if (item.type === "error") {
    return {
      ordinal,
      itemType: "error",
      ...(typeof item.status === "string" ? { status: item.status } : {}),
    }
  }
  return {
    ordinal,
    itemType: "unknown",
    ...(typeof item.status === "string" ? { status: item.status } : {}),
  }
}

async function reportCodexTurn(input: {
  eventReporter: CodexTurnReporter | undefined
  runInput: CodexAgentRunInput
  sessionRef: string | undefined
  turnIndex: number
  usage: CodexTurnUsage | undefined
  items: ReadonlyArray<CodexTurnReportItem>
  rawEvents: ReadonlyArray<RawCodexThreadEvent>
}) {
  if (
    input.eventReporter === undefined ||
    input.runInput.assignmentRef === undefined ||
    input.runInput.leaseRef === undefined ||
    input.runInput.pylonRef === undefined ||
    input.usage === undefined
  ) {
    return
  }
  await input.eventReporter({
    assignmentRef: input.runInput.assignmentRef,
    leaseRef: input.runInput.leaseRef,
    pylonRef: input.runInput.pylonRef,
    ...(input.runInput.runRef === undefined ? {} : { runRef: input.runInput.runRef }),
    ...(input.sessionRef === undefined ? {} : { sessionRef: input.sessionRef }),
    ...(input.runInput.workspaceRef === undefined ? {} : { workspaceRef: input.runInput.workspaceRef }),
    turnIndex: input.turnIndex,
    observedAt: new Date().toISOString(),
    usage: input.usage,
    items: input.items,
    rawEvents: input.rawEvents,
  }).catch(() => undefined)
}

/**
 * The production runner: one Codex SDK thread pinned to the bounded
 * workspace with approvalPolicy "never" (unattended, nothing to approve
 * against), owner-local full access, network enabled, and a wall-clock budget
 * enforced through the turn AbortSignal.
 * Every reported file change is validated against the workspace post hoc.
 * Lazy-imports the optional SDK dependency.
 */
export async function runWithCodexSdk(input: CodexAgentRunInput): Promise<CodexAgentRunResult> {
  const env = pylonAccountEnvironment(
    input.env ?? (Bun.env as Record<string, string | undefined>),
    input.account,
  )
  const sdk = (await import(CODEX_AGENT_SDK_PACKAGE)) as {
    Codex: new (options?: { env?: Record<string, string | undefined> }) => {
      startThread: (options: Record<string, unknown>) => {
        runStreamed: (
          prompt: string,
          turnOptions?: Record<string, unknown>,
        ) => Promise<{ events: AsyncIterable<unknown> }>
      }
    }
  }
  const abort = new AbortController()
  const timer = setTimeout(() => abort.abort(), input.timeoutMs)
  let escaped = false
  let editedFileCount = 0
  let commandCount = 0
  let turnCount = 0
  let threadId: string | null = null
  let failed = false
  let activeTurnIndex = 0
  let itemOrdinal = 0
  let currentTurnItems: Array<CodexTurnReportItem> = []
  let currentRawEvents: Array<RawCodexThreadEvent> = []
  let pendingRawEvents: Array<RawCodexThreadEvent> = []

  try {
    const codex = new sdk.Codex({ env })
    const thread = codex.startThread({
      workingDirectory: input.cwd,
      sandboxMode: input.sandboxMode,
      approvalPolicy: CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
      skipGitRepoCheck: true,
      networkAccessEnabled: input.networkAccessEnabled,
      ...(input.model === undefined ? {} : { model: input.model }),
    })
    const { events } = await thread.runStreamed(input.instructions, { signal: abort.signal })
    for await (const raw of events) {
      const event = raw as CodexThreadEvent
      const rawEvent = raw !== null && typeof raw === "object" ? (raw as RawCodexThreadEvent) : undefined
      if (rawEvent !== undefined && event.type !== "turn.started") {
        if (activeTurnIndex > 0) {
          currentRawEvents.push(rawEvent)
        } else {
          pendingRawEvents.push(rawEvent)
        }
      }
      if (event.type === "thread.started" && typeof event.thread_id === "string") {
        threadId = event.thread_id
      }
      if (event.type === "turn.started") {
        activeTurnIndex = turnCount + 1
        currentTurnItems = []
        currentRawEvents = rawEvent === undefined ? pendingRawEvents : [...pendingRawEvents, rawEvent]
        pendingRawEvents = []
      }
      if (event.type === "turn.completed") {
        const completedTurnIndex = activeTurnIndex > 0 ? activeTurnIndex : turnCount + 1
        const completedRawEvents = activeTurnIndex > 0 ? currentRawEvents : pendingRawEvents
        turnCount += 1
        const sessionRef =
          threadId === null ? undefined : stableRef("session.pylon.codex_agent", threadId)
        await reportCodexTurn({
          eventReporter: input.eventReporter,
          items: currentTurnItems,
          rawEvents: completedRawEvents,
          runInput: input,
          sessionRef,
          turnIndex: completedTurnIndex,
          usage: usageFromTurnCompleted(event),
        })
        currentTurnItems = []
        currentRawEvents = []
        pendingRawEvents = []
        activeTurnIndex = 0
      }
      if (event.type === "turn.failed" || event.type === "error") failed = true
      if (event.type === "item.completed" && event.item?.type === "command_execution") {
        commandCount += 1
      }
      if (event.type === "item.completed") {
        const projected = projectCodexItem(event.item, itemOrdinal + 1)
        if (projected !== undefined) {
          itemOrdinal += 1
          currentTurnItems.push(projected)
        }
      }
      if (event.type === "item.completed" && event.item?.type === "file_change") {
        const changes = Array.isArray(event.item.changes) ? event.item.changes : []
        editedFileCount += changes.length
        for (const change of changes) {
          if (typeof change.path === "string" && fileChangeEscapesWorkspace(change.path, input.cwd)) {
            escaped = true
            abort.abort()
          }
        }
      }
    }
  } catch (error) {
    if (escaped) {
      return { outcome: "workspace_escape_blocked", turnCount, editedFileCount, commandCount, sessionRef: null }
    }
    if (abort.signal.aborted) {
      return { outcome: "budget_exceeded", turnCount, editedFileCount, commandCount, sessionRef: null }
    }
    throw error
  } finally {
    clearTimeout(timer)
  }

  const sessionRef = threadId === null ? null : stableRef("session.pylon.codex_agent", threadId)
  if (escaped) {
    return { outcome: "workspace_escape_blocked", turnCount, editedFileCount, commandCount, sessionRef }
  }
  if (abort.signal.aborted) {
    return { outcome: "budget_exceeded", turnCount, editedFileCount, commandCount, sessionRef }
  }
  if (failed) {
    return { outcome: "refused", turnCount, editedFileCount, commandCount, sessionRef }
  }
  return { outcome: "completed", turnCount, editedFileCount, commandCount, sessionRef }
}

async function materializeCodexAgentWorkspace(input: {
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
  state: PylonLocalState
  task: CodexAgentTaskPayload
}) {
  if (input.task.workspace !== undefined) {
    const materialized = await materializeGitCheckoutWorkspaceWithLease({
      cacheRoot: join(input.state.paths.cache, "codex-agent-tasks"),
      checkout: input.task.workspace,
      ...(input.checkoutRunner === undefined ? {} : { checkoutRunner: input.checkoutRunner }),
      leaseRef: input.leaseRef,
      refPrefix: "workspace.pylon.codex_agent_task",
      repositoryCacheRoot: join(input.state.paths.cache, "workspace-git-cache"),
      workspaceStateRoot: join(input.state.paths.cache, "workspace-leases"),
    })
    return {
      acceptanceResultRef: "git_checkout_verified",
      artifactSourceRef: materialized.sourceRef,
      instructions: [
        "You are working in a bounded public repository checkout.",
        `Task objective: ${input.task.objectiveSummary ?? "complete the referenced Autopilot task"}.`,
        "Only modify files inside this checkout.",
        `Run the verification command ref ${input.task.workspace.verificationCommand.commandRef} before finishing.`,
      ].join(" "),
      verificationArgs: input.task.workspace.verificationCommand.args,
      workspace: materialized.workingDirectory,
      workspaceRef: materialized.workspaceRef,
    }
  }

  const workspaceRef = stableRef("workspace.pylon.codex_agent_task", input.leaseRef)
  const workspace = join(input.state.paths.cache, "codex-agent-tasks", workspaceRef)
  const fixtureRef = input.task.fixtureRef ?? CODEX_AGENT_SUM_REPAIR_FIXTURE_REF
  const fixture = CODEX_AGENT_FIXTURES[fixtureRef] ?? CODEX_AGENT_FIXTURES[CODEX_AGENT_SUM_REPAIR_FIXTURE_REF]
  await mkdir(workspace, { recursive: true })
  for (const [relativePath, contents] of Object.entries(fixture.files)) {
    await writeFile(join(workspace, relativePath), contents)
  }
  return {
    acceptanceResultRef: "fixture_repair",
    artifactSourceRef: fixtureRef,
    instructions: fixture.instructions,
    verificationArgs: fixture.verificationArgs,
    workspace,
    workspaceRef,
  }
}

type CodexAgentLease = {
  assignmentRef: string
  leaseRef: string
  codingAssignment?: unknown
}

function refusalRecord(input: {
  lease: CodexAgentLease
  runRef: string
  blockerRefs: string[]
  resultRef: string
  summaryRef: string
  message: string
}) {
  const failureRef = stableRef(
    "proof.pylon.codex_agent_task.refused",
    `${input.lease.leaseRef}:${input.blockerRefs.join(",")}`,
  )
  return {
    artifactRefs: [],
    blockerRefs: input.blockerRefs,
    buildRefs: [input.runRef],
    message: input.message,
    previewRefs: [],
    proofRefs: [failureRef],
    resultRefs: [input.resultRef],
    runRefs: [input.runRef],
    status: "rejected" as const,
    summaryRefs: [input.summaryRef],
    testRefs: [failureRef],
  }
}

/**
 * Executes a codex_sdk coding assignment. Returns null when the assignment
 * does not carry this work class, a typed refusal record when the lane is
 * not ready or the thread breaks its bounds, and an accepted closeout
 * record only when the agent's change passes the fixture's real
 * verification command on this device.
 */
export async function executeCodexAgentAssignment(
  state: PylonLocalState,
  lease: CodexAgentLease,
  now: Date,
  options: CodexAgentExecutionOptions = {},
) {
  const task = codexAgentTaskFrom(lease.codingAssignment)
  if (task === null) return null

  const runRef = stableRef(
    "run.pylon.codex_agent_task",
    `${lease.leaseRef}:${task.fixtureRef ?? task.workspace?.repository.commitSha ?? "unspecified"}:${now.toISOString()}`,
  )

  const config = await loadCodexAgentConfig({ paths: { config: state.paths.config } })
  const env = pylonAccountEnvironment(options.codexAgentProbe?.env ?? Bun.env, options.account)
  const probed = await probeCodexAgentReadiness({ ...options.codexAgentProbe, config, env })
  if (probed.state !== "ready") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_unavailable", ...probed.blockerRefs],
      resultRef: "result.public.pylon.codex_agent_task.unavailable",
      summaryRef: "summary.public.pylon.codex_agent_task.unavailable",
      message: `Local Codex lane is not ready on this device (${probed.state}).`,
    })
  }

  let materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
  try {
    materialized = await materializeCodexAgentWorkspace({
      ...(options.checkoutRunner === undefined ? {} : { checkoutRunner: options.checkoutRunner }),
      leaseRef: lease.leaseRef,
      state,
      task,
    })
  } catch {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_workspace_checkout_failed"],
      resultRef: "result.public.pylon.codex_agent_task.workspace_checkout_failed",
      summaryRef: "summary.public.pylon.codex_agent_task.workspace_checkout_failed",
      message: "Local Codex thread refused because the bounded workspace checkout could not be materialized.",
    })
  }

  const dependencies = await prepareWorkspaceDependencies({
    ...(options.dependencyInstaller === undefined ? {} : { installer: options.dependencyInstaller }),
    workspace: materialized.workspace,
  })
  if (!dependencies.ok) {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_workspace_dependency_install_failed"],
      resultRef: "result.public.pylon.codex_agent_task.workspace_dependency_install_failed",
      summaryRef: "summary.public.pylon.codex_agent_task.workspace_dependency_install_failed",
      message: "Local Codex thread refused because workspace dependencies could not be prepared.",
    })
  }

  const runner = options.codexAgentRunner ?? runWithCodexSdk
  const eventReporter =
    options.codexTurnReporter ??
    createPylonCodexTurnReporter({
      ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
      ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
      ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
    })
  let run: CodexAgentRunResult
  try {
    run = await runner({
      assignmentRef: lease.assignmentRef,
      cwd: materialized.workspace,
      account: options.account,
      env,
      ...(eventReporter === undefined ? {} : { eventReporter }),
      instructions: materialized.instructions,
      leaseRef: lease.leaseRef,
      networkAccessEnabled: true,
      pylonRef: state.identity.pylonRef,
      runRef,
      sandboxMode: effectiveSandboxMode(task.sandboxMode, config.sandboxMode),
      timeoutMs:
        boundedNumber(
          task.timeoutSeconds ?? config.timeoutSeconds,
          DEFAULT_TIMEOUT_SECONDS,
          MAX_TIMEOUT_SECONDS,
        ) * 1000,
      ...(config.model === undefined ? {} : { model: config.model }),
      workspaceRef: materialized.workspaceRef,
    })
  } catch {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_execution_refused"],
      resultRef: "result.public.pylon.codex_agent_task.execution_refused",
      summaryRef: "summary.public.pylon.codex_agent_task.execution_refused",
      message: "Local Codex thread refused with a typed execution error.",
    })
  }

  if (run.outcome === "workspace_escape_blocked") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_workspace_escape_blocked"],
      resultRef: "result.public.pylon.codex_agent_task.workspace_escape_blocked",
      summaryRef: "summary.public.pylon.codex_agent_task.workspace_escape_blocked",
      message: "Local Codex thread was stopped: a reported file change targeted paths outside the bounded workspace.",
    })
  }
  if (run.outcome === "budget_exceeded") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_budget_exceeded"],
      resultRef: "result.public.pylon.codex_agent_task.budget_exceeded",
      summaryRef: "summary.public.pylon.codex_agent_task.budget_exceeded",
      message: "Local Codex thread exceeded its wall-clock budget before completing the task.",
    })
  }
  if (run.outcome === "refused") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.codex_agent_execution_refused"],
      resultRef: "result.public.pylon.codex_agent_task.execution_refused",
      summaryRef: "summary.public.pylon.codex_agent_task.execution_refused",
      message: "Local Codex thread ended with an execution error before completing the task.",
    })
  }

  const verification = await runCommand({ args: materialized.verificationArgs, cwd: materialized.workspace })
  const commandRef = stableRef(
    "command.pylon.codex_agent_task.verification",
    `${lease.leaseRef}:${verification.exitCode}:${verification.stdoutBytes}:${verification.stderrBytes}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.codex_agent_task.patch",
    `${lease.assignmentRef}:${materialized.artifactSourceRef}:${run.editedFileCount}`,
  )
  const proofRef = stableRef(
    "proof.pylon.codex_agent_task.test",
    `${artifactRef}:${commandRef}`,
  )
  const passed = verification.exitCode === 0
  const sessionRefs = run.sessionRef === null ? [] : [run.sessionRef]

  return {
    artifactRefs: [artifactRef],
    blockerRefs: passed ? [] : ["blocker.assignment.codex_agent_test_failed"],
    buildRefs: [commandRef],
    message: passed
      ? `Local Codex completed the bounded coding task: ${run.editedFileCount} file edit(s), ${run.commandCount} command(s), ${run.turnCount} turn(s), verification test passed on this device.`
      : "Local Codex thread completed but the verification test command failed; the change is not accepted.",
    previewRefs: [materialized.workspaceRef],
    proofRefs: [proofRef],
    resultRefs: [
      passed
        ? `result.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_passed`
        : `result.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_failed`,
      `result.public.pylon.codex_agent_task.edited_files.${run.editedFileCount}`,
    ],
    runRefs: [runRef, ...sessionRefs],
    status: passed ? ("accepted" as const) : ("rejected" as const),
    summaryRefs: [
      passed
        ? `summary.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_passed`
        : `summary.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_failed`,
    ],
    testRefs: [commandRef],
  }
}
