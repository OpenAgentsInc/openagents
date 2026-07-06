import { access, lstat, mkdir, readFile, readlink, rename, rm, symlink, writeFile } from "node:fs/promises"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"
import { createHash, randomUUID } from "node:crypto"
import { Effect } from "effect"
import {
  CODEX_AGENT_SDK_PACKAGE,
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  type CodexAgentProbeOptions,
  type CodexAgentSandboxMode,
} from "./codex-agent.js"
import {
  assertNoLongLivedScmCredentials,
  cleanupOldestMaterializedWorkspaces,
  gitCheckoutWorkspaceFrom,
  materializeGitCheckoutWorkspaceWithLease,
  pruneWorkspaceCacheDirectories,
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
  PYLON_CODEX_CUSTODY_ASSIGNMENT_BLOCKER_REF,
  reprimePylonCodexAccountAuthFromCustody,
} from "./codex-custody-reprime.js"
import type { PylonCodexAuthValidityProbe } from "./account-connect.js"
import { probeAndRecordCodexAccountAuthHealth } from "./codex-account-auth-health.js"
import {
  publishAssignmentPullRequest,
  type AssignmentPrTitleBody,
  type AssignmentPrTitleBodyGenerator,
  type AssignmentPullRequestPublisher,
} from "./codex-pr-publisher.js"
import { runCodexComposerStream } from "./codex-composer.js"
import type { CodexAgentConfig } from "./codex-agent.js"
import { scopedTimeout } from "./effect-runtime-patterns.js"
import {
  createPylonCodexEventChunkReporter,
  createPylonCodexTurnReporter,
  type CodexEventChunkReporter,
  type CodexTurnReportItem,
  type CodexTurnReporter,
  type CodexTurnUsage,
} from "./codex-turn-reporter.js"
import type { PylonLocalState } from "./state.js"
import { installCodexRipgrepGuard } from "./codex-rg-guard.js"
import { classifyQuotaSignal } from "./account-quota.js"
import { recordQuotaBlock } from "./account-quota-ledger.js"
import {
  inferCodexQuotaBlockKindFromProviderSnapshots,
  loadAccountUsageStore,
} from "./account-usage.js"
import {
  classifyCodexAccountFailure,
  codexAccountFailureBlockerRefs,
  type PylonCodexAccountFailure,
} from "./codex-account-health.js"
import { recordCodexAccountHealthFailure } from "./codex-account-health-ledger.js"
import {
  claudeSecondPassVerdictRef,
  runClaudeSecondPassReview,
  type ClaudeSecondPassReviewOptions,
} from "./claude-second-pass-reviewer.js"

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
  eventChunkReporter?: CodexEventChunkReporter
  eventReporter?: CodexTurnReporter
  networkAccessEnabled: boolean
  progressTokenEstimates?: boolean
  sandboxMode: CodexAgentRuntimeSandboxMode
  timeoutMs: number
  model?: string
  onProgress?: (progress: CodexAgentRuntimeProgress) => void | Promise<void>
}

export type CodexAgentRuntimePhase =
  | "materializing"
  | "installing"
  | "running"
  | "testing"
  | "diff"
  | "proof"

export type CodexAgentRuntimeProgress = {
  phase: CodexAgentRuntimePhase
  tokensSoFar?: number
  tokenCountKind?: "exact" | "estimated"
  lastProgressEvent?: string
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
  refusal?: PylonCodexAccountFailure
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
  codexAuthValidityProbe?: PylonCodexAuthValidityProbe
  codexEventChunkReporter?: CodexEventChunkReporter
  codexTurnReporter?: CodexTurnReporter
  dependencyInstaller?: LocalCommandRunner
  fetch?: typeof fetch
  /**
   * Test/override seam for the assignment pull-request publisher (#6439).
   * Production uses the default real publisher, which commits the verified
   * diff, pushes a scoped branch, and opens one PR against the base branch.
   */
  pullRequestPublisher?: AssignmentPullRequestPublisher
  claudeSecondPassReview?: ClaudeSecondPassReviewOptions
  onProgress?: (progress: CodexAgentRuntimeProgress) => void | Promise<void>
}

type CodexAgentFixture = {
  files: Record<string, string>
  instructions: string
  verificationArgs: string[]
}

const DEFAULT_TIMEOUT_SECONDS = 300
const MAX_TIMEOUT_SECONDS = 2400
const CODEX_AGENT_WORKSPACE_CACHE_MAX_ENTRIES = 200
const CODEX_AGENT_TASK_MAX_MATERIALIZED_WORKSPACES = 64
const CODEX_AGENT_TASK_CACHE_PRUNE_MIN_AGE_SECONDS = MAX_TIMEOUT_SECONDS + 300

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

type PackageManifest = {
  dependencies?: Record<string, unknown>
  devDependencies?: Record<string, unknown>
  scripts?: Record<string, string>
  workspaces?: unknown
}

async function readPackageManifest(directory: string): Promise<PackageManifest | null> {
  try {
    const parsed = JSON.parse(await readFile(join(directory, "package.json"), "utf8")) as unknown
    return parsed !== null && typeof parsed === "object" ? (parsed as PackageManifest) : null
  } catch {
    return null
  }
}

function verificationCwdFromArgs(workspace: string, args: string[]): string | null {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--cwd" || arg === "-C") {
      const value = args[index + 1]
      if (typeof value !== "string" || value.length === 0) return null
      const resolved = resolve(workspace, value)
      const workspaceRoot = resolve(workspace)
      return resolved === workspaceRoot || resolved.startsWith(`${workspaceRoot}/`) ? resolved : null
    }
    if (arg.startsWith("--cwd=")) {
      const resolved = resolve(workspace, arg.slice("--cwd=".length))
      const workspaceRoot = resolve(workspace)
      return resolved === workspaceRoot || resolved.startsWith(`${workspaceRoot}/`) ? resolved : null
    }
  }
  return null
}

function bunScriptNameFromArgs(args: string[]): string | null {
  if (args[0] !== "bun") return null
  for (let index = 1; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--cwd" || arg === "-C") {
      index += 1
      continue
    }
    if (arg.startsWith("--cwd=") || arg.startsWith("-")) continue
    if (arg === "run") continue
    return arg
  }
  return null
}

function packageProvidesVerifierTool(
  manifest: PackageManifest,
  verificationArgs: string[],
  scriptCommand: string | undefined,
): boolean {
  const devDependencies = manifest.devDependencies ?? {}
  const evidence = `${verificationArgs.join(" ")} ${scriptCommand ?? ""}`
  if (/\b(test|vitest)\b/.test(evidence) && Object.hasOwn(devDependencies, "vitest")) return true
  if (/\b(typecheck|tsc)\b/.test(evidence) && Object.hasOwn(devDependencies, "typescript")) return true
  if (/\b(deploy|wrangler)\b/.test(evidence) && Object.hasOwn(devDependencies, "wrangler")) return true
  if (
    /\bplaywright\b/.test(evidence) &&
    (Object.hasOwn(devDependencies, "playwright") ||
      Object.hasOwn(devDependencies, "@playwright/test"))
  ) {
    return true
  }
  return false
}

