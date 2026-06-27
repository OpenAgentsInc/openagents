import { realpathSync } from "node:fs"
import { mkdir, writeFile } from "node:fs/promises"
import { isAbsolute, join, resolve } from "node:path"
import { createHash } from "node:crypto"
import {
  CLAUDE_AGENT_SDK_PACKAGE,
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  type ClaudeAgentProbeOptions,
} from "./claude-agent.js"
import {
  defaultGitCheckoutRunner,
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
  createPylonClaudeTurnReporter,
  type ClaudeTurnReporter,
} from "./claude-turn-reporter.js"
import type { PylonLocalState } from "./state.js"

/**
 * The local Claude Agent executor gate (issue #4719, promise
 * pylon.local_claude_agent_bridge.v1).
 *
 * Recognizes the claude_agent_sdk coding work class on a Pylon assignment,
 * materializes a bounded fixture workspace, drives one Claude Agent SDK
 * session inside it, verifies the result with the fixture's real test
 * command, and digests everything into public-safe closeout refs. Raw SDK
 * messages, prompts, file contents, and local paths never leave the device;
 * the instruction text lives in the locally-shipped fixture, not on the
 * wire, so assignment payloads stay ref-only.
 */

export const CLAUDE_AGENT_TASK_SCHEMA = "openagents.pylon.claude_agent_task.v0.3"
export const CLAUDE_AGENT_TASK_AGENT_KIND = "claude_agent_sdk"
export const CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF = "fixture.public.pylon.claude_agent.sum_repair.v1"

// The git_checkout workspace contract now lives in the adapter-neutral
// materializer module (#4798). These aliases and re-exports keep existing
// import sites working; new code should import ./workspace-materializer.
export type ClaudeAgentGitCheckoutWorkspace = GitCheckoutWorkspace
export type ClaudeAgentCheckoutRunner = WorkspaceCheckoutRunner
export { gitCheckoutWorkspaceFrom } from "./workspace-materializer.js"
export const defaultClaudeAgentCheckoutRunner: WorkspaceCheckoutRunner = defaultGitCheckoutRunner

export type ClaudeAgentTaskPayload = {
  schema: typeof CLAUDE_AGENT_TASK_SCHEMA
  agentKind: typeof CLAUDE_AGENT_TASK_AGENT_KIND
  fixtureRef?: string
  allowedToolKinds?: string[]
  maxTurns?: number
  objectiveSummary?: string
  timeoutSeconds?: number
  workspace?: ClaudeAgentGitCheckoutWorkspace
}

export type ClaudeAgentRunInput = {
  cwd: string
  instructions: string
  account?: ResolvedPylonAccountSelection | null
  env?: Record<string, string | undefined>
  allowedTools: string[]
  maxTurns: number
  timeoutMs: number
  model?: string
}

export type ClaudeAgentRunOutcome =
  | "completed"
  | "budget_exceeded"
  | "workspace_escape_blocked"
  | "refused"

export type ClaudeAgentTurnUsage = {
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
}

export type ClaudeAgentRunResult = {
  outcome: ClaudeAgentRunOutcome
  turnCount: number
  editedFileCount: number
  commandCount: number
  sessionRef: string | null
  // Cumulative exact token usage for the Claude Agent SDK session, read from
  // the SDK `result` message. `null` when the SDK did not surface usage.
  usage: ClaudeAgentTurnUsage | null
}

export type ClaudeAgentRunner = (input: ClaudeAgentRunInput) => Promise<ClaudeAgentRunResult>

export type ClaudeAgentExecutionOptions = {
  account?: ResolvedPylonAccountSelection | null
  agentToken?: string
  baseUrl?: string
  checkoutRunner?: WorkspaceCheckoutRunner
  claudeAgentRunner?: ClaudeAgentRunner
  claudeAgentProbe?: ClaudeAgentProbeOptions
  // Injectable reporter override (tests). When omitted, a default reporter is
  // built from agentToken + baseUrl and posts the exact own-capacity Claude turn
  // token usage to /api/pylon/claude/turns. Fail-soft: a reporter failure never
  // aborts the local coding task.
  claudeTurnReporter?: ClaudeTurnReporter
  fetch?: typeof fetch
}

