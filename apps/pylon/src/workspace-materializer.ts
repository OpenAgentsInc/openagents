import { existsSync, realpathSync } from "node:fs"
import { chmod, cp, lstat, mkdir, readdir, readFile, rename, rm, stat, utimes, writeFile } from "node:fs/promises"
import { dirname, join, relative, resolve } from "node:path"
import { createHash } from "node:crypto"
import { Context, Effect, Layer, type Scope } from "effect"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent.js"
import { CODEX_AGENT_CAPABILITY_REF } from "./codex-agent.js"

/**
 * The adapter-neutral git_checkout workspace materializer (issue #4798).
 *
 * This module owns the shared workspace contract for coding adapters: the
 * `workspace.kind = "git_checkout"` payload validator, the bounded checkout
 * runner, and assignment-scoped workspace materialization under the
 * Pylon-owned cache. The Claude Agent lane (B2 #4756) and the Codex lane
 * (CX5 #4792) consume the identical contract from here — never forked.
 *
 * Redaction law: `workingDirectory` is local-only. It must never appear in
 * progress events, artifact refs, closeouts, public projections, issue
 * comments, Forum posts, or browser UI. Surfaces emit `workspaceRef` and
 * `cleanupRef` instead.
 */

export type GitCheckoutWorkspace = {
  kind: "git_checkout"
  repository: {
    branch: string
    commitSha: string
    fullName: string
    provider: "github"
    visibility: "public" | "private"
  }
  virtualBranch?: {
    kind: "pylon_virtual_merge_queue"
    branchName: string
    baseCommitSha: string
    queueRef: string
  }
  verificationCommand: {
    args: string[]
    commandRef: string
  }
  scmAuthBroker?: ScmAuthBrokerConfig
}

export const SCM_AUTH_BROKER_SCHEMA = "openagents.pylon.scm_auth_broker.v1"
export const SCM_AUTH_BROKER_HELPER_REF = "helper.pylon.scm_auth_broker.git_credential.v1"
export const SCM_AUTH_BROKER_DEFAULT_CACHE_TTL_SECONDS = 60
export const SCM_AUTH_BROKER_MAX_CACHE_TTL_SECONDS = 60 * 60

export type ScmAuthBrokerFallback = "anonymous_read_only" | "fail_closed"

export type ScmAuthBrokerConfig = {
  schema: typeof SCM_AUTH_BROKER_SCHEMA
  kind: "forge_git_access" | "github_user_oauth"
  brokerUrl: string
  authRefs: string[]
  repositoryRef: string
  allowed: {
    protocol: "https"
    host: string
    pathPrefix: string
  }
  cacheTtlSeconds?: number
  fallback?: ScmAuthBrokerFallback
  username?: string
}

export type WorkspaceCheckoutRunner = (
  workingDirectory: string,
  checkout: GitCheckoutWorkspace,
) => Promise<void>

export type MaterializedWorkspace = {
  workspaceRef: string
  workingDirectory: string
  sourceRef: string
  cleanupRef: string
  preparedWorktreeCache?: WorkspacePreparedWorktreeCacheUse
  prebuiltBaselineCache?: WorkspacePrebuiltBaselineCacheMetric
}

export const WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA =
  "openagents.pylon.prepared_worktree_cache.v1"
export const DEFAULT_PREPARED_WORKTREE_CACHE_DISK_BUDGET_BYTES = 5 * 1024 * 1024 * 1024

export type WorkspacePreparedWorktreeCacheReuseReason =
  | "post_completion_snapshot"
  | "restore_quick_sync_reset"

export type WorkspacePreparedWorktreeCacheUse = {
  schema: typeof WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA
  cacheKey: string
  state: "hit"
  reuseReason: Extract<WorkspacePreparedWorktreeCacheReuseReason, "restore_quick_sync_reset">
  integrityRef: string
}

export type WorkspacePreparedWorktreeCacheRecord = {
  schema: typeof WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA
  cacheKey: string
  repositoryFullName: string
  baselineCommitSha: string
  sourceRef: string
  state: "ready"
  reuseReason: WorkspacePreparedWorktreeCacheReuseReason
  integrityRef: string
  createdAt: string
  updatedAt: string
  lastUsedAt: string
  useCount: number
  sizeBytes: number
  local: {
    preparedDirectory: string
  }
}

export const WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA =
  "openagents.pylon.prebuilt_baseline_cache.v1"
export const DEFAULT_PREBUILT_BASELINE_REFRESH_CADENCE_SECONDS = 15 * 60
export const WORKSPACE_PREBUILT_BASELINE_BUN_INSTALL_SETUP_REF =
  "setup.pylon.prebuilt_baseline.bun_install_frozen_lockfile.v1"

export type WorkspacePrebuiltBaselineSetupResult = {
  state: "completed" | "skipped"
  setupRef: string
  commandRef?: string
}

export type WorkspacePrebuiltBaselineSetupRunner = (input: {
  checkout: GitCheckoutWorkspace
  workingDirectory: string
}) => Promise<WorkspacePrebuiltBaselineSetupResult>

export type WorkspacePrebuiltBaselineCacheMetric = {
  schema: typeof WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA
  cacheKey: string
  registryRef: string
  repositoryFullName: string
  branch: string
  requestedCommitSha: string
  state: "hit" | "miss"
  reasonRef: string
  checkedAt: string
  hitCount: number
  missCount: number
  baselineCommitSha?: string
}

export type WorkspacePrebuiltBaselineCacheRecord = {
  schema: typeof WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA
  cacheKey: string
  registryRef: string
  repositoryFullName: string
  branch: string
  baselineCommitSha: string
  sourceRef: string
  state: "ready"
  integrityRef: string
  createdAt: string
  updatedAt: string
  upstreamCheckedAt: string
  refreshedAt: string
  refreshCadenceSeconds: number
  lastUsedAt: string
  hitCount: number
  missCount: number
  lastMissReasonRef?: string
  sizeBytes: number
  setup: WorkspacePrebuiltBaselineSetupResult
  local: {
    prebuiltDirectory: string
  }
}

export const WORKSPACE_SCM_CREDENTIAL_SCAN_SCHEMA =
  "openagents.pylon.workspace_scm_credential_scan.v1"

export type WorkspaceScmCredentialScanRoot = {
  rootRef: string
  path: string
}

export type WorkspaceScmCredentialFinding = {
  findingRef: string
  rootRef: string
  relativePath: string
  reasonRef: string
}

export type WorkspaceScmCredentialScan = {
  schema: typeof WORKSPACE_SCM_CREDENTIAL_SCAN_SCHEMA
  state: "clean" | "leaked"
  scannedFileCount: number
  findingRefs: string[]
  findings: WorkspaceScmCredentialFinding[]
}

export class WorkspaceScmCredentialPolicyError extends Error {
  readonly _tag = "WorkspaceScmCredentialPolicyError"
  readonly scan: WorkspaceScmCredentialScan

  constructor(scan: WorkspaceScmCredentialScan) {
    super("workspace SCM credential policy failed")
    this.name = "WorkspaceScmCredentialPolicyError"
    this.scan = scan
  }
}

export const WORKSPACE_CHANGE_CAPTURE_SCHEMA = "openagents.pylon.workspace_change_capture.v1"

export type WorkspaceChangeCaptureState = "clean" | "dirty" | "committed"

export type WorkspaceChangeCapture = {
  schema: typeof WORKSPACE_CHANGE_CAPTURE_SCHEMA
  workspaceRef: string
  sourceRef: string
  baseCommit: string
  headCommit: string
  state: WorkspaceChangeCaptureState
  changedCount: number
  fileRefs: string[]
  generatedAt: string
  commitRef?: string
  /**
   * Local-only mechanics. Public surfaces use fileRefs and counts, never
   * raw local paths from the materialized checkout.
   */
  local: {
    changedPaths: string[]
  }
}

export type WorkspaceChangeConflict = {
  conflictRef: string
  fileRef: string
  sourceRef: string
  workspaceRefs: string[]
}

export type WorkspaceChangeConflictScan = {
  state: "clear" | "conflicted"
  conflictRefs: string[]
  conflicts: WorkspaceChangeConflict[]
}

export type InFlightVirtualBranchChange = {
  virtualBranchRef: string
  target: {
    repositoryFullName: string
    branch: string
  }
  capture: WorkspaceChangeCapture
}

export type VirtualBranchChangeConflict = {
  conflictRef: string
  fileRef: string
  repositoryFullName: string
  targetBranch: string
  sourceRefs: string[]
  virtualBranchRefs: string[]
  workspaceRefs: string[]
}

export type VirtualBranchChangeConflictScan = {
  state: "clear" | "conflicted"
  conflictRefs: string[]
  conflicts: VirtualBranchChangeConflict[]
}