async function dependencyInstallDirectories(input: {
  verificationArgs: string[]
  workspace: string
}): Promise<string[]> {
  const workspaceRoot = resolve(input.workspace)
  const dirs = [workspaceRoot]
  const verifierCwd = verificationCwdFromArgs(workspaceRoot, input.verificationArgs)
  if (verifierCwd === null || verifierCwd === workspaceRoot) return dirs

  const scriptName = bunScriptNameFromArgs(input.verificationArgs)
  const verifierManifest = await readPackageManifest(verifierCwd)
  const scriptCommand =
    scriptName === null ? undefined : verifierManifest?.scripts?.[scriptName]
  const candidates: string[] = []
  let current = verifierCwd
  while (current !== workspaceRoot && current.startsWith(`${workspaceRoot}/`)) {
    candidates.push(current)
    current = dirname(current)
  }
  for (const candidate of candidates.reverse()) {
    const manifest = await readPackageManifest(candidate)
    if (manifest === null) continue
    if (
      manifest.workspaces !== undefined ||
      packageProvidesVerifierTool(manifest, input.verificationArgs, scriptCommand)
    ) {
      dirs.push(candidate)
    }
  }
  return [...new Set(dirs)]
}

/**
 * Shared node_modules cache across Codex worktrees (concurrency fix).
 *
 * A git worktree shares the bare object store but never shares
 * `node_modules`, so N concurrent `codex_agent_task` assignments each ran a
 * fresh `bun install` and thrashed disk/CPU, serializing fleet startup. The
 * helpers below let the FIRST task install once and every subsequent task with
 * a matching lockfile symlink the prebuilt `node_modules` in instantly.
 *
 * Correctness rule: reuse is keyed by the SHA-256 of the workspace `bun.lock`,
 * so stale dependency trees are never shared. Any miss/mismatch falls back to a
 * fresh `bun install`. Sharing is a read-only symlink; the one-time cache
 * populate is guarded by an atomic mkdir lock so concurrent first-runs cannot
 * corrupt the shared entry.
 */
async function workspaceLockfileHash(workspaceRoot: string): Promise<string | null> {
  for (const name of ["bun.lock", "bun.lockb"]) {
    try {
      const bytes = await readFile(join(workspaceRoot, name))
      return createHash("sha256").update(bytes).digest("hex").slice(0, 24)
    } catch {
      // Try the next candidate lockfile name.
    }
  }
  return null
}

/**
 * Filesystem-safe per-install-directory key. The workspace root and each
 * hoisted/per-package install directory get a distinct cached `node_modules`.
 */
function sharedInstallDirectoryKey(workspaceRoot: string, directory: string): string {
  const rel = relative(workspaceRoot, directory)
  if (rel.length === 0) return "__root__"
  return `dir-${createHash("sha256").update(rel).digest("hex").slice(0, 24)}`
}

/**
 * Symlinks a shared, lockfile-matched `node_modules` into a worktree install
 * directory. Clears a stale/broken symlink first but never clobbers a real
 * directory. Returns true only when the worktree now points at the shared tree.
 */
async function linkSharedNodeModules(cacheEntryDir: string, targetNodeModules: string): Promise<boolean> {
  try {
    const existing = await lstat(targetNodeModules).catch(() => null)
    if (existing !== null) {
      if (!existing.isSymbolicLink()) return false
      await rm(targetNodeModules, { force: true })
    }
    await symlink(cacheEntryDir, targetNodeModules, "dir")
    return true
  } catch {
    return false
  }
}

type SharedCachePopulation = "linked" | "exists" | "skipped"

/**
 * Moves a freshly installed `node_modules` into the shared cache and symlinks
 * it back into the worktree, guarded by an atomic mkdir lock so concurrent
 * first-runs cannot collide. Best-effort: any failure leaves the worktree's
 * real `node_modules` untouched and returns "skipped".
 */
async function populateSharedNodeModules(input: {
  cacheEntryDir: string
  lockDir: string
  sourceNodeModules: string
  targetNodeModules: string
}): Promise<SharedCachePopulation> {
  try {
    await mkdir(dirname(input.lockDir), { recursive: true })
  } catch {
    return "skipped"
  }
  try {
    await mkdir(input.lockDir)
  } catch {
    // Another worktree is populating this exact lockfile/dir entry right now.
    return "skipped"
  }
  try {
    if (await pathExists(input.cacheEntryDir)) return "exists"
    const tmp = `${input.cacheEntryDir}.tmp-${randomUUID()}`
    await rm(tmp, { force: true, recursive: true })
    await rename(input.sourceNodeModules, tmp)
    await rename(tmp, input.cacheEntryDir)
    await linkSharedNodeModules(input.cacheEntryDir, input.targetNodeModules)
    return "linked"
  } catch {
    return "skipped"
  } finally {
    await rm(input.lockDir, { force: true, recursive: true }).catch(() => {})
  }
}

