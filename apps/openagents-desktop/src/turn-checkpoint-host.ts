/**
 * Hidden-ref turn checkpoint host (GIT-1, #8781).
 *
 * Main-process service that captures workspace state at coding-turn
 * boundaries as hidden Git refs, computes typed turn-over-turn diffs, and
 * applies explicit staged reverts. The mechanism follows T3 Code's
 * CheckpointStore: every snapshot is built through an ISOLATED temporary
 * `GIT_INDEX_FILE`, so the user's real index, branches, HEAD, stashes, and
 * worktree are never written by capture. Refs live under
 * `refs/openagents/checkpoints/<thread>/<turn>` — outside refs/heads and
 * refs/tags, so no branch UI, `git status`, or default push ever sees them.
 *
 * Capture bounds (T3 posture): tracked + non-ignored untracked files only,
 * with a per-file size exclusion and a total-file refusal bound.
 *
 * Revert follows the OpenCode V2 teardown §17 staged-rewind lessons:
 * stage -> inspect -> commit/clear, an explicit irreversible-effects
 * statement on every staged revert, refusal on dirty conflicting state
 * (uncheckpointed worktree edits in paths the revert would rewrite), and a
 * pre-revert baseline snapshot retained as redo material. Snapshots may
 * contain secrets: refs stay local-only and never enter Sync projections.
 *
 * This is main-process code (owner-local executor invariant); it is not
 * reachable from an untrusted renderer surface.
 */
import { createHash } from "node:crypto"
import { mkdtempSync, rmSync, lstatSync } from "node:fs"
import os from "node:os"
import path from "node:path"
import { spawn, type ChildProcess, type SpawnOptions } from "node:child_process"

import { workspaceGitEnvironment } from "./git-process-environment.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./ide/portable-mutation-authority.ts"
import {
  TURN_CHECKPOINT_IRREVERSIBLE_EFFECTS,
  TURN_CHECKPOINT_MAX_DIFF_BYTES,
  TURN_CHECKPOINT_MAX_FILE_BYTES,
  TURN_CHECKPOINT_MAX_FILES,
  TURN_CHECKPOINT_REF_ROOT,
  turnCheckpointError,
  type StagedTurnCheckpointRevert,
  type TurnCheckpointCaptureInput,
  type TurnCheckpointCaptureResult,
  type TurnCheckpointClearRevertResult,
  type TurnCheckpointCommitRevertResult,
  type TurnCheckpointDeleteThreadResult,
  type TurnCheckpointDiffFile,
  type TurnCheckpointDiffInput,
  type TurnCheckpointDiffResult,
  type TurnCheckpointError,
  type TurnCheckpointInspectResult,
  type TurnCheckpointRecord,
  type TurnCheckpointRevertPlanEntry,
  type TurnCheckpointSignal,
  type TurnCheckpointStageResult,
} from "./turn-checkpoint-contract.ts"

const gitTimeoutMs = 15_000
const maxBuffer = 64_000_000
const maxConflictPathsReported = 20

// ---------------------------------------------------------------------------
// Ref naming
// ---------------------------------------------------------------------------

/**
 * One thread/turn ref component: a lowercase [a-z0-9-] slug plus a stable
 * 8-hex content hash. No dots can appear, so `..`, `.lock`, and leading-dot
 * ref rules cannot be violated, and distinct refs never collide on slugs.
 */
export const checkpointRefComponent = (value: string): string => {
  const slug = value
    .toLowerCase()
    .replaceAll(/[^a-z0-9-]+/gu, "-")
    .replaceAll(/-{2,}/gu, "-")
    .replaceAll(/^-+|-+$/gu, "")
    .slice(0, 48)
  const digest = createHash("sha256").update(value, "utf8").digest("hex").slice(0, 8)
  return `${slug === "" ? "x" : slug}-${digest}`
}

export const checkpointThreadRefRoot = (threadRef: string): string =>
  `${TURN_CHECKPOINT_REF_ROOT}/${checkpointRefComponent(threadRef)}`

export const checkpointRefName = (threadRef: string, turnRef: string): string =>
  `${checkpointThreadRefRoot(threadRef)}/${checkpointRefComponent(turnRef)}`