export type WorkspaceCommitResult =
  | {
      state: "clean"
      capture: WorkspaceChangeCapture
    }
  | {
      state: "committed"
      capture: WorkspaceChangeCapture
      commitRef: string
      commitSha: string
    }

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function canonicalPath(path: string): string {
  try {
    return realpathSync(path)
  } catch {
    return resolve(path)
  }
}

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitBranchNamePattern =
  /^(?!-)(?!refs\/)(?!.*(?:^|\/)\.)(?!.*(?:^|\/)$)(?!.*\.\.)(?!.*@{)(?!.*\/\/)(?!.*\.lock(?:\/|$))(?!.*\.$)[A-Za-z0-9][A-Za-z0-9._/-]{0,119}$/i
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const publicRefPattern = /^[A-Za-z0-9_.:/=@+-]{1,160}$/
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/
const scmBrokerHostPattern = /^[A-Za-z0-9.-]{1,120}$/
const scmBrokerPathPrefixPattern = /^\/?[A-Za-z0-9_.:/=@+-]{1,240}$/
const scmBrokerUsernamePattern = /^[A-Za-z0-9_.:@+-]{1,80}$/
const rawCredentialMaterialPattern =
  /(bearer\s+|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|oa_forge_git_[A-Za-z0-9_]+|password=|secret|token_value|credential_value|sk-[A-Za-z0-9_-]{16,})/i

const longLivedScmCredentialPatterns: ReadonlyArray<{
  reasonRef: string
  pattern: RegExp
}> = [
  {
    reasonRef: "reason.workspace_scm_credentials.github_pat",
    pattern: /\b(?:gh[opsu]_[A-Za-z0-9_]{16,}|github_pat_[A-Za-z0-9_]{16,})\b/i,
  },
  {
    reasonRef: "reason.workspace_scm_credentials.raw_forge_git_token",
    pattern: /\boa_forge_git_[A-Za-z0-9_]{8,}\b/i,
  },
  {
    reasonRef: "reason.workspace_scm_credentials.credentialed_git_url",
    pattern: /https?:\/\/[^/\s:@]+:[^@\s]+@[A-Za-z0-9.-]+[^\s]*/i,
  },
  {
    reasonRef: "reason.workspace_scm_credentials.git_extraheader_authorization",
    pattern: /\bextraheader\b\s*=\s*(?:authorization|bearer|basic)\b/i,
  },
]

const credentialScanIgnoredDirectoryNames = new Set([
  ".bun",
  ".cache",
  ".next",
  ".turbo",
  "build",
  "coverage",
  "dist",
  "node_modules",
  "target",
])

const credentialScanIgnoredGitAdminDirectoryNames = new Set(["objects", "logs"])
const defaultCredentialScanMaxFileBytes = 256 * 1024

function longLivedScmCredentialReasonFor(contents: string): string | null {
  for (const candidate of longLivedScmCredentialPatterns) {
    if (candidate.pattern.test(contents)) return candidate.reasonRef
  }
  return null
}

function shouldSkipCredentialScanDirectory(relativePath: string, name: string): boolean {
  if (credentialScanIgnoredDirectoryNames.has(name)) return true
  const parts = relativePath.split("/").filter(Boolean)
  return parts.at(-2) === ".git" && credentialScanIgnoredGitAdminDirectoryNames.has(name)
}

async function scanCredentialPath(input: {
  absolutePath: string
  findings: WorkspaceScmCredentialFinding[]
  maxFileBytes: number
  rootPath: string
  rootRef: string
  seenRealPaths: Set<string>
}): Promise<number> {
  let info: Awaited<ReturnType<typeof lstat>>
  try {
    info = await lstat(input.absolutePath)
  } catch {
    return 0
  }
  if (info.isSymbolicLink()) return 0

  let realPath: string
  try {
    realPath = realpathSync(input.absolutePath)
  } catch {
    realPath = resolve(input.absolutePath)
  }
  if (input.seenRealPaths.has(realPath)) return 0
  input.seenRealPaths.add(realPath)

  const relativePath = relative(input.rootPath, input.absolutePath) || "."
  if (info.isDirectory()) {
    const name = relativePath === "." ? "" : relativePath.split("/").at(-1) ?? ""
    if (name !== "" && shouldSkipCredentialScanDirectory(relativePath, name)) return 0
    let entries: Array<{ name: string }>
    try {
      entries = await readdir(input.absolutePath, { withFileTypes: true })
    } catch {
      return 0
    }
    let scannedFileCount = 0
    for (const entry of entries) {
      scannedFileCount += await scanCredentialPath({
        ...input,
        absolutePath: join(input.absolutePath, entry.name),
      })
    }
    return scannedFileCount
  }

  if (!info.isFile() || info.size > input.maxFileBytes) return 0
  let contents: string
  try {
    contents = await readFile(input.absolutePath, "utf8")
  } catch {
    return 0
  }
  const reasonRef = longLivedScmCredentialReasonFor(contents)
  if (reasonRef !== null) {
    const findingRef = stableRef(
      "finding.pylon.workspace_scm_credential",
      `${input.rootRef}:${relativePath}:${reasonRef}`,
    )
    input.findings.push({
      findingRef,
      rootRef: input.rootRef,
      relativePath,
      reasonRef,
    })
  }
  return 1
}

export async function scanLongLivedScmCredentials(input: {
  roots: ReadonlyArray<WorkspaceScmCredentialScanRoot>
  maxFileBytes?: number
}): Promise<WorkspaceScmCredentialScan> {
  const findings: WorkspaceScmCredentialFinding[] = []
  let scannedFileCount = 0
  const maxFileBytes = input.maxFileBytes ?? defaultCredentialScanMaxFileBytes
  for (const root of input.roots) {
    scannedFileCount += await scanCredentialPath({
      absolutePath: resolve(root.path),
      findings,
      maxFileBytes,
      rootPath: resolve(root.path),
      rootRef: root.rootRef,
      seenRealPaths: new Set(),
    })
  }
  return {
    schema: WORKSPACE_SCM_CREDENTIAL_SCAN_SCHEMA,
    state: findings.length === 0 ? "clean" : "leaked",
    scannedFileCount,
    findingRefs: findings.map((finding) => finding.findingRef),
    findings,
  }
}

export async function assertNoLongLivedScmCredentials(input: {
  roots: ReadonlyArray<WorkspaceScmCredentialScanRoot>
  maxFileBytes?: number
}): Promise<WorkspaceScmCredentialScan> {
  const scan = await scanLongLivedScmCredentials(input)
  if (scan.state === "leaked") throw new WorkspaceScmCredentialPolicyError(scan)
  return scan
}

function boundedScmAuthBrokerCacheTtlSeconds(value: unknown): number | undefined {
  if (value === undefined) return undefined
  if (typeof value !== "number") return undefined
  if (!Number.isInteger(value) || value < 0 || value > SCM_AUTH_BROKER_MAX_CACHE_TTL_SECONDS) {
    return undefined
  }
  return value
}

function scmAuthBrokerFrom(value: unknown): ScmAuthBrokerConfig | null | undefined {
  if (value === undefined) return undefined
  if (value === null || typeof value !== "object") return null
  const payload = value as ScmAuthBrokerConfig
  if (
    payload.schema !== SCM_AUTH_BROKER_SCHEMA ||
    (payload.kind !== "forge_git_access" && payload.kind !== "github_user_oauth")
  ) {
    return null
  }
  if (typeof payload.brokerUrl !== "string" || rawCredentialMaterialPattern.test(payload.brokerUrl)) return null
  let brokerUrl: URL
  try {
    brokerUrl = new URL(payload.brokerUrl)
  } catch {
    return null
  }
  if (brokerUrl.protocol !== "https:" || brokerUrl.username !== "" || brokerUrl.password !== "") return null
  if (!Array.isArray(payload.authRefs) || payload.authRefs.length === 0 || payload.authRefs.length > 8) return null
  if (!payload.authRefs.every((ref) => publicRefPattern.test(ref) && !rawCredentialMaterialPattern.test(ref))) return null
  if (typeof payload.repositoryRef !== "string" || !publicRefPattern.test(payload.repositoryRef)) return null
  if (rawCredentialMaterialPattern.test(payload.repositoryRef)) return null
  if (payload.allowed?.protocol !== "https") return null
  if (typeof payload.allowed.host !== "string" || !scmBrokerHostPattern.test(payload.allowed.host)) return null
  if (typeof payload.allowed.pathPrefix !== "string" || !scmBrokerPathPrefixPattern.test(payload.allowed.pathPrefix)) {
    return null
  }
  const normalizedPathPrefix = `/${payload.allowed.pathPrefix.replace(/^\/+/, "")}`
  if (normalizedPathPrefix.includes("..")) return null
  const cacheTtlSeconds = boundedScmAuthBrokerCacheTtlSeconds(payload.cacheTtlSeconds)
  if (payload.cacheTtlSeconds !== undefined && cacheTtlSeconds === undefined) return null
  const fallback = payload.fallback ?? "fail_closed"
  if (fallback !== "fail_closed" && fallback !== "anonymous_read_only") return null
  if (payload.username !== undefined && !scmBrokerUsernamePattern.test(payload.username)) return null
  return {
    schema: SCM_AUTH_BROKER_SCHEMA,
    kind: payload.kind,
    brokerUrl: brokerUrl.toString(),
    authRefs: [...payload.authRefs],
    repositoryRef: payload.repositoryRef,
    allowed: {
      protocol: "https",
      host: payload.allowed.host.toLowerCase(),
      pathPrefix: normalizedPathPrefix,
    },
    ...(cacheTtlSeconds === undefined ? {} : { cacheTtlSeconds }),
    fallback,
    ...(payload.username === undefined ? {} : { username: payload.username }),
  }
}

const githubUserOAuthBrokerRepositoryRef = (fullName: string): string =>
  `repo.github/${fullName}`

const githubUserOAuthBrokerMatchesCheckout = (
  checkout: GitCheckoutWorkspace,
  broker: ScmAuthBrokerConfig | undefined,
): boolean => {
  if (checkout.repository.visibility !== "private") return true
  if (broker?.kind !== "github_user_oauth") return false
  if (broker.fallback !== "fail_closed") return false
  if (broker.allowed.protocol !== "https") return false
  if (broker.allowed.host !== "github.com") return false
  const expectedPath = `/${checkout.repository.fullName}.git`
  if (broker.allowed.pathPrefix.toLowerCase() !== expectedPath.toLowerCase()) return false
  return broker.repositoryRef.toLowerCase() ===
    githubUserOAuthBrokerRepositoryRef(checkout.repository.fullName).toLowerCase()
}

/**
 * Decodes and validates the shared git_checkout workspace payload from a
 * normalized coding assignment. Rejects unbrokered private repositories,
 * unsafe repository names, unpinned commits, absolute verification paths,
 * `..` traversal, and shell-shaped command strings — foreign or malformed
 * shapes never reach filesystem work.
 */
export function gitCheckoutWorkspaceFrom(codingAssignment: unknown): GitCheckoutWorkspace | null {
  const workspace = (codingAssignment as { workspace?: unknown } | null)?.workspace
  if (workspace === null || typeof workspace !== "object") return null
  const payload = workspace as GitCheckoutWorkspace
  if (payload.kind !== "git_checkout") return null
  if (payload.repository?.provider !== "github") return null
  if (payload.repository.visibility !== "public" && payload.repository.visibility !== "private") return null
  if (!githubFullNamePattern.test(payload.repository.fullName)) return null
  if (!gitCommitShaPattern.test(payload.repository.commitSha)) return null
  if (typeof payload.repository.branch !== "string" || !gitBranchNamePattern.test(payload.repository.branch)) return null
  if (!virtualBranchIsValid(payload.virtualBranch)) return null
  if (!Array.isArray(payload.verificationCommand?.args) || payload.verificationCommand.args.length === 0) return null
  if (typeof payload.verificationCommand.commandRef !== "string") return null
  const safeArgs = payload.verificationCommand.args.every((arg) =>
    typeof arg === "string" &&
    verificationCommandArgPattern.test(arg) &&
    !arg.includes("..") &&
    !arg.startsWith("/")
  )
  const scmAuthBroker = scmAuthBrokerFrom(payload.scmAuthBroker)
  if (scmAuthBroker === null) return null
  if (!githubUserOAuthBrokerMatchesCheckout(payload, scmAuthBroker)) return null
  return safeArgs
    ? {
        ...payload,
        ...(scmAuthBroker === undefined ? {} : { scmAuthBroker }),
      }
    : null
}

function virtualBranchIsValid(virtualBranch: GitCheckoutWorkspace["virtualBranch"] | undefined): boolean {
  if (virtualBranch === undefined) return true
  if (virtualBranch === null || typeof virtualBranch !== "object") return false
  return (
    virtualBranch.kind === "pylon_virtual_merge_queue" &&
    typeof virtualBranch.branchName === "string" &&
    gitBranchNamePattern.test(virtualBranch.branchName) &&
    virtualBranch.branchName.startsWith("pylon/virtual-") &&
    typeof virtualBranch.baseCommitSha === "string" &&
    gitCommitShaPattern.test(virtualBranch.baseCommitSha) &&
    typeof virtualBranch.queueRef === "string" &&
    publicRefPattern.test(virtualBranch.queueRef)
  )
}

export function checkoutBaseCommitSha(checkout: GitCheckoutWorkspace): string {
  return checkout.virtualBranch?.baseCommitSha ?? checkout.repository.commitSha
}

export function checkoutSourceRef(checkout: GitCheckoutWorkspace): string {
  return `${checkout.repository.fullName}:${checkoutBaseCommitSha(checkout)}`
}

// Concurrent assignments materialize against a shared bare object store, so
// git operations can transiently lose a race for an internal lockfile
// (index.lock, ref locks, packed-refs.lock, shallow.lock, config.lock) or
// collide with a background `git gc --auto` that escaped the critical section.
// These are recoverable: retry the same git command a bounded number of times
// with exponential backoff instead of surfacing a hard checkout failure.
const gitCommandMaxAttempts = 6
const gitCommandRetryBaseMs = 40
const gitCommandRetryMaxMs = 1_500
export const WORKSPACE_GIT_LOCK_RETRY_POLICY = {
  maxAttempts: gitCommandMaxAttempts,
  baseDelayMs: gitCommandRetryBaseMs,
  maxDelayMs: gitCommandRetryMaxMs,
} as const

const transientGitLockPattern =
  /(?:index|config|shallow|packed-refs|HEAD|ORIG_HEAD|FETCH_HEAD)\.lock|unable to create '[^']*\.lock'|cannot lock ref|unable to lock|could not lock|gc is already running|another git process seems to be running|Unable to create '[^']*': File exists|fatal: Unable to create/i

function isTransientGitLockStderr(stderr: string): boolean {
  return transientGitLockPattern.test(stderr)
}

async function spawnGitCommand(
  args: string[],
  cwd: string,
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  return { exitCode, stdout, stderr }
}

/**
 * Runs a git command, retrying with bounded exponential backoff only when the
 * failure is a transient git lock collision. Non-lock failures (a missing
 * commit, an unreachable remote, a `cat-file -e` "not present" probe) return
 * immediately so existence checks and hard errors are never masked or delayed.
 */
async function runGitCommand(
  args: string[],
  cwd: string,
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  let result = await spawnGitCommand(args, cwd)
  for (let attempt = 1; attempt < gitCommandMaxAttempts; attempt += 1) {
    if (result.exitCode === 0 || !isTransientGitLockStderr(result.stderr)) return result
    const backoff = Math.min(gitCommandRetryBaseMs * 2 ** (attempt - 1), gitCommandRetryMaxMs)
    await sleep(backoff + Math.floor(Math.random() * gitCommandRetryBaseMs))
    result = await spawnGitCommand(args, cwd)
  }
  return result
}

async function runQuietCommand(args: string[], cwd: string): Promise<number> {
  return (await runGitCommand(args, cwd)).exitCode
}

async function runTextCommand(args: string[], cwd: string): Promise<{ exitCode: number; stdout: string }> {
  const { exitCode, stdout } = await runGitCommand(args, cwd)
  return { exitCode, stdout }
}

export class WorkspaceCheckoutError extends Error {
  readonly reasonRef: string

  constructor(reasonRef: string) {
    super(`workspace checkout failed: ${reasonRef}`)
    this.name = "WorkspaceCheckoutError"
    this.reasonRef = reasonRef
  }
}

export function workspaceCheckoutFailureReasonRef(error: unknown): string | null {
  return error instanceof WorkspaceCheckoutError ? error.reasonRef : null
}

async function runCheckedCommand(args: string[], cwd: string, reasonRef?: string): Promise<void> {
  const exitCode = await runQuietCommand(args, cwd)
  if (exitCode !== 0) {
    if (reasonRef !== undefined) throw new WorkspaceCheckoutError(reasonRef)
    throw new Error(`command failed: ${args[0] ?? "unknown"}`)
  }
}

function shellSingleQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`
}

export type GitCredentialHelperRuntimePaths = {
  gitDirectory: string
  configPath: string
  gitConfigPath: string
  helperPath: string
  cachePath: string
}

async function gitAdminDirectoryFor(cwd: string): Promise<string> {
  const gitDir = await runTextCommand(["git", "rev-parse", "--git-dir"], cwd)
  if (gitDir.exitCode !== 0) {
    throw new WorkspaceCheckoutError("reason.workspace_checkout.git_dir_unavailable")
  }
  const value = gitDir.stdout.trim()
  if (value.length === 0) {
    throw new WorkspaceCheckoutError("reason.workspace_checkout.git_dir_unavailable")
  }
  return resolve(cwd, value)
}

export async function gitCredentialHelperRuntimePathsFor(
  gitWorktreeOrDirectory: string,
): Promise<GitCredentialHelperRuntimePaths> {
  const gitDirectory = await gitAdminDirectoryFor(gitWorktreeOrDirectory)
  return {
    cachePath: join(gitDirectory, "pylon-scm-auth-cache.json"),
    configPath: join(gitDirectory, "pylon-scm-auth-broker.json"),
    gitConfigPath: join(gitDirectory, "pylon-scm-auth-broker.gitconfig"),
    gitDirectory,
    helperPath: join(gitDirectory, "pylon-git-credential-helper.mjs"),
  }
}

const GIT_CREDENTIAL_HELPER_SCRIPT = `#!/usr/bin/env bun
import { chmod, readFile, writeFile } from "node:fs/promises"

const CONFIG_PATH = process.argv[2]
const OPERATION = process.argv[3] || "get"
const MAX_CACHE_TTL_SECONDS = 60 * 60
const SKEW_MS = 30 * 1000

const readStdin = async () => {
  let input = ""
  for await (const chunk of process.stdin) input += chunk
  return input
}

const parseCredentialInput = (input) => {
  const record = {}
  for (const line of input.split(/\\r?\\n/)) {
    if (line.length === 0) continue
    const index = line.indexOf("=")
    if (index <= 0) continue
    record[line.slice(0, index)] = line.slice(index + 1)
  }
  return record
}

const fail = (config, reason) => {
  if (config.fallback === "anonymous_read_only") process.exit(0)
  process.stderr.write("pylon git credential helper refused: " + reason + "\\n")
  process.exit(1)
}

const normalizedPath = (value) => "/" + String(value || "").replace(/^\\/+/, "")

const inScope = (config, request) => {
  if (request.protocol !== config.allowed.protocol) return false
  if (String(request.host || "").toLowerCase() !== config.allowed.host) return false
  return normalizedPath(request.path).startsWith(config.allowed.pathPrefix)
}

const cacheKeyFor = (request) =>
  [request.protocol || "", String(request.host || "").toLowerCase(), normalizedPath(request.path)].join("\\u0000")

const readCache = async (config) => {
  try {
    return JSON.parse(await readFile(config.cachePath, "utf8"))
  } catch {
    return { entries: {} }
  }
}

const writeCache = async (config, cache) => {
  await writeFile(config.cachePath, JSON.stringify(cache, null, 2) + "\\n", { mode: 0o600 })
  await chmod(config.cachePath, 0o600).catch(() => undefined)
}

const cachedCredential = async (config, request) => {
  const cache = await readCache(config)
  const entry = cache.entries?.[cacheKeyFor(request)]
  if (!entry || typeof entry !== "object") return null
  const now = Date.now()
  if (typeof entry.cachedUntilMs !== "number" || entry.cachedUntilMs <= now) return null
  if (typeof entry.expiresAtMs !== "number" || entry.expiresAtMs - SKEW_MS <= now) return null
  if (typeof entry.username !== "string" || typeof entry.password !== "string") return null
  return { username: entry.username, password: entry.password }
}

const rememberCredential = async (config, request, credential) => {
  const cache = await readCache(config)
  const ttlSeconds = Math.max(0, Math.min(config.cacheTtlSeconds || 60, MAX_CACHE_TTL_SECONDS))
  const now = Date.now()
  const expiresAtMs = Date.parse(credential.expiresAt)
  cache.entries = cache.entries || {}
  cache.entries[cacheKeyFor(request)] = {
    username: credential.username,
    password: credential.password,
    cachedUntilMs: now + ttlSeconds * 1000,
    expiresAtMs: Number.isFinite(expiresAtMs) ? expiresAtMs : now,
  }
  await writeCache(config, cache)
}

const bearerToken = (config) => {
  for (const envName of config.controlPlaneAuthEnv || []) {
    const value = process.env[envName]
    if (typeof value === "string" && value.trim() !== "") return value.trim()
  }
  return null
}

const fetchCredential = async (config, request) => {
  const headers = { "content-type": "application/json" }
  const bearer = bearerToken(config)
  if (bearer !== null) headers.authorization = "Bearer " + bearer
  const response = await fetch(config.brokerUrl, {
    method: "POST",
    headers,
    body: JSON.stringify({
      schema: "openagents.pylon.git_credential_broker_request.v1",
      helperRef: config.helperRef,
      repositoryRef: config.repositoryRef,
      authRefs: config.authRefs,
      protocol: request.protocol,
      host: request.host,
      path: normalizedPath(request.path),
    }),
  })
  if (!response.ok) throw new Error("broker_http_" + response.status)
  const body = await response.json()
  const username = typeof body.username === "string" && body.username !== "" ? body.username : config.username
  const password = typeof body.password === "string" ? body.password : undefined
  const expiresAt = typeof body.expiresAt === "string" ? body.expiresAt : undefined
  if (typeof username !== "string" || username === "" || typeof password !== "string" || password === "" || expiresAt === undefined) {
    throw new Error("broker_invalid_response")
  }
  return { username, password, expiresAt }
}

if (!CONFIG_PATH) fail({ fallback: "fail_closed" }, "missing_config")
if (OPERATION !== "get") process.exit(0)

const config = JSON.parse(await readFile(CONFIG_PATH, "utf8"))
const request = parseCredentialInput(await readStdin())

if (!inScope(config, request)) fail(config, "out_of_scope")

const cached = await cachedCredential(config, request)
const credential = cached || await fetchCredential(config, request)
if (!cached) await rememberCredential(config, request, credential)

process.stdout.write("username=" + credential.username + "\\n")
process.stdout.write("password=" + credential.password + "\\n\\n")
`

type InstallGitCredentialHelperInput = {
  checkout: GitCheckoutWorkspace
  cwd: string
  configScope?: "gitdir_include" | "local"
}

function gitCredentialHelperCommand(paths: GitCredentialHelperRuntimePaths): string {
  return `!${shellSingleQuote(paths.helperPath)} ${shellSingleQuote(paths.configPath)}`
}

function gitCredentialHelperConfigFragment(paths: GitCredentialHelperRuntimePaths): string {
  return [
    "[credential]",
    "\thelper =",
    `\thelper = ${gitCredentialHelperCommand(paths)}`,
    "\tuseHttpPath = true",
    "\tinteractive = never",
    "",
  ].join("\n")
}

export async function installScmAuthBrokerGitCredentialHelper(
  input: InstallGitCredentialHelperInput,
): Promise<GitCredentialHelperRuntimePaths | null> {
  const broker = input.checkout.scmAuthBroker
  if (broker === undefined) return null

  const paths = await gitCredentialHelperRuntimePathsFor(input.cwd)
  const cacheTtlSeconds = Math.min(
    broker.cacheTtlSeconds ?? SCM_AUTH_BROKER_DEFAULT_CACHE_TTL_SECONDS,
    SCM_AUTH_BROKER_MAX_CACHE_TTL_SECONDS,
  )
  await writeFile(
    paths.configPath,
    `${JSON.stringify(
      {
        schema: SCM_AUTH_BROKER_SCHEMA,
        helperRef: SCM_AUTH_BROKER_HELPER_REF,
        brokerUrl: broker.brokerUrl,
        authRefs: broker.authRefs,
        repositoryRef: broker.repositoryRef,
        allowed: broker.allowed,
        cachePath: paths.cachePath,
        cacheTtlSeconds,
        fallback: broker.fallback ?? "fail_closed",
        username: broker.username ?? "x-access-token",
        controlPlaneAuthEnv: [
          "PYLON_GIT_CREDENTIAL_BROKER_TOKEN",
          "OPENAGENTS_AGENT_TOKEN",
          "PYLON_AGENT_TOKEN",
        ],
      },
      null,
      2,
    )}\n`,
    { mode: 0o600 },
  )
  await chmod(paths.configPath, 0o600).catch(() => undefined)
  await writeFile(paths.helperPath, GIT_CREDENTIAL_HELPER_SCRIPT, { mode: 0o700 })
  await chmod(paths.helperPath, 0o700).catch(() => undefined)

  if (input.configScope === "gitdir_include") {
    await writeFile(paths.gitConfigPath, gitCredentialHelperConfigFragment(paths), { mode: 0o600 })
    await chmod(paths.gitConfigPath, 0o600).catch(() => undefined)
    await runCheckedCommand(
      [
        "git",
        "config",
        "--local",
        `includeIf.gitdir:${paths.gitDirectory}/.path`,
        paths.gitConfigPath,
      ],
      input.cwd,
      "reason.workspace_checkout.credential_helper_config_failed",
    )
    return paths
  }

  const scopeArg = "--local"
  await runQuietCommand(["git", "config", scopeArg, "--unset-all", "credential.helper"], input.cwd)
  await runCheckedCommand(
    ["git", "config", scopeArg, "credential.helper", ""],
    input.cwd,
    "reason.workspace_checkout.credential_helper_config_failed",
  )
  await runCheckedCommand(
    ["git", "config", scopeArg, "--add", "credential.helper", gitCredentialHelperCommand(paths)],
    input.cwd,
    "reason.workspace_checkout.credential_helper_config_failed",
  )
  await runCheckedCommand(
    ["git", "config", scopeArg, "credential.useHttpPath", "true"],
    input.cwd,
    "reason.workspace_checkout.credential_helper_config_failed",
  )
  await runCheckedCommand(
    ["git", "config", scopeArg, "credential.interactive", "never"],
    input.cwd,
    "reason.workspace_checkout.credential_helper_config_failed",
  )
  return paths
}

function assertPylonOwnedWorkspaceTarget(input: {
  cacheRoot: string
  workingDirectory: string
}): { cacheRoot: string; target: string } {
  const cacheRoot = resolve(input.cacheRoot)
  const target = resolve(input.workingDirectory)
  if (target === cacheRoot || !target.startsWith(`${cacheRoot}/`)) {
    throw new Error("workspace operation refused: target is outside the Pylon-owned cache root")
  }
  return { cacheRoot, target }
}

function assertSafeWorkspaceRelativePath(relativePath: string): void {
  if (relativePath.length === 0) throw new Error("workspace path refused: empty path")
  if (relativePath.includes("\0")) throw new Error("workspace path refused: nul byte")
  if (relativePath.startsWith("/") || /^[A-Za-z]:[\\/]/.test(relativePath)) {
    throw new Error("workspace path refused: absolute path")
  }
  const parts = relativePath.split(/[\\/]+/)
  if (parts.includes("..")) throw new Error("workspace path refused: traversal")
  if (parts[0] === ".git") throw new Error("workspace path refused: git metadata")
}

async function assertWorkspaceGitRoot(workingDirectory: string): Promise<void> {
  const root = await runTextCommand(["git", "rev-parse", "--show-toplevel"], workingDirectory)
  if (root.exitCode !== 0) {
    throw new Error("workspace change capture requires a git worktree")
  }
  if (canonicalPath(root.stdout.trim()) !== canonicalPath(workingDirectory)) {
    throw new Error("workspace change capture refused: git root does not match the lane workspace")
  }
}

function decodeNulSeparatedPaths(stdout: string): string[] {
  return stdout.split("\0").filter((path) => path.length > 0)
}

async function listWorkspaceChangedPaths(workingDirectory: string): Promise<string[]> {
  const tracked = await runTextCommand(["git", "diff", "--name-only", "-z", "HEAD", "--"], workingDirectory)
  if (tracked.exitCode !== 0) throw new Error("workspace change capture failed: git diff failed")
  const untracked = await runTextCommand(
    ["git", "ls-files", "--others", "--exclude-standard", "-z"],
    workingDirectory,
  )
  if (untracked.exitCode !== 0) throw new Error("workspace change capture failed: git ls-files failed")
  const paths = [...new Set([...decodeNulSeparatedPaths(tracked.stdout), ...decodeNulSeparatedPaths(untracked.stdout)])].sort()
  for (const path of paths) assertSafeWorkspaceRelativePath(path)
  return paths
}

async function workspaceDirtyState(workingDirectory: string): Promise<"clean" | "dirty" | "unknown"> {
  if (!existsSync(workingDirectory) || !existsSync(join(workingDirectory, ".git"))) return "clean"
  const inside = await runQuietCommand(["git", "rev-parse", "--is-inside-work-tree"], workingDirectory)
  if (inside !== 0) return "clean"
  try {
    return (await listWorkspaceChangedPaths(workingDirectory)).length === 0 ? "clean" : "dirty"
  } catch {
    return "unknown"
  }
}

function sourceBaseCommit(sourceRef: string): string {
  const baseCommit = sourceRef.split(":").at(-1) ?? ""
  return gitCommitShaPattern.test(baseCommit) ? baseCommit : "unknown"
}

export function workspaceChangeFileRef(sourceRef: string, relativePath: string): string {
  assertSafeWorkspaceRelativePath(relativePath)
  return stableRef("file.pylon.workspace", `${sourceRef}:${relativePath}`)
}

export function virtualBranchChangeFileRef(input: {
  repositoryFullName: string
  targetBranch: string
  relativePath: string
}): string {
  if (!githubFullNamePattern.test(input.repositoryFullName)) {
    throw new Error("virtual branch conflict scan refused: invalid repository name")
  }
  if (!gitBranchNamePattern.test(input.targetBranch)) {
    throw new Error("virtual branch conflict scan refused: invalid target branch")
  }
  assertSafeWorkspaceRelativePath(input.relativePath)
  return stableRef(
    "file.pylon.virtual_branch",
    `${input.repositoryFullName}:${input.targetBranch}:${input.relativePath}`,
  )
}

export async function captureWorkspaceChanges(input: {
  cacheRoot: string
  workingDirectory: string
  workspaceRef: string
  sourceRef: string
  now?: Date
}): Promise<WorkspaceChangeCapture> {
  const { target } = assertPylonOwnedWorkspaceTarget(input)
  await assertWorkspaceGitRoot(target)
  const changedPaths = await listWorkspaceChangedPaths(target)
  const head = await runTextCommand(["git", "rev-parse", "HEAD"], target)
  if (head.exitCode !== 0) throw new Error("workspace change capture failed: unable to read HEAD")
  const headCommit = head.stdout.trim()
  return {
    schema: WORKSPACE_CHANGE_CAPTURE_SCHEMA,
    workspaceRef: input.workspaceRef,
    sourceRef: input.sourceRef,
    baseCommit: sourceBaseCommit(input.sourceRef),
    headCommit,
    state: changedPaths.length === 0 ? "clean" : "dirty",
    changedCount: changedPaths.length,
    fileRefs: changedPaths.map((path) => workspaceChangeFileRef(input.sourceRef, path)),
    generatedAt: (input.now ?? new Date()).toISOString(),
    local: { changedPaths },
  }
}

export function publicWorkspaceChangeCaptureProjection(capture: WorkspaceChangeCapture) {
  return {
    schema: capture.schema,
    workspaceRef: capture.workspaceRef,
    sourceRef: capture.sourceRef,
    baseCommit: capture.baseCommit,
    headCommit: capture.headCommit,
    state: capture.state,
    changedCount: capture.changedCount,
    fileRefs: capture.fileRefs,
    generatedAt: capture.generatedAt,
    ...(capture.commitRef === undefined ? {} : { commitRef: capture.commitRef }),
  }
}

export async function stageWorkspacePaths(input: {
  cacheRoot: string
  workingDirectory: string
  relativePaths: string[]
}): Promise<{ stagedCount: number }> {
  const { target } = assertPylonOwnedWorkspaceTarget(input)
  await assertWorkspaceGitRoot(target)
  const relativePaths = [...new Set(input.relativePaths)]
  for (const path of relativePaths) assertSafeWorkspaceRelativePath(path)
  if (relativePaths.length === 0) return { stagedCount: 0 }

  const changedPaths = new Set(await listWorkspaceChangedPaths(target))
  const foreignPath = relativePaths.find((path) => !changedPaths.has(path))
  if (foreignPath !== undefined) {
    throw new Error(`workspace staging refused: path is not in the lane change set`)
  }

  await runCheckedCommand(["git", "add", "--", ...relativePaths], target)
  return { stagedCount: relativePaths.length }
}

export async function commitWorkspaceChanges(input: {
  cacheRoot: string
  workingDirectory: string
  workspaceRef: string
  sourceRef: string
  message: string
  now?: Date
}): Promise<WorkspaceCommitResult> {
  if (input.message.length === 0 || input.message.includes("\0")) {
    throw new Error("workspace commit refused: invalid commit message")
  }
  const before = await captureWorkspaceChanges(input)
  if (before.state === "clean") return { state: "clean", capture: before }
  await stageWorkspacePaths({
    cacheRoot: input.cacheRoot,
    workingDirectory: input.workingDirectory,
    relativePaths: before.local.changedPaths,
  })
  const { target } = assertPylonOwnedWorkspaceTarget(input)
  const staged = await runQuietCommand(["git", "diff", "--cached", "--quiet"], target)
  if (staged === 0) return { state: "clean", capture: before }
  await runCheckedCommand(
    [
      "git",
      "-c",
      "user.email=pylon-workspace@example.invalid",
      "-c",
      "user.name=Pylon Workspace Materializer",
      "commit",
      "-m",
      input.message,
    ],
    target,
  )
  const head = await runTextCommand(["git", "rev-parse", "HEAD"], target)
  if (head.exitCode !== 0) throw new Error("workspace commit failed: unable to read HEAD")
  const commitSha = head.stdout.trim()
  const commitRef = stableRef("commit.pylon.workspace", commitSha)
  return {
    state: "committed",
    commitRef,
    commitSha,
    capture: {
      ...before,
      state: "committed",
      headCommit: commitSha,
      commitRef,
      generatedAt: (input.now ?? new Date()).toISOString(),
    },
  }
}

export function detectWorkspaceChangeConflicts(
  captures: ReadonlyArray<WorkspaceChangeCapture>,
): WorkspaceChangeConflictScan {
  const byFileRef = new Map<string, { sourceRef: string; workspaceRefs: Set<string> }>()
  for (const capture of captures) {
    if (capture.state === "clean") continue
    for (const fileRef of capture.fileRefs) {
      const existing = byFileRef.get(fileRef) ?? { sourceRef: capture.sourceRef, workspaceRefs: new Set<string>() }
      existing.workspaceRefs.add(capture.workspaceRef)
      byFileRef.set(fileRef, existing)
    }
  }
  const conflicts = [...byFileRef.entries()]
    .map(([fileRef, value]) => ({
      conflictRef: stableRef("conflict.pylon.workspace", `${fileRef}:${[...value.workspaceRefs].sort().join(":")}`),
      fileRef,
      sourceRef: value.sourceRef,
      workspaceRefs: [...value.workspaceRefs].sort(),
    }))
    .filter((conflict) => conflict.workspaceRefs.length > 1)
  return {
    state: conflicts.length === 0 ? "clear" : "conflicted",
    conflictRefs: conflicts.map((conflict) => conflict.conflictRef),
    conflicts,
  }
}

export function detectInFlightVirtualBranchConflicts(
  changes: ReadonlyArray<InFlightVirtualBranchChange>,
): VirtualBranchChangeConflictScan {
  type ConflictBucket = {
    repositoryFullName: string
    targetBranch: string
    sourceRefs: Set<string>
    virtualBranchRefs: Set<string>
    workspaceRefs: Set<string>
  }
  const byFileRef = new Map<string, ConflictBucket>()

  for (const change of changes) {
    if (change.capture.state === "clean") continue
    if (!githubFullNamePattern.test(change.target.repositoryFullName)) {
      throw new Error("virtual branch conflict scan refused: invalid repository name")
    }
    if (!gitBranchNamePattern.test(change.target.branch)) {
      throw new Error("virtual branch conflict scan refused: invalid target branch")
    }
    for (const path of change.capture.local.changedPaths) {
      const fileRef = virtualBranchChangeFileRef({
        repositoryFullName: change.target.repositoryFullName,
        targetBranch: change.target.branch,
        relativePath: path,
      })
      const existing = byFileRef.get(fileRef) ?? {
        repositoryFullName: change.target.repositoryFullName,
        targetBranch: change.target.branch,
        sourceRefs: new Set<string>(),
        virtualBranchRefs: new Set<string>(),
        workspaceRefs: new Set<string>(),
      }
      existing.sourceRefs.add(change.capture.sourceRef)
      existing.virtualBranchRefs.add(change.virtualBranchRef)
      existing.workspaceRefs.add(change.capture.workspaceRef)
      byFileRef.set(fileRef, existing)
    }
  }

  const conflicts = [...byFileRef.entries()]
    .map(([fileRef, value]) => ({
      conflictRef: stableRef(
        "conflict.pylon.virtual_branch",
        `${fileRef}:${[...value.virtualBranchRefs].sort().join(":")}`,
      ),
      fileRef,
      repositoryFullName: value.repositoryFullName,
      targetBranch: value.targetBranch,
      sourceRefs: [...value.sourceRefs].sort(),
      virtualBranchRefs: [...value.virtualBranchRefs].sort(),
      workspaceRefs: [...value.workspaceRefs].sort(),
    }))
    .filter((conflict) => conflict.virtualBranchRefs.length > 1)

  return {
    state: conflicts.length === 0 ? "clear" : "conflicted",
    conflictRefs: conflicts.map((conflict) => conflict.conflictRef),
    conflicts,
  }
}

/**
 * The default checkout strategy: an isolated detached checkout of the
 * pinned commit, fetched depth-1 from the single public origin. Runner
 * materialization never depends on mutable branch state.
 */
export const defaultGitCheckoutRunner: WorkspaceCheckoutRunner = async (
  workingDirectory,
  checkout,
) => {
  const baseCommitSha = checkoutBaseCommitSha(checkout)
  await rm(workingDirectory, { recursive: true, force: true })
  await mkdir(workingDirectory, { recursive: true })
  await runCheckedCommand(["git", "init"], workingDirectory, "reason.workspace_checkout.init_failed")
  // Each detached checkout is isolated, but disabling auto maintenance keeps a
  // background `git gc` from ever racing a concurrent sibling materialization.
  await disableGitAutoMaintenance(workingDirectory)
  await installScmAuthBrokerGitCredentialHelper({ checkout, cwd: workingDirectory })
  await runCheckedCommand(
    [
      "git",
      "remote",
      "add",
      "origin",
      `https://github.com/${checkout.repository.fullName}.git`,
    ],
    workingDirectory,
    "reason.workspace_checkout.remote_add_failed",
  )
  await runCheckedCommand(
    ["git", "fetch", "--depth", "1", "origin", baseCommitSha],
    workingDirectory,
    "reason.workspace_checkout.fetch_failed",
  )
  await runCheckedCommand(
    ["git", "checkout", "--detach", baseCommitSha],
    workingDirectory,
    "reason.workspace_checkout.checkout_failed",
  )
}

