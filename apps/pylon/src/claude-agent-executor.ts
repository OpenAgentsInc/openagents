import { mkdir, writeFile } from "node:fs/promises"
import { isAbsolute, join, resolve } from "node:path"
import { createHash } from "node:crypto"
import {
  CLAUDE_AGENT_SDK_PACKAGE,
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  type ClaudeAgentProbeOptions,
} from "./claude-agent"
import type { PylonLocalState } from "./state"

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

export type ClaudeAgentTaskPayload = {
  schema: typeof CLAUDE_AGENT_TASK_SCHEMA
  agentKind: typeof CLAUDE_AGENT_TASK_AGENT_KIND
  fixtureRef: string
  allowedToolKinds?: string[]
  maxTurns?: number
  timeoutSeconds?: number
}

export type ClaudeAgentRunInput = {
  cwd: string
  instructions: string
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

export type ClaudeAgentRunResult = {
  outcome: ClaudeAgentRunOutcome
  turnCount: number
  editedFileCount: number
  commandCount: number
  sessionRef: string | null
}

export type ClaudeAgentRunner = (input: ClaudeAgentRunInput) => Promise<ClaudeAgentRunResult>

export type ClaudeAgentExecutionOptions = {
  claudeAgentRunner?: ClaudeAgentRunner
  claudeAgentProbe?: ClaudeAgentProbeOptions
}

type ClaudeAgentFixture = {
  files: Record<string, string>
  instructions: string
  verificationArgs: string[]
}

const TOOL_KIND_MAP: Record<string, string> = {
  read: "Read",
  edit: "Edit",
  write: "Write",
  bash: "Bash",
  glob: "Glob",
  grep: "Grep",
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
  if (typeof payload.fixtureRef !== "string") return null
  if (CLAUDE_AGENT_FIXTURES[payload.fixtureRef] === undefined) return null
  return payload
}

function allowedToolsFrom(payload: ClaudeAgentTaskPayload): string[] {
  const kinds = payload.allowedToolKinds
  if (!Array.isArray(kinds) || kinds.length === 0) return [...DEFAULT_ALLOWED_TOOLS]
  const tools = kinds
    .map((kind) => TOOL_KIND_MAP[String(kind).toLowerCase()])
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

  for (const key of ["file_path", "path", "notebook_path"]) {
    const value = input[key]
    if (typeof value !== "string" || value.length === 0) continue
    const resolved = isAbsolute(value) ? resolve(value) : resolve(workspaceRoot, value)
    if (resolved !== workspaceRoot && !resolved.startsWith(`${workspaceRoot}/`)) return true
  }

  if (toolName === "Bash") {
    const command = input.command
    if (typeof command === "string") {
      if (command.includes("..")) return true
      const allowedPrefixes = [`${workspaceRoot}/`, "/dev/", "/usr/", "/bin/", "/sbin/", "/opt/"]
      const absolutePaths = command.match(/(?:^|[\s='"])(\/[^\s'"]+)/g) ?? []
      for (const match of absolutePaths) {
        const candidate = resolve(match.replace(/^[\s='"]+/, ""))
        const allowed =
          candidate === workspaceRoot ||
          allowedPrefixes.some((prefix) => candidate.startsWith(prefix))
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

/**
 * The production runner: one Claude Agent SDK session with the workspace
 * boundary enforced by a PreToolUse hook (deny + abort on first escape
 * attempt), user settings excluded via settingSources, and turn/wall-clock
 * budgets. Lazy-imports the optional SDK dependency.
 */
export async function runWithClaudeAgentSdk(
  input: ClaudeAgentRunInput,
): Promise<ClaudeAgentRunResult> {
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
      const record = message as { type?: string; subtype?: string; session_id?: string }
      if (record.type === "system" && record.subtype === "init" && typeof record.session_id === "string") {
        sessionId = record.session_id
      }
      if (record.type === "assistant") turnCount += 1
      if (record.type === "result") resultSubtype = record.subtype ?? null
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

  const sessionRef = sessionId === null ? null : stableRef("session.pylon.claude_agent", sessionId)
  if (escaped) {
    return { outcome: "workspace_escape_blocked", turnCount, editedFileCount, commandCount, sessionRef }
  }
  if (resultSubtype !== null && resultSubtype.includes("max_turns")) {
    return { outcome: "budget_exceeded", turnCount, editedFileCount, commandCount, sessionRef }
  }
  if (resultSubtype !== null && resultSubtype.startsWith("error")) {
    return { outcome: "refused", turnCount, editedFileCount, commandCount, sessionRef }
  }
  return { outcome: "completed", turnCount, editedFileCount, commandCount, sessionRef }
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
  const probed = await probeClaudeAgentReadiness({ ...options.claudeAgentProbe, config })
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

  const fixture = CLAUDE_AGENT_FIXTURES[task.fixtureRef]
  const workspaceRef = stableRef("workspace.pylon.claude_agent_task", lease.leaseRef)
  const workspace = join(state.paths.cache, "claude-agent-tasks", workspaceRef)
  await mkdir(workspace, { recursive: true })
  for (const [relativePath, contents] of Object.entries(fixture.files)) {
    await writeFile(join(workspace, relativePath), contents)
  }

  const runner = options.claudeAgentRunner ?? runWithClaudeAgentSdk
  let run: ClaudeAgentRunResult
  try {
    run = await runner({
      cwd: workspace,
      instructions: fixture.instructions,
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

  const verification = await runCommand({ args: fixture.verificationArgs, cwd: workspace })
  const commandRef = stableRef(
    "command.pylon.claude_agent_task.verification",
    `${lease.leaseRef}:${verification.exitCode}:${verification.stdoutBytes}:${verification.stderrBytes}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.claude_agent_task.patch",
    `${lease.assignmentRef}:${task.fixtureRef}:${run.editedFileCount}`,
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
    previewRefs: [workspaceRef],
    proofRefs: [proofRef],
    resultRefs: [
      passed
        ? "result.public.pylon.claude_agent_task.fixture_repair_passed"
        : "result.public.pylon.claude_agent_task.fixture_repair_failed",
      `result.public.pylon.claude_agent_task.edited_files.${run.editedFileCount}`,
    ],
    runRefs: [runRef, ...sessionRefs],
    status: passed ? ("accepted" as const) : ("rejected" as const),
    summaryRefs: [
      passed
        ? "summary.public.pylon.claude_agent_task.fixture_repair_passed"
        : "summary.public.pylon.claude_agent_task.fixture_repair_failed",
    ],
    testRefs: [commandRef],
  }
}
