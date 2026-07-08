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
  assertNoLongLivedScmCredentials,
  defaultGitCheckoutRunner,
  gitCheckoutWorkspaceFrom,
  materializeGitCheckoutWorkspaceWithLease,
  releaseWorkspace,
  workspaceCheckoutFailureReasonRef,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
  type WorkspaceScmCredentialScanRoot,
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

// A model that guesses one wrong absolute path should get the denial and
// recover; only repeated boundary offenses abort the run (issue #7914).
const WORKSPACE_ESCAPE_DENIAL_BUDGET = 3

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
      "You are working in a bounded fixture workspace; your current working",
      "directory IS that workspace — use relative paths (./sum.ts), never",
      "assumed absolute paths.",
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
 * resolve path-like tokens under the same root and deny only real escapes.
 * Deny-by-default sandbox policy for the bounded fixture lane.
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
      const systemPrefixes = ["/dev/", "/usr/", "/bin/", "/sbin/", "/opt/"]
      for (const token of bashPathLikeTokens(command)) {
        const candidate = isAbsolute(token) ? resolve(token) : resolve(workspaceRealRoot, token)
        const allowed =
          insideWorkspace(candidate) ||
          systemPrefixes.some((prefix) => candidate.startsWith(prefix))
        if (!allowed) return true
      }
    }
  }

  return false
}

function bashPathLikeTokens(command: string): string[] {
  const tokens: string[] = []
  let current = ""
  let quote: "'" | "\"" | null = null
  let escaped = false

  const push = () => {
    const token = shellPathTokenFrom(current)
    if (token !== null) tokens.push(token)
    current = ""
  }

  for (const char of command) {
    if (escaped) {
      current += char
      escaped = false
      continue
    }
    if (char === "\\" && quote !== "'") {
      escaped = true
      continue
    }
    if ((char === "'" || char === "\"") && quote === null) {
      quote = char
      continue
    }
    if (char === quote) {
      quote = null
      continue
    }
    if (quote === null && /\s/.test(char)) {
      push()
      continue
    }
    current += char
  }
  if (escaped) current += "\\"
  push()
  return tokens
}

function shellPathTokenFrom(rawToken: string): string | null {
  const token = rawToken
    .replace(/^[;|&(){}[\]<>]+/, "")
    .replace(/[;|&(){}[\]<>]+$/, "")
    .replace(/^[A-Za-z_][A-Za-z0-9_]*=/, "")
    // A dash-flag glued to a value (`--output=../x`, `-o../x`) executes with
    // the flag prefix stripped, so evaluate the value as the path candidate;
    // resolving the literal `--output=..` segment would hide the traversal.
    .replace(/^-{1,2}[A-Za-z0-9][\w-]*=/, "")
    .replace(/^-[A-Za-z](?=\.{1,2}\/)/, "")
  if (token.length === 0) return null
  if (token === "." || token === "..") return token
  if (token.startsWith("/") || token.startsWith("./") || token.startsWith("../")) return token
  if (token.includes("/")) return token
  return null
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
      ...(input.checkoutRunner === undefined
        ? {
            preparedWorktreeCacheRoot: join(input.state.paths.cache, "workspace-prepared-cache"),
            prebuiltBaselineCacheRoot: join(input.state.paths.cache, "workspace-prebuilt-baselines"),
          }
        : {}),
      refPrefix: "workspace.pylon.claude_agent_task",
      repositoryCacheRoot: join(input.state.paths.cache, "workspace-git-cache"),
      workspaceStateRoot: join(input.state.paths.cache, "workspace-leases"),
    })
    return {
      acceptanceResultRef: "git_checkout_verified",
      artifactSourceRef: materialized.sourceRef,
      instructions: [
        "You are working in a bounded public repository checkout; your",
        "current working directory IS that checkout — use relative paths,",
        "never assumed absolute paths.",
        `Task objective: ${input.task.objectiveSummary ?? "complete the referenced Autopilot task"}.`,
        "Only modify files inside this checkout.",
        `Run the verification command ref ${input.task.workspace.verificationCommand.commandRef} before finishing.`,
      ].join(" "),
      verificationArgs: input.task.workspace.verificationCommand.args,
      workspace: materialized.workingDirectory,
      workspaceRef: materialized.workspaceRef,
      workspaceStateRoot: join(input.state.paths.cache, "workspace-leases"),
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
    workspaceStateRoot: undefined,
  }
}

