import { existsSync } from "node:fs"
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { createHash } from "node:crypto"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent"
import { CODEX_AGENT_CAPABILITY_REF } from "./codex-agent"

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
    visibility: "public"
  }
  verificationCommand: {
    args: string[]
    commandRef: string
  }
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
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/

/**
 * Decodes and validates the shared git_checkout workspace payload from a
 * normalized coding assignment. Rejects private repositories, unsafe
 * repository names, unpinned commits, absolute verification paths, `..`
 * traversal, and shell-shaped command strings — foreign or malformed
 * shapes never reach filesystem work.
 */
export function gitCheckoutWorkspaceFrom(codingAssignment: unknown): GitCheckoutWorkspace | null {
  const workspace = (codingAssignment as { workspace?: unknown } | null)?.workspace
  if (workspace === null || typeof workspace !== "object") return null
  const payload = workspace as GitCheckoutWorkspace
  if (payload.kind !== "git_checkout") return null
  if (payload.repository?.provider !== "github" || payload.repository.visibility !== "public") return null
  if (!githubFullNamePattern.test(payload.repository.fullName)) return null
  if (!gitCommitShaPattern.test(payload.repository.commitSha)) return null
  if (typeof payload.repository.branch !== "string" || payload.repository.branch.includes("..")) return null
  if (!Array.isArray(payload.verificationCommand?.args) || payload.verificationCommand.args.length === 0) return null
  if (typeof payload.verificationCommand.commandRef !== "string") return null
  const safeArgs = payload.verificationCommand.args.every((arg) =>
    typeof arg === "string" &&
    verificationCommandArgPattern.test(arg) &&
    !arg.includes("..") &&
    !arg.startsWith("/")
  )
  return safeArgs ? payload : null
}

async function runQuietCommand(args: string[], cwd: string): Promise<number> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  return proc.exited
}