/** Redo-baseline ref for a staged revert. Turn components always carry a hash
 * suffix, so this literal name cannot collide with a turn checkpoint ref. */
export const checkpointBaselineRefName = (threadRef: string): string =>
  `${checkpointThreadRefRoot(threadRef)}/revert-baseline`

/**
 * The thread's last-accounted-state ref: every capture points it at the new
 * checkpoint, and a committed revert points it at the revert target. Worktree
 * changes past this ref are uncheckpointed owner work (the dirty-conflict
 * boundary). An explicit ref, not a date sort — commit timestamps have
 * one-second resolution and consecutive turn boundaries tie.
 */
export const checkpointLatestRefName = (threadRef: string): string =>
  `${checkpointThreadRefRoot(threadRef)}/latest`

// ---------------------------------------------------------------------------
// Bounded exec (no throw; typed outcome)
// ---------------------------------------------------------------------------

type ExecOptions = Readonly<{
  cwd: string
  env?: NodeJS.ProcessEnv
  input?: string
}>

type ExecResult =
  | Readonly<{ ok: true; stdout: string; stdoutBytes: Buffer }>
  | Readonly<{
      ok: false
      kind: "enoent" | "timeout" | "nonzero" | "revoked" | "quiesced"
      code: number | null
    }>

type GitSpawn = (
  command: string,
  args: ReadonlyArray<string>,
  options: SpawnOptions,
) => ChildProcess

type Permit = IdePortableMutationPermit | null

type GitRunner = (
  args: ReadonlyArray<string>,
  options: ExecOptions,
  permit?: Permit,
  mutates?: boolean,
) => Promise<ExecResult>

/**
 * Async, bounded git exec: turn-boundary captures run on the Electron main
 * process, so this seam must never block the event loop the way a spawnSync
 * host does. Credential prompts are disabled; the exit status is authority.
 */
const execRefusal = (result: Extract<ExecResult, { ok: false }>): TurnCheckpointError =>
  turnCheckpointError(result.kind === "enoent" ? "git_unavailable" : "operation_failed")

// ---------------------------------------------------------------------------
// Snapshot capture through an isolated temporary index
// ---------------------------------------------------------------------------

type SnapshotTree = Readonly<{
  ok: true
  tree: string
  fileCount: number
  excludedOversizeCount: number
}>

/**
 * Build a tree object for the current worktree state without touching the
 * user's index: enumerate tracked + non-ignored untracked files, exclude
 * oversized files, stage the survivors into a temp `GIT_INDEX_FILE`, and
 * `write-tree`. Only object-database writes happen; the user's worktree,
 * index, and refs are read-only inputs.
 */
const writeSnapshotTree = async (
  root: string,
  runGit: GitRunner,
  permit: Permit,
): Promise<SnapshotTree | TurnCheckpointError> => {
  const listed = await runGit(
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    { cwd: root },
  )
  if (!listed.ok) return execRefusal(listed)
  const candidates = listed.stdout.split("\0").filter(entry => entry !== "")
  if (candidates.length > TURN_CHECKPOINT_MAX_FILES) {
    return turnCheckpointError("workspace_too_large")
  }
  const admitted: string[] = []
  let excludedOversizeCount = 0
  for (const candidate of candidates) {
    let stats
    try {
      stats = lstatSync(path.join(root, candidate))
    } catch {
      continue // tracked but deleted from the worktree: absent from the snapshot
    }
    if (!stats.isFile() && !stats.isSymbolicLink()) continue
    if (stats.isFile() && stats.size > TURN_CHECKPOINT_MAX_FILE_BYTES) {
      excludedOversizeCount += 1
      continue
    }
    admitted.push(candidate)
  }
  const temp = mkdtempSync(path.join(os.tmpdir(), "oa-turn-checkpoint-"))
  const env = { ...workspaceGitEnvironment(), GIT_INDEX_FILE: path.join(temp, "index") }
  try {
    if (admitted.length > 0) {
      const staged = await runGit(
        ["update-index", "--add", "-z", "--stdin"],
        { cwd: root, env, input: `${admitted.join("\0")}\0` },
        permit,
        true,
      )
      if (!staged.ok) return execRefusal(staged)
    }
    const written = await runGit(["write-tree"], { cwd: root, env }, permit, true)
    if (!written.ok) return execRefusal(written)
    return {
      ok: true,
      tree: written.stdout.trim(),
      fileCount: admitted.length,
      excludedOversizeCount,
    }
  } finally {
    rmSync(temp, { recursive: true, force: true })
  }
}