/**
 * Turns off git's background auto-gc/auto-maintenance for a repository. Auto
 * maintenance can fire asynchronously after a fetch/worktree command, escape
 * the materializer's critical section, and then collide with a concurrent
 * sibling materialization on the shared object store. Best-effort: a failure
 * to set config never fails the checkout.
 */
async function disableGitAutoMaintenance(gitDirectory: string): Promise<void> {
  await runQuietCommand(["git", "config", "gc.auto", "0"], gitDirectory)
  await runQuietCommand(["git", "config", "maintenance.auto", "false"], gitDirectory)
}

/**
 * Materializes an assignment-scoped workspace for a validated git_checkout
 * payload under the adapter's Pylon-owned cache root. The workspace ref is
 * derived from the lease, so two concurrent assignments for the same
 * repository always get separate refs and directories.
 */
export async function materializeGitCheckoutWorkspace(input: {
  cacheRoot: string
  checkout: GitCheckoutWorkspace
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
  refPrefix: string
}): Promise<MaterializedWorkspace> {
  const workspaceRef = stableRef(input.refPrefix, input.leaseRef)
  const workingDirectory = join(input.cacheRoot, workspaceRef)
  await mkdir(input.cacheRoot, { recursive: true })
  await (input.checkoutRunner ?? defaultGitCheckoutRunner)(workingDirectory, input.checkout)
  await installScmAuthBrokerGitCredentialHelper({ checkout: input.checkout, cwd: workingDirectory })
  return {
    cleanupRef: stableRef("cleanup.pylon.workspace", workspaceRef),
    sourceRef: checkoutSourceRef(input.checkout),
    workingDirectory,
    workspaceRef,
  }
}