export async function prepareWorkspaceDependencies(input: {
  installer?: LocalCommandRunner
  sharedCacheRoot?: string
  verificationArgs: string[]
  workspace: string
}): Promise<DependencyPreparation> {
  const workspaceRoot = resolve(input.workspace)
  const hasPackageJson = await pathExists(join(input.workspace, "package.json"))
  const hasBunLock =
    (await pathExists(join(input.workspace, "bun.lock"))) ||
    (await pathExists(join(input.workspace, "bun.lockb")))
  if (!hasPackageJson || !hasBunLock) {
    return { ok: true, prepared: false }
  }

  const installDirectories = await dependencyInstallDirectories({
    verificationArgs: input.verificationArgs,
    workspace: input.workspace,
  })
  const missingInstallDirectories: string[] = []
  for (const directory of installDirectories) {
    if (!(await pathExists(join(directory, "node_modules")))) {
      missingInstallDirectories.push(directory)
    }
  }
  if (missingInstallDirectories.length === 0) {
    return {
      ok: true,
      prepared: false,
      receiptRef: "dependency.pylon.codex_agent_task.node_modules_present",
    }
  }

  // Lockfile-keyed shared cache root: only reuse when bun.lock matches exactly.
  const lockHash = await workspaceLockfileHash(workspaceRoot)
  const sharedCacheRoot =
    input.sharedCacheRoot !== undefined && lockHash !== null
      ? join(input.sharedCacheRoot, lockHash)
      : null

  const installer = input.installer ?? runCommand
  const installReceipts: string[] = []
  let installedDirectoryCount = 0
  for (const directory of missingInstallDirectories) {
    const sharedKey = sharedInstallDirectoryKey(workspaceRoot, directory)
    const sharedEntryDir =
      sharedCacheRoot === null ? null : join(sharedCacheRoot, sharedKey, "node_modules")
    const targetNodeModules = join(directory, "node_modules")

    // Cache hit: symlink the shared, lockfile-matched node_modules and skip install.
    if (sharedEntryDir !== null && (await pathExists(sharedEntryDir))) {
      if (await linkSharedNodeModules(sharedEntryDir, targetNodeModules)) {
        installReceipts.push(
          stableRef(
            "dependency.pylon.codex_agent_task.node_modules_shared_reuse",
            `${relative(workspaceRoot, directory) || "."}:${lockHash}`,
          ),
        )
        continue
      }
    }

    // Cache miss (or mismatch/missing cache): install into this worktree directory.
    const install = await installer({
      args: ["bun", "install", "--no-save", "--ignore-scripts"],
      cwd: directory,
      timeoutMs: 5 * 60 * 1000,
    })
    const receiptRef = stableRef(
      "dependency.pylon.codex_agent_task.bun_install",
      `${directory}:${install.exitCode}:${install.timedOut}:${install.stdoutBytes}:${install.stderrBytes}`,
    )
    installReceipts.push(receiptRef)
    if (install.exitCode !== 0 || install.timedOut) {
      return { ok: false, receiptRef }
    }
    installedDirectoryCount += 1

    // Populate the shared cache so the next concurrent/later task reuses it.
    if (sharedCacheRoot !== null && sharedEntryDir !== null && (await pathExists(targetNodeModules))) {
      await populateSharedNodeModules({
        cacheEntryDir: sharedEntryDir,
        lockDir: join(sharedCacheRoot, sharedKey, ".populate.lock"),
        sourceNodeModules: targetNodeModules,
        targetNodeModules,
      })
    }
  }

  // Only a real `bun install` can dirty tracked files (package.json/bun.lock);
  // a pure symlink-reuse pass leaves the checkout pristine, so skip the restore.
  if (installedDirectoryCount === 0) {
    return {
      ok: true,
      prepared: true,
      receiptRef: stableRef(
        "dependency.pylon.codex_agent_task.node_modules_shared_reuse",
        installReceipts.join(":"),
      ),
    }
  }
  const restore = await installer({
    args: ["git", "restore", "--source=HEAD", "--staged", "--worktree", "."],
    cwd: input.workspace,
    timeoutMs: 30 * 1000,
  })
  if (restore.exitCode !== 0 || restore.timedOut) {
    return {
      ok: false,
      receiptRef: stableRef(
        "dependency.pylon.codex_agent_task.workspace_restore",
        `${restore.exitCode}:${restore.timedOut}:${restore.stdoutBytes}:${restore.stderrBytes}`,
      ),
    }
  }
  return {
    ok: true,
    prepared: true,
    receiptRef: stableRef(
      "dependency.pylon.codex_agent_task.bun_install",
      installReceipts.join(":"),
    ),
  }
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

function totalTokensFromUsage(usage: CodexTurnUsage | undefined): number | undefined {
  if (usage === undefined) return undefined
  return usage.inputTokens + usage.outputTokens + (usage.reasoningOutputTokens ?? 0)
}

const ESTIMATED_CHARS_PER_TOKEN = 4

function textTokenEstimate(text: string): number {
  const trimmed = text.replace(/\s+/g, " ").trim()
  if (trimmed.length === 0) return 0
  return Math.max(1, Math.ceil(trimmed.length / ESTIMATED_CHARS_PER_TOKEN))
}

function usageRecordFromUnknown(value: unknown): Record<string, unknown> | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (record.total_token_usage !== null && typeof record.total_token_usage === "object") {
    return record.total_token_usage as Record<string, unknown>
  }
  if (record.last_token_usage !== null && typeof record.last_token_usage === "object") {
    return record.last_token_usage as Record<string, unknown>
  }
  if (
    typeof record.input_tokens === "number" ||
    typeof record.output_tokens === "number" ||
    typeof record.reasoning_output_tokens === "number"
  ) {
    return record
  }
  return null
}

function tokenField(record: Record<string, unknown>, key: string): number {
  const value = record[key]
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0
}

export function liveTokenTotalFromCodexRawEvent(raw: unknown): number | undefined {
  if (raw === null || typeof raw !== "object") return undefined
  const event = raw as Record<string, unknown>
  const payload = event.payload !== null && typeof event.payload === "object"
    ? event.payload as Record<string, unknown>
    : null
  const info = payload?.info ?? event.info
  const usage = usageRecordFromUnknown(info)
  if (usage === null) return undefined
  const total =
    tokenField(usage, "input_tokens") +
    tokenField(usage, "output_tokens") +
    tokenField(usage, "reasoning_output_tokens")
  return total > 0 ? total : undefined
}

function textFragmentsFromCodexRawEvent(raw: unknown): string[] {
  if (raw === null || typeof raw !== "object") return []
  const event = raw as Record<string, unknown>
  const payload = event.payload !== null && typeof event.payload === "object"
    ? event.payload as Record<string, unknown>
    : null
  const item = event.item !== null && typeof event.item === "object"
    ? event.item as Record<string, unknown>
    : null
  const records = [payload, item].filter((record): record is Record<string, unknown> => record !== null)
  const fragments: string[] = []

  for (const record of records) {
    for (const key of ["message", "last_agent_message", "text", "aggregated_output"]) {
      const value = record[key]
      if (typeof value === "string") fragments.push(value)
    }
    const summary = record.summary
    if (typeof summary === "string") fragments.push(summary)
    if (Array.isArray(summary)) {
      for (const entry of summary) {
        if (typeof entry === "string") fragments.push(entry)
        if (entry !== null && typeof entry === "object") {
          const text = (entry as Record<string, unknown>).text
          if (typeof text === "string") fragments.push(text)
        }
      }
    }
    const content = record.content
    if (Array.isArray(content)) {
      for (const block of content) {
        if (block !== null && typeof block === "object") {
          const text = (block as Record<string, unknown>).text
          if (typeof text === "string") fragments.push(text)
        }
      }
    }
  }

  return fragments
}

export function estimatedTokenDeltaFromCodexRawEvent(raw: unknown): number {
  return textFragmentsFromCodexRawEvent(raw)
    .reduce((sum, text) => sum + textTokenEstimate(text), 0)
}

