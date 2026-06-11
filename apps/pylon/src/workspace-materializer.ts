import { mkdir, rm } from "node:fs/promises"
import { join, resolve } from "node:path"
import { createHash } from "node:crypto"

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

async function runCheckedCommand(args: string[], cwd: string): Promise<void> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const exitCode = await proc.exited
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