/**
 * Removes one materialized workspace. Refuses to delete anything that does
 * not resolve strictly inside the given Pylon-owned cache root — cleanup
 * never operates from user text or outside internal workspace refs.
 */
export async function removeMaterializedWorkspace(input: {
  cacheRoot: string
  workingDirectory: string
}): Promise<void> {
  const { target } = assertPylonOwnedWorkspaceTarget(input)
  await rm(target, { recursive: true, force: true })
}

export async function pruneWorkspaceCacheDirectories(input: {
  cacheRoot: string
  maxEntries: number
  protectedWorkspaceRefs?: ReadonlyArray<string>
}): Promise<{ removedWorkspaceRefs: string[] }> {
  if (!Number.isFinite(input.maxEntries) || input.maxEntries < 1) {
    return { removedWorkspaceRefs: [] }
  }
  let entries: string[]
  try {
    entries = await readdir(input.cacheRoot)
  } catch {
    return { removedWorkspaceRefs: [] }
  }
  const protectedRefs = new Set(input.protectedWorkspaceRefs ?? [])
  const candidates: Array<{ workspaceRef: string; mtimeMs: number }> = []
  for (const entry of entries) {
    if (!entry.startsWith("workspace.pylon.") || protectedRefs.has(entry)) continue
    const workingDirectory = join(input.cacheRoot, entry)
    try {
      assertPylonOwnedWorkspaceTarget({ cacheRoot: input.cacheRoot, workingDirectory })
      const info = await stat(workingDirectory)
      if (!info.isDirectory()) continue
      candidates.push({ workspaceRef: entry, mtimeMs: info.mtimeMs })
    } catch {
      // Ignore malformed or concurrently removed entries.
    }
  }
  if (candidates.length <= input.maxEntries) return { removedWorkspaceRefs: [] }
  const stale = candidates
    .sort((left, right) => left.mtimeMs - right.mtimeMs)
    .slice(0, candidates.length - input.maxEntries)
  const removedWorkspaceRefs: string[] = []
  for (const candidate of stale) {
    try {
      await removeMaterializedWorkspace({
        cacheRoot: input.cacheRoot,
        workingDirectory: join(input.cacheRoot, candidate.workspaceRef),
      })
      removedWorkspaceRefs.push(candidate.workspaceRef)
    } catch {
      // Best-effort cache pressure relief; materialization correctness does not depend on it.
    }
  }
  return { removedWorkspaceRefs }
}

/**
 * Native git worktree support behind the materializer (issue #4799).
 *
 * "Worktree" is an implementation strategy inside Pylon — the wire contract
 * stays `workspace.kind = "git_checkout"`. A shared bare-repo cache keyed by
 * a stable hash of the public repository full name holds fetched objects;
 * each assignment gets an isolated detached worktree under the adapter's
 * cache root. Workspace lease records carry materialization state, TTL,
 * retention policy, and cleanup receipt refs; an opportunistic sweep on
 * every materialization keeps the cache self-maintaining without a daemon.
 */

export const WORKSPACE_MATERIALIZER_CAPABILITY_REF = "capability.pylon.workspace_materializer.v1"
export const WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF = "capability.pylon.workspace_cleanup_receipts.v1"
export const WORKSPACE_LEASE_SCHEMA = "openagents.pylon.workspace_lease.v1"

export const DEFAULT_WORKSPACE_TTL_SECONDS = 24 * 60 * 60

export type WorkspaceRetentionPolicy = "retain_until_ttl" | "remove_on_closeout"
export type WorkspaceStrategy = "git_worktree" | "detached_checkout" | "injected"
export type WorkspaceLeaseState = "materialized" | "cleaned"

export type WorkspaceLeaseRecord = {
  schema: typeof WORKSPACE_LEASE_SCHEMA
  workspaceRef: string
  cleanupRef: string
  sourceRef: string
  strategy: WorkspaceStrategy
  state: WorkspaceLeaseState
  materializedAt: string
  ttlSeconds: number
  retentionPolicy: WorkspaceRetentionPolicy
  generatedAt: string
  cleanupReceiptRef?: string
  cleanedAt?: string
  retentionReasonRef?: string
  lastCleanupAttemptAt?: string
  /**
   * Local-only mechanics. Never projected, never emitted in progress
   * events, closeouts, or any public surface.
   */
  local: {
    cacheRoot: string
    workingDirectory: string
    repositoryCacheDirectory?: string
    preparedWorktreeCache?: {
      root: string
      cacheKey: string
      diskBudgetBytes: number
      restore?: WorkspacePreparedWorktreeCacheUse
    }
    prebuiltBaselineCache?: {
      root: string
      cacheKey: string
      refreshCadenceSeconds: number
      metric?: WorkspacePrebuiltBaselineCacheMetric
    }
  }
}

export type PylonWorkspaceMaterializerOperation =
  | "workspace.materialize_with_lease"
  | "workspace.cleanup_expired"
  | "workspace.cleanup_oldest"
  | "workspace.release"
  | "workspace.lease_record_read"

export class PylonWorkspaceMaterializerError extends Error {
  readonly _tag = "PylonWorkspaceMaterializerError"
  readonly operation: PylonWorkspaceMaterializerOperation
  readonly reasonRef: string
  readonly causeRef: string
  readonly fallbackCloseoutUsed: boolean

  constructor(input: {
    readonly operation: PylonWorkspaceMaterializerOperation
    readonly reasonRef: string
    readonly causeRef: string
    readonly fallbackCloseoutUsed: boolean
  }) {
    super(`${input.operation} failed: ${input.reasonRef}`)
    this.name = "PylonWorkspaceMaterializerError"
    this.operation = input.operation
    this.reasonRef = input.reasonRef
    this.causeRef = input.causeRef
    this.fallbackCloseoutUsed = input.fallbackCloseoutUsed
  }
}

function workspaceMaterializerError(
  operation: PylonWorkspaceMaterializerOperation,
  reasonRef: string,
  cause: unknown,
): PylonWorkspaceMaterializerError {
  const causeLabel = cause instanceof Error ? `${cause.name}:${cause.message}` : String(cause)
  return new PylonWorkspaceMaterializerError({
    operation,
    reasonRef,
    causeRef: stableRef("cause.pylon.workspace_materializer", causeLabel),
    fallbackCloseoutUsed: false,
  })
}

export class PylonWorkspaceMaterializer extends Context.Service<
  PylonWorkspaceMaterializer,
  {
    readonly materializeWithLease: (
      input: MaterializeWithLeaseInput,
    ) => Effect.Effect<MaterializedWorkspace, PylonWorkspaceMaterializerError>
    readonly cleanupExpired: (input: {
      workspaceStateRoot: string
      now?: Date
    }) => Effect.Effect<
      { cleanupReceiptRefs: string[]; retainedWorkspaceRefs: string[] },
      PylonWorkspaceMaterializerError
    >
    readonly cleanupOldest: (input: {
      workspaceStateRoot: string
      maxMaterializedWorkspaces: number
      minimumAgeSeconds?: number
      now?: Date
    }) => Effect.Effect<
      { cleanupReceiptRefs: string[]; retainedWorkspaceRefs: string[] },
      PylonWorkspaceMaterializerError
    >
    readonly release: (input: {
      workspaceStateRoot: string
      workspaceRef: string
      now?: Date
    }) => Effect.Effect<
      { cleanupReceiptRef?: string; retainedWorkspaceRef?: string; retentionReasonRef?: string } | null,
      PylonWorkspaceMaterializerError
    >
    readonly leaseRecordFor: (input: {
      workspaceStateRoot: string
      workspaceRef: string
    }) => Effect.Effect<WorkspaceLeaseRecord, PylonWorkspaceMaterializerError>
  }
>()("PylonWorkspaceMaterializer") {}

/** Stable cache key for one public repository's shared bare clone. */
export function repositoryCacheKeyFor(fullName: string): string {
  return createHash("sha256").update(fullName).digest("hex").slice(0, 24)
}

export function preparedWorktreeCacheKeyFor(input: {
  repositoryFullName: string
  baselineCommitSha: string
}): string {
  return createHash("sha256")
    .update(`${input.repositoryFullName}\0${input.baselineCommitSha.toLowerCase()}`)
    .digest("hex")
    .slice(0, 32)
}

function preparedWorktreeCacheKeyForCheckout(checkout: GitCheckoutWorkspace): string {
  return preparedWorktreeCacheKeyFor({
    baselineCommitSha: checkoutBaseCommitSha(checkout),
    repositoryFullName: checkout.repository.fullName,
  })
}

function preparedWorktreeCacheDirectory(root: string, cacheKey: string): string {
  return join(root, `prepared.${cacheKey}`)
}

function preparedWorktreeCacheRecordPath(preparedDirectory: string): string {
  return `${preparedDirectory}.json`
}

function preparedWorktreeIntegrityRef(input: {
  cacheKey: string
  repositoryFullName: string
  baselineCommitSha: string
  state: "ready"
}): string {
  return stableRef(
    "integrity.pylon.prepared_worktree",
    `${input.cacheKey}:${input.repositoryFullName}:${input.baselineCommitSha}:${input.state}`,
  )
}

export function prebuiltBaselineCacheKeyFor(input: {
  repositoryFullName: string
  branch: string
}): string {
  return createHash("sha256")
    .update(`${input.repositoryFullName}\0${input.branch}`)
    .digest("hex")
    .slice(0, 32)
}

function prebuiltBaselineCacheKeyForCheckout(checkout: GitCheckoutWorkspace): string {
  return prebuiltBaselineCacheKeyFor({
    branch: checkout.repository.branch,
    repositoryFullName: checkout.repository.fullName,
  })
}

function prebuiltBaselineCacheDirectory(root: string, cacheKey: string): string {
  return join(root, `prebuilt.${cacheKey}`)
}

function prebuiltBaselineCacheRecordPath(prebuiltDirectory: string): string {
  return `${prebuiltDirectory}.json`
}

function prebuiltBaselineRegistryRef(cacheKey: string): string {
  return stableRef("registry.pylon.prebuilt_baseline", cacheKey)
}

function prebuiltBaselineIntegrityRef(input: {
  cacheKey: string
  repositoryFullName: string
  branch: string
  baselineCommitSha: string
  state: "ready"
}): string {
  return stableRef(
    "integrity.pylon.prebuilt_baseline",
    `${input.cacheKey}:${input.repositoryFullName}:${input.branch}:${input.baselineCommitSha}:${input.state}`,
  )
}

function boundedPrebuiltBaselineRefreshCadenceSeconds(value: number | undefined): number {
  if (value === undefined) return DEFAULT_PREBUILT_BASELINE_REFRESH_CADENCE_SECONDS
  if (!Number.isFinite(value) || value < 1) return DEFAULT_PREBUILT_BASELINE_REFRESH_CADENCE_SECONDS
  return Math.floor(value)
}

export const defaultPrebuiltBaselineSetupRunner: WorkspacePrebuiltBaselineSetupRunner = async ({
  workingDirectory,
}) => {
  const skipped: WorkspacePrebuiltBaselineSetupResult = {
    state: "skipped",
    setupRef: "setup.pylon.prebuilt_baseline.no_supported_lockfile.v1",
  }
  if (!existsSync(join(workingDirectory, "package.json"))) return skipped
  if (!existsSync(join(workingDirectory, "bun.lock")) && !existsSync(join(workingDirectory, "bun.lockb"))) {
    return skipped
  }
  await runCheckedCommand(
    ["bun", "install", "--frozen-lockfile"],
    workingDirectory,
    "reason.workspace_prebuilt_baseline.setup_failed",
  )
  return {
    state: "completed",
    setupRef: WORKSPACE_PREBUILT_BASELINE_BUN_INSTALL_SETUP_REF,
    commandRef: "command.pylon.prebuilt_baseline.bun_install_frozen_lockfile",
  }
}

export type GitWorktreeCheckoutRunnerOptions = {
  repositoryCacheRoot: string
  /**
   * Test seam only. Production call sites must never override the remote:
   * the URL is always derived from the validated public repository full
   * name, and assignment payloads never carry URLs or local paths.
   */
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
}

// Multiple Pylon processes can materialize concurrent assignments for the same
// repository; git's own lockfiles (shallow.lock, ref locks) make concurrent
// fetch/worktree-add against one bare repo flaky, so cache operations are
// serialized per bare directory across processes.
const repositoryCacheLocks = new Map<string, Promise<unknown>>()
const repositoryCacheProcessLockTimeoutMs = 5 * 60 * 1000
// The live holder heartbeats the lock directory mtime, so a still-running but
// slow holder is never considered stale. Only a holder that has stopped
// heartbeating (crashed/killed) for longer than this window is reclaimed.
const repositoryCacheProcessLockStaleMs = 30 * 1000
const repositoryCacheProcessLockHeartbeatMs = 5 * 1000
const repositoryCacheProcessLockPollMs = 50

function isNodeErrorWithCode(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && (error as { code?: unknown }).code === code
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms))
}

type RepositoryCacheProcessLock = { lockDirectory: string; heartbeat: ReturnType<typeof setInterval> }

type RepositoryCacheProcessLockOwner = {
  pid: number
  acquiredAt?: string
}

function repositoryCacheLockOwnerFrom(value: unknown): RepositoryCacheProcessLockOwner | null {
  if (value === null || typeof value !== "object") return null
  const record = value as { pid?: unknown; acquiredAt?: unknown }
  if (typeof record.pid !== "number" || !Number.isInteger(record.pid) || record.pid <= 0) return null
  return {
    pid: record.pid,
    ...(typeof record.acquiredAt === "string" ? { acquiredAt: record.acquiredAt } : {}),
  }
}

export async function repositoryCacheProcessLockOwnerIsLive(lockDirectory: string): Promise<boolean> {
  let owner: RepositoryCacheProcessLockOwner | null = null
  try {
    owner = repositoryCacheLockOwnerFrom(JSON.parse(await readFile(join(lockDirectory, "owner.json"), "utf8")))
  } catch {
    return false
  }
  if (owner === null) return false
  try {
    process.kill(owner.pid, 0)
    return true
  } catch (error) {
    const code = error instanceof Error && "code" in error ? (error as { code?: unknown }).code : undefined
    return code === "EPERM"
  }
}

async function acquireRepositoryCacheProcessLock(bareDirectory: string): Promise<RepositoryCacheProcessLock> {
  const lockDirectory = `${bareDirectory}.pylon-lock`
  const startedAt = Date.now()
  await mkdir(dirname(lockDirectory), { recursive: true })

  while (true) {
    try {
      await mkdir(lockDirectory)
      await writeFile(
        join(lockDirectory, "owner.json"),
        `${JSON.stringify({ pid: process.pid, acquiredAt: new Date().toISOString() }, null, 2)}\n`,
      )
      // Refresh the lock directory mtime while we hold it so a concurrent waiter
      // never mistakes a slow-but-live holder for a crashed one and steals it.
      const heartbeat = setInterval(() => {
        const stamp = new Date()
        void utimes(lockDirectory, stamp, stamp).catch(() => undefined)
      }, repositoryCacheProcessLockHeartbeatMs)
      if (typeof heartbeat.unref === "function") heartbeat.unref()
      return { lockDirectory, heartbeat }
    } catch (error) {
      if (!isNodeErrorWithCode(error, "EEXIST")) throw error
    }

    const lockStat = await stat(lockDirectory).catch((error) => {
      if (isNodeErrorWithCode(error, "ENOENT")) return null
      throw error
    })
    if (lockStat === null) continue

    const now = Date.now()
    if (now - lockStat.mtimeMs > repositoryCacheProcessLockStaleMs) {
      if (await repositoryCacheProcessLockOwnerIsLive(lockDirectory)) {
        await sleep(repositoryCacheProcessLockPollMs)
        continue
      }
      await rm(lockDirectory, { recursive: true, force: true })
      continue
    }
    if (now - startedAt > repositoryCacheProcessLockTimeoutMs) {
      throw new WorkspaceCheckoutError("reason.workspace_checkout.cache_lock_timeout")
    }
    await sleep(repositoryCacheProcessLockPollMs)
  }
}

async function withRepositoryCacheProcessLock<T>(
  bareDirectory: string,
  work: () => Promise<T>,
): Promise<T> {
  const lock = await acquireRepositoryCacheProcessLock(bareDirectory)
  try {
    return await work()
  } finally {
    clearInterval(lock.heartbeat)
    await rm(lock.lockDirectory, { recursive: true, force: true })
  }
}