async function emitCodexProgress(
  reporter: CodexAgentRunInput["onProgress"],
  progress: CodexAgentRuntimeProgress,
) {
  if (reporter === undefined) return
  await Promise.resolve(reporter(progress)).catch(() => undefined)
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
    roleRef: "coder",
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

async function reportCodexEventChunk(input: {
  eventChunkReporter: CodexEventChunkReporter | undefined
  runInput: CodexAgentRunInput
  sessionRef: string | undefined
  turnIndex: number
  chunkIndex: number
  items?: ReadonlyArray<CodexTurnReportItem>
  rawEvents: ReadonlyArray<RawCodexThreadEvent>
}) {
  if (
    input.eventChunkReporter === undefined ||
    input.runInput.assignmentRef === undefined ||
    input.runInput.leaseRef === undefined ||
    input.runInput.pylonRef === undefined ||
    input.rawEvents.length === 0
  ) {
    return true
  }
  try {
    await input.eventChunkReporter({
      assignmentRef: input.runInput.assignmentRef,
      leaseRef: input.runInput.leaseRef,
      pylonRef: input.runInput.pylonRef,
      ...(input.runInput.runRef === undefined ? {} : { runRef: input.runInput.runRef }),
      ...(input.sessionRef === undefined ? {} : { sessionRef: input.sessionRef }),
      ...(input.runInput.workspaceRef === undefined ? {} : { workspaceRef: input.runInput.workspaceRef }),
      turnIndex: input.turnIndex,
      chunkIndex: input.chunkIndex,
      observedAt: new Date().toISOString(),
      rawEvents: input.rawEvents,
      ...(input.items === undefined || input.items.length === 0 ? {} : { items: input.items }),
    })
    return true
  } catch {
    return false
  }
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
  let tokensSoFar = 0
  let liveExactTokensSoFar = 0
  let liveEstimatedTokensSoFar = input.progressTokenEstimates === true
    ? textTokenEstimate(input.instructions)
    : 0
  let lastTokenProgressReportAtMs = 0
  let lastTokenProgressReportValue = 0
  let itemOrdinal = 0
  let currentTurnChunkIndex = 0
  let currentTurnItems: Array<CodexTurnReportItem> = []
  let currentRawEvents: Array<RawCodexThreadEvent> = []
  let pendingRawEvents: Array<RawCodexThreadEvent> = []
  let pendingChunkItems: Array<CodexTurnReportItem> = []
  let pendingChunkRawEvents: Array<RawCodexThreadEvent> = []
  let failure: PylonCodexAccountFailure | undefined

  const flushEventChunk = async (turnIndex: number) => {
    if (pendingChunkRawEvents.length === 0) return
    const nextChunkIndex = currentTurnChunkIndex + 1
    const sessionRef =
      threadId === null ? undefined : stableRef("session.pylon.codex_agent", threadId)
    const reported = await reportCodexEventChunk({
      chunkIndex: nextChunkIndex,
      eventChunkReporter: input.eventChunkReporter,
      items: pendingChunkItems,
      rawEvents: pendingChunkRawEvents,
      runInput: input,
      sessionRef,
      turnIndex,
    })
    if (!reported) return
    currentTurnChunkIndex = nextChunkIndex
    pendingChunkItems = []
    pendingChunkRawEvents = []
  }

  const bestLiveTokensSoFar = () =>
    Math.max(tokensSoFar, liveExactTokensSoFar, liveEstimatedTokensSoFar)

  const liveTokenCountKind = (): "exact" | "estimated" =>
    Math.max(tokensSoFar, liveExactTokensSoFar) >= liveEstimatedTokensSoFar
      ? "exact"
      : "estimated"

  const maybeEmitLiveTokenProgress = async (lastProgressEvent: string | undefined) => {
    const nextTokensSoFar = bestLiveTokensSoFar()
    if (nextTokensSoFar <= 0) return
    const nowMs = Date.now()
    const enoughTime = nowMs - lastTokenProgressReportAtMs >= 5_000
    const enoughTokens = Math.abs(nextTokensSoFar - lastTokenProgressReportValue) >= 500
    if (!enoughTime && !enoughTokens) return
    lastTokenProgressReportAtMs = nowMs
    lastTokenProgressReportValue = nextTokensSoFar
    await emitCodexProgress(input.onProgress, {
      phase: "running",
      tokensSoFar: nextTokensSoFar,
      tokenCountKind: liveTokenCountKind(),
      ...(lastProgressEvent === undefined ? {} : { lastProgressEvent }),
    })
  }

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
      if (rawEvent !== undefined) {
        pendingChunkRawEvents.push(rawEvent)
        const exactTotal = liveTokenTotalFromCodexRawEvent(rawEvent)
        if (exactTotal !== undefined) {
          liveExactTokensSoFar = Math.max(liveExactTokensSoFar, exactTotal)
        } else if (input.progressTokenEstimates === true) {
          liveEstimatedTokensSoFar += estimatedTokenDeltaFromCodexRawEvent(rawEvent)
        }
        await maybeEmitLiveTokenProgress(event.type)
      }
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
        currentTurnChunkIndex = 0
        currentTurnItems = []
        currentRawEvents = rawEvent === undefined ? pendingRawEvents : [...pendingRawEvents, rawEvent]
        pendingRawEvents = []
        await flushEventChunk(activeTurnIndex)
        await emitCodexProgress(input.onProgress, {
          phase: "running",
          lastProgressEvent: "turn.started",
        })
      }
      if (event.type === "turn.completed") {
        const completedTurnIndex = activeTurnIndex > 0 ? activeTurnIndex : turnCount + 1
        const completedRawEvents = activeTurnIndex > 0 ? currentRawEvents : pendingRawEvents
        await flushEventChunk(completedTurnIndex)
        turnCount += 1
        const sessionRef =
          threadId === null ? undefined : stableRef("session.pylon.codex_agent", threadId)
        const usage = usageFromTurnCompleted(event)
        await reportCodexTurn({
          eventReporter: input.eventReporter,
          items: currentTurnItems,
          rawEvents: completedRawEvents,
          runInput: input,
          sessionRef,
          turnIndex: completedTurnIndex,
          usage,
        })
        tokensSoFar += totalTokensFromUsage(usage) ?? 0
        liveExactTokensSoFar = Math.max(liveExactTokensSoFar, tokensSoFar)
        await emitCodexProgress(input.onProgress, {
          phase: "running",
          tokensSoFar,
          tokenCountKind: "exact",
          lastProgressEvent: "turn.completed",
        })
        currentTurnItems = []
        currentRawEvents = []
        pendingRawEvents = []
        pendingChunkItems = []
        pendingChunkRawEvents = []
        activeTurnIndex = 0
      }
      if (event.type === "turn.failed" || event.type === "error") {
        failed = true
        failure = classifyCodexAccountFailure(event.error?.message ?? event.item?.text ?? event.type)
      }
      if (event.type === "item.completed" && event.item?.type === "command_execution") {
        commandCount += 1
        await emitCodexProgress(input.onProgress, {
          phase: "testing",
          lastProgressEvent: "command_execution.completed",
        })
      }
      if (event.type === "item.completed") {
        const projected = projectCodexItem(event.item, itemOrdinal + 1)
        if (projected !== undefined) {
          itemOrdinal += 1
          currentTurnItems.push(projected)
          pendingChunkItems.push(projected)
          await flushEventChunk(activeTurnIndex > 0 ? activeTurnIndex : turnCount + 1)
        }
      }
      if (event.type === "item.completed" && event.item?.type === "file_change") {
        const changes = Array.isArray(event.item.changes) ? event.item.changes : []
        editedFileCount += changes.length
        await emitCodexProgress(input.onProgress, {
          phase: "diff",
          lastProgressEvent: "file_change.completed",
        })
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
    return {
      outcome: "refused",
      turnCount,
      editedFileCount,
      commandCount,
      sessionRef,
      ...(failure === undefined ? {} : { refusal: failure }),
    }
  }
  return { outcome: "completed", turnCount, editedFileCount, commandCount, sessionRef }
}

async function materializeCodexAgentWorkspace(input: {
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
  state: PylonLocalState
  task: CodexAgentTaskPayload
}) {
  const cacheRoot = join(input.state.paths.cache, "codex-agent-tasks")
  await pruneWorkspaceCacheDirectories({
    cacheRoot,
    maxEntries: CODEX_AGENT_WORKSPACE_CACHE_MAX_ENTRIES,
  })
  const workspaceStateRoot = join(input.state.paths.cache, "workspace-leases")
  if (input.task.workspace !== undefined) {
    const materialized = await materializeGitCheckoutWorkspaceWithLease({
      cacheRoot,
      checkout: input.task.workspace,
      ...(input.checkoutRunner === undefined ? {} : { checkoutRunner: input.checkoutRunner }),
      leaseRef: input.leaseRef,
      ...(input.checkoutRunner === undefined
        ? {
            preparedWorktreeCacheRoot: join(input.state.paths.cache, "workspace-prepared-cache"),
            prebuiltBaselineCacheRoot: join(input.state.paths.cache, "workspace-prebuilt-baselines"),
          }
        : {}),
      refPrefix: "workspace.pylon.codex_agent_task",
      repositoryCacheRoot: join(input.state.paths.cache, "workspace-git-cache"),
      retentionPolicy: "remove_on_closeout",
      workspaceStateRoot,
    })
    await cleanupOldestMaterializedWorkspaces({
      maxMaterializedWorkspaces: CODEX_AGENT_TASK_MAX_MATERIALIZED_WORKSPACES,
      minimumAgeSeconds: CODEX_AGENT_TASK_CACHE_PRUNE_MIN_AGE_SECONDS,
      workspaceStateRoot,
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
      workspaceStateRoot,
    }
  }

  const workspaceRef = stableRef("workspace.pylon.codex_agent_task", input.leaseRef)
  const workspace = join(cacheRoot, workspaceRef)
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
    workspaceStateRoot: undefined,
  }
}