async function runCheckedCommand(args: string[], cwd: string): Promise<void> {
  const exitCode = await runQuietCommand(args, cwd)
  if (exitCode !== 0) {
    throw new Error(`command failed: ${args[0] ?? "unknown"}`)
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
  await rm(workingDirectory, { recursive: true, force: true })
  await mkdir(workingDirectory, { recursive: true })
  await runCheckedCommand(["git", "init"], workingDirectory)
  await runCheckedCommand(
    [
      "git",
      "remote",
      "add",
      "origin",
      `https://github.com/${checkout.repository.fullName}.git`,
    ],
    workingDirectory,
  )
  await runCheckedCommand(
    ["git", "fetch", "--depth", "1", "origin", checkout.repository.commitSha],
    workingDirectory,
  )
  await runCheckedCommand(
    ["git", "checkout", "--detach", checkout.repository.commitSha],
    workingDirectory,
  )
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
  return {
    cleanupRef: stableRef("cleanup.pylon.workspace", workspaceRef),
    sourceRef: `${input.checkout.repository.fullName}:${input.checkout.repository.commitSha}`,
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
  const cacheRoot = resolve(input.cacheRoot)
  const target = resolve(input.workingDirectory)
  if (target === cacheRoot || !target.startsWith(`${cacheRoot}/`)) {
    throw new Error("workspace cleanup refused: target is outside the Pylon-owned cache root")
  }
  await rm(target, { recursive: true, force: true })
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
  /**
   * Local-only mechanics. Never projected, never emitted in progress
   * events, closeouts, or any public surface.
   */
  local: {
    cacheRoot: string
    workingDirectory: string
    repositoryCacheDirectory?: string
  }
}

/** Stable cache key for one public repository's shared bare clone. */
export function repositoryCacheKeyFor(fullName: string): string {
  return createHash("sha256").update(fullName).digest("hex").slice(0, 24)
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

// One Pylon process can materialize concurrent assignments for the same
// repository; git's own lockfiles (shallow.lock, ref locks) make concurrent
// fetch/worktree-add against one bare repo flaky, so cache operations are
// serialized per bare directory in process.
const repositoryCacheLocks = new Map<string, Promise<unknown>>()

async function withRepositoryCacheLock<T>(bareDirectory: string, work: () => Promise<T>): Promise<T> {
  const previous = repositoryCacheLocks.get(bareDirectory) ?? Promise.resolve()
  const next = previous.catch(() => undefined).then(work)
  repositoryCacheLocks.set(bareDirectory, next.catch(() => undefined))
  return next
}

/**
 * Builds the worktree checkout strategy: ensure the shared bare cache for
 * the repository, fetch the pinned commit only when its object is missing,
 * verify the commit object exists before any worktree work, then add an
 * isolated detached worktree at the assignment-scoped path. The pinned
 * commit gets a cache-local ref so gc can never collect it while cached.
 */
export function createGitWorktreeCheckoutRunner(
  options: GitWorktreeCheckoutRunnerOptions,
): WorkspaceCheckoutRunner {
  return async (workingDirectory, checkout) => {
    const repository = checkout.repository
    const bareDirectory = join(
      options.repositoryCacheRoot,
      `${repositoryCacheKeyFor(repository.fullName)}.git`,
    )
    await withRepositoryCacheLock(bareDirectory, async () => {
      await mkdir(options.repositoryCacheRoot, { recursive: true })
      if (!existsSync(join(bareDirectory, "HEAD"))) {
        await runCheckedCommand(["git", "init", "--bare", bareDirectory], options.repositoryCacheRoot)
      }
      const commitArg = `${repository.commitSha}^{commit}`
      const cached = (await runQuietCommand(["git", "cat-file", "-e", commitArg], bareDirectory)) === 0
      if (!cached) {
        const remoteUrl =
          options.remoteUrlFor?.(checkout) ?? `https://github.com/${repository.fullName}.git`
        await runCheckedCommand(
          ["git", "fetch", "--depth", "1", remoteUrl, repository.commitSha],
          bareDirectory,
        )
        await runCheckedCommand(["git", "cat-file", "-e", commitArg], bareDirectory)
      }
      // best-effort gc pin; materialization correctness never depends on it
      await runQuietCommand(
        ["git", "update-ref", `refs/pinned/${repository.commitSha}`, repository.commitSha],
        bareDirectory,
      )
      await rm(workingDirectory, { recursive: true, force: true })
      // a previously removed worktree leaves admin metadata that would block
      // re-adding the same path
      await runQuietCommand(["git", "worktree", "prune"], bareDirectory)
      await runCheckedCommand(
        ["git", "worktree", "add", "--detach", workingDirectory, repository.commitSha],
        bareDirectory,
      )
    })
  }
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

async function writeLeaseRecord(workspaceStateRoot: string, record: WorkspaceLeaseRecord) {
  await mkdir(workspaceStateRoot, { recursive: true })
  await writeFile(
    leaseRecordPath(workspaceStateRoot, record.workspaceRef),
    `${JSON.stringify(record, null, 2)}\n`,
  )
}

export type MaterializeWithLeaseInput = {
  cacheRoot: string
  checkout: GitCheckoutWorkspace
  checkoutRunner?: WorkspaceCheckoutRunner
  leaseRef: string
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
  const materialized = await materializeGitCheckoutWorkspace({
    cacheRoot: input.cacheRoot,
    checkout: input.checkout,
    checkoutRunner,
    leaseRef: input.leaseRef,
    refPrefix: input.refPrefix,
  })

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
      ...(strategy === "git_worktree"
        ? {
            repositoryCacheDirectory: join(
              input.repositoryCacheRoot,
              `${repositoryCacheKeyFor(input.checkout.repository.fullName)}.git`,
            ),
          }
        : {}),
    },
  }
  await writeLeaseRecord(input.workspaceStateRoot, record)
  return materialized
}

async function cleanLeaseRecord(
  workspaceStateRoot: string,
  record: WorkspaceLeaseRecord,
  now: Date,
): Promise<string | null> {
  try {
    await removeMaterializedWorkspace({
      cacheRoot: record.local.cacheRoot,
      workingDirectory: record.local.workingDirectory,
    })
  } catch {
    // a record pointing outside the cache root is never acted on
    return null
  }
  if (record.local.repositoryCacheDirectory !== undefined) {
    await runQuietCommand(["git", "worktree", "prune"], record.local.repositoryCacheDirectory)
  }
  const cleanedAt = now.toISOString()
  const cleanupReceiptRef = stableRef(
    "receipt.pylon.workspace_cleanup",
    `${record.workspaceRef}:${cleanedAt}`,
  )
  await writeLeaseRecord(workspaceStateRoot, {
    ...record,
    state: "cleaned",
    cleanedAt,
    cleanupReceiptRef,
    generatedAt: cleanedAt,
  })
  return cleanupReceiptRef
}

/**
 * Removes every materialized workspace whose TTL has expired, pruning only
 * inside Pylon-owned cache roots recorded by internal workspace refs.
 * Returns the cleanup receipt refs it minted.
 */
export async function cleanupExpiredWorkspaces(input: {
  workspaceStateRoot: string
  now?: Date
}): Promise<{ cleanupReceiptRefs: string[] }> {
  const now = input.now ?? new Date()
  let entries: string[]
  try {
    entries = await readdir(input.workspaceStateRoot)
  } catch {
    return { cleanupReceiptRefs: [] }
  }
  const cleanupReceiptRefs: string[] = []
  for (const entry of entries) {
    if (!entry.endsWith(".json")) continue
    const record = await readLeaseRecord(join(input.workspaceStateRoot, entry))
    if (record === null || record.state !== "materialized") continue
    const expiresAtMs = Date.parse(record.materializedAt) + record.ttlSeconds * 1000
    if (!Number.isFinite(expiresAtMs) || now.getTime() < expiresAtMs) continue
    const receipt = await cleanLeaseRecord(input.workspaceStateRoot, record, now)
    if (receipt !== null) cleanupReceiptRefs.push(receipt)
  }
  return { cleanupReceiptRefs }
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
}): Promise<{ cleanupReceiptRef: string } | null> {
  const record = await readLeaseRecord(
    leaseRecordPath(input.workspaceStateRoot, input.workspaceRef),
  )
  if (record === null || record.state !== "materialized") return null
  const receipt = await cleanLeaseRecord(input.workspaceStateRoot, record, input.now ?? new Date())
  return receipt === null ? null : { cleanupReceiptRef: receipt }
}

/** Reads one lease record by workspace ref (local diagnostics only). */
export async function workspaceLeaseRecordFor(input: {
  workspaceStateRoot: string
  workspaceRef: string
}): Promise<WorkspaceLeaseRecord | null> {
  return readLeaseRecord(leaseRecordPath(input.workspaceStateRoot, input.workspaceRef))
}

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