async function withRepositoryCacheLock<T>(bareDirectory: string, work: () => Promise<T>): Promise<T> {
  const previous = repositoryCacheLocks.get(bareDirectory) ?? Promise.resolve()
  const next = previous.catch(() => undefined).then(() => withRepositoryCacheProcessLock(bareDirectory, work))
  repositoryCacheLocks.set(bareDirectory, next.catch(() => undefined))
  return next
}

/**
 * Builds the worktree checkout strategy: ensure the shared bare cache for
 * the repository, fetch the branch window as an optimization, fall back to the
 * pinned commit when the branch name is stale or too shallow, verify the commit
 * object exists before any worktree work, then add an isolated detached
 * worktree at the assignment-scoped path. The pinned commit gets a cache-local
 * ref so gc can never collect it while cached.
 */
export function createGitWorktreeCheckoutRunner(
  options: GitWorktreeCheckoutRunnerOptions,
): WorkspaceCheckoutRunner {
  return async (workingDirectory, checkout) => {
    const repository = checkout.repository
    const baseCommitSha = checkoutBaseCommitSha(checkout)
    const bareDirectory = join(
      options.repositoryCacheRoot,
      `${repositoryCacheKeyFor(repository.fullName)}.git`,
    )
    await withRepositoryCacheLock(bareDirectory, async () => {
      await mkdir(options.repositoryCacheRoot, { recursive: true })
      if (!existsSync(join(bareDirectory, "HEAD"))) {
        await runCheckedCommand(
          ["git", "init", "--bare", bareDirectory],
          options.repositoryCacheRoot,
          "reason.workspace_checkout.bare_init_failed",
        )
      }
      // Auto-gc on the shared bare object store is the main source of
      // out-of-lock contention: a background gc triggered by one assignment's
      // fetch can hold pack/ref locks while the next assignment's worktree add
      // runs. Disable it on every materialization so pre-existing caches that
      // were created before this fix are also covered.
      await disableGitAutoMaintenance(bareDirectory)
      await installScmAuthBrokerGitCredentialHelper({ checkout, cwd: bareDirectory })
      const commitArg = `${baseCommitSha}^{commit}`
      const cached = (await runQuietCommand(["git", "cat-file", "-e", commitArg], bareDirectory)) === 0
      if (!cached) {
        const remoteUrl =
          options.remoteUrlFor?.(checkout) ?? `https://github.com/${repository.fullName}.git`
        const branchRefSpec = `+refs/heads/${repository.branch}:refs/remotes/pylon/${repository.branch}`
        const branchFetchOk = (await runQuietCommand(
          ["git", "fetch", "--depth", "50", remoteUrl, branchRefSpec],
          bareDirectory,
        )) === 0
        const branchWindowCached =
          (await runQuietCommand(["git", "cat-file", "-e", commitArg], bareDirectory)) === 0
        if (branchFetchOk && !branchWindowCached) {
          await runQuietCommand(
            ["git", "fetch", "--deepen", "450", remoteUrl, branchRefSpec],
            bareDirectory,
          )
        }
        const deepenedCached =
          (await runQuietCommand(["git", "cat-file", "-e", commitArg], bareDirectory)) === 0
        if (!deepenedCached) {
          await runQuietCommand(
            ["git", "fetch", "--depth", "1", remoteUrl, baseCommitSha],
            bareDirectory,
          )
        }
        await runCheckedCommand(
          ["git", "cat-file", "-e", commitArg],
          bareDirectory,
          "reason.workspace_checkout.commit_missing_after_fetch",
        )
      }
      // best-effort gc pin; materialization correctness never depends on it
      await runQuietCommand(
        ["git", "update-ref", `refs/pinned/${baseCommitSha}`, baseCommitSha],
        bareDirectory,
      )
      if (checkout.virtualBranch !== undefined) {
        await runQuietCommand(
          ["git", "update-ref", `refs/virtual/${checkout.virtualBranch.branchName}`, baseCommitSha],
          bareDirectory,
        )
      }
      await rm(workingDirectory, { recursive: true, force: true })
      // a previously removed worktree leaves admin metadata that would block
      // re-adding the same path
      await runQuietCommand(["git", "worktree", "prune"], bareDirectory)
      await runCheckedCommand(
        ["git", "worktree", "add", "--detach", workingDirectory, baseCommitSha],
        bareDirectory,
        "reason.workspace_checkout.worktree_add_failed",
      )
      if (checkout.scmAuthBroker !== undefined) {
        await installScmAuthBrokerGitCredentialHelper({
          checkout,
          configScope: "gitdir_include",
          cwd: workingDirectory,
        })
      }
    })
  }
}

function boundedPreparedWorktreeDiskBudgetBytes(value: number | undefined): number {
  if (value === undefined) return DEFAULT_PREPARED_WORKTREE_CACHE_DISK_BUDGET_BYTES
  if (!Number.isFinite(value) || value < 0) return DEFAULT_PREPARED_WORKTREE_CACHE_DISK_BUDGET_BYTES
  return Math.floor(value)
}

async function directorySizeBytes(path: string, seenRealPaths = new Set<string>()): Promise<number> {
  let info: Awaited<ReturnType<typeof lstat>>
  try {
    info = await lstat(path)
  } catch {
    return 0
  }
  if (info.isSymbolicLink()) return 0

  let realPath: string
  try {
    realPath = realpathSync(path)
  } catch {
    realPath = resolve(path)
  }
  if (seenRealPaths.has(realPath)) return 0
  seenRealPaths.add(realPath)

  const blocks = (info as { blocks?: number }).blocks
  const ownBytes = typeof blocks === "number" && Number.isFinite(blocks) ? blocks * 512 : info.size
  if (!info.isDirectory()) return ownBytes

  let entries: string[]
  try {
    entries = await readdir(path)
  } catch {
    return ownBytes
  }
  let total = ownBytes
  for (const entry of entries) {
    total += await directorySizeBytes(join(path, entry), seenRealPaths)
  }
  return total
}

function preparedWorktreeCacheRecordFrom(value: unknown): WorkspacePreparedWorktreeCacheRecord | null {
  if (value === null || typeof value !== "object") return null
  const record = value as WorkspacePreparedWorktreeCacheRecord
  if (record.schema !== WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA) return null
  if (record.state !== "ready") return null
  if (typeof record.cacheKey !== "string" || !/^[a-f0-9]{32}$/.test(record.cacheKey)) return null
  if (typeof record.repositoryFullName !== "string" || !githubFullNamePattern.test(record.repositoryFullName)) return null
  if (typeof record.baselineCommitSha !== "string" || !gitCommitShaPattern.test(record.baselineCommitSha)) return null
  if (record.sourceRef !== `${record.repositoryFullName}:${record.baselineCommitSha}`) return null
  if (record.reuseReason !== "post_completion_snapshot" && record.reuseReason !== "restore_quick_sync_reset") return null
  if (typeof record.integrityRef !== "string" || !publicRefPattern.test(record.integrityRef)) return null
  if (typeof record.createdAt !== "string" || typeof record.updatedAt !== "string" || typeof record.lastUsedAt !== "string") {
    return null
  }
  if (typeof record.useCount !== "number" || !Number.isInteger(record.useCount) || record.useCount < 0) return null
  if (typeof record.sizeBytes !== "number" || !Number.isFinite(record.sizeBytes) || record.sizeBytes < 0) return null
  if (record.local === null || typeof record.local !== "object") return null
  if (typeof record.local.preparedDirectory !== "string" || record.local.preparedDirectory.length === 0) return null
  const expectedIntegrityRef = preparedWorktreeIntegrityRef({
    baselineCommitSha: record.baselineCommitSha,
    cacheKey: record.cacheKey,
    repositoryFullName: record.repositoryFullName,
    state: "ready",
  })
  return record.integrityRef === expectedIntegrityRef ? record : null
}

async function readPreparedWorktreeCacheRecord(
  preparedDirectory: string,
): Promise<WorkspacePreparedWorktreeCacheRecord | null> {
  try {
    return preparedWorktreeCacheRecordFrom(
      JSON.parse(await readFile(preparedWorktreeCacheRecordPath(preparedDirectory), "utf8")),
    )
  } catch {
    return null
  }
}

async function writePreparedWorktreeCacheRecord(
  preparedDirectory: string,
  record: WorkspacePreparedWorktreeCacheRecord,
): Promise<void> {
  await mkdir(dirname(preparedDirectory), { recursive: true })
  await writeFile(
    preparedWorktreeCacheRecordPath(preparedDirectory),
    `${JSON.stringify(record, null, 2)}\n`,
    { mode: 0o600 },
  )
  await chmod(preparedWorktreeCacheRecordPath(preparedDirectory), 0o600).catch(() => undefined)
}

async function removePreparedWorktreeCacheEntry(preparedDirectory: string): Promise<void> {
  await rm(preparedDirectory, { recursive: true, force: true })
  await rm(preparedWorktreeCacheRecordPath(preparedDirectory), { force: true })
}

function prebuiltBaselineSetupResultFrom(value: unknown): WorkspacePrebuiltBaselineSetupResult | null {
  if (value === null || typeof value !== "object") return null
  const setup = value as WorkspacePrebuiltBaselineSetupResult
  if (setup.state !== "completed" && setup.state !== "skipped") return null
  if (typeof setup.setupRef !== "string" || !publicRefPattern.test(setup.setupRef)) return null
  if (setup.commandRef !== undefined && (typeof setup.commandRef !== "string" || !publicRefPattern.test(setup.commandRef))) {
    return null
  }
  return {
    state: setup.state,
    setupRef: setup.setupRef,
    ...(setup.commandRef === undefined ? {} : { commandRef: setup.commandRef }),
  }
}

function prebuiltBaselineCacheRecordFrom(value: unknown): WorkspacePrebuiltBaselineCacheRecord | null {
  if (value === null || typeof value !== "object") return null
  const record = value as WorkspacePrebuiltBaselineCacheRecord
  if (record.schema !== WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA) return null
  if (record.state !== "ready") return null
  if (typeof record.cacheKey !== "string" || !/^[a-f0-9]{32}$/.test(record.cacheKey)) return null
  if (record.registryRef !== prebuiltBaselineRegistryRef(record.cacheKey)) return null
  if (typeof record.repositoryFullName !== "string" || !githubFullNamePattern.test(record.repositoryFullName)) return null
  if (typeof record.branch !== "string" || !gitBranchNamePattern.test(record.branch)) return null
  if (typeof record.baselineCommitSha !== "string" || !gitCommitShaPattern.test(record.baselineCommitSha)) return null
  if (record.sourceRef !== `${record.repositoryFullName}:${record.baselineCommitSha}`) return null
  if (typeof record.integrityRef !== "string" || !publicRefPattern.test(record.integrityRef)) return null
  const expectedIntegrityRef = prebuiltBaselineIntegrityRef({
    baselineCommitSha: record.baselineCommitSha,
    branch: record.branch,
    cacheKey: record.cacheKey,
    repositoryFullName: record.repositoryFullName,
    state: "ready",
  })
  if (record.integrityRef !== expectedIntegrityRef) return null
  for (const value of [
    record.createdAt,
    record.updatedAt,
    record.upstreamCheckedAt,
    record.refreshedAt,
    record.lastUsedAt,
  ]) {
    if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) return null
  }
  if (
    typeof record.refreshCadenceSeconds !== "number" ||
    !Number.isInteger(record.refreshCadenceSeconds) ||
    record.refreshCadenceSeconds < 1
  ) {
    return null
  }
  if (typeof record.hitCount !== "number" || !Number.isInteger(record.hitCount) || record.hitCount < 0) return null
  if (typeof record.missCount !== "number" || !Number.isInteger(record.missCount) || record.missCount < 0) return null
  if (record.lastMissReasonRef !== undefined && (typeof record.lastMissReasonRef !== "string" || !publicRefPattern.test(record.lastMissReasonRef))) {
    return null
  }
  if (typeof record.sizeBytes !== "number" || !Number.isFinite(record.sizeBytes) || record.sizeBytes < 0) return null
  const setup = prebuiltBaselineSetupResultFrom(record.setup)
  if (setup === null) return null
  if (record.local === null || typeof record.local !== "object") return null
  if (typeof record.local.prebuiltDirectory !== "string" || record.local.prebuiltDirectory.length === 0) return null
  return { ...record, setup }
}

async function readPrebuiltBaselineCacheRecord(
  prebuiltDirectory: string,
): Promise<WorkspacePrebuiltBaselineCacheRecord | null> {
  try {
    return prebuiltBaselineCacheRecordFrom(
      JSON.parse(await readFile(prebuiltBaselineCacheRecordPath(prebuiltDirectory), "utf8")),
    )
  } catch {
    return null
  }
}

async function writePrebuiltBaselineCacheRecord(
  prebuiltDirectory: string,
  record: WorkspacePrebuiltBaselineCacheRecord,
): Promise<void> {
  await mkdir(dirname(prebuiltDirectory), { recursive: true })
  await writeFile(
    prebuiltBaselineCacheRecordPath(prebuiltDirectory),
    `${JSON.stringify(record, null, 2)}\n`,
    { mode: 0o600 },
  )
  await chmod(prebuiltBaselineCacheRecordPath(prebuiltDirectory), 0o600).catch(() => undefined)
}

async function removePrebuiltBaselineCacheEntry(prebuiltDirectory: string): Promise<void> {
  await rm(prebuiltDirectory, { recursive: true, force: true })
  await rm(prebuiltBaselineCacheRecordPath(prebuiltDirectory), { force: true })
}

async function validatePrebuiltBaselineCacheEntry(input: {
  cacheKey: string
  checkout: GitCheckoutWorkspace
  prebuiltDirectory: string
}): Promise<WorkspacePrebuiltBaselineCacheRecord | null> {
  const record = await readPrebuiltBaselineCacheRecord(input.prebuiltDirectory)
  if (record === null) return null
  if (record.cacheKey !== input.cacheKey) return null
  if (record.repositoryFullName !== input.checkout.repository.fullName) return null
  if (record.branch !== input.checkout.repository.branch) return null
  if (canonicalPath(record.local.prebuiltDirectory) !== canonicalPath(input.prebuiltDirectory)) return null
  if (!existsSync(join(input.prebuiltDirectory, ".git"))) return null
  const root = await runTextCommand(["git", "rev-parse", "--show-toplevel"], input.prebuiltDirectory)
  if (root.exitCode !== 0 || canonicalPath(root.stdout.trim()) !== canonicalPath(input.prebuiltDirectory)) return null
  const head = await runTextCommand(["git", "rev-parse", "HEAD"], input.prebuiltDirectory)
  if (head.exitCode !== 0 || head.stdout.trim().toLowerCase() !== record.baselineCommitSha.toLowerCase()) return null
  const commitExists = await runQuietCommand(["git", "cat-file", "-e", `${record.baselineCommitSha}^{commit}`], input.prebuiltDirectory)
  if (commitExists !== 0) return null
  const unstaged = await runQuietCommand(["git", "diff", "--quiet", "HEAD", "--"], input.prebuiltDirectory)
  const staged = await runQuietCommand(["git", "diff", "--cached", "--quiet"], input.prebuiltDirectory)
  if (unstaged !== 0 || staged !== 0) return null
  return record
}

async function resolvePrebuiltBaselineUpstreamCommit(input: {
  checkout: GitCheckoutWorkspace
  prebuiltBaselineCacheRoot: string
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
}): Promise<string | null> {
  await mkdir(input.prebuiltBaselineCacheRoot, { recursive: true })
  const remoteUrl = input.remoteUrlFor?.(input.checkout) ?? `https://github.com/${input.checkout.repository.fullName}.git`
  const result = await runTextCommand(
    ["git", "ls-remote", remoteUrl, `refs/heads/${input.checkout.repository.branch}`],
    input.prebuiltBaselineCacheRoot,
  )
  if (result.exitCode !== 0) return null
  const commitSha = result.stdout.trim().split(/\s+/)[0] ?? ""
  return gitCommitShaPattern.test(commitSha) ? commitSha : null
}