async function releaseClaudeAgentWorkspace(input: {
  materialized: Awaited<ReturnType<typeof materializeClaudeAgentWorkspace>>
  now: Date
}): Promise<{ resultRefs: string[] }> {
  if (input.materialized.workspaceStateRoot === undefined) return { resultRefs: [] }
  const result = await releaseWorkspace({
    now: input.now,
    workspaceRef: input.materialized.workspaceRef,
    workspaceStateRoot: input.materialized.workspaceStateRoot,
  }).catch(() => null)
  if (result?.cleanupReceiptRef !== undefined) {
    return {
      resultRefs: [
        "result.public.pylon.claude_agent_task.workspace_cleaned_on_closeout",
        result.cleanupReceiptRef,
      ],
    }
  }
  if (result?.retentionReasonRef !== undefined) {
    return {
      resultRefs: [
        "result.public.pylon.claude_agent_task.workspace_retained_on_closeout",
        result.retentionReasonRef,
      ],
    }
  }
  return { resultRefs: [] }
}

function claudeScmCredentialScanRoots(input: {
  account: ResolvedPylonAccountSelection | null | undefined
  materialized: Awaited<ReturnType<typeof materializeClaudeAgentWorkspace>>
}): WorkspaceScmCredentialScanRoot[] {
  return [
    { rootRef: input.materialized.workspaceRef, path: input.materialized.workspace },
    ...(input.account === null || input.account === undefined
      ? []
      : [
          // The selected worker home is an isolated Claude account home; it is
          // expected to hold that account's own Claude OAuth login. Still scan
          // it for long-lived SCM credentials (a stray GitHub PAT / forge git
          // token / credentialed git URL there would let the agent push
          // anywhere), but do not misclassify the account's own provider login
          // as a leak (issue #8583 — `sk-ant-oat01-…` tripped the OpenAI-key
          // pattern and refused every real-account run).
          {
            rootRef: input.account.accountRefHash,
            path: input.account.home,
            providerAuthHome: true,
          },
        ]),
  ]
}

async function enforceClaudeScmCredentialPolicy(input: {
  account: ResolvedPylonAccountSelection | null | undefined
  lease: ClaudeAgentLease
  materialized: Awaited<ReturnType<typeof materializeClaudeAgentWorkspace>>
  now: Date
  runRef: string
}) {
  try {
    await assertNoLongLivedScmCredentials({
      roots: claudeScmCredentialScanRoots({
        account: input.account,
        materialized: input.materialized,
      }),
    })
    return null
  } catch {
    await releaseClaudeAgentWorkspace({ materialized: input.materialized, now: input.now })
    return refusalRecord({
      lease: input.lease,
      runRef: input.runRef,
      blockerRefs: ["blocker.assignment.claude_agent_long_lived_scm_credentials_detected"],
      resultRef: "result.public.pylon.claude_agent_task.scm_credential_policy_failed",
      summaryRef: "summary.public.pylon.claude_agent_task.scm_credential_policy_failed",
      message: "Local Claude Agent session stopped because long-lived SCM credential material was detected in the bounded workspace or selected worker home.",
    })
  }
}

const finiteToken = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0

const firstFiniteToken = (...values: unknown[]): number => {
  for (const value of values) {
    const token = finiteToken(value)
    if (token > 0) return token
  }
  return 0
}