type ClaudeAgentFixture = {
  files: Record<string, string>
  instructions: string
  verificationArgs: string[]
}

const TOOL_KIND_MAP: Record<string, string[]> = {
  read: ["Read"],
  edit: ["Edit"],
  write: ["Write"],
  file: ["Read"],
  git: ["Bash"],
  shell: ["Bash"],
  test_runner: ["Bash"],
  search: ["Grep", "Glob"],
  bash: ["Bash"],
  glob: ["Glob"],
  grep: ["Grep"],
}

const DEFAULT_ALLOWED_TOOLS = ["Read", "Edit", "Write", "Bash", "Glob", "Grep"]
const DEFAULT_MAX_TURNS = 16
const MAX_MAX_TURNS = 50
const DEFAULT_TIMEOUT_SECONDS = 300
const MAX_TIMEOUT_SECONDS = 1200

const CLAUDE_AGENT_FIXTURES: Record<string, ClaudeAgentFixture> = {
  [CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF]: {
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

export function claudeAgentTaskFrom(codingAssignment: unknown): ClaudeAgentTaskPayload | null {
  const claudeAgent = (codingAssignment as { claudeAgent?: unknown } | null)?.claudeAgent
  if (claudeAgent === null || typeof claudeAgent !== "object") return null
  const payload = claudeAgent as ClaudeAgentTaskPayload
  if (payload.schema !== CLAUDE_AGENT_TASK_SCHEMA) return null
  if (payload.agentKind !== CLAUDE_AGENT_TASK_AGENT_KIND) return null
  const workspace = gitCheckoutWorkspaceFrom(codingAssignment)
  const objective = (codingAssignment as { objective?: { publicSummary?: unknown } } | null)?.objective
  const hasFixture =
    typeof payload.fixtureRef === "string" &&
    CLAUDE_AGENT_FIXTURES[payload.fixtureRef] !== undefined
  if (!hasFixture && workspace === null) return null
  return {
    ...payload,
    ...(typeof objective?.publicSummary === "string" ? { objectiveSummary: objective.publicSummary } : {}),
    ...(workspace === null ? {} : { workspace }),
  }
}

function allowedToolsFrom(payload: ClaudeAgentTaskPayload): string[] {
  const kinds = payload.allowedToolKinds
  if (!Array.isArray(kinds) || kinds.length === 0) return [...DEFAULT_ALLOWED_TOOLS]
  const tools = kinds
    .flatMap((kind) => TOOL_KIND_MAP[String(kind).toLowerCase()] ?? [])
    .filter((tool): tool is string => tool !== undefined)
  return tools.length > 0 ? [...new Set(tools)] : [...DEFAULT_ALLOWED_TOOLS]
}

function boundedNumber(value: number | undefined, fallback: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return fallback
  return Math.min(Math.floor(value), max)
}

/**
 * Returns true when a tool call targets anything outside the bounded
 * workspace. Path fields must resolve under the workspace; Bash commands
 * may not contain parent-directory traversal or absolute paths that leave
 * it. Deny-by-default sandbox policy for the bounded fixture lane.
 */
export function toolInputEscapesWorkspace(
  toolName: string | undefined,
  toolInput: unknown,
  workspace: string,
): boolean {
  if (toolInput === null || typeof toolInput !== "object") return false
  const input = toolInput as Record<string, unknown>
  const workspaceRoot = resolve(workspace)
  // The SDK canonicalizes its cwd, so on platforms where the workspace sits
  // behind a symlink (macOS /tmp -> /private/tmp, $TMPDIR under /var) tool
  // paths arrive in realpath form. Accept both spellings of the same root —
  // and only those two; this widens nothing beyond the workspace itself.
  let workspaceRealRoot = workspaceRoot
  try {
    workspaceRealRoot = realpathSync(workspaceRoot)
  } catch {
    // unmaterialized workspace: fall back to the resolved root only
  }
  const roots =
    workspaceRealRoot === workspaceRoot ? [workspaceRoot] : [workspaceRoot, workspaceRealRoot]
  const insideWorkspace = (candidate: string) =>
    roots.some((root) => candidate === root || candidate.startsWith(`${root}/`))

  for (const key of ["file_path", "path", "notebook_path"]) {
    const value = input[key]
    if (typeof value !== "string" || value.length === 0) continue
    const resolved = isAbsolute(value) ? resolve(value) : resolve(workspaceRealRoot, value)
    if (!insideWorkspace(resolved)) return true
  }

  if (toolName === "Bash") {
    const command = input.command
    if (typeof command === "string") {
      if (command.includes("..")) return true
      const systemPrefixes = ["/dev/", "/usr/", "/bin/", "/sbin/", "/opt/"]
      const absolutePaths = command.match(/(?:^|[\s='"])(\/[^\s'"]+)/g) ?? []
      for (const match of absolutePaths) {
        const candidate = resolve(match.replace(/^[\s='"]+/, ""))
        const allowed =
          insideWorkspace(candidate) ||
          systemPrefixes.some((prefix) => candidate.startsWith(prefix))
        if (!allowed) return true
      }
    }
  }

  return false
}

async function runCommand(input: { args: string[]; cwd: string }) {
  const proc = Bun.spawn(input.args, { cwd: input.cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).arrayBuffer(),
    new Response(proc.stderr).arrayBuffer(),
    proc.exited,
  ])
  return { exitCode, stderrBytes: stderr.byteLength, stdoutBytes: stdout.byteLength }
}

async function materializeClaudeAgentWorkspace(input: {
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
  state: PylonLocalState
  task: ClaudeAgentTaskPayload
}) {
  if (input.task.workspace !== undefined) {
    const materialized = await materializeGitCheckoutWorkspaceWithLease({
      cacheRoot: join(input.state.paths.cache, "claude-agent-tasks"),
      checkout: input.task.workspace,
      ...(input.checkoutRunner === undefined ? {} : { checkoutRunner: input.checkoutRunner }),
      leaseRef: input.leaseRef,
      refPrefix: "workspace.pylon.claude_agent_task",
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

  const workspaceRef = stableRef("workspace.pylon.claude_agent_task", input.leaseRef)
  const workspace = join(input.state.paths.cache, "claude-agent-tasks", workspaceRef)
  const fixtureRef = input.task.fixtureRef ?? CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF
  const fixture = CLAUDE_AGENT_FIXTURES[fixtureRef] ?? CLAUDE_AGENT_FIXTURES[CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF]
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

const finiteToken = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0

/**
 * Reads cumulative exact token usage from the Claude Agent SDK `result`
 * message. The SDK reports usage with Anthropic-native field names
 * (`input_tokens`, `output_tokens`, `cache_read_input_tokens`). Returns null
 * when no positive usage is present so a missing/zero usage does not post a
 * fabricated token row.
 */
export function claudeUsageFrom(value: unknown): ClaudeAgentTurnUsage | null {
  if (value === null || typeof value !== "object") return null
  const usage = value as {
    input_tokens?: unknown
    output_tokens?: unknown
    cache_read_input_tokens?: unknown
    cache_creation_input_tokens?: unknown
  }
  const inputTokens = finiteToken(usage.input_tokens)
  const outputTokens = finiteToken(usage.output_tokens)
  const cachedInputTokens =
    finiteToken(usage.cache_read_input_tokens) +
    finiteToken(usage.cache_creation_input_tokens)
  if (inputTokens === 0 && outputTokens === 0 && cachedInputTokens === 0) {
    return null
  }
  return { cachedInputTokens, inputTokens, outputTokens }
}

/**
 * The production runner: one Claude Agent SDK session with the workspace
 * boundary enforced by a PreToolUse hook (deny + abort on first escape
 * attempt), user settings excluded via settingSources, and turn/wall-clock
 * budgets. Lazy-imports the optional SDK dependency.
 */
export async function runWithClaudeAgentSdk(
  input: ClaudeAgentRunInput,
): Promise<ClaudeAgentRunResult> {
  const env = pylonAccountEnvironment(
    input.env ?? (Bun.env as Record<string, string | undefined>),
    input.account,
  )
  const sdk = (await import(CLAUDE_AGENT_SDK_PACKAGE)) as {
    query: (options: { prompt: string; options: Record<string, unknown> }) => AsyncIterable<unknown>
  }
  const abort = new AbortController()
  const timer = setTimeout(() => abort.abort(), input.timeoutMs)
  let escaped = false
  let editedFileCount = 0
  let commandCount = 0
  let turnCount = 0
  let sessionId: string | null = null
  let resultSubtype: string | null = null
  let usage: ClaudeAgentTurnUsage | null = null

  const guard = async (hookInput: unknown) => {
    const record = hookInput as { tool_name?: string; tool_input?: unknown }
    if (toolInputEscapesWorkspace(record.tool_name, record.tool_input, input.cwd)) {
      escaped = true
      abort.abort()
      return {
        hookSpecificOutput: {
          hookEventName: "PreToolUse" as const,
          permissionDecision: "deny" as const,
          permissionDecisionReason: "claude_agent.workspace_boundary",
        },
      }
    }
    return {}
  }
  const countEdit = async () => {
    editedFileCount += 1
    return {}
  }
  const countCommand = async () => {
    commandCount += 1
    return {}
  }

  try {
    const session = sdk.query({
      prompt: input.instructions,
      options: {
        cwd: input.cwd,
        env,
        allowedTools: input.allowedTools,
        maxTurns: input.maxTurns,
        settingSources: [],
        abortController: abort,
        ...(input.model === undefined ? {} : { model: input.model }),
        hooks: {
          PreToolUse: [{ hooks: [guard] }],
          PostToolUse: [
            { matcher: "Edit|Write", hooks: [countEdit] },
            { matcher: "Bash", hooks: [countCommand] },
          ],
        },
      },
    })
    for await (const message of session) {
      const record = message as {
        type?: string
        subtype?: string
        session_id?: string
        usage?: unknown
      }
      if (record.type === "system" && record.subtype === "init" && typeof record.session_id === "string") {
        sessionId = record.session_id
      }
      if (record.type === "assistant") turnCount += 1
      if (record.type === "result") {
        resultSubtype = record.subtype ?? null
        const captured = claudeUsageFrom(record.usage)
        if (captured !== null) usage = captured
      }
    }
  } catch (error) {
    if (escaped) {
      return { outcome: "workspace_escape_blocked", turnCount, editedFileCount, commandCount, sessionRef: null , usage }
    }
    if (abort.signal.aborted) {
      return { outcome: "budget_exceeded", turnCount, editedFileCount, commandCount, sessionRef: null , usage }
    }
    throw error
  } finally {
    clearTimeout(timer)
  }

  const sessionRef = sessionId === null ? null : stableRef("session.pylon.claude_agent", sessionId)
  if (escaped) {
    return { outcome: "workspace_escape_blocked", turnCount, editedFileCount, commandCount, sessionRef , usage }
  }
  if (resultSubtype !== null && resultSubtype.includes("max_turns")) {
    return { outcome: "budget_exceeded", turnCount, editedFileCount, commandCount, sessionRef , usage }
  }
  if (resultSubtype !== null && resultSubtype.startsWith("error")) {
    return { outcome: "refused", turnCount, editedFileCount, commandCount, sessionRef , usage }
  }
  return { outcome: "completed", turnCount, editedFileCount, commandCount, sessionRef , usage }
}

type ClaudeAgentLease = {
  assignmentRef: string
  leaseRef: string
  codingAssignment?: unknown
}

function refusalRecord(input: {
  lease: ClaudeAgentLease
  runRef: string
  blockerRefs: string[]
  resultRef: string
  summaryRef: string
  message: string
}) {
  const failureRef = stableRef(
    "proof.pylon.claude_agent_task.refused",
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
 * Executes a claude_agent_sdk coding assignment. Returns null when the
 * assignment does not carry this work class, a typed refusal record when
 * the lane is not ready or the session breaks its bounds, and an accepted
 * closeout record only when the agent's change passes the fixture's real
 * verification command on this device.
 */
export async function executeClaudeAgentAssignment(
  state: PylonLocalState,
  lease: ClaudeAgentLease,
  now: Date,
  options: ClaudeAgentExecutionOptions = {},
) {
  const task = claudeAgentTaskFrom(lease.codingAssignment)
  if (task === null) return null

  const runRef = stableRef(
    "run.pylon.claude_agent_task",
    `${lease.leaseRef}:${task.fixtureRef}:${now.toISOString()}`,
  )

  const config = await loadClaudeAgentConfig({ paths: { config: state.paths.config } })
  const env = pylonAccountEnvironment(options.claudeAgentProbe?.env ?? Bun.env, options.account)
  const probed = await probeClaudeAgentReadiness({ ...options.claudeAgentProbe, config, env })
  if (probed.state !== "ready") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_unavailable", ...probed.blockerRefs],
      resultRef: "result.public.pylon.claude_agent_task.unavailable",
      summaryRef: "summary.public.pylon.claude_agent_task.unavailable",
      message: `Local Claude Agent lane is not ready on this device (${probed.state}).`,
    })
  }

  let materialized: Awaited<ReturnType<typeof materializeClaudeAgentWorkspace>>
  try {
    materialized = await materializeClaudeAgentWorkspace({
      ...(options.checkoutRunner === undefined ? {} : { checkoutRunner: options.checkoutRunner }),
      leaseRef: lease.leaseRef,
      state,
      task,
    })
  } catch {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_workspace_checkout_failed"],
      resultRef: "result.public.pylon.claude_agent_task.workspace_checkout_failed",
      summaryRef: "summary.public.pylon.claude_agent_task.workspace_checkout_failed",
      message: "Local Claude Agent session refused because the bounded workspace checkout could not be materialized.",
    })
  }

  const runner = options.claudeAgentRunner ?? runWithClaudeAgentSdk
  let run: ClaudeAgentRunResult
  try {
    run = await runner({
      cwd: materialized.workspace,
      account: options.account,
      env,
      instructions: materialized.instructions,
      allowedTools: allowedToolsFrom(task),
      maxTurns: boundedNumber(task.maxTurns ?? config.maxTurns, DEFAULT_MAX_TURNS, MAX_MAX_TURNS),
      timeoutMs:
        boundedNumber(
          task.timeoutSeconds ?? config.timeoutSeconds,
          DEFAULT_TIMEOUT_SECONDS,
          MAX_TIMEOUT_SECONDS,
        ) * 1000,
      ...(config.model === undefined ? {} : { model: config.model }),
    })
  } catch {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_execution_refused"],
      resultRef: "result.public.pylon.claude_agent_task.execution_refused",
      summaryRef: "summary.public.pylon.claude_agent_task.execution_refused",
      message: "Local Claude Agent session refused with a typed execution error.",
    })
  }

  if (run.outcome === "workspace_escape_blocked") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_workspace_escape_blocked"],
      resultRef: "result.public.pylon.claude_agent_task.workspace_escape_blocked",
      summaryRef: "summary.public.pylon.claude_agent_task.workspace_escape_blocked",
      message: "Local Claude Agent session was stopped: a tool call targeted paths outside the bounded workspace.",
    })
  }
  if (run.outcome === "budget_exceeded") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_budget_exceeded"],
      resultRef: "result.public.pylon.claude_agent_task.budget_exceeded",
      summaryRef: "summary.public.pylon.claude_agent_task.budget_exceeded",
      message: "Local Claude Agent session exceeded its turn or wall-clock budget before completing the task.",
    })
  }
  if (run.outcome === "refused") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_execution_refused"],
      resultRef: "result.public.pylon.claude_agent_task.execution_refused",
      summaryRef: "summary.public.pylon.claude_agent_task.execution_refused",
      message: "Local Claude Agent session ended with an execution error before completing the task.",
    })
  }

  // #6391: post the exact own-capacity Claude Agent SDK token usage for this
  // completed turn so a `token_usage_events` row exists (provider
  // `pylon-claude-own-capacity`, model `openagents/pylon-claude`). Fail-soft: a
  // reporter outage must never fail the local coding task. Skipped when the SDK
  // surfaced no usage (never fabricate a row).
  if (run.usage != null) {
    const reporter =
      options.claudeTurnReporter ??
      createPylonClaudeTurnReporter({
        ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
        ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
        ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
      })
    if (reporter !== undefined) {
      try {
        await reporter({
          assignmentRef: lease.assignmentRef,
          leaseRef: lease.leaseRef,
          pylonRef: state.identity.pylonRef,
          runRef,
          ...(run.sessionRef === null ? {} : { sessionRef: run.sessionRef }),
          workspaceRef: materialized.workspaceRef,
          turnIndex: 1,
          observedAt: now.toISOString(),
          usage: {
            inputTokens: run.usage.inputTokens,
            cachedInputTokens: run.usage.cachedInputTokens,
            outputTokens: run.usage.outputTokens,
          },
        })
      } catch {
        // Fail-soft: token-ingest outage does not abort the local coding task.
        // The exact token row can be reconciled/retried; the local work stands.
      }
    }
  }

  const verification = await runCommand({ args: materialized.verificationArgs, cwd: materialized.workspace })
  const commandRef = stableRef(
    "command.pylon.claude_agent_task.verification",
    `${lease.leaseRef}:${verification.exitCode}:${verification.stdoutBytes}:${verification.stderrBytes}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.claude_agent_task.patch",
    `${lease.assignmentRef}:${materialized.artifactSourceRef}:${run.editedFileCount}`,
  )
  const proofRef = stableRef(
    "proof.pylon.claude_agent_task.test",
    `${artifactRef}:${commandRef}`,
  )
  const passed = verification.exitCode === 0
  const sessionRefs = run.sessionRef === null ? [] : [run.sessionRef]

  return {
    artifactRefs: [artifactRef],
    blockerRefs: passed ? [] : ["blocker.assignment.claude_agent_test_failed"],
    buildRefs: [commandRef],
    message: passed
      ? `Local Claude Agent completed the bounded coding task: ${run.editedFileCount} file edit(s), ${run.commandCount} command(s), ${run.turnCount} turn(s), verification test passed on this device.`
      : "Local Claude Agent session completed but the verification test command failed; the change is not accepted.",
    previewRefs: [materialized.workspaceRef],
    proofRefs: [proofRef],
    resultRefs: [
      passed
        ? `result.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_passed`
        : `result.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_failed`,
      `result.public.pylon.claude_agent_task.edited_files.${run.editedFileCount}`,
    ],
    runRefs: [runRef, ...sessionRefs],
    status: passed ? ("accepted" as const) : ("rejected" as const),
    summaryRefs: [
      passed
        ? `summary.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_passed`
        : `summary.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_failed`,
    ],
    testRefs: [commandRef],
  }
}