async function releaseCodexAgentWorkspace(input: {
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
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
        "result.public.pylon.codex_agent_task.workspace_cleaned_on_closeout",
        result.cleanupReceiptRef,
      ],
    }
  }
  if (result?.retentionReasonRef !== undefined) {
    return {
      resultRefs: [
        "result.public.pylon.codex_agent_task.workspace_retained_on_closeout",
        result.retentionReasonRef,
      ],
    }
  }
  return { resultRefs: [] }
}

function codexScmCredentialScanRoots(input: {
  account: ResolvedPylonAccountSelection | null | undefined
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
}): WorkspaceScmCredentialScanRoot[] {
  return [
    { rootRef: input.materialized.workspaceRef, path: input.materialized.workspace },
    ...(input.account === null || input.account === undefined
      ? []
      : [{ rootRef: input.account.accountRefHash, path: input.account.home }]),
  ]
}

async function enforceCodexScmCredentialPolicy(input: {
  account: ResolvedPylonAccountSelection | null | undefined
  lease: CodexAgentLease
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
  now: Date
  runRef: string
}) {
  try {
    await assertNoLongLivedScmCredentials({
      roots: codexScmCredentialScanRoots({
        account: input.account,
        materialized: input.materialized,
      }),
    })
    return null
  } catch {
    await releaseCodexAgentWorkspace({ materialized: input.materialized, now: input.now })
    return refusalRecord({
      lease: input.lease,
      runRef: input.runRef,
      blockerRefs: ["blocker.assignment.codex_agent_long_lived_scm_credentials_detected"],
      resultRef: "result.public.pylon.codex_agent_task.scm_credential_policy_failed",
      summaryRef: "summary.public.pylon.codex_agent_task.scm_credential_policy_failed",
      message: "Local Codex thread stopped because long-lived SCM credential material was detected in the bounded workspace or selected worker home.",
    })
  }
}

export type CodexAgentLease = {
  assignmentRef: string
  leaseRef: string
  codingAssignment?: unknown
}

export type CodexAgentExecutionResult = Awaited<ReturnType<typeof executeCodexAgentAssignment>>

function deadlineBudgetExceededResult(): CodexAgentRunResult {
  return {
    outcome: "budget_exceeded",
    turnCount: 0,
    editedFileCount: 0,
    commandCount: 0,
    sessionRef: null,
  }
}

async function runCodexAgentWithOuterDeadline(
  runner: CodexAgentRunner,
  input: CodexAgentRunInput,
): Promise<CodexAgentRunResult> {
  return Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const runnerPromise = runner(input)
        runnerPromise.catch(() => undefined)
        let resolveDeadline!: (result: CodexAgentRunResult) => void
        const timeoutPromise = new Promise<CodexAgentRunResult>((resolve) => {
          resolveDeadline = resolve
        })
        yield* scopedTimeout({
          delayMs: input.timeoutMs,
          onTimeout: () => resolveDeadline(deadlineBudgetExceededResult()),
        })
        return yield* Effect.promise(() => Promise.race([runnerPromise, timeoutPromise]))
      }),
    ),
  )
}

function refusalRecord(input: {
  lease: CodexAgentLease
  runRef: string
  blockerRefs: string[]
  resultRef: string
  summaryRef: string
  message: string
  status?: "rejected" | "timed-out"
}) {
  const failureRef = stableRef(
    "proof.pylon.codex_agent_task.refused",
    `${input.lease.leaseRef}:${input.blockerRefs.join(",")}`,
  )
  const artifactRef = stableRef(
    "artifact.pylon.codex_agent_task.refused",
    `${input.lease.leaseRef}:${input.resultRef}:${input.summaryRef}`,
  )
  return {
    artifactRefs: [artifactRef],
    blockerRefs: input.blockerRefs,
    buildRefs: [input.runRef],
    message: input.message,
    previewRefs: [],
    proofRefs: [failureRef],
    resultRefs: [input.resultRef],
    runRefs: [input.runRef],
    status: input.status ?? "rejected",
    summaryRefs: [input.summaryRef],
    testRefs: [failureRef],
  }
}

async function recordCodexExecutionFailureHealth(input: {
  state: PylonLocalState
  account: ResolvedPylonAccountSelection | null | undefined
  failure: PylonCodexAccountFailure
  now: Date
}) {
  const accountRefHash = input.account?.accountRefHash
  if (accountRefHash === undefined) return
  let failure = input.failure
  let quota: ReturnType<typeof classifyQuotaSignal> | null = null
  let kind: "cooldown" | "weekly_exhausted" | "unknown" = "unknown"
  if (failureMeansQuotaBlock(input.failure)) {
    quota = classifyQuotaSignal(input.failure.publicMessage, "codex")
    const usageStore = await loadAccountUsageStore(input.state).catch(() => null)
    const latestProviderSnapshots =
      usageStore?.accounts[accountRefHash]?.providerTruth?.snapshots ?? []
    const inferredKind = inferCodexQuotaBlockKindFromProviderSnapshots(latestProviderSnapshots)
    kind = inferredKind === "unknown" && input.failure.reason === "usage_limited"
      ? "weekly_exhausted"
      : inferredKind
    if (kind === "cooldown" && input.failure.reason === "usage_limited") {
      failure = { ...input.failure, reason: "rate_limited" }
    }
  }
  await recordCodexAccountHealthFailure(input.state, {
    accountRefHash,
    failure,
    now: input.now,
  }).catch(() => undefined)
  if (quota) {
    await recordQuotaBlock(input.state, {
      accountRefHash,
      provider: "codex",
      retryAtIso: quota.retryAtIso,
      kind,
      sourceDigestRef: quota.sourceDigestRef,
      now: input.now,
    }).catch(() => undefined)
  }
}