/**
 * Reads cumulative exact token usage from the Claude Agent SDK `result`
 * message. SDK builds have surfaced both Anthropic-native snake_case fields and
 * JS-friendly camelCase fields; accept both but still return null when no
 * positive usage is present so a missing/zero usage does not post a fabricated
 * token row.
 */
export function claudeUsageFrom(value: unknown): ClaudeAgentTurnUsage | null {
  if (value === null || typeof value !== "object") return null
  const usage = value as {
    cachedInputTokens?: unknown
    cacheCreationInputTokens?: unknown
    cacheReadInputTokens?: unknown
    input_tokens?: unknown
    inputTokens?: unknown
    output_tokens?: unknown
    outputTokens?: unknown
    cache_read_input_tokens?: unknown
    cache_creation_input_tokens?: unknown
  }
  const inputTokens = firstFiniteToken(usage.input_tokens, usage.inputTokens)
  const outputTokens = firstFiniteToken(usage.output_tokens, usage.outputTokens)
  const cacheReadTokens = firstFiniteToken(usage.cache_read_input_tokens, usage.cacheReadInputTokens)
  const cacheCreationTokens = firstFiniteToken(
    usage.cache_creation_input_tokens,
    usage.cacheCreationInputTokens,
  )
  const cachedInputTokens =
    cacheReadTokens + cacheCreationTokens > 0
      ? cacheReadTokens + cacheCreationTokens
      : firstFiniteToken(usage.cachedInputTokens)
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
  let escapeDenials = 0
  let editedFileCount = 0
  let commandCount = 0
  let turnCount = 0
  let sessionId: string | null = null
  let resultSubtype: string | null = null
  let usage: ClaudeAgentTurnUsage | null = null

  const guard = async (hookInput: unknown) => {
    const record = hookInput as { tool_name?: string; tool_input?: unknown }
    if (toolInputEscapesWorkspace(record.tool_name, record.tool_input, input.cwd)) {
      escapeDenials += 1
      if (Bun.env.PYLON_CLAUDE_GUARD_DEBUG === "1") {
        // Owner-local diagnostic only; tool_input may carry local paths so it
        // must never reach public projections or closeout refs.
        console.error(
          `[claude-guard] denied ${record.tool_name ?? "unknown"} (${escapeDenials}/${WORKSPACE_ESCAPE_DENIAL_BUDGET}):`,
          JSON.stringify(record.tool_input)?.slice(0, 400),
        )
      }
      // Every offending call is denied (containment), but a model that
      // guesses a wrong absolute path (seen live: Read /root/task/sum.ts in
      // the fixture, issue #7914) gets the denial reason back and can
      // recover with the correct cwd-relative path. Only repeated offenses
      // abort the run.
      if (escapeDenials >= WORKSPACE_ESCAPE_DENIAL_BUDGET) {
        escaped = true
        abort.abort()
      }
      return {
        hookSpecificOutput: {
          hookEventName: "PreToolUse" as const,
          permissionDecision: "deny" as const,
          permissionDecisionReason:
            "claude_agent.workspace_boundary: path is outside the assignment workspace; use paths relative to the working directory",
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

type ClaudeTokenUsageReportDiagnostic = {
  blockerRefs: string[]
  proofRefs: string[]
  resultRefs: string[]
  summaryRefs: string[]
}

type ClaudeTokenUsageReportState =
  | "reported"
  | "missing"
  | "report_failed"
  | "reporter_unconfigured"

function claudeTokenUsageReportDiagnostic(
  state: ClaudeTokenUsageReportState,
  seed: string,
): ClaudeTokenUsageReportDiagnostic {
  const resultRef = `result.public.pylon.claude_agent_task.token_usage_${state}`
  const summaryRef = `summary.public.pylon.claude_agent_task.token_usage_${state}`
  return {
    blockerRefs:
      state === "reported"
        ? []
        : [`blocker.assignment.claude_agent_token_usage_${state}`],
    proofRefs: [stableRef(`proof.pylon.claude_agent_task.token_usage_${state}`, seed)],
    resultRefs: [resultRef],
    summaryRefs: [summaryRef],
  }
}

async function reportClaudeAgentTurnUsage(input: {
  lease: ClaudeAgentLease
  materialized: Awaited<ReturnType<typeof materializeClaudeAgentWorkspace>>
  now: Date
  options: ClaudeAgentExecutionOptions
  run: ClaudeAgentRunResult
  runRef: string
  state: PylonLocalState
}): Promise<ClaudeTokenUsageReportDiagnostic> {
  const seed = [
    input.lease.assignmentRef,
    input.lease.leaseRef,
    input.runRef,
    input.run.sessionRef ?? "session.pending",
    input.materialized.workspaceRef,
  ].join(":")
  if (input.run.usage === null) {
    return claudeTokenUsageReportDiagnostic("missing", seed)
  }

  const reporter =
    input.options.claudeTurnReporter ??
    createPylonClaudeTurnReporter({
      ...(input.options.agentToken === undefined ? {} : { agentToken: input.options.agentToken }),
      ...(input.options.baseUrl === undefined ? {} : { baseUrl: input.options.baseUrl }),
      ...(input.options.fetch === undefined ? {} : { fetch: input.options.fetch }),
    })
  if (reporter === undefined) {
    return claudeTokenUsageReportDiagnostic("reporter_unconfigured", seed)
  }

  try {
    await reporter({
      assignmentRef: input.lease.assignmentRef,
      leaseRef: input.lease.leaseRef,
      pylonRef: input.state.identity.pylonRef,
      runRef: input.runRef,
      ...(input.run.sessionRef === null ? {} : { sessionRef: input.run.sessionRef }),
      workspaceRef: input.materialized.workspaceRef,
      turnIndex: 1,
      observedAt: input.now.toISOString(),
      usage: {
        inputTokens: input.run.usage.inputTokens,
        cachedInputTokens: input.run.usage.cachedInputTokens,
        outputTokens: input.run.usage.outputTokens,
      },
    })
    return claudeTokenUsageReportDiagnostic("reported", seed)
  } catch {
    return claudeTokenUsageReportDiagnostic("report_failed", seed)
  }
}

function refusalRecord(input: {
  lease: ClaudeAgentLease
  runRef: string
  blockerRefs: string[]
  proofRefs?: string[]
  resultRef: string
  resultRefs?: string[]
  summaryRef: string
  summaryRefs?: string[]
  message: string
}) {
  const failureRef = stableRef(
    "proof.pylon.claude_agent_task.refused",
    `${input.lease.leaseRef}:${input.blockerRefs.join(",")}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.claude_agent_task.refused",
    `${input.lease.leaseRef}:${input.resultRef}:${input.summaryRef}`,
  )
  return {
    artifactRefs: [artifactRef],
    blockerRefs: input.blockerRefs,
    buildRefs: [input.runRef],
    message: input.message,
    previewRefs: [],
    proofRefs: [failureRef, ...(input.proofRefs ?? [])],
    resultRefs: [input.resultRef, ...(input.resultRefs ?? [])],
    runRefs: [input.runRef],
    status: "rejected" as const,
    summaryRefs: [input.summaryRef, ...(input.summaryRefs ?? [])],
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
  } catch (error) {
    const checkoutReasonRef = workspaceCheckoutFailureReasonRef(error)
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.claude_agent_workspace_checkout_failed",
        ...(checkoutReasonRef === null ? [] : [checkoutReasonRef]),
      ],
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
    await releaseClaudeAgentWorkspace({ materialized, now })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: ["blocker.assignment.claude_agent_execution_refused"],
      resultRef: "result.public.pylon.claude_agent_task.execution_refused",
      summaryRef: "summary.public.pylon.claude_agent_task.execution_refused",
      message: "Local Claude Agent session refused with a typed execution error.",
    })
  }

  const scmCredentialPolicyRefusal = await enforceClaudeScmCredentialPolicy({
    account: options.account,
    lease,
    materialized,
    now,
    runRef,
  })
  if (scmCredentialPolicyRefusal !== null) return scmCredentialPolicyRefusal

  const tokenUsageReport = await reportClaudeAgentTurnUsage({
    lease,
    materialized,
    now,
    options,
    run,
    runRef,
    state,
  })

  if (run.outcome === "workspace_escape_blocked") {
    await releaseClaudeAgentWorkspace({ materialized, now })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.claude_agent_workspace_escape_blocked",
        ...tokenUsageReport.blockerRefs,
      ],
      proofRefs: tokenUsageReport.proofRefs,
      resultRef: "result.public.pylon.claude_agent_task.workspace_escape_blocked",
      resultRefs: tokenUsageReport.resultRefs,
      summaryRef: "summary.public.pylon.claude_agent_task.workspace_escape_blocked",
      summaryRefs: tokenUsageReport.summaryRefs,
      message: "Local Claude Agent session was stopped: a tool call targeted paths outside the bounded workspace.",
    })
  }
  if (run.outcome === "budget_exceeded") {
    await releaseClaudeAgentWorkspace({ materialized, now })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.claude_agent_budget_exceeded",
        ...tokenUsageReport.blockerRefs,
      ],
      proofRefs: tokenUsageReport.proofRefs,
      resultRef: "result.public.pylon.claude_agent_task.budget_exceeded",
      resultRefs: tokenUsageReport.resultRefs,
      summaryRef: "summary.public.pylon.claude_agent_task.budget_exceeded",
      summaryRefs: tokenUsageReport.summaryRefs,
      message: "Local Claude Agent session exceeded its turn or wall-clock budget before completing the task.",
    })
  }
  if (run.outcome === "refused") {
    await releaseClaudeAgentWorkspace({ materialized, now })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.claude_agent_execution_refused",
        ...tokenUsageReport.blockerRefs,
      ],
      proofRefs: tokenUsageReport.proofRefs,
      resultRef: "result.public.pylon.claude_agent_task.execution_refused",
      resultRefs: tokenUsageReport.resultRefs,
      summaryRef: "summary.public.pylon.claude_agent_task.execution_refused",
      summaryRefs: tokenUsageReport.summaryRefs,
      message: "Local Claude Agent session ended with an execution error before completing the task.",
    })
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
  const workspaceCleanup = await releaseClaudeAgentWorkspace({ materialized, now })

  return {
    artifactRefs: [artifactRef],
    blockerRefs: [
      ...(passed ? [] : ["blocker.assignment.claude_agent_test_failed"]),
      ...tokenUsageReport.blockerRefs,
    ],
    buildRefs: [commandRef],
    message: passed
      ? `Local Claude Agent completed the bounded coding task: ${run.editedFileCount} file edit(s), ${run.commandCount} command(s), ${run.turnCount} turn(s), verification test passed on this device.`
      : "Local Claude Agent session completed but the verification test command failed; the change is not accepted.",
    previewRefs: [materialized.workspaceRef],
    proofRefs: [proofRef, ...tokenUsageReport.proofRefs],
    resultRefs: [
      passed
        ? `result.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_passed`
        : `result.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_failed`,
      `result.public.pylon.claude_agent_task.edited_files.${run.editedFileCount}`,
      ...tokenUsageReport.resultRefs,
      ...workspaceCleanup.resultRefs,
    ],
    runRefs: [runRef, ...sessionRefs],
    status: passed ? ("accepted" as const) : ("rejected" as const),
    summaryRefs: [
      passed
        ? `summary.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_passed`
        : `summary.public.pylon.claude_agent_task.${materialized.acceptanceResultRef}_failed`,
      ...tokenUsageReport.summaryRefs,
    ],
    testRefs: [commandRef],
  }
}