const commitTree = async (
  root: string,
  tree: string,
  message: string,
  runGit: GitRunner,
  permit: Permit,
): Promise<string | TurnCheckpointError> => {
  const head = await runGit(["rev-parse", "--verify", "--quiet", "HEAD"], { cwd: root })
  const committed = await runGit(
    ["commit-tree", tree, ...(head.ok ? ["-p", head.stdout.trim()] : []), "-m", message],
    {
      cwd: root,
      env: {
        ...workspaceGitEnvironment(),
        GIT_AUTHOR_NAME: "OpenAgents Desktop",
        GIT_AUTHOR_EMAIL: "checkpoints@openagents.local",
        GIT_COMMITTER_NAME: "OpenAgents Desktop",
        GIT_COMMITTER_EMAIL: "checkpoints@openagents.local",
      },
    },
    permit,
    true,
  )
  if (!committed.ok) return execRefusal(committed)
  return committed.stdout.trim()
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

export type DesktopTurnCheckpointService = Readonly<{
  capture: (input: TurnCheckpointCaptureInput) => Promise<TurnCheckpointCaptureResult>
  hasCheckpoint: (threadRef: string, turnRef: string) => Promise<boolean>
  diffTurns: (input: TurnCheckpointDiffInput) => Promise<TurnCheckpointDiffResult>
  stageRevert: (threadRef: string, turnRef: string) => Promise<TurnCheckpointStageResult>
  inspectStagedRevert: (threadRef: string) => Promise<TurnCheckpointInspectResult>
  commitStagedRevert: (threadRef: string) => Promise<TurnCheckpointCommitRevertResult>
  clearStagedRevert: (threadRef: string) => Promise<TurnCheckpointClearRevertResult>
  deleteThreadCheckpoints: (threadRef: string) => Promise<TurnCheckpointDeleteThreadResult>
  quiesce: () => Promise<void>
  dispose: () => Promise<void>
}>

export const openTurnCheckpointService = (options: Readonly<{
  resolveRoot: () => string | null
  resolveGrantRef?: () => string | null
  mutationAuthority?: IdePortableMutationAuthority
  onSignal?: (signal: TurnCheckpointSignal) => void
  gitExecutable?: string
  spawnGit?: GitSpawn
  gitTimeoutMs?: number
  beforeGitSpawn?: (args: ReadonlyArray<string>) => void
  afterGitProcess?: (args: ReadonlyArray<string>) => void
}>): DesktopTurnCheckpointService => {
  const stagedByThread = new Map<string, StagedTurnCheckpointRevert>()
  const activeGit = new Set<ChildProcess>()
  const activeGitSettled = new Map<ChildProcess, Promise<void>>()
  const spawnGit = options.spawnGit ?? spawn
  const mutationAuthority = options.mutationAuthority
  let quiesced = false
  let disposed = false

  const terminateProcessTree = (child: ChildProcess): void => {
    const pid = child.pid
    if (pid !== undefined && process.platform !== "win32") {
      try {
        process.kill(-pid, "SIGKILL")
        return
      } catch {
        // The process can exit between the authority check and cancellation.
      }
    }
    try { child.kill("SIGKILL") } catch { /* already closed */ }
  }

  const permitIsCurrent = (permit: Permit): boolean =>
    !quiesced && !disposed && (
      mutationAuthority === undefined
      || (permit !== null && mutationAuthority.reauthorize(permit))
    )

  const authorizeMutation = (): Permit | TurnCheckpointError => {
    if (quiesced || disposed) return turnCheckpointError("operation_failed")
    if (mutationAuthority === undefined) return null
    const grantRef = options.resolveGrantRef?.() ?? null
    if (grantRef === null) return turnCheckpointError("operation_failed")
    const authorized = mutationAuthority.authorize(grantRef)
    if (authorized._tag === "Refused") return turnCheckpointError("operation_failed")
    return mutationAuthority.reauthorize(authorized.permit)
      ? authorized.permit
      : turnCheckpointError("operation_failed")
  }

  const runGit: GitRunner = (args, execOptions, permit = null, mutates = false) =>
    new Promise(resolve => {
      if (quiesced || disposed) {
        resolve({ ok: false, kind: "quiesced", code: null })
        return
      }
      if (mutates && !permitIsCurrent(permit)) {
        resolve({ ok: false, kind: "revoked", code: null })
        return
      }
      let child: ChildProcess
      try {
        options.beforeGitSpawn?.(args)
        child = spawnGit(options.gitExecutable ?? "git", [...args], {
          cwd: execOptions.cwd,
          detached: process.platform !== "win32",
          env: {
            ...(execOptions.env ?? workspaceGitEnvironment()),
            GIT_TERMINAL_PROMPT: "0",
          },
          stdio: ["pipe", "pipe", "ignore"],
        })
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code
        resolve({ ok: false, kind: code === "ENOENT" ? "enoent" : "nonzero", code: null })
        return
      }

      activeGit.add(child)
      let markSettled: () => void = () => undefined
      activeGitSettled.set(child, new Promise<void>(resolve => { markSettled = resolve }))
      const stdout: Buffer[] = []
      let stdoutBytes = 0
      let settled = false
      let cancellation: "timeout" | "revoked" | "quiesced" | null = null
      const timeout = setTimeout(() => {
        cancellation = "timeout"
        terminateProcessTree(child)
      }, options.gitTimeoutMs ?? gitTimeoutMs)
      const authorityPoll = mutates && mutationAuthority !== undefined
        ? setInterval(() => {
            if (permitIsCurrent(permit)) return
            cancellation = quiesced || disposed ? "quiesced" : "revoked"
            terminateProcessTree(child)
          }, 10)
        : null
      const finish = (result: ExecResult): void => {
        if (settled) return
        settled = true
        clearTimeout(timeout)
        if (authorityPoll !== null) clearInterval(authorityPoll)
        activeGit.delete(child)
        activeGitSettled.delete(child)
        markSettled()
        try { options.afterGitProcess?.(args) } catch { /* observer defects are isolated */ }
        resolve(result)
      }

      child.stdout?.on("data", (chunk: Buffer | string) => {
        const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
        stdoutBytes += bytes.byteLength
        if (stdoutBytes > maxBuffer) {
          cancellation = "timeout"
          terminateProcessTree(child)
          return
        }
        stdout.push(bytes)
      })
      child.once("error", (error: NodeJS.ErrnoException) => {
        finish({ ok: false, kind: error.code === "ENOENT" ? "enoent" : "nonzero", code: null })
      })
      child.once("close", code => {
        if (cancellation !== null) {
          finish({ ok: false, kind: cancellation, code: null })
          return
        }
        if (mutates && !permitIsCurrent(permit)) {
          finish({ ok: false, kind: quiesced || disposed ? "quiesced" : "revoked", code })
          return
        }
        if (code !== 0) {
          finish({ ok: false, kind: "nonzero", code })
          return
        }
        const bytes = Buffer.concat(stdout)
        finish({ ok: true, stdout: bytes.toString("utf8"), stdoutBytes: bytes })
      })
      if (child.stdin !== null) {
        child.stdin.on("error", () => undefined)
        if (execOptions.input !== undefined) child.stdin.write(execOptions.input)
        child.stdin.end()
      }
    })

  // One mutation at a time: capture/stage/commit/delete interleavings would
  // otherwise race on refs and staged state. A simple promise chain is enough
  // for a single-process host.
  let chain: Promise<unknown> = Promise.resolve()
  const serialized = <T>(work: () => Promise<T>): Promise<T> => {
    const next = chain.then(work, work)
    chain = next.catch(() => undefined)
    return next
  }

  const emit = (signal: TurnCheckpointSignal): void => {
    try {
      options.onSignal?.(signal)
    } catch {
      // A listener defect never breaks checkpoint truth.
    }
  }

  const repoRoot = async (): Promise<string | TurnCheckpointError> => {
    const root = options.resolveRoot()
    if (root === null) return turnCheckpointError("no_workspace")
    // Normalize to the worktree top so every repo-relative path this service
    // exchanges with git (snapshot lists, revert plans, checkout-index, rm)
    // shares one base directory.
    const probe = await runGit(["rev-parse", "--show-toplevel"], { cwd: path.resolve(root) })
    if (!probe.ok) {
      return turnCheckpointError(probe.kind === "enoent" ? "git_unavailable" : "not_a_repo")
    }
    const top = probe.stdout.trim()
    return top === "" ? path.resolve(root) : top
  }

  const resolveCheckpointCommit = async (
    root: string,
    threadRef: string,
    turnRef: string,
  ): Promise<string | null> => {
    const resolved = await runGit(
      ["rev-parse", "--verify", "--quiet", `${checkpointRefName(threadRef, turnRef)}^{commit}`],
      { cwd: root },
    )
    return resolved.ok ? resolved.stdout.trim() : null
  }

  /** target/current tree-ish -> restore/delete plan from the target's side. */
  const revertPlan = async (
    root: string,
    targetCommit: string,
    currentTree: string,
  ): Promise<ReadonlyArray<TurnCheckpointRevertPlanEntry> | TurnCheckpointError> => {
    const diffed = await runGit(
      ["diff", "--name-status", "--no-renames", "-z", targetCommit, currentTree],
      { cwd: root },
    )
    if (!diffed.ok) return execRefusal(diffed)
    const parts = diffed.stdout.split("\0").filter(entry => entry !== "")
    const plan: TurnCheckpointRevertPlanEntry[] = []
    for (let index = 0; index + 1 < parts.length; index += 2) {
      const status = parts[index]!
      const file = parts[index + 1]!
      // A = exists now but not in the target -> delete; everything else is
      // restored to the target's exact bytes.
      plan.push({ path: file, action: status.startsWith("A") ? "delete" : "restore" })
    }
    return plan
  }

  const changedPaths = async (
    root: string,
    fromTree: string,
    toTree: string,
  ): Promise<ReadonlyArray<string> | TurnCheckpointError> => {
    const diffed = await runGit(
      ["diff", "--name-only", "--no-renames", "-z", fromTree, toTree],
      { cwd: root },
    )
    if (!diffed.ok) return execRefusal(diffed)
    return diffed.stdout.split("\0").filter(entry => entry !== "")
  }

  const boundedPatch = async (
    root: string,
    fromCommit: string,
    toCommit: string,
  ): Promise<Readonly<{ patch: string; truncated: boolean }> | TurnCheckpointError> => {
    const diffed = await runGit(
      ["diff", "--no-color", "--no-ext-diff", "--no-renames", fromCommit, toCommit],
      { cwd: root },
    )
    if (!diffed.ok) return execRefusal(diffed)
    const bytes = diffed.stdoutBytes
    return bytes.byteLength > TURN_CHECKPOINT_MAX_DIFF_BYTES
      ? {
          patch: bytes.subarray(0, TURN_CHECKPOINT_MAX_DIFF_BYTES).toString("utf8"),
          truncated: true,
        }
      : { patch: diffed.stdout, truncated: false }
  }

  const capture: DesktopTurnCheckpointService["capture"] = (input) =>
    serialized(async () => {
      const permit = authorizeMutation()
      if (permit !== null && "error" in permit) return permit
      const root = await repoRoot()
      if (typeof root !== "string") return root
      const snapshot = await writeSnapshotTree(root, runGit, permit)
      if (!snapshot.ok) return snapshot
      const commit = await commitTree(
        root,
        snapshot.tree,
        `openagents checkpoint ${input.threadRef} ${input.turnRef} ${input.boundary}`,
        runGit,
        permit,
      )
      if (typeof commit !== "string") return commit
      const refName = checkpointRefName(input.threadRef, input.turnRef)
      // One ref per (thread, turn): the completion capture supersedes the
      // start capture for the same turn, matching turn-over-turn diffs.
      const stored = await runGit(
        ["update-ref", "--stdin"],
        {
          cwd: root,
          input: [
            "start",
            `update ${refName} ${commit}`,
            `update ${checkpointLatestRefName(input.threadRef)} ${commit}`,
            "prepare",
            "commit",
            "",
          ].join("\n"),
        },
        permit,
        true,
      )
      if (!stored.ok) return execRefusal(stored)
      if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
      const record: TurnCheckpointRecord = {
        schema: "openagents.desktop.turn_checkpoint.v1",
        threadRef: input.threadRef,
        turnRef: input.turnRef,
        boundary: input.boundary,
        refName,
        commit,
        capturedAt: new Date().toISOString(),
        fileCount: snapshot.fileCount,
        excludedOversizeCount: snapshot.excludedOversizeCount,
      }
      emit({ kind: "checkpoint_captured", record })
      return { ok: true, record }
    })

  const hasCheckpoint: DesktopTurnCheckpointService["hasCheckpoint"] = async (
    threadRef,
    turnRef,
  ) => {
    const root = await repoRoot()
    if (typeof root !== "string") return false
    return (await resolveCheckpointCommit(root, threadRef, turnRef)) !== null
  }

  const diffTurns: DesktopTurnCheckpointService["diffTurns"] = async (input) => {
    const root = await repoRoot()
    if (typeof root !== "string") return root
    const fromCommit = await resolveCheckpointCommit(root, input.threadRef, input.fromTurnRef)
    const toCommit = await resolveCheckpointCommit(root, input.threadRef, input.toTurnRef)
    if (fromCommit === null || toCommit === null) {
      return turnCheckpointError("checkpoint_missing")
    }
    const numstat = await runGit(
      ["diff", "--numstat", "--no-renames", "-z", fromCommit, toCommit],
      { cwd: root },
    )
    if (!numstat.ok) return execRefusal(numstat)
    const files: TurnCheckpointDiffFile[] = []
    for (const entry of numstat.stdout.split("\0")) {
      if (entry === "") continue
      const match = /^(\d+|-)\t(\d+|-)\t(.+)$/su.exec(entry)
      if (match === null) continue
      const binary = match[1] === "-" || match[2] === "-"
      files.push({
        path: match[3]!,
        additions: binary ? 0 : Number.parseInt(match[1]!, 10),
        deletions: binary ? 0 : Number.parseInt(match[2]!, 10),
        binary,
      })
    }
    const patch = await boundedPatch(root, fromCommit, toCommit)
    if ("error" in patch) return patch
    return {
      ok: true,
      threadRef: input.threadRef,
      fromTurnRef: input.fromTurnRef,
      toTurnRef: input.toTurnRef,
      files,
      patch: patch.patch,
      truncated: patch.truncated,
    }
  }

  const stageRevert: DesktopTurnCheckpointService["stageRevert"] = (threadRef, turnRef) =>
    serialized(async () => {
      if (stagedByThread.has(threadRef)) return turnCheckpointError("revert_already_staged")
      const permit = authorizeMutation()
      if (permit !== null && "error" in permit) return permit
      const root = await repoRoot()
      if (typeof root !== "string") return root
      const targetCommit = await resolveCheckpointCommit(root, threadRef, turnRef)
      if (targetCommit === null) return turnCheckpointError("checkpoint_missing")

      // Latest checkpoint of the thread = the last state any turn accounted
      // for. Worktree changes past it are uncheckpointed owner work.
      const latest = await runGit(
        ["rev-parse", "--verify", "--quiet", `${checkpointLatestRefName(threadRef)}^{commit}`],
        { cwd: root },
      )
      if (!latest.ok) return turnCheckpointError("checkpoint_missing")
      const latestCommit = latest.stdout.trim()

      const snapshot = await writeSnapshotTree(root, runGit, permit)
      if (!snapshot.ok) return snapshot

      const plan = await revertPlan(root, targetCommit, snapshot.tree)
      if ("error" in plan) return plan

      // Dirty-conflict refusal: any path the revert would rewrite that also
      // differs between the worktree and the thread's LATEST checkpoint holds
      // uncheckpointed edits the revert would silently destroy.
      const dirty = await changedPaths(root, latestCommit, snapshot.tree)
      if ("error" in dirty) return dirty
      const planPaths = new Set(plan.map(entry => entry.path))
      const conflicting = dirty.filter(file => planPaths.has(file))
      if (conflicting.length > 0) {
        return turnCheckpointError(
          "dirty_conflicting_state",
          conflicting.slice(0, maxConflictPathsReported),
        )
      }

      const baselineCommit = await commitTree(
        root,
        snapshot.tree,
        `openagents checkpoint ${threadRef} revert-baseline`,
        runGit,
        permit,
      )
      if (typeof baselineCommit !== "string") return baselineCommit
      const baselineStored = await runGit(
        ["update-ref", checkpointBaselineRefName(threadRef), baselineCommit],
        { cwd: root },
        permit,
        true,
      )
      if (!baselineStored.ok) return execRefusal(baselineStored)
      if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")

      const staged: StagedTurnCheckpointRevert = {
        threadRef,
        turnRef,
        targetCommit,
        baselineCommit,
        plan,
        irreversibleEffects: TURN_CHECKPOINT_IRREVERSIBLE_EFFECTS,
        stagedAt: new Date().toISOString(),
      }
      stagedByThread.set(threadRef, staged)
      return { ok: true, staged }
    })

  const inspectStagedRevert: DesktopTurnCheckpointService["inspectStagedRevert"] = async (
    threadRef,
  ) => {
    const staged = stagedByThread.get(threadRef)
    if (staged === undefined) return turnCheckpointError("no_staged_revert")
    const root = await repoRoot()
    if (typeof root !== "string") return root
    const patch = await boundedPatch(root, staged.baselineCommit, staged.targetCommit)
    if ("error" in patch) return patch
    return { ok: true, staged, patch: patch.patch, truncated: patch.truncated }
  }

  const commitStagedRevert: DesktopTurnCheckpointService["commitStagedRevert"] = (threadRef) =>
    serialized(async () => {
      const staged = stagedByThread.get(threadRef)
      if (staged === undefined) return turnCheckpointError("no_staged_revert")
      const permit = authorizeMutation()
      if (permit !== null && "error" in permit) return permit
      const root = await repoRoot()
      if (typeof root !== "string") return root

      // The worktree must still match the staged baseline in every planned
      // path: anything written since stage-time is uncheckpointed and would
      // be silently destroyed.
      const snapshot = await writeSnapshotTree(root, runGit, permit)
      if (!snapshot.ok) return snapshot
      const drift = await changedPaths(root, staged.baselineCommit, snapshot.tree)
      if ("error" in drift) return drift
      const planPaths = new Set(staged.plan.map(entry => entry.path))
      const conflicting = drift.filter(file => planPaths.has(file))
      if (conflicting.length > 0) {
        return turnCheckpointError(
          "dirty_conflicting_state",
          conflicting.slice(0, maxConflictPathsReported),
        )
      }

      const restorePaths = staged.plan
        .filter(entry => entry.action === "restore")
        .map(entry => entry.path)
      if (restorePaths.length > 0) {
        // Isolated temp index again: read the target tree into it and let git
        // write the exact recorded bytes/modes for ONLY the planned paths.
        // The user's real index is never opened for writing.
        const temp = mkdtempSync(path.join(os.tmpdir(), "oa-turn-checkpoint-"))
        const env = { ...workspaceGitEnvironment(), GIT_INDEX_FILE: path.join(temp, "index") }
        try {
          const loaded = await runGit(
            ["read-tree", staged.targetCommit],
            { cwd: root, env },
            permit,
            true,
          )
          if (!loaded.ok) return execRefusal(loaded)
          const restored = await runGit(
            ["checkout-index", "--force", "-z", "--stdin"],
            { cwd: root, env, input: `${restorePaths.join("\0")}\0` },
            permit,
            true,
          )
          if (!restored.ok) return execRefusal(restored)
        } finally {
          rmSync(temp, { recursive: true, force: true })
        }
      }
      let deletedCount = 0
      for (const entry of staged.plan) {
        if (entry.action !== "delete") continue
        if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
        try {
          rmSync(path.join(root, entry.path), { force: true })
          if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
          deletedCount += 1
        } catch {
          return turnCheckpointError("operation_failed")
        }
      }
      // The revert target is now the last accounted state: point the thread's
      // latest ref at it so subsequent stages see a clean worktree.
      const latestStored = await runGit(
        ["update-ref", checkpointLatestRefName(threadRef), staged.targetCommit],
        { cwd: root },
        permit,
        true,
      )
      if (!latestStored.ok) return execRefusal(latestStored)
      if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
      stagedByThread.delete(threadRef)
      emit({
        kind: "revert_committed",
        threadRef,
        turnRef: staged.turnRef,
        restoredCount: restorePaths.length,
        deletedCount,
        committedAt: new Date().toISOString(),
      })
      return { ok: true, restoredCount: restorePaths.length, deletedCount }
    })

  const clearStagedRevert: DesktopTurnCheckpointService["clearStagedRevert"] = (threadRef) =>
    serialized(async () => {
      const staged = stagedByThread.get(threadRef)
      if (staged === undefined) return turnCheckpointError("no_staged_revert")
      const permit = authorizeMutation()
      if (permit !== null && "error" in permit) return permit
      const root = await repoRoot()
      if (typeof root === "string") {
        const cleared = await runGit(
          ["update-ref", "-d", checkpointBaselineRefName(threadRef)],
          { cwd: root },
          permit,
          true,
        )
        if (!cleared.ok) return execRefusal(cleared)
      } else return root
      if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
      stagedByThread.delete(threadRef)
      return { ok: true }
    })

  const deleteThreadCheckpoints: DesktopTurnCheckpointService["deleteThreadCheckpoints"] = (
    threadRef,
  ) =>
    serialized(async () => {
      const permit = authorizeMutation()
      if (permit !== null && "error" in permit) return permit
      const root = await repoRoot()
      if (typeof root !== "string") return root
      const listed = await runGit(
        ["for-each-ref", "--format=%(refname)", `${checkpointThreadRefRoot(threadRef)}/*`],
        { cwd: root },
      )
      if (!listed.ok) return execRefusal(listed)
      const refNames = listed.stdout.split("\n").map(ref => ref.trim()).filter(ref => ref !== "")
      if (refNames.length > 0) {
        const removed = await runGit(
          ["update-ref", "--stdin"],
          {
            cwd: root,
            input: [
              "start",
              ...refNames.map(refName => `delete ${refName}`),
              "prepare",
              "commit",
              "",
            ].join("\n"),
          },
          permit,
          true,
        )
        if (!removed.ok) return execRefusal(removed)
      }
      if (!permitIsCurrent(permit)) return turnCheckpointError("operation_failed")
      stagedByThread.delete(threadRef)
      return { ok: true, deletedRefCount: refNames.length }
    })

  const quiesce = async (): Promise<void> => {
    if (quiesced) {
      await Promise.all([...activeGitSettled.values()])
      return chain.then(() => undefined, () => undefined)
    }
    quiesced = true
    const settling = [...activeGitSettled.values()]
    for (const child of activeGit) terminateProcessTree(child)
    await Promise.all(settling)
    await chain.then(() => undefined, () => undefined)
  }

  const dispose = async (): Promise<void> => {
    if (disposed) {
      await Promise.all([...activeGitSettled.values()])
      return chain.then(() => undefined, () => undefined)
    }
    disposed = true
    quiesced = true
    const settling = [...activeGitSettled.values()]
    for (const child of activeGit) terminateProcessTree(child)
    await Promise.all(settling)
    await chain.then(() => undefined, () => undefined)
  }

  return {
    capture,
    hasCheckpoint,
    diffTurns,
    stageRevert,
    inspectStagedRevert,
    commitStagedRevert,
    clearStagedRevert,
    deleteThreadCheckpoints,
    quiesce,
    dispose,
  }
}