async function buildPrebuiltBaselineCacheEntry(input: {
  baselineCommitSha: string
  cacheKey: string
  checkout: GitCheckoutWorkspace
  now: Date
  prebuiltBaselineCacheRoot: string
  refreshCadenceSeconds: number
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  setupRunner: WorkspacePrebuiltBaselineSetupRunner
}): Promise<WorkspacePrebuiltBaselineCacheRecord> {
  const prebuiltDirectory = prebuiltBaselineCacheDirectory(input.prebuiltBaselineCacheRoot, input.cacheKey)
  const tempDirectory = `${prebuiltDirectory}.tmp-${process.pid}-${Date.now()}`
  const timestamp = input.now.toISOString()
  const checkout = {
    ...input.checkout,
    repository: {
      ...input.checkout.repository,
      commitSha: input.baselineCommitSha,
    },
    virtualBranch: undefined,
  } satisfies GitCheckoutWorkspace

  await withRepositoryCacheLock(prebuiltDirectory, async () => {
    await mkdir(input.prebuiltBaselineCacheRoot, { recursive: true })
    await rm(tempDirectory, { recursive: true, force: true })
    await rm(prebuiltBaselineCacheRecordPath(tempDirectory), { force: true })
    await mkdir(tempDirectory, { recursive: true })
    await runCheckedCommand(["git", "init"], tempDirectory, "reason.workspace_prebuilt_baseline.init_failed")
    await disableGitAutoMaintenance(tempDirectory)
    await installScmAuthBrokerGitCredentialHelper({ checkout, cwd: tempDirectory })
    await runCheckedCommand(
      [
        "git",
        "remote",
        "add",
        "origin",
        input.remoteUrlFor?.(checkout) ?? `https://github.com/${checkout.repository.fullName}.git`,
      ],
      tempDirectory,
      "reason.workspace_prebuilt_baseline.remote_add_failed",
    )
    await runCheckedCommand(
      ["git", "fetch", "--depth", "1", "origin", input.baselineCommitSha],
      tempDirectory,
      "reason.workspace_prebuilt_baseline.fetch_failed",
    )
    await runCheckedCommand(
      ["git", "checkout", "--detach", input.baselineCommitSha],
      tempDirectory,
      "reason.workspace_prebuilt_baseline.checkout_failed",
    )
    const setup = await input.setupRunner({ checkout, workingDirectory: tempDirectory })
    const cleanTracked = await runQuietCommand(["git", "diff", "--quiet", "HEAD", "--"], tempDirectory)
    const cleanIndex = await runQuietCommand(["git", "diff", "--cached", "--quiet"], tempDirectory)
    if (cleanTracked !== 0 || cleanIndex !== 0) {
      throw new WorkspaceCheckoutError("reason.workspace_prebuilt_baseline.setup_left_tracked_changes")
    }
    const unignored = await runTextCommand(
      ["git", "ls-files", "--others", "--exclude-standard", "-z"],
      tempDirectory,
    )
    if (unignored.exitCode !== 0 || unignored.stdout.length > 0) {
      throw new WorkspaceCheckoutError("reason.workspace_prebuilt_baseline.setup_left_unignored_artifacts")
    }
    const credentialScan = await scanLongLivedScmCredentials({
      roots: [{ rootRef: prebuiltBaselineRegistryRef(input.cacheKey), path: tempDirectory }],
    })
    if (credentialScan.state === "leaked") throw new WorkspaceScmCredentialPolicyError(credentialScan)

    const previous = await readPrebuiltBaselineCacheRecord(prebuiltDirectory)
    const integrityRef = prebuiltBaselineIntegrityRef({
      baselineCommitSha: input.baselineCommitSha,
      branch: checkout.repository.branch,
      cacheKey: input.cacheKey,
      repositoryFullName: checkout.repository.fullName,
      state: "ready",
    })
    const preliminary: WorkspacePrebuiltBaselineCacheRecord = {
      schema: WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA,
      cacheKey: input.cacheKey,
      registryRef: prebuiltBaselineRegistryRef(input.cacheKey),
      repositoryFullName: checkout.repository.fullName,
      branch: checkout.repository.branch,
      baselineCommitSha: input.baselineCommitSha,
      sourceRef: `${checkout.repository.fullName}:${input.baselineCommitSha}`,
      state: "ready",
      integrityRef,
      createdAt: previous?.createdAt ?? timestamp,
      updatedAt: timestamp,
      upstreamCheckedAt: timestamp,
      refreshedAt: timestamp,
      refreshCadenceSeconds: input.refreshCadenceSeconds,
      lastUsedAt: previous?.lastUsedAt ?? timestamp,
      hitCount: previous?.hitCount ?? 0,
      missCount: previous?.missCount ?? 0,
      ...(previous?.lastMissReasonRef === undefined ? {} : { lastMissReasonRef: previous.lastMissReasonRef }),
      sizeBytes: 0,
      setup,
      local: { prebuiltDirectory },
    }
    await writePrebuiltBaselineCacheRecord(tempDirectory, preliminary)
    const sizeBytes =
      await directorySizeBytes(tempDirectory) +
      await directorySizeBytes(prebuiltBaselineCacheRecordPath(tempDirectory))
    await writePrebuiltBaselineCacheRecord(tempDirectory, { ...preliminary, sizeBytes })
    await removePrebuiltBaselineCacheEntry(prebuiltDirectory)
    await rename(tempDirectory, prebuiltDirectory)
    await rename(prebuiltBaselineCacheRecordPath(tempDirectory), prebuiltBaselineCacheRecordPath(prebuiltDirectory))
  })

  const record = await readPrebuiltBaselineCacheRecord(prebuiltDirectory)
  if (record === null) throw new WorkspaceCheckoutError("reason.workspace_prebuilt_baseline.registry_write_failed")
  return record
}

async function ensurePrebuiltBaselineCacheEntry(input: {
  cacheKey: string
  checkout: GitCheckoutWorkspace
  now: Date
  prebuiltBaselineCacheRoot: string
  refreshCadenceSeconds: number
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  setupRunner: WorkspacePrebuiltBaselineSetupRunner
}): Promise<WorkspacePrebuiltBaselineCacheRecord | null> {
  const prebuiltDirectory = prebuiltBaselineCacheDirectory(input.prebuiltBaselineCacheRoot, input.cacheKey)
  let existing = await validatePrebuiltBaselineCacheEntry({
    cacheKey: input.cacheKey,
    checkout: input.checkout,
    prebuiltDirectory,
  })
  if (existing === null) {
    await removePrebuiltBaselineCacheEntry(prebuiltDirectory)
  } else {
    const checkedAtMs = Date.parse(existing.upstreamCheckedAt)
    if (Number.isFinite(checkedAtMs) && input.now.getTime() - checkedAtMs < input.refreshCadenceSeconds * 1000) {
      return existing
    }
  }

  const upstreamCommitSha = await resolvePrebuiltBaselineUpstreamCommit({
    checkout: input.checkout,
    prebuiltBaselineCacheRoot: input.prebuiltBaselineCacheRoot,
    ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
  })
  if (upstreamCommitSha === null) return existing

  if (existing !== null && existing.baselineCommitSha.toLowerCase() === upstreamCommitSha.toLowerCase()) {
    const timestamp = input.now.toISOString()
    existing = {
      ...existing,
      updatedAt: timestamp,
      upstreamCheckedAt: timestamp,
      refreshCadenceSeconds: input.refreshCadenceSeconds,
      sizeBytes: await directorySizeBytes(existing.local.prebuiltDirectory),
    }
    await writePrebuiltBaselineCacheRecord(existing.local.prebuiltDirectory, existing)
    return existing
  }

  return buildPrebuiltBaselineCacheEntry({
    baselineCommitSha: upstreamCommitSha,
    cacheKey: input.cacheKey,
    checkout: input.checkout,
    now: input.now,
    prebuiltBaselineCacheRoot: input.prebuiltBaselineCacheRoot,
    refreshCadenceSeconds: input.refreshCadenceSeconds,
    ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
    setupRunner: input.setupRunner,
  })
}

function prebuiltBaselineMetric(input: {
  baselineCommitSha?: string
  branch: string
  cacheKey: string
  checkedAt: string
  hitCount: number
  missCount: number
  reasonRef: string
  repositoryFullName: string
  requestedCommitSha: string
  state: "hit" | "miss"
}): WorkspacePrebuiltBaselineCacheMetric {
  return {
    schema: WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA,
    cacheKey: input.cacheKey,
    registryRef: prebuiltBaselineRegistryRef(input.cacheKey),
    repositoryFullName: input.repositoryFullName,
    branch: input.branch,
    requestedCommitSha: input.requestedCommitSha,
    state: input.state,
    reasonRef: input.reasonRef,
    checkedAt: input.checkedAt,
    hitCount: input.hitCount,
    missCount: input.missCount,
    ...(input.baselineCommitSha === undefined ? {} : { baselineCommitSha: input.baselineCommitSha }),
  }
}

async function recordPrebuiltBaselineMiss(input: {
  checkout: GitCheckoutWorkspace
  cacheKey: string
  now: Date
  reasonRef: string
  record?: WorkspacePrebuiltBaselineCacheRecord | null
}): Promise<WorkspacePrebuiltBaselineCacheMetric> {
  const requestedCommitSha = checkoutBaseCommitSha(input.checkout)
  if (input.record === undefined || input.record === null) {
    return prebuiltBaselineMetric({
      branch: input.checkout.repository.branch,
      cacheKey: input.cacheKey,
      checkedAt: input.now.toISOString(),
      hitCount: 0,
      missCount: 1,
      reasonRef: input.reasonRef,
      repositoryFullName: input.checkout.repository.fullName,
      requestedCommitSha,
      state: "miss",
    })
  }
  const timestamp = input.now.toISOString()
  const nextRecord: WorkspacePrebuiltBaselineCacheRecord = {
    ...input.record,
    updatedAt: timestamp,
    lastMissReasonRef: input.reasonRef,
    missCount: input.record.missCount + 1,
  }
  await writePrebuiltBaselineCacheRecord(input.record.local.prebuiltDirectory, nextRecord)
  return prebuiltBaselineMetric({
    baselineCommitSha: nextRecord.baselineCommitSha,
    branch: nextRecord.branch,
    cacheKey: nextRecord.cacheKey,
    checkedAt: timestamp,
    hitCount: nextRecord.hitCount,
    missCount: nextRecord.missCount,
    reasonRef: input.reasonRef,
    repositoryFullName: nextRecord.repositoryFullName,
    requestedCommitSha,
    state: "miss",
  })
}

async function recordPrebuiltBaselineHit(input: {
  checkout: GitCheckoutWorkspace
  now: Date
  record: WorkspacePrebuiltBaselineCacheRecord
}): Promise<WorkspacePrebuiltBaselineCacheMetric> {
  const timestamp = input.now.toISOString()
  const sizeBytes = await directorySizeBytes(input.record.local.prebuiltDirectory)
  const nextRecord: WorkspacePrebuiltBaselineCacheRecord = {
    ...input.record,
    updatedAt: timestamp,
    lastUsedAt: timestamp,
    hitCount: input.record.hitCount + 1,
    sizeBytes,
  }
  await writePrebuiltBaselineCacheRecord(input.record.local.prebuiltDirectory, nextRecord)
  return prebuiltBaselineMetric({
    baselineCommitSha: nextRecord.baselineCommitSha,
    branch: nextRecord.branch,
    cacheKey: nextRecord.cacheKey,
    checkedAt: timestamp,
    hitCount: nextRecord.hitCount,
    missCount: nextRecord.missCount,
    reasonRef: "reason.workspace_prebuilt_baseline.hit",
    repositoryFullName: nextRecord.repositoryFullName,
    requestedCommitSha: checkoutBaseCommitSha(input.checkout),
    state: "hit",
  })
}

type PrebuiltBaselineRestoreAttempt =
  | { state: "hit"; metric: WorkspacePrebuiltBaselineCacheMetric }
  | { state: "miss"; metric: WorkspacePrebuiltBaselineCacheMetric }

async function restorePrebuiltBaselineCacheEntry(input: {
  cacheRoot: string
  checkout: GitCheckoutWorkspace
  now: Date
  prebuiltBaselineCacheRoot: string
  refreshCadenceSeconds: number
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  setupRunner: WorkspacePrebuiltBaselineSetupRunner
  workingDirectory: string
}): Promise<PrebuiltBaselineRestoreAttempt> {
  const cacheKey = prebuiltBaselineCacheKeyForCheckout(input.checkout)
  const requestedCommitSha = checkoutBaseCommitSha(input.checkout)
  if (input.checkout.virtualBranch !== undefined) {
    return {
      state: "miss",
      metric: await recordPrebuiltBaselineMiss({
        cacheKey,
        checkout: input.checkout,
        now: input.now,
        reasonRef: "reason.workspace_prebuilt_baseline.virtual_branch_unsupported",
      }),
    }
  }

  let record: WorkspacePrebuiltBaselineCacheRecord | null = null
  try {
    record = await ensurePrebuiltBaselineCacheEntry({
      cacheKey,
      checkout: input.checkout,
      now: input.now,
      prebuiltBaselineCacheRoot: input.prebuiltBaselineCacheRoot,
      refreshCadenceSeconds: input.refreshCadenceSeconds,
      ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
      setupRunner: input.setupRunner,
    })
  } catch {
    return {
      state: "miss",
      metric: await recordPrebuiltBaselineMiss({
        cacheKey,
        checkout: input.checkout,
        now: input.now,
        reasonRef: "reason.workspace_prebuilt_baseline.refresh_failed",
        record,
      }),
    }
  }

  if (record === null) {
    return {
      state: "miss",
      metric: await recordPrebuiltBaselineMiss({
        cacheKey,
        checkout: input.checkout,
        now: input.now,
        reasonRef: "reason.workspace_prebuilt_baseline.upstream_unavailable",
      }),
    }
  }
  if (record.baselineCommitSha.toLowerCase() !== requestedCommitSha.toLowerCase()) {
    return {
      state: "miss",
      metric: await recordPrebuiltBaselineMiss({
        cacheKey,
        checkout: input.checkout,
        now: input.now,
        reasonRef: "reason.workspace_prebuilt_baseline.requested_commit_not_prebuilt",
        record,
      }),
    }
  }

  assertPylonOwnedWorkspaceTarget({
    cacheRoot: input.cacheRoot,
    workingDirectory: input.workingDirectory,
  })
  try {
    await mkdir(input.cacheRoot, { recursive: true })
    await rm(input.workingDirectory, { recursive: true, force: true })
    await cp(record.local.prebuiltDirectory, input.workingDirectory, {
      recursive: true,
      force: false,
      errorOnExist: false,
    })
    await disableGitAutoMaintenance(input.workingDirectory)
    await runCheckedCommand(
      ["git", "reset", "--hard", requestedCommitSha],
      input.workingDirectory,
      "reason.workspace_prebuilt_baseline.restore_reset_failed",
    )
    const head = await runTextCommand(["git", "rev-parse", "HEAD"], input.workingDirectory)
    if (head.exitCode !== 0 || head.stdout.trim().toLowerCase() !== requestedCommitSha.toLowerCase()) {
      await rm(input.workingDirectory, { recursive: true, force: true })
      return {
        state: "miss",
        metric: await recordPrebuiltBaselineMiss({
          cacheKey,
          checkout: input.checkout,
          now: input.now,
          reasonRef: "reason.workspace_prebuilt_baseline.restore_integrity_failed",
          record,
        }),
      }
    }
    await setWorkspaceOriginRemote({
      checkout: input.checkout,
      ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
      workingDirectory: input.workingDirectory,
    })
    await installScmAuthBrokerGitCredentialHelper({ checkout: input.checkout, cwd: input.workingDirectory })
  } catch {
    await rm(input.workingDirectory, { recursive: true, force: true })
    return {
      state: "miss",
      metric: await recordPrebuiltBaselineMiss({
        cacheKey,
        checkout: input.checkout,
        now: input.now,
        reasonRef: "reason.workspace_prebuilt_baseline.restore_failed",
        record,
      }),
    }
  }

  return {
    state: "hit",
    metric: await recordPrebuiltBaselineHit({ checkout: input.checkout, now: input.now, record }),
  }
}

async function validatePreparedWorktreeCacheEntry(input: {
  cacheKey: string
  checkout: GitCheckoutWorkspace
  preparedDirectory: string
}): Promise<WorkspacePreparedWorktreeCacheRecord | null> {
  const baselineCommitSha = checkoutBaseCommitSha(input.checkout)
  const record = await readPreparedWorktreeCacheRecord(input.preparedDirectory)
  if (record === null) return null
  if (record.cacheKey !== input.cacheKey) return null
  if (record.repositoryFullName !== input.checkout.repository.fullName) return null
  if (record.baselineCommitSha.toLowerCase() !== baselineCommitSha.toLowerCase()) return null
  if (canonicalPath(record.local.preparedDirectory) !== canonicalPath(input.preparedDirectory)) return null
  if (!existsSync(join(input.preparedDirectory, ".git"))) return null

  const root = await runTextCommand(["git", "rev-parse", "--show-toplevel"], input.preparedDirectory)
  if (root.exitCode !== 0 || canonicalPath(root.stdout.trim()) !== canonicalPath(input.preparedDirectory)) return null
  const head = await runTextCommand(["git", "rev-parse", "HEAD"], input.preparedDirectory)
  if (head.exitCode !== 0 || head.stdout.trim().toLowerCase() !== baselineCommitSha.toLowerCase()) return null
  const commitExists = await runQuietCommand(["git", "cat-file", "-e", `${baselineCommitSha}^{commit}`], input.preparedDirectory)
  if (commitExists !== 0) return null
  const status = await runTextCommand(["git", "status", "--porcelain"], input.preparedDirectory)
  if (status.exitCode !== 0 || status.stdout.trim() !== "") return null
  return record
}