function failureMeansQuotaBlock(failure: PylonCodexAccountFailure): boolean {
  return failure.reason === "usage_limited" || failure.reason === "rate_limited"
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
  const baseEnv = pylonAccountEnvironment(options.codexAgentProbe?.env ?? Bun.env, options.account)
  const custodyReprime = await reprimePylonCodexAccountAuthFromCustody({
    account: options.account,
    ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
    ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
    env: baseEnv,
    ...(options.fetch === undefined ? {} : { fetcher: options.fetch }),
    now,
  })
  if (custodyReprime.status === "blocked") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        PYLON_CODEX_CUSTODY_ASSIGNMENT_BLOCKER_REF,
        ...custodyReprime.blockerRefs,
      ],
      resultRef: "result.public.pylon.codex_agent_task.custody_unavailable",
      summaryRef: "summary.public.pylon.codex_agent_task.custody_unavailable",
      message: `Local Codex lane cannot re-prime linked account auth from custody (${custodyReprime.reason}).`,
    })
  }
  const env = custodyReprime.env
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
  const authHealth = await probeAndRecordCodexAccountAuthHealth(state, {
    account: options.account,
    env,
    now,
    ...(options.codexAuthValidityProbe === undefined ? {} : { probe: options.codexAuthValidityProbe }),
  })
  if (authHealth.state !== "valid") {
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.codex_agent_execution_refused",
        ...codexAccountFailureBlockerRefs(authHealth.failure.reason),
        authHealth.failure.sourceDigestRef,
      ],
      resultRef: `result.public.pylon.codex_agent_task.execution_refused.${authHealth.failure.reason}`,
      summaryRef: `summary.public.pylon.codex_agent_task.execution_refused.${authHealth.failure.reason}`,
      message: `Local Codex account failed pre-dispatch auth health with ${authHealth.failure.reason}: ${authHealth.failure.publicMessage || "no public error message available"}.`,
    })
  }

  let materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
  try {
    await Promise.resolve(options.onProgress?.({
      phase: "materializing",
      lastProgressEvent: "workspace.materializing",
    })).catch(() => undefined)
    materialized = await materializeCodexAgentWorkspace({
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
        "blocker.assignment.codex_agent_workspace_checkout_failed",
        ...(checkoutReasonRef === null ? [] : [checkoutReasonRef]),
      ],
      resultRef: "result.public.pylon.codex_agent_task.workspace_checkout_failed",
      summaryRef: "summary.public.pylon.codex_agent_task.workspace_checkout_failed",
      message: "Local Codex thread refused because the bounded workspace checkout could not be materialized.",
    })
  }

  const dependencies = await prepareWorkspaceDependencies({
    ...(options.dependencyInstaller === undefined ? {} : { installer: options.dependencyInstaller }),
    sharedCacheRoot: join(state.paths.cache, "codex-agent-tasks-node-modules-shared"),
    verificationArgs: materialized.verificationArgs,
    workspace: materialized.workspace,
  })
  await Promise.resolve(options.onProgress?.({
    phase: "installing",
    lastProgressEvent: dependencies.ok ? "dependencies.prepared" : "dependencies.failed",
  })).catch(() => undefined)
  if (!dependencies.ok) {
    await releaseCodexAgentWorkspace({ materialized, now })
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
  const eventChunkReporter =
    options.codexEventChunkReporter ??
    createPylonCodexEventChunkReporter({
      ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
      ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
      ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
    })
  let run: CodexAgentRunResult
  try {
    const timeoutMs =
      boundedNumber(
        task.timeoutSeconds ?? config.timeoutSeconds,
        DEFAULT_TIMEOUT_SECONDS,
        MAX_TIMEOUT_SECONDS,
      ) * 1000
    const guardedEnv = installCodexRipgrepGuard({
      env,
      workspaceRoot: materialized.workspace,
    }).env
    run = await runCodexAgentWithOuterDeadline(runner, {
      assignmentRef: lease.assignmentRef,
      cwd: materialized.workspace,
      account: options.account,
      env: guardedEnv,
      ...(eventChunkReporter === undefined ? {} : { eventChunkReporter }),
      ...(eventReporter === undefined ? {} : { eventReporter }),
      instructions: materialized.instructions,
      leaseRef: lease.leaseRef,
      networkAccessEnabled: true,
      pylonRef: state.identity.pylonRef,
      progressTokenEstimates: guardedEnv.OPENAGENTS_PYLON_CODEX_PROGRESS_TOKEN_ESTIMATES === "1",
      runRef,
      sandboxMode: effectiveSandboxMode(task.sandboxMode, config.sandboxMode),
      timeoutMs,
      ...(config.model === undefined ? {} : { model: config.model }),
      ...(options.onProgress === undefined ? {} : { onProgress: options.onProgress }),
      workspaceRef: materialized.workspaceRef,
    })
  } catch (error) {
    await releaseCodexAgentWorkspace({ materialized, now })
    const timedOut = /\b(timed?\s*out|timeout)\b/i.test(error instanceof Error ? error.message : String(error))
    const failure = classifyCodexAccountFailure(error)
    await recordCodexExecutionFailureHealth({
      state,
      account: options.account,
      failure,
      now,
    })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        timedOut
          ? "blocker.assignment.codex_agent_execution_timed_out"
          : "blocker.assignment.codex_agent_execution_refused",
        ...codexAccountFailureBlockerRefs(failure.reason),
        failure.sourceDigestRef,
      ],
      resultRef: timedOut
        ? "result.public.pylon.codex_agent_task.execution_timed_out"
        : `result.public.pylon.codex_agent_task.execution_refused.${failure.reason}`,
      status: timedOut ? "timed-out" : "rejected",
      summaryRef: timedOut
        ? "summary.public.pylon.codex_agent_task.execution_timed_out"
        : `summary.public.pylon.codex_agent_task.execution_refused.${failure.reason}`,
      message: timedOut
        ? "Local Codex thread timed out before completing the task."
        : `Local Codex thread refused with ${failure.reason}: ${failure.publicMessage || "no public error message available"}.`,
    })
	  }

  const scmCredentialPolicyRefusal = await enforceCodexScmCredentialPolicy({
    account: options.account,
    lease,
    materialized,
    now,
    runRef,
  })
  if (scmCredentialPolicyRefusal !== null) return scmCredentialPolicyRefusal

  if (run.outcome === "workspace_escape_blocked") {
    await releaseCodexAgentWorkspace({ materialized, now })
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
    await releaseCodexAgentWorkspace({ materialized, now })
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
    await releaseCodexAgentWorkspace({ materialized, now })
    const failure = run.refusal ?? classifyCodexAccountFailure("Codex SDK stream ended with an execution error.")
    await recordCodexExecutionFailureHealth({
      state,
      account: options.account,
      failure,
      now,
    })
    return refusalRecord({
      lease,
      runRef,
      blockerRefs: [
        "blocker.assignment.codex_agent_execution_refused",
        ...codexAccountFailureBlockerRefs(failure.reason),
        failure.sourceDigestRef,
      ],
      resultRef: `result.public.pylon.codex_agent_task.execution_refused.${failure.reason}`,
      summaryRef: `summary.public.pylon.codex_agent_task.execution_refused.${failure.reason}`,
      message: `Local Codex thread ended with ${failure.reason}: ${failure.publicMessage || "no public error message available"}.`,
    })
  }

  await Promise.resolve(options.onProgress?.({
    phase: "testing",
    lastProgressEvent: "verification.started",
  })).catch(() => undefined)
  const verification = await runCommand({ args: materialized.verificationArgs, cwd: materialized.workspace })
  await Promise.resolve(options.onProgress?.({
    phase: "proof",
    lastProgressEvent: verification.exitCode === 0 ? "verification.passed" : "verification.failed",
  })).catch(() => undefined)
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
  const claudeReview = await maybeRunClaudeSecondPassReview({
    assignmentRef: lease.assignmentRef,
    commandRef,
    materialized,
    options: options.claudeSecondPassReview,
    passed,
  })

  // PR-per-assignment (#6439). When a git_checkout assignment produces a
  // verified, non-empty diff, open exactly one scoped pull request and record
  // the public-safe PR refs back into the closeout. Fixture tasks have no
  // workspace and never open PRs. The publisher is fail-soft: PR-creation
  // problems never flip an otherwise-accepted closeout to rejected.
  const pullRequest = await maybePublishAssignmentPullRequest({
    task,
    materialized,
    lease,
    state,
    verification,
    passed,
    now,
    publisher: options.pullRequestPublisher,
    config,
    env,
    account: options.account,
  })
  const workspaceCleanup = await releaseCodexAgentWorkspace({ materialized, now })

  return {
    artifactRefs: [artifactRef],
    blockerRefs: passed ? [] : ["blocker.assignment.codex_agent_test_failed"],
    buildRefs: [commandRef],
    message: passed
      ? `Local Codex completed the bounded coding task: ${run.editedFileCount} file edit(s), ${run.commandCount} command(s), ${run.turnCount} turn(s), verification test passed on this device.${pullRequest.messageSuffix}`
      : "Local Codex thread completed but the verification test command failed; the change is not accepted.",
    previewRefs: [materialized.workspaceRef, ...pullRequest.previewRefs],
    proofRefs: [proofRef],
    resultRefs: [
      passed
        ? `result.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_passed`
        : `result.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_failed`,
      `result.public.pylon.codex_agent_task.edited_files.${run.editedFileCount}`,
      ...claudeReview.resultRefs,
      ...pullRequest.resultRefs,
      ...workspaceCleanup.resultRefs,
    ],
    runRefs: [runRef, ...sessionRefs],
    status: passed ? ("accepted" as const) : ("rejected" as const),
    summaryRefs: [
      passed
        ? `summary.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_passed`
        : `summary.public.pylon.codex_agent_task.${materialized.acceptanceResultRef}_failed`,
      ...claudeReview.summaryRefs,
    ],
    testRefs: [commandRef],
  }
}