async function setWorkspaceOriginRemote(input: {
  checkout: GitCheckoutWorkspace
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  workingDirectory: string
}): Promise<void> {
  await runQuietCommand(["git", "remote", "remove", "origin"], input.workingDirectory)
  await runCheckedCommand(
    [
      "git",
      "remote",
      "add",
      "origin",
      input.remoteUrlFor?.(input.checkout) ?? `https://github.com/${input.checkout.repository.fullName}.git`,
    ],
    input.workingDirectory,
    "reason.workspace_prepared_cache.remote_add_failed",
  )
}

async function restorePreparedWorktreeCacheEntry(input: {
  cacheRoot: string
  checkout: GitCheckoutWorkspace
  now: Date
  preparedWorktreeCacheRoot: string
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  workingDirectory: string
}): Promise<WorkspacePreparedWorktreeCacheUse | null> {
  const cacheKey = preparedWorktreeCacheKeyForCheckout(input.checkout)
  const preparedDirectory = preparedWorktreeCacheDirectory(input.preparedWorktreeCacheRoot, cacheKey)
  const validated = await validatePreparedWorktreeCacheEntry({
    cacheKey,
    checkout: input.checkout,
    preparedDirectory,
  })
  if (validated === null) {
    await removePreparedWorktreeCacheEntry(preparedDirectory)
    return null
  }

  assertPylonOwnedWorkspaceTarget({
    cacheRoot: input.cacheRoot,
    workingDirectory: input.workingDirectory,
  })
  try {
    await mkdir(input.cacheRoot, { recursive: true })
    await rm(input.workingDirectory, { recursive: true, force: true })
    await runCheckedCommand(
      ["git", "clone", "--local", preparedDirectory, input.workingDirectory],
      input.cacheRoot,
      "reason.workspace_prepared_cache.restore_clone_failed",
    )
    await disableGitAutoMaintenance(input.workingDirectory)
    await runCheckedCommand(
      ["git", "reset", "--hard", checkoutBaseCommitSha(input.checkout)],
      input.workingDirectory,
      "reason.workspace_prepared_cache.restore_reset_failed",
    )
    await runCheckedCommand(
      ["git", "clean", "-ffdx"],
      input.workingDirectory,
      "reason.workspace_prepared_cache.restore_clean_failed",
    )
    const head = await runTextCommand(["git", "rev-parse", "HEAD"], input.workingDirectory)
    if (head.exitCode !== 0 || head.stdout.trim().toLowerCase() !== checkoutBaseCommitSha(input.checkout).toLowerCase()) {
      await rm(input.workingDirectory, { recursive: true, force: true })
      await removePreparedWorktreeCacheEntry(preparedDirectory)
      return null
    }
    const dirty = await workspaceDirtyState(input.workingDirectory)
    if (dirty !== "clean") {
      await rm(input.workingDirectory, { recursive: true, force: true })
      await removePreparedWorktreeCacheEntry(preparedDirectory)
      return null
    }
    await setWorkspaceOriginRemote({
      checkout: input.checkout,
      remoteUrlFor: input.remoteUrlFor,
      workingDirectory: input.workingDirectory,
    })
    await installScmAuthBrokerGitCredentialHelper({ checkout: input.checkout, cwd: input.workingDirectory })
  } catch {
    await rm(input.workingDirectory, { recursive: true, force: true })
    await removePreparedWorktreeCacheEntry(preparedDirectory)
    return null
  }

  const updatedAt = input.now.toISOString()
  const sizeBytes = await directorySizeBytes(preparedDirectory)
  const nextRecord: WorkspacePreparedWorktreeCacheRecord = {
    ...validated,
    reuseReason: "restore_quick_sync_reset",
    updatedAt,
    lastUsedAt: updatedAt,
    useCount: validated.useCount + 1,
    sizeBytes,
  }
  await writePreparedWorktreeCacheRecord(preparedDirectory, nextRecord)
  return {
    schema: WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA,
    cacheKey,
    state: "hit",
    reuseReason: "restore_quick_sync_reset",
    integrityRef: nextRecord.integrityRef,
  }
}

function sourceRefParts(sourceRef: string): { repositoryFullName: string; baselineCommitSha: string } | null {
  const separator = sourceRef.lastIndexOf(":")
  if (separator <= 0) return null
  const repositoryFullName = sourceRef.slice(0, separator)
  const baselineCommitSha = sourceRef.slice(separator + 1)
  if (!githubFullNamePattern.test(repositoryFullName) || !gitCommitShaPattern.test(baselineCommitSha)) return null
  return { repositoryFullName, baselineCommitSha }
}

export async function enforcePreparedWorktreeCacheBudget(input: {
  preparedWorktreeCacheRoot: string
  diskBudgetBytes?: number
}): Promise<{ removedCacheKeys: string[]; totalBytes: number }> {
  const diskBudgetBytes = boundedPreparedWorktreeDiskBudgetBytes(input.diskBudgetBytes)
  let entries: string[]
  try {
    entries = await readdir(input.preparedWorktreeCacheRoot)
  } catch {
    return { removedCacheKeys: [], totalBytes: 0 }
  }

  const candidates: Array<{
    cacheKey: string
    directory: string
    lastUsedAtMs: number
    sizeBytes: number
  }> = []
  let totalBytes = 0
  for (const entry of entries) {
    if (!entry.startsWith("prepared.")) continue
    const cacheKey = entry.slice("prepared.".length)
    if (!/^[a-f0-9]{32}$/.test(cacheKey)) continue
    const directory = join(input.preparedWorktreeCacheRoot, entry)
    const info = await stat(directory).catch(() => null)
    if (info === null || !info.isDirectory()) continue
    const record = await readPreparedWorktreeCacheRecord(directory)
    const sizeBytes = record?.sizeBytes ?? await directorySizeBytes(directory)
    totalBytes += sizeBytes
    const lastUsedAtMs = Date.parse(record?.lastUsedAt ?? "")
    candidates.push({
      cacheKey,
      directory,
      lastUsedAtMs: Number.isFinite(lastUsedAtMs) ? lastUsedAtMs : 0,
      sizeBytes,
    })
  }

  if (totalBytes <= diskBudgetBytes) return { removedCacheKeys: [], totalBytes }

  const removedCacheKeys: string[] = []
  for (const candidate of candidates.sort((left, right) => left.lastUsedAtMs - right.lastUsedAtMs)) {
    await removePreparedWorktreeCacheEntry(candidate.directory)
    removedCacheKeys.push(candidate.cacheKey)
    totalBytes -= candidate.sizeBytes
    if (totalBytes <= diskBudgetBytes) break
  }
  return { removedCacheKeys, totalBytes: Math.max(0, totalBytes) }
}

async function snapshotPreparedWorktreeCacheEntry(input: {
  diskBudgetBytes: number
  now: Date
  preparedWorktreeCacheRoot: string
  record: WorkspaceLeaseRecord
}): Promise<WorkspacePreparedWorktreeCacheRecord | null> {
  const source = sourceRefParts(input.record.sourceRef)
  if (source === null) return null
  const cacheKey = input.record.local.preparedWorktreeCache?.cacheKey ??
    preparedWorktreeCacheKeyFor({
      baselineCommitSha: source.baselineCommitSha,
      repositoryFullName: source.repositoryFullName,
    })
  const preparedDirectory = preparedWorktreeCacheDirectory(input.preparedWorktreeCacheRoot, cacheKey)
  const tempDirectory = `${preparedDirectory}.tmp-${process.pid}-${Date.now()}`
  const timestamp = input.now.toISOString()

  await withRepositoryCacheLock(preparedDirectory, async () => {
    await mkdir(input.preparedWorktreeCacheRoot, { recursive: true })
    await rm(tempDirectory, { recursive: true, force: true })
    await rm(preparedWorktreeCacheRecordPath(tempDirectory), { force: true })
    await runCheckedCommand(
      ["git", "clone", "--local", input.record.local.workingDirectory, tempDirectory],
      input.preparedWorktreeCacheRoot,
      "reason.workspace_prepared_cache.snapshot_clone_failed",
    )
    await disableGitAutoMaintenance(tempDirectory)
    await runCheckedCommand(
      ["git", "reset", "--hard", source.baselineCommitSha],
      tempDirectory,
      "reason.workspace_prepared_cache.snapshot_reset_failed",
    )
    await runCheckedCommand(
      ["git", "clean", "-ffdx"],
      tempDirectory,
      "reason.workspace_prepared_cache.snapshot_clean_failed",
    )
    const status = await runTextCommand(["git", "status", "--porcelain"], tempDirectory)
    if (status.exitCode !== 0 || status.stdout.trim() !== "") {
      throw new WorkspaceCheckoutError("reason.workspace_prepared_cache.snapshot_dirty")
    }
    const previous = await readPreparedWorktreeCacheRecord(preparedDirectory)
    const integrityRef = preparedWorktreeIntegrityRef({
      baselineCommitSha: source.baselineCommitSha,
      cacheKey,
      repositoryFullName: source.repositoryFullName,
      state: "ready",
    })
    const preliminary: WorkspacePreparedWorktreeCacheRecord = {
      schema: WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA,
      cacheKey,
      repositoryFullName: source.repositoryFullName,
      baselineCommitSha: source.baselineCommitSha,
      sourceRef: input.record.sourceRef,
      state: "ready",
      reuseReason: "post_completion_snapshot",
      integrityRef,
      createdAt: previous?.createdAt ?? timestamp,
      updatedAt: timestamp,
      lastUsedAt: previous?.lastUsedAt ?? timestamp,
      useCount: previous?.useCount ?? 0,
      sizeBytes: 0,
      local: { preparedDirectory },
    }
    await writePreparedWorktreeCacheRecord(tempDirectory, preliminary)
    const sizeBytes = await directorySizeBytes(tempDirectory) + await directorySizeBytes(preparedWorktreeCacheRecordPath(tempDirectory))
    await writePreparedWorktreeCacheRecord(tempDirectory, { ...preliminary, sizeBytes })
    await removePreparedWorktreeCacheEntry(preparedDirectory)
    await rename(tempDirectory, preparedDirectory)
    await rename(preparedWorktreeCacheRecordPath(tempDirectory), preparedWorktreeCacheRecordPath(preparedDirectory))
  })

  await enforcePreparedWorktreeCacheBudget({
    diskBudgetBytes: input.diskBudgetBytes,
    preparedWorktreeCacheRoot: input.preparedWorktreeCacheRoot,
  })
  return readPreparedWorktreeCacheRecord(preparedDirectory)
}

function leaseRecordPath(workspaceStateRoot: string, workspaceRef: string) {
  return join(workspaceStateRoot, `${workspaceRef}.json`)
}

async function readLeaseRecord(path: string): Promise<WorkspaceLeaseRecord | null> {
  try {
    const record = JSON.parse(await readFile(path, "utf8")) as WorkspaceLeaseRecord
    return record.schema === WORKSPACE_LEASE_SCHEMA ? record : null
  } catch {
    return null
  }
}

async function readLeaseRecordOrThrow(path: string): Promise<WorkspaceLeaseRecord> {
  let raw: string
  try {
    raw = await readFile(path, "utf8")
  } catch (error) {
    if (isNodeErrorWithCode(error, "ENOENT")) {
      throw workspaceMaterializerError(
        "workspace.lease_record_read",
        "reason.workspace_lease.not_found",
        error,
      )
    }
    throw workspaceMaterializerError(
      "workspace.lease_record_read",
      "reason.workspace_lease.storage_failed",
      error,
    )
  }

  try {
    const record = JSON.parse(raw) as WorkspaceLeaseRecord
    if (record.schema !== WORKSPACE_LEASE_SCHEMA) {
      throw new Error("unexpected workspace lease schema")
    }
    return record
  } catch (error) {
    throw workspaceMaterializerError(
      "workspace.lease_record_read",
      "reason.workspace_lease.malformed",
      error,
    )
  }
}

async function writeLeaseRecord(workspaceStateRoot: string, record: WorkspaceLeaseRecord) {
  await mkdir(workspaceStateRoot, { recursive: true })
  await writeFile(
    leaseRecordPath(workspaceStateRoot, record.workspaceRef),
    `${JSON.stringify(record, null, 2)}\n`,
  )
}

function errorFromUnknown(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error))
}

function promiseEffect<T>(work: () => Promise<T>): Effect.Effect<T, Error> {
  return Effect.tryPromise({
    try: work,
    catch: errorFromUnknown,
  })
}

export type MaterializeWithLeaseInput = {
  cacheRoot: string
  checkout: GitCheckoutWorkspace
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
  prebuiltBaselineCacheRoot?: string
  prebuiltBaselineRefreshCadenceSeconds?: number
  prebuiltBaselineSetupRunner?: WorkspacePrebuiltBaselineSetupRunner
  preparedWorktreeCacheRoot?: string
  preparedWorktreeDiskBudgetBytes?: number
  refPrefix: string
  repositoryCacheRoot: string
  workspaceStateRoot: string
  now?: Date
  /** Test seam only — see GitWorktreeCheckoutRunnerOptions.remoteUrlFor. */
  remoteUrlFor?: (checkout: GitCheckoutWorkspace) => string
  retentionPolicy?: WorkspaceRetentionPolicy
  ttlSeconds?: number
}

/**
 * Materializes a git_checkout workspace through the worktree strategy and
 * records a workspace lease. An injected checkout runner (the test seam)
 * replaces the strategy but still gets a lease record, so cleanup and
 * projection behave identically in every mode. Each call starts with an
 * opportunistic TTL sweep over existing leases.
 */