type ClaudeSecondPassCloseoutContribution = {
  resultRefs: string[]
  summaryRefs: string[]
}

const EMPTY_CLAUDE_SECOND_PASS_CONTRIBUTION: ClaudeSecondPassCloseoutContribution = {
  resultRefs: [],
  summaryRefs: [],
}

type ClaudeSecondPassDiffCapture =
  | { state: "captured"; diffText: string }
  | { state: "skipped"; reason: ClaudeSecondPassSkipReason }

type ClaudeSecondPassSkipReason = "fixture_workspace" | "not_git_workspace" | "empty_diff" | "diff_unavailable"

function skippedClaudeSecondPassContribution(reason: ClaudeSecondPassSkipReason): ClaudeSecondPassCloseoutContribution {
  return {
    resultRefs: [`result.public.pylon.codex_agent_task.claude_second_pass_skipped.${reason}`],
    summaryRefs: [`summary.public.pylon.codex_agent_task.claude_second_pass_skipped.${reason}`],
  }
}

async function runGitForClaudeSecondPass(args: string[], workspace: string): Promise<{ stdout: string; exitCode: number }> {
  const proc = Bun.spawn(args, { cwd: workspace, stdout: "pipe", stderr: "pipe" })
  const [stdout, exitCode] = await Promise.all([new Response(proc.stdout).text(), proc.exited])
  return { stdout, exitCode }
}

async function gitDiffForClaudeSecondPass(
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>,
): Promise<ClaudeSecondPassDiffCapture> {
  if (materialized.workspaceStateRoot === undefined) {
    return { state: "skipped", reason: "fixture_workspace" }
  }
  const inside = await runGitForClaudeSecondPass(["git", "rev-parse", "--is-inside-work-tree"], materialized.workspace)
  if (inside.exitCode !== 0 || inside.stdout.trim() !== "true") {
    return { state: "skipped", reason: "not_git_workspace" }
  }
  const intentToAdd = await runGitForClaudeSecondPass(["git", "add", "-N", "--", "."], materialized.workspace)
  if (intentToAdd.exitCode !== 0) {
    return { state: "skipped", reason: "diff_unavailable" }
  }
  const diff = await runGitForClaudeSecondPass(["git", "diff", "--", "."], materialized.workspace)
  if (diff.exitCode !== 0) {
    return { state: "skipped", reason: "diff_unavailable" }
  }
  if (diff.stdout.trim().length === 0) {
    return { state: "skipped", reason: "empty_diff" }
  }
  return { state: "captured", diffText: diff.stdout }
}

async function maybeRunClaudeSecondPassReview(input: {
  assignmentRef: string
  commandRef: string
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
  options?: ClaudeSecondPassReviewOptions
  passed: boolean
}): Promise<ClaudeSecondPassCloseoutContribution> {
  if (!input.passed || input.options?.enabled !== true) return EMPTY_CLAUDE_SECOND_PASS_CONTRIBUTION
  try {
    const diff = await gitDiffForClaudeSecondPass(input.materialized)
    if (diff.state === "skipped") return skippedClaudeSecondPassContribution(diff.reason)
    const reviewer = input.options.reviewer ?? runClaudeSecondPassReview
    if (input.options.reviewer === undefined && input.options.account?.provider !== "claude_agent") {
      return {
        resultRefs: ["result.public.pylon.codex_agent_task.claude_second_pass_unavailable"],
        summaryRefs: ["summary.public.pylon.codex_agent_task.claude_second_pass_unavailable"],
      }
    }
    const verdict = await reviewer({
      assignmentRef: input.assignmentRef,
      workspace: input.materialized.workspace,
      diffText: diff.diffText,
      verifyCommandRef: input.commandRef,
      verifyCommand: input.materialized.verificationArgs,
      account: input.options.account ?? null,
      ...(input.options.timeoutMs === undefined ? {} : { timeoutMs: input.options.timeoutMs }),
    })
    const verdictRef = claudeSecondPassVerdictRef(verdict)
    return {
      resultRefs: [
        verdictRef,
        `result.public.pylon.codex_agent_task.claude_second_pass.${verdict.recommendation}`,
        ...verdict.riskRefs,
      ],
      summaryRefs: [`summary.public.pylon.codex_agent_task.claude_second_pass.${verdict.recommendation}`],
    }
  } catch {
    return {
      resultRefs: ["result.public.pylon.codex_agent_task.claude_second_pass_unavailable"],
      summaryRefs: ["summary.public.pylon.codex_agent_task.claude_second_pass_unavailable"],
    }
  }
}

type PullRequestCloseoutContribution = {
  resultRefs: string[]
  previewRefs: string[]
  messageSuffix: string
}

const EMPTY_PULL_REQUEST_CONTRIBUTION: PullRequestCloseoutContribution = {
  resultRefs: [],
  previewRefs: [],
  messageSuffix: "",
}

const PR_TITLE_MODEL_TIMEOUT_MS = 90 * 1000
const PR_TITLE_MODEL_MAX_DIFF_CHARS = 8000

/** Public-safe prompt for one-shot PR title/body generation. */
function buildPrTitleBodyPrompt(input: Parameters<AssignmentPrTitleBodyGenerator>[0]): string {
  const issueLine =
    input.issueNumber === null ? "(no linked issue)" : `#${input.issueNumber}`
  const titleLine = input.issueTitle === null ? "(unknown)" : input.issueTitle
  const diff =
    input.diffText.length > PR_TITLE_MODEL_MAX_DIFF_CHARS
      ? `${input.diffText.slice(0, PR_TITLE_MODEL_MAX_DIFF_CHARS)}\n…(diff truncated)`
      : input.diffText
  return [
    "You are writing the title and body for a GitHub pull request that resolves a public issue.",
    "Reply with a SINGLE JSON object and nothing else, shaped exactly as:",
    '{"title": "<conventional commit title>", "body": "<markdown body>"}',
    "",
    "Rules:",
    "- title: a concise Conventional Commits subject (feat/fix/docs/test/refactor/chore(scope): ...), <= 70 chars, no trailing period.",
    `- body: start with "Addresses ${issueLine}.", then a "### Changes" section summarizing the diff, then a "### Verification" section with the command and result.`,
    "- Do not invent changes that are not in the diff. Do not include secrets, file paths outside the repo, or prompt text.",
    "",
    `Issue: ${issueLine}`,
    `Issue title: ${titleLine}`,
    `Verification command: ${input.verifyCommand}`,
    `Verification result: ${input.verifyPassed ? `passed (exit ${input.verifyExitCode})` : `exit ${input.verifyExitCode}`}`,
    `Changed files (${input.changedPaths.length}): ${input.changedPaths.slice(0, 50).join(", ")}`,
    "",
    "Diff stat:",
    input.diffStat.length === 0 ? "(unavailable)" : input.diffStat,
    "",
    "Unified diff:",
    diff.length === 0 ? "(unavailable)" : diff,
  ].join("\n")
}

function parsePrTitleBodyJson(text: string): AssignmentPrTitleBody | null {
  const start = text.indexOf("{")
  const end = text.lastIndexOf("}")
  if (start === -1 || end === -1 || end <= start) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(text.slice(start, end + 1))
  } catch {
    return null
  }
  if (parsed === null || typeof parsed !== "object") return null
  const record = parsed as { title?: unknown; body?: unknown }
  if (typeof record.title !== "string" || typeof record.body !== "string") return null
  const title = record.title.trim()
  const body = record.body.trim()
  if (title.length === 0 || body.length === 0) return null
  return { title, body }
}

/**
 * Builds the own-capacity (no-spend, read-only, network-off) Codex title/body
 * generator. Fully fail-soft: any readiness, timeout, parse, or model failure
 * returns null so the publisher's deterministic fallback takes over.
 */
function createOwnCapacityTitleBodyGenerator(options: {
  cwd: string
  config?: CodexAgentConfig
  env?: Record<string, string | undefined>
  account?: ResolvedPylonAccountSelection | null
}): AssignmentPrTitleBodyGenerator {
  return async (input) => {
    try {
      const result = await runCodexComposerStream(buildPrTitleBodyPrompt(input), {
        cwd: options.cwd,
        account: options.account ?? null,
        ...(options.config === undefined ? {} : { config: options.config }),
        ...(options.env === undefined ? {} : { env: options.env }),
        sandboxMode: "read-only",
        executionMode: "local_bounded",
        approvalPolicy: "never",
        networkAccessEnabled: false,
        timeoutMs: PR_TITLE_MODEL_TIMEOUT_MS,
      })
      return parsePrTitleBodyJson(result.text)
    } catch {
      return null
    }
  }
}

/**
 * Opens one scoped pull request for a verified, non-empty git_checkout diff and
 * maps the typed outcome to public-safe closeout refs. Only git_checkout tasks
 * are eligible (fixtures have no workspace). Honors the
 * `OPENAGENTS_PYLON_DISABLE_ASSIGNMENT_PR` kill switch. Never throws.
 */
async function maybePublishAssignmentPullRequest(input: {
  task: CodexAgentTaskPayload
  materialized: Awaited<ReturnType<typeof materializeCodexAgentWorkspace>>
  lease: CodexAgentLease
  state: PylonLocalState
  verification: LocalCommandResult
  passed: boolean
  now: Date
  publisher?: AssignmentPullRequestPublisher
  config?: CodexAgentConfig
  env?: Record<string, string | undefined>
  account?: ResolvedPylonAccountSelection | null
}): Promise<PullRequestCloseoutContribution> {
  if (!input.passed) return EMPTY_PULL_REQUEST_CONTRIBUTION
  const workspace = input.task.workspace
  if (workspace === undefined) return EMPTY_PULL_REQUEST_CONTRIBUTION
  if (Bun.env.OPENAGENTS_PYLON_DISABLE_ASSIGNMENT_PR === "1") {
    return {
      resultRefs: ["result.public.pylon.codex_agent_task.pull_request_disabled"],
      previewRefs: [],
      messageSuffix: "",
    }
  }

  const publisher = input.publisher ?? publishAssignmentPullRequest
  // Own-capacity title/body generation is on by default; the deterministic
  // fallback inside the publisher covers any failure. The kill switch disables
  // the extra model turn entirely.
  const generateTitleBody =
    Bun.env.OPENAGENTS_PYLON_DISABLE_PR_TITLE_MODEL === "1"
      ? undefined
      : createOwnCapacityTitleBodyGenerator({
          cwd: input.materialized.workspace,
          ...(input.config === undefined ? {} : { config: input.config }),
          ...(input.env === undefined ? {} : { env: input.env }),
          account: input.account ?? null,
        })
  let result: Awaited<ReturnType<AssignmentPullRequestPublisher>>
  try {
    result = await publisher({
      cacheRoot: join(input.state.paths.cache, "codex-agent-tasks"),
      workingDirectory: input.materialized.workspace,
      workspaceRef: input.materialized.workspaceRef,
      sourceRef: input.materialized.artifactSourceRef,
      repository: {
        branch: workspace.repository.branch,
        commitSha: workspace.repository.commitSha,
        fullName: workspace.repository.fullName,
      },
      ...(workspace.scmAuthBroker === undefined ? {} : { scmAuthBroker: workspace.scmAuthBroker }),
      assignmentRef: input.lease.assignmentRef,
      ...(input.task.objectiveSummary === undefined ? {} : { objectiveSummary: input.task.objectiveSummary }),
      verification: {
        args: input.materialized.verificationArgs,
        exitCode: input.verification.exitCode,
        passed: input.passed,
      },
      now: input.now,
      ...(generateTitleBody === undefined ? {} : { generateTitleBody }),
    })
  } catch {
    return {
      resultRefs: ["result.public.pylon.codex_agent_task.pull_request_failed"],
      previewRefs: [],
      messageSuffix: " PR creation failed; the verified diff remains in the local workspace.",
    }
  }

  if (result.state === "opened") {
    return {
      resultRefs: [
        result.reused
          ? "result.public.pylon.codex_agent_task.pull_request_reused"
          : "result.public.pylon.codex_agent_task.pull_request_opened",
        `result.public.pylon.codex_agent_task.pull_request_changed_files.${result.changedCount}`,
      ],
      previewRefs: [result.prUrl, result.branchUrl],
      messageSuffix: ` Opened PR ${result.prUrl} (branch ${result.branch}, ${result.changedCount} file(s) changed).`,
    }
  }
  if (result.state === "no_change") {
    return {
      resultRefs: ["result.public.pylon.codex_agent_task.pull_request_no_change"],
      previewRefs: [],
      messageSuffix: " No diff produced; no PR opened.",
    }
  }
  if (result.state === "skipped") {
    return {
      resultRefs: ["result.public.pylon.codex_agent_task.pull_request_skipped"],
      previewRefs: [],
      messageSuffix: "",
    }
  }
  return {
    resultRefs: ["result.public.pylon.codex_agent_task.pull_request_failed"],
    previewRefs: [],
    messageSuffix: " PR creation failed; the verified diff remains in the local workspace.",
  }
}