export async function materializeGitCheckoutWorkspaceWithLease(
  input: MaterializeWithLeaseInput,
): Promise<MaterializedWorkspace> {
  const now = input.now ?? new Date()
  await cleanupExpiredWorkspaces({ now, workspaceStateRoot: input.workspaceStateRoot })

  const strategy: WorkspaceStrategy =
    input.checkoutRunner === undefined
      ? "git_worktree"
      : input.checkoutRunner === defaultGitCheckoutRunner
        ? "detached_checkout"
        : "injected"
  const checkoutRunner =
    input.checkoutRunner ??
    createGitWorktreeCheckoutRunner({
      repositoryCacheRoot: input.repositoryCacheRoot,
      ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
    })
  const preparedDiskBudgetBytes = boundedPreparedWorktreeDiskBudgetBytes(input.preparedWorktreeDiskBudgetBytes)
  const preparedCacheKey =
    input.preparedWorktreeCacheRoot === undefined || strategy !== "git_worktree"
      ? undefined
      : preparedWorktreeCacheKeyForCheckout(input.checkout)
  const prebuiltRefreshCadenceSeconds = boundedPrebuiltBaselineRefreshCadenceSeconds(
    input.prebuiltBaselineRefreshCadenceSeconds,
  )
  const prebuiltCacheKey =
    input.prebuiltBaselineCacheRoot === undefined || strategy !== "git_worktree"
      ? undefined
      : prebuiltBaselineCacheKeyForCheckout(input.checkout)
  const workspaceRef = stableRef(input.refPrefix, input.leaseRef)
  const workingDirectory = join(input.cacheRoot, workspaceRef)
  const preparedRestore =
    input.preparedWorktreeCacheRoot === undefined || strategy !== "git_worktree"
      ? null
      : await restorePreparedWorktreeCacheEntry({
          cacheRoot: input.cacheRoot,
          checkout: input.checkout,
          now,
          preparedWorktreeCacheRoot: input.preparedWorktreeCacheRoot,
          ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
          workingDirectory,
        })
  const prebuiltRestore =
    preparedRestore !== null || input.prebuiltBaselineCacheRoot === undefined || strategy !== "git_worktree"
      ? null
      : await restorePrebuiltBaselineCacheEntry({
          cacheRoot: input.cacheRoot,
          checkout: input.checkout,
          now,
          prebuiltBaselineCacheRoot: input.prebuiltBaselineCacheRoot,
          refreshCadenceSeconds: prebuiltRefreshCadenceSeconds,
          ...(input.remoteUrlFor === undefined ? {} : { remoteUrlFor: input.remoteUrlFor }),
          setupRunner: input.prebuiltBaselineSetupRunner ?? defaultPrebuiltBaselineSetupRunner,
          workingDirectory,
        })
  const materialized: MaterializedWorkspace =
    preparedRestore !== null
      ? {
          cleanupRef: stableRef("cleanup.pylon.workspace", workspaceRef),
          preparedWorktreeCache: preparedRestore,
          sourceRef: checkoutSourceRef(input.checkout),
          workingDirectory,
          workspaceRef,
        }
      : prebuiltRestore?.state === "hit"
        ? {
            cleanupRef: stableRef("cleanup.pylon.workspace", workspaceRef),
            prebuiltBaselineCache: prebuiltRestore.metric,
            sourceRef: checkoutSourceRef(input.checkout),
            workingDirectory,
            workspaceRef,
          }
        : {
            ...(await materializeGitCheckoutWorkspace({
              cacheRoot: input.cacheRoot,
              checkout: input.checkout,
              checkoutRunner,
              leaseRef: input.leaseRef,
              refPrefix: input.refPrefix,
            })),
            ...(prebuiltRestore === null ? {} : { prebuiltBaselineCache: prebuiltRestore.metric }),
          }

  const record: WorkspaceLeaseRecord = {
    schema: WORKSPACE_LEASE_SCHEMA,
    workspaceRef: materialized.workspaceRef,
    cleanupRef: materialized.cleanupRef,
    sourceRef: materialized.sourceRef,
    strategy,
    state: "materialized",
    materializedAt: now.toISOString(),
    ttlSeconds: input.ttlSeconds ?? DEFAULT_WORKSPACE_TTL_SECONDS,
    retentionPolicy: input.retentionPolicy ?? "retain_until_ttl",
    generatedAt: now.toISOString(),
    local: {
      cacheRoot: input.cacheRoot,
      workingDirectory: materialized.workingDirectory,
      ...(strategy === "git_worktree" && preparedRestore === null && prebuiltRestore?.state !== "hit"
        ? {
            repositoryCacheDirectory: join(
              input.repositoryCacheRoot,
              `${repositoryCacheKeyFor(input.checkout.repository.fullName)}.git`,
            ),
          }
        : {}),
      ...(input.preparedWorktreeCacheRoot === undefined || preparedCacheKey === undefined
        ? {}
        : {
            preparedWorktreeCache: {
              root: input.preparedWorktreeCacheRoot,
              cacheKey: preparedCacheKey,
              diskBudgetBytes: preparedDiskBudgetBytes,
              ...(preparedRestore === null ? {} : { restore: preparedRestore }),
            },
          }),
      ...(input.prebuiltBaselineCacheRoot === undefined || prebuiltCacheKey === undefined
        ? {}
        : {
            prebuiltBaselineCache: {
              root: input.prebuiltBaselineCacheRoot,
              cacheKey: prebuiltCacheKey,
              refreshCadenceSeconds: prebuiltRefreshCadenceSeconds,
              ...(prebuiltRestore === null ? {} : { metric: prebuiltRestore.metric }),
            },
          }),
    },
  }
  await writeLeaseRecord(input.workspaceStateRoot, record)
  return materialized
}

/**
 * Effect-scoped lease materialization for long-running assignment runners.
 *
 * The caller runs this inside `Effect.scoped`; if the fiber is interrupted
 * after acquisition, the release finalizer removes the workspace through the
 * same lease cleanup path used by closeout. Legacy Promise call sites keep
 * using `materializeGitCheckoutWorkspaceWithLease` until they are migrated.
 */
export function scopedMaterializedGitCheckoutWorkspace(
  input: MaterializeWithLeaseInput,
): Effect.Effect<MaterializedWorkspace, Error, Scope.Scope> {
  return Effect.acquireRelease(
    promiseEffect(() => materializeGitCheckoutWorkspaceWithLease(input)),
    (materialized) =>
      promiseEffect(() =>
        releaseWorkspace({
          workspaceStateRoot: input.workspaceStateRoot,
          workspaceRef: materialized.workspaceRef,
        }),
      ).pipe(Effect.asVoid, Effect.catch(() => Effect.void)),
  )
}

type CleanLeaseRecordResult =
  | { state: "cleaned"; cleanupReceiptRef: string }
  | { state: "retained"; retentionReasonRef: string; workspaceRef: string }

async function removeWorkspaceAndWriteCleanupReceipt(input: {
  workspaceStateRoot: string
  record: WorkspaceLeaseRecord
  now: Date
}): Promise<{ state: "cleaned"; cleanupReceiptRef: string }> {
  await removeMaterializedWorkspace({
    cacheRoot: input.record.local.cacheRoot,
    workingDirectory: input.record.local.workingDirectory,
  })
  const repositoryCacheDirectory = input.record.local.repositoryCacheDirectory
  if (repositoryCacheDirectory !== undefined) {
    await withRepositoryCacheLock(repositoryCacheDirectory, async () => {
      await runQuietCommand(["git", "worktree", "prune"], repositoryCacheDirectory)
    })
  }
  const cleanedAt = input.now.toISOString()
  const cleanupReceiptRef = stableRef(
    "receipt.pylon.workspace_cleanup",
    `${input.record.workspaceRef}:${cleanedAt}`,
  )
  await writeLeaseRecord(input.workspaceStateRoot, {
    ...input.record,
    state: "cleaned",
    cleanedAt,
    cleanupReceiptRef,
    retentionReasonRef: undefined,
    lastCleanupAttemptAt: undefined,
    generatedAt: cleanedAt,
  })
  return { state: "cleaned", cleanupReceiptRef }
}

async function cleanLeaseRecord(
  workspaceStateRoot: string,
  record: WorkspaceLeaseRecord,
  now: Date,
): Promise<CleanLeaseRecordResult | null> {
  try {
    assertPylonOwnedWorkspaceTarget({
      cacheRoot: record.local.cacheRoot,
      workingDirectory: record.local.workingDirectory,
    })
  } catch {
    // a record pointing outside the cache root is never acted on
    return null
  }
  const credentialScan = await scanLongLivedScmCredentials({
    roots: [{ rootRef: record.workspaceRef, path: record.local.workingDirectory }],
  })
  if (credentialScan.state === "leaked") {
    return removeWorkspaceAndWriteCleanupReceipt({ now, record, workspaceStateRoot })
  }

  const dirtyState = await workspaceDirtyState(record.local.workingDirectory)
  if (dirtyState !== "clean") {
    const attemptedAt = now.toISOString()
    const retentionReasonRef =
      dirtyState === "dirty" ? "retention.workspace.dirty" : "retention.workspace.dirty_state_unknown"
    await writeLeaseRecord(workspaceStateRoot, {
      ...record,
      retentionReasonRef,
      lastCleanupAttemptAt: attemptedAt,
      generatedAt: attemptedAt,
    })
    return { state: "retained", retentionReasonRef, workspaceRef: record.workspaceRef }
  }
  const preparedCache = record.local.preparedWorktreeCache
  if (preparedCache !== undefined) {
    await snapshotPreparedWorktreeCacheEntry({
      diskBudgetBytes: preparedCache.diskBudgetBytes,
      now,
      preparedWorktreeCacheRoot: preparedCache.root,
      record,
    }).catch(() => null)
  }
  return removeWorkspaceAndWriteCleanupReceipt({ now, record, workspaceStateRoot })
}

/**
 * Removes every materialized workspace whose TTL has expired, pruning only
 * inside Pylon-owned cache roots recorded by internal workspace refs.
 * Returns the cleanup receipt refs it minted.
 */
export async function cleanupExpiredWorkspaces(input: {
  workspaceStateRoot: string
  now?: Date
}): Promise<{ cleanupReceiptRefs: string[]; retainedWorkspaceRefs: string[] }> {
  const now = input.now ?? new Date()
  let entries: string[]
  try {
    entries = await readdir(input.workspaceStateRoot)
  } catch {
    return { cleanupReceiptRefs: [], retainedWorkspaceRefs: [] }
  }
  const cleanupReceiptRefs: string[] = []
  const retainedWorkspaceRefs: string[] = []
  for (const entry of entries) {
    if (!entry.endsWith(".json")) continue
    const record = await readLeaseRecord(join(input.workspaceStateRoot, entry))
    if (record === null || record.state !== "materialized") continue
    const expiresAtMs = Date.parse(record.materializedAt) + record.ttlSeconds * 1000
    if (!Number.isFinite(expiresAtMs) || now.getTime() < expiresAtMs) continue
    const result = await cleanLeaseRecord(input.workspaceStateRoot, record, now)
    if (result?.state === "cleaned") cleanupReceiptRefs.push(result.cleanupReceiptRef)
    if (result?.state === "retained") retainedWorkspaceRefs.push(result.workspaceRef)
  }
  return { cleanupReceiptRefs, retainedWorkspaceRefs }
}

/**
 * Count-bounded cleanup for hot assignment caches. Unlike the TTL sweep, this
 * runs when a lane knows its cache root is high churn and must stay under a
 * fixed number of materialized workspaces.
 */
export async function cleanupOldestMaterializedWorkspaces(input: {
  workspaceStateRoot: string
  maxMaterializedWorkspaces: number
  minimumAgeSeconds?: number
  now?: Date
}): Promise<{ cleanupReceiptRefs: string[]; retainedWorkspaceRefs: string[] }> {
  const maxMaterializedWorkspaces = Math.max(
    0,
    Math.floor(input.maxMaterializedWorkspaces),
  )
  let entries: string[]
  try {
    entries = await readdir(input.workspaceStateRoot)
  } catch {
    return { cleanupReceiptRefs: [], retainedWorkspaceRefs: [] }
  }

  const materializedRecords: WorkspaceLeaseRecord[] = []
  for (const entry of entries) {
    if (!entry.endsWith(".json")) continue
    const record = await readLeaseRecord(join(input.workspaceStateRoot, entry))
    if (record === null || record.state !== "materialized") continue
    materializedRecords.push(record)
  }
  if (materializedRecords.length <= maxMaterializedWorkspaces) {
    return { cleanupReceiptRefs: [], retainedWorkspaceRefs: [] }
  }

  const now = input.now ?? new Date()
  const minimumAgeMs =
    Math.max(0, Math.floor(input.minimumAgeSeconds ?? 0)) * 1000
  const cleanupReceiptRefs: string[] = []
  const retainedWorkspaceRefs: string[] = []
  const eligibleOldestFirst = materializedRecords
    .filter((record) => {
      const materializedAtMs = Date.parse(record.materializedAt)
      if (!Number.isFinite(materializedAtMs)) return true
      return now.getTime() - materializedAtMs >= minimumAgeMs
    })
    .sort((left, right) => {
      const leftMs = Date.parse(left.materializedAt)
      const rightMs = Date.parse(right.materializedAt)
      return (
        (Number.isFinite(leftMs) ? leftMs : 0) -
        (Number.isFinite(rightMs) ? rightMs : 0)
      )
    })
  const cleanupCount = materializedRecords.length - maxMaterializedWorkspaces
  for (const record of eligibleOldestFirst.slice(0, cleanupCount)) {
    const result = await cleanLeaseRecord(input.workspaceStateRoot, record, now)
    if (result?.state === "cleaned") cleanupReceiptRefs.push(result.cleanupReceiptRef)
    if (result?.state === "retained") retainedWorkspaceRefs.push(result.workspaceRef)
  }
  return { cleanupReceiptRefs, retainedWorkspaceRefs }
}

/**
 * Explicit release for the remove_on_closeout retention policy: removes
 * one workspace by its internal ref and mints its cleanup receipt. Returns
 * null when the lease is unknown or already cleaned.
 */
export async function releaseWorkspace(input: {
  workspaceStateRoot: string
  workspaceRef: string
  now?: Date
}): Promise<{ cleanupReceiptRef?: string; retainedWorkspaceRef?: string; retentionReasonRef?: string } | null> {
  const record = await readLeaseRecord(
    leaseRecordPath(input.workspaceStateRoot, input.workspaceRef),
  )
  if (record === null || record.state !== "materialized") return null
  const result = await cleanLeaseRecord(input.workspaceStateRoot, record, input.now ?? new Date())
  if (result?.state === "cleaned") return { cleanupReceiptRef: result.cleanupReceiptRef }
  if (result?.state === "retained") {
    return { retainedWorkspaceRef: result.workspaceRef, retentionReasonRef: result.retentionReasonRef }
  }
  return null
}

/** Reads one lease record by workspace ref (local diagnostics only). */
export async function workspaceLeaseRecordFor(input: {
  workspaceStateRoot: string
  workspaceRef: string
}): Promise<WorkspaceLeaseRecord | null> {
  return readLeaseRecord(leaseRecordPath(input.workspaceStateRoot, input.workspaceRef))
}

export const PylonWorkspaceMaterializerLive = Layer.succeed(PylonWorkspaceMaterializer, {
  materializeWithLease: (input: MaterializeWithLeaseInput) =>
    Effect.tryPromise({
      try: () => materializeGitCheckoutWorkspaceWithLease(input),
      catch: (error: unknown) =>
        workspaceMaterializerError(
          "workspace.materialize_with_lease",
          workspaceCheckoutFailureReasonRef(error) ?? "reason.workspace_materializer.materialize_failed",
          error,
        ),
    }),
  cleanupExpired: (input: { workspaceStateRoot: string; now?: Date }) =>
    Effect.tryPromise({
      try: () => cleanupExpiredWorkspaces(input),
      catch: (error: unknown) =>
        workspaceMaterializerError(
          "workspace.cleanup_expired",
          "reason.workspace_materializer.cleanup_expired_failed",
          error,
        ),
    }),
  cleanupOldest: (input: {
    workspaceStateRoot: string
    maxMaterializedWorkspaces: number
    minimumAgeSeconds?: number
    now?: Date
  }) =>
    Effect.tryPromise({
      try: () => cleanupOldestMaterializedWorkspaces(input),
      catch: (error: unknown) =>
        workspaceMaterializerError(
          "workspace.cleanup_oldest",
          "reason.workspace_materializer.cleanup_oldest_failed",
          error,
        ),
    }),
  release: (input: { workspaceStateRoot: string; workspaceRef: string; now?: Date }) =>
    Effect.tryPromise({
      try: () => releaseWorkspace(input),
      catch: (error: unknown) =>
        workspaceMaterializerError(
          "workspace.release",
          "reason.workspace_materializer.release_failed",
          error,
        ),
    }),
  leaseRecordFor: (input: { workspaceStateRoot: string; workspaceRef: string }) =>
    Effect.tryPromise({
      try: () => readLeaseRecordOrThrow(leaseRecordPath(input.workspaceStateRoot, input.workspaceRef)),
      catch: (error: unknown) =>
        error instanceof PylonWorkspaceMaterializerError
          ? error
          : workspaceMaterializerError(
              "workspace.lease_record_read",
              "reason.workspace_lease.storage_failed",
              error,
            ),
    }),
})

/**
 * The public-safe projection of a workspace lease: refs, state, policy,
 * and freshness only. Local paths and cache mechanics never appear here
 * (#4751 projection law: carries generatedAt, rebuilt on every state
 * transition by the write paths above).
 */
export function publicWorkspaceLeaseProjection(record: WorkspaceLeaseRecord) {
  return {
    schema: record.schema,
    workspaceRef: record.workspaceRef,
    cleanupRef: record.cleanupRef,
    sourceRef: record.sourceRef,
    strategy: record.strategy,
    state: record.state,
    materializedAt: record.materializedAt,
    ttlSeconds: record.ttlSeconds,
    retentionPolicy: record.retentionPolicy,
    generatedAt: record.generatedAt,
    ...(record.cleanupReceiptRef === undefined ? {} : { cleanupReceiptRef: record.cleanupReceiptRef }),
    ...(record.cleanedAt === undefined ? {} : { cleanedAt: record.cleanedAt }),
    ...(record.retentionReasonRef === undefined ? {} : { retentionReasonRef: record.retentionReasonRef }),
    ...(record.lastCleanupAttemptAt === undefined ? {} : { lastCleanupAttemptAt: record.lastCleanupAttemptAt }),
  }
}

/**
 * Declares the workspace materializer and cleanup-receipt capabilities
 * when this Pylon has at least one local coding lane, and strips stale
 * declarations when it has none — the workspace service is only
 * advertisable where a coding adapter can actually consume it.
 */
export function withWorkspaceMaterializerCapability(
  capabilityRefs: ReadonlyArray<string>,
): string[] {
  const base = capabilityRefs.filter(
    (ref) =>
      ref !== WORKSPACE_MATERIALIZER_CAPABILITY_REF &&
      ref !== WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF,
  )
  const hasCodingLane =
    base.includes(CLAUDE_AGENT_CAPABILITY_REF) || base.includes(CODEX_AGENT_CAPABILITY_REF)
  return hasCodingLane
    ? [...new Set([...base, WORKSPACE_MATERIALIZER_CAPABILITY_REF, WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF])]
    : [...new Set(base)]
}
