/**
 * Enforced oracles for hidden-ref turn checkpoints (GIT-1, #8781):
 * `openagents_desktop.workbench.turn_checkpoints.v1`.
 *
 * Proves, against a real fixture repository:
 * 1. capture on a fixture turn creates the hidden ref WITHOUT touching user
 *    branches, HEAD, the user index, or the worktree;
 * 2. the typed turn-over-turn diff query reports real file changes;
 * 3. committed revert restores exact bytes (text and binary), including
 *    files deleted and created between turns;
 * 4. staged revert transitions honestly: stage -> inspect (irreversible-
 *    effects statement present) -> commit/clear, and refuses double-stage
 *    and commit-without-stage;
 * 5. dirty conflicting state refuses both stage and commit;
 * 6. thread deletion cleans up every hidden ref for that thread only;
 * 7. capture bounds hold (ignored + oversized files excluded) and capture
 *    emits the typed local completion signal;
 * 8. the contract registry carries this suite as the enforced oracle and
 *    main.ts wires capture at both turn boundaries on both local lanes.
 */
import { afterAll, describe, expect, test } from "vite-plus/test"
import {
  execFileSync,
  spawn,
  type ChildProcess,
  type SpawnOptions,
} from "node:child_process"
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import os from "node:os"
import path from "node:path"

import {
  checkpointRefComponent,
  checkpointRefName,
  checkpointLatestRefName,
  checkpointThreadRefRoot,
  openTurnCheckpointService,
} from "../src/turn-checkpoint-host.ts"
import {
  TURN_CHECKPOINT_CONTRACT,
  TURN_CHECKPOINT_IRREVERSIBLE_EFFECTS,
  TURN_CHECKPOINT_MAX_FILE_BYTES,
  TURN_CHECKPOINT_REF_ROOT,
  type TurnCheckpointSignal,
} from "../src/turn-checkpoint-contract.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"
import { isolatedGitEnvironment, runGitFixture } from "./git-fixture.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "../src/ide/portable-mutation-authority.ts"

const testsDir = path.dirname(new URL(import.meta.url).pathname)
const appDir = path.dirname(testsDir)

const cleanups: Array<() => void> = []
afterAll(() => {
  for (const cleanup of cleanups) cleanup()
})

const fixtureRepo = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-checkpoint-fixture-"))
  cleanups.push(() => rmSync(root, { recursive: true, force: true }))
  runGitFixture(root, ["init", "--quiet", "-b", "main"])
  runGitFixture(root, ["config", "user.email", "checkpoint-test@openagents.test"])
  runGitFixture(root, ["config", "user.name", "Checkpoint Test"])
  writeFileSync(path.join(root, "app.ts"), "export const value = 1\n")
  writeFileSync(path.join(root, "notes.txt"), "keep these notes\n")
  writeFileSync(path.join(root, ".gitignore"), "ignored.txt\n")
  runGitFixture(root, ["add", "-A"])
  runGitFixture(root, ["commit", "--quiet", "-m", "base"])
  return root
}

const gitState = (root: string): Readonly<{
  head: string
  branches: string
  status: string
  index: string
  stashes: string
}> => ({
  head: runGitFixture(root, ["rev-parse", "HEAD"]).trim(),
  branches: runGitFixture(root, ["for-each-ref", "refs/heads", "refs/tags"]),
  status: runGitFixture(root, ["status", "--porcelain=v2"]),
  index: runGitFixture(root, ["ls-files", "-s"]),
  stashes: runGitFixture(root, ["stash", "list"]),
})

const service = (root: string, signals?: TurnCheckpointSignal[]) =>
  openTurnCheckpointService({
    resolveRoot: () => root,
    ...(signals === undefined
      ? {}
      : { onSignal: (signal: TurnCheckpointSignal) => signals.push(signal) }),
  })

const portableAuthority = (): Readonly<{
  authority: IdePortableMutationAuthority
  revoke: () => void
}> => {
  let active = true
  const permit: IdePortableMutationPermit = {
    _tag: "Portable",
    key: "portable:grant:session:context:attachment:1:target",
    grantRef: "workspace.grant.fixture",
    sessionRef: "portable.session.fixture",
    workContextRef: "portable.context.fixture",
    attachmentRef: "portable.attachment.fixture",
    generation: 1,
    targetRef: "portable.target.fixture",
  }
  return {
    authority: {
      authorize: grantRef => active && grantRef === permit.grantRef
        ? { _tag: "Permitted", permit }
        : { _tag: "Refused", reason: "admission_unavailable" },
      reauthorize: candidate => active && candidate.key === permit.key,
    },
    revoke: () => { active = false },
  }
}

const portableService = (
  root: string,
  authority: IdePortableMutationAuthority,
  options: Readonly<{
    beforeGitSpawn?: (args: ReadonlyArray<string>) => void
    afterGitProcess?: (args: ReadonlyArray<string>) => void
    spawnGit?: (
      command: string,
      args: ReadonlyArray<string>,
      options: SpawnOptions,
    ) => ChildProcess
    signals?: TurnCheckpointSignal[]
  }> = {},
) => openTurnCheckpointService({
  resolveRoot: () => root,
  resolveGrantRef: () => "workspace.grant.fixture",
  mutationAuthority: authority,
  ...(options.beforeGitSpawn === undefined ? {} : { beforeGitSpawn: options.beforeGitSpawn }),
  ...(options.afterGitProcess === undefined ? {} : { afterGitProcess: options.afterGitProcess }),
  ...(options.spawnGit === undefined ? {} : { spawnGit: options.spawnGit }),
  ...(options.signals === undefined
    ? {}
    : { onSignal: (signal: TurnCheckpointSignal) => options.signals?.push(signal) }),
})

describe("openagents_desktop.workbench.turn_checkpoints.v1", () => {
  test("capture creates the hidden ref without touching user branches, index, HEAD, or worktree", async () => {
    const root = fixtureRepo()
    const signals: TurnCheckpointSignal[] = []
    const checkpoints = service(root, signals)

    // Simulated turn output: a modified tracked file plus a new file, left
    // deliberately uncommitted/unstaged the way a real coding turn leaves it.
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    writeFileSync(path.join(root, "generated.ts"), "export const generated = true\n")
    // Excluded material: an ignored file and an oversized file.
    writeFileSync(path.join(root, "ignored.txt"), "never captured\n")
    writeFileSync(
      path.join(root, "huge.bin"),
      Buffer.alloc(TURN_CHECKPOINT_MAX_FILE_BYTES + 1, 7),
    )

    const before = gitState(root)
    const captured = await checkpoints.capture({
      threadRef: "thread.alpha",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })
    expect(captured.ok).toBe(true)
    if (!captured.ok) throw new Error(captured.error)

    // The hidden ref exists, under refs/openagents/, never refs/heads.
    const refName = checkpointRefName("thread.alpha", "turn.1")
    expect(refName.startsWith(`${TURN_CHECKPOINT_REF_ROOT}/`)).toBe(true)
    expect(captured.record.refName).toBe(refName)
    const resolved = runGitFixture(root, ["rev-parse", "--verify", `${refName}^{commit}`]).trim()
    expect(resolved).toBe(captured.record.commit)

    // Zero changes to user-visible git state.
    const after = gitState(root)
    expect(after).toEqual(before)

    // Snapshot content: tracked + new non-ignored file in; ignored and
    // oversized files out.
    const tree = runGitFixture(root, ["ls-tree", "-r", "--name-only", refName])
    expect(tree).toContain("app.ts")
    expect(tree).toContain("generated.ts")
    expect(tree).not.toContain("ignored.txt")
    expect(tree).not.toContain("huge.bin")
    expect(captured.record.excludedOversizeCount).toBe(1)
    expect(captured.record.boundary).toBe("turn_completed")

    // The exact modified bytes were recorded, not the HEAD version.
    const recorded = runGitFixture(root, ["show", `${refName}:app.ts`])
    expect(recorded).toBe("export const value = 2\n")

    // Typed local completion signal (SIG-1-shaped) was emitted.
    expect(signals).toHaveLength(1)
    expect(signals[0]).toEqual({ kind: "checkpoint_captured", record: captured.record })

    expect(await checkpoints.hasCheckpoint("thread.alpha", "turn.1")).toBe(true)
    expect(await checkpoints.hasCheckpoint("thread.alpha", "turn.none")).toBe(false)
  })

  test("typed turn-over-turn diff reports the real file changes between checkpoints", async () => {
    const root = fixtureRepo()
    const checkpoints = service(root)

    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })

    writeFileSync(path.join(root, "app.ts"), "export const value = 1\nexport const extra = 2\n")
    rmSync(path.join(root, "notes.txt"))
    writeFileSync(path.join(root, "created.txt"), "made in turn 2\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })

    const diff = await checkpoints.diffTurns({
      threadRef: "t",
      fromTurnRef: "turn.1",
      toTurnRef: "turn.2",
    })
    expect(diff.ok).toBe(true)
    if (!diff.ok) throw new Error(diff.error)
    const byPath = new Map(diff.files.map(file => [file.path, file]))
    expect(byPath.get("app.ts")?.additions).toBe(1)
    expect(byPath.get("notes.txt")?.deletions).toBe(1)
    expect(byPath.get("created.txt")?.additions).toBe(1)
    expect(diff.patch).toContain("made in turn 2")
    expect(diff.truncated).toBe(false)

    const missing = await checkpoints.diffTurns({
      threadRef: "t",
      fromTurnRef: "turn.absent",
      toTurnRef: "turn.2",
    })
    expect(missing.ok).toBe(false)
    if (!missing.ok) expect(missing.error).toBe("checkpoint_missing")
  })

  test("staged revert: stage -> inspect -> commit restores exact bytes, including binary", async () => {
    const root = fixtureRepo()
    const signals: TurnCheckpointSignal[] = []
    const checkpoints = service(root, signals)

    const binaryBytes = Buffer.from([0, 1, 2, 3, 255, 254, 10, 13, 0, 42])
    writeFileSync(path.join(root, "data.bin"), binaryBytes)
    const turnOneApp = "export const value = 1 // turn one exact\n"
    writeFileSync(path.join(root, "app.ts"), turnOneApp)
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })

    // Turn 2 mutates everything: edits, deletes, creates.
    writeFileSync(path.join(root, "app.ts"), "export const value = 999\n")
    writeFileSync(path.join(root, "data.bin"), Buffer.from([9, 9, 9]))
    rmSync(path.join(root, "notes.txt"))
    writeFileSync(path.join(root, "turn2-artifact.txt"), "should be deleted by revert\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })

    const indexBefore = runGitFixture(root, ["ls-files", "-s"])
    const branchesBefore = runGitFixture(root, ["for-each-ref", "refs/heads", "refs/tags"])
    const headBefore = runGitFixture(root, ["rev-parse", "HEAD"]).trim()

    // Commit without a stage refuses.
    const premature = await checkpoints.commitStagedRevert("t")
    expect(premature.ok).toBe(false)
    if (!premature.ok) expect(premature.error).toBe("no_staged_revert")

    const staged = await checkpoints.stageRevert("t", "turn.1")
    expect(staged.ok).toBe(true)
    if (!staged.ok) throw new Error(staged.error)
    const actions = new Map(staged.staged.plan.map(entry => [entry.path, entry.action]))
    expect(actions.get("app.ts")).toBe("restore")
    expect(actions.get("data.bin")).toBe("restore")
    expect(actions.get("notes.txt")).toBe("restore")
    expect(actions.get("turn2-artifact.txt")).toBe("delete")

    // Staging is inspection prep, not mutation: worktree still holds turn 2.
    expect(readFileSync(path.join(root, "app.ts"), "utf8")).toBe("export const value = 999\n")

    // Double stage refuses.
    const restaged = await checkpoints.stageRevert("t", "turn.2")
    expect(restaged.ok).toBe(false)
    if (!restaged.ok) expect(restaged.error).toBe("revert_already_staged")

    // Inspect carries the irreversible-effects statement and a real patch.
    const inspected = await checkpoints.inspectStagedRevert("t")
    expect(inspected.ok).toBe(true)
    if (!inspected.ok) throw new Error(inspected.error)
    expect(inspected.staged.irreversibleEffects).toEqual(TURN_CHECKPOINT_IRREVERSIBLE_EFFECTS)
    expect(inspected.staged.irreversibleEffects.join(" ")).toContain("not erasure")
    expect(inspected.patch).toContain("turn one exact")

    const committed = await checkpoints.commitStagedRevert("t")
    expect(committed.ok).toBe(true)
    if (!committed.ok) throw new Error(committed.error)
    expect(committed.restoredCount).toBe(3)
    expect(committed.deletedCount).toBe(1)

    // Exact bytes restored, created artifact removed.
    expect(readFileSync(path.join(root, "app.ts"), "utf8")).toBe(turnOneApp)
    expect(Buffer.compare(readFileSync(path.join(root, "data.bin")), binaryBytes)).toBe(0)
    expect(readFileSync(path.join(root, "notes.txt"), "utf8")).toBe("keep these notes\n")
    expect(existsSync(path.join(root, "turn2-artifact.txt"))).toBe(false)

    // Revert rewrote the worktree ONLY: user index, branches, HEAD untouched.
    expect(runGitFixture(root, ["ls-files", "-s"])).toBe(indexBefore)
    expect(runGitFixture(root, ["for-each-ref", "refs/heads", "refs/tags"])).toBe(branchesBefore)
    expect(runGitFixture(root, ["rev-parse", "HEAD"]).trim()).toBe(headBefore)

    // Staged state is consumed; the typed revert signal was emitted.
    const consumed = await checkpoints.inspectStagedRevert("t")
    expect(consumed.ok).toBe(false)
    if (!consumed.ok) expect(consumed.error).toBe("no_staged_revert")
    const revertSignal = signals.find(signal => signal.kind === "revert_committed")
    expect(revertSignal).toEqual({
      kind: "revert_committed",
      threadRef: "t",
      turnRef: "turn.1",
      restoredCount: 3,
      deletedCount: 1,
      committedAt: expect.any(String),
    })
  })

  test("clear abandons a staged revert without touching the worktree", async () => {
    const root = fixtureRepo()
    const checkpoints = service(root)
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })

    const before = gitState(root)
    const staged = await checkpoints.stageRevert("t", "turn.1")
    expect(staged.ok).toBe(true)
    const cleared = await checkpoints.clearStagedRevert("t")
    expect(cleared.ok).toBe(true)
    expect(gitState(root)).toEqual(before)
    expect(readFileSync(path.join(root, "app.ts"), "utf8")).toBe("export const value = 2\n")

    // Cleared means gone: a second clear and an inspect both refuse.
    const clearedAgain = await checkpoints.clearStagedRevert("t")
    expect(clearedAgain.ok).toBe(false)
    if (!clearedAgain.ok) expect(clearedAgain.error).toBe("no_staged_revert")
    // And the thread can stage again after clearing.
    const restaged = await checkpoints.stageRevert("t", "turn.1")
    expect(restaged.ok).toBe(true)
  })

  test("dirty conflicting state refuses stage, and post-stage drift refuses commit", async () => {
    const root = fixtureRepo()
    const checkpoints = service(root)
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })

    // Uncheckpointed OWNER edit after the last capture, in a path the revert
    // would rewrite: stage must refuse rather than destroy it.
    writeFileSync(path.join(root, "app.ts"), "export const value = 3 // manual owner edit\n")
    const refused = await checkpoints.stageRevert("t", "turn.1")
    expect(refused.ok).toBe(false)
    if (!refused.ok) {
      expect(refused.error).toBe("dirty_conflicting_state")
      expect(refused.conflictingPaths).toEqual(["app.ts"])
    }

    // Restore the checkpointed state; stage now succeeds.
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    const staged = await checkpoints.stageRevert("t", "turn.1")
    expect(staged.ok).toBe(true)

    // Drift after stage-time in a planned path refuses commit.
    writeFileSync(path.join(root, "app.ts"), "export const value = 4 // drift\n")
    const drifted = await checkpoints.commitStagedRevert("t")
    expect(drifted.ok).toBe(false)
    if (!drifted.ok) {
      expect(drifted.error).toBe("dirty_conflicting_state")
      expect(drifted.conflictingPaths).toEqual(["app.ts"])
    }
  })

  test("thread deletion removes every hidden ref for that thread only", async () => {
    const root = fixtureRepo()
    const checkpoints = service(root)
    await checkpoints.capture({ threadRef: "thread.b", turnRef: "turn.1", boundary: "turn_start" })
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    await checkpoints.capture({
      threadRef: "thread.b",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })
    await checkpoints.capture({
      threadRef: "thread.b",
      turnRef: "turn.2",
      boundary: "turn_completed",
    })
    await checkpoints.capture({
      threadRef: "thread.c",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })

    const deleted = await checkpoints.deleteThreadCheckpoints("thread.b")
    expect(deleted.ok).toBe(true)
    // turn.1 + turn.2 + the thread's latest ref.
    if (deleted.ok) expect(deleted.deletedRefCount).toBe(3)

    // Note: a trailing-slash prefix (not "/*") lists ALL nesting levels.
    const remaining = runGitFixture(root, [
      "for-each-ref",
      "--format=%(refname)",
      `${TURN_CHECKPOINT_REF_ROOT}/`,
    ])
    expect(remaining).not.toContain(checkpointThreadRefRoot("thread.b"))
    expect(remaining).toContain(checkpointRefName("thread.c", "turn.1"))

    expect(await checkpoints.hasCheckpoint("thread.b", "turn.1")).toBe(false)
    expect(await checkpoints.hasCheckpoint("thread.c", "turn.1")).toBe(true)
  })

  test("non-repo and missing workspaces refuse typed, never throw", async () => {
    const plainDir = mkdtempSync(path.join(os.tmpdir(), "oa-checkpoint-plain-"))
    cleanups.push(() => rmSync(plainDir, { recursive: true, force: true }))
    const notARepo = await openTurnCheckpointService({ resolveRoot: () => plainDir }).capture({
      threadRef: "t",
      turnRef: "turn.1",
      boundary: "turn_start",
    })
    expect(notARepo.ok).toBe(false)
    if (!notARepo.ok) expect(notARepo.error).toBe("not_a_repo")

    const noWorkspace = await openTurnCheckpointService({ resolveRoot: () => null }).capture({
      threadRef: "t",
      turnRef: "turn.1",
      boundary: "turn_start",
    })
    expect(noWorkspace.ok).toBe(false)
    if (!noWorkspace.ok) expect(noWorkspace.error).toBe("no_workspace")
  })

  test("capture works on an unborn branch (no commits yet)", async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "oa-checkpoint-unborn-"))
    cleanups.push(() => rmSync(root, { recursive: true, force: true }))
    execFileSync("git", ["init", "--quiet", "-b", "main"], {
      cwd: root,
      env: isolatedGitEnvironment(),
      stdio: "ignore",
    })
    writeFileSync(path.join(root, "first.txt"), "first bytes\n")
    const checkpoints = openTurnCheckpointService({ resolveRoot: () => root })
    const captured = await checkpoints.capture({
      threadRef: "t",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })
    expect(captured.ok).toBe(true)
    if (captured.ok) {
      const tree = runGitFixture(root, ["ls-tree", "-r", "--name-only", captured.record.refName])
      expect(tree).toContain("first.txt")
    }
  })

  test("revocation before capture starts no Git process and writes no checkpoint ref", async () => {
    const root = fixtureRepo()
    const portable = portableAuthority()
    let spawnCount = 0
    portable.revoke()
    const checkpoints = portableService(root, portable.authority, {
      beforeGitSpawn: () => { spawnCount += 1 },
    })
    const result = await checkpoints.capture({
      threadRef: "revoked",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })
    expect(result).toEqual({ ok: false, error: "operation_failed" })
    expect(spawnCount).toBe(0)
    expect(runGitFixture(root, ["for-each-ref", checkpointThreadRefRoot("revoked")])).toBe("")
    await checkpoints.dispose()
  })

  test("revocation during a blocked commit kills it and withholds hidden refs", async () => {
    const root = fixtureRepo()
    const portable = portableAuthority()
    let startedResolve: (() => void) | null = null
    const started = new Promise<void>(resolve => { startedResolve = resolve })
    const checkpoints = portableService(root, portable.authority, {
      spawnGit: (command, args, options) => {
        if (args[0] === "commit-tree") {
          const child = spawn(
            process.execPath,
            ["-e", "setInterval(() => undefined, 1000)"],
            options,
          )
          startedResolve?.()
          return child
        }
        return spawn(command, [...args], options)
      },
    })
    const capture = checkpoints.capture({
      threadRef: "blocked",
      turnRef: "turn.1",
      boundary: "turn_completed",
    })
    await started
    portable.revoke()
    expect(await capture).toEqual({ ok: false, error: "operation_failed" })
    expect(runGitFixture(root, ["for-each-ref", checkpointThreadRefRoot("blocked")])).toBe("")
    await checkpoints.quiesce()
    expect(await checkpoints.capture({
      threadRef: "blocked",
      turnRef: "turn.2",
      boundary: "turn_completed",
    })).toEqual({ ok: false, error: "operation_failed" })
  })

  test("revocation during blocked checkout leaves the worktree and staged truth unchanged", async () => {
    const root = fixtureRepo()
    const portable = portableAuthority()
    let blockCheckout = false
    let startedResolve: (() => void) | null = null
    const started = new Promise<void>(resolve => { startedResolve = resolve })
    const checkpoints = portableService(root, portable.authority, {
      spawnGit: (command, args, options) => {
        if (blockCheckout && args[0] === "checkout-index") {
          const child = spawn(
            process.execPath,
            ["-e", "setInterval(() => undefined, 1000)"],
            options,
          )
          startedResolve?.()
          return child
        }
        return spawn(command, [...args], options)
      },
    })
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })
    expect((await checkpoints.stageRevert("t", "turn.1")).ok).toBe(true)
    const latestBefore = runGitFixture(root, ["rev-parse", checkpointLatestRefName("t")]).trim()
    blockCheckout = true
    const commit = checkpoints.commitStagedRevert("t")
    await started
    portable.revoke()
    expect(await commit).toEqual({ ok: false, error: "operation_failed" })
    expect(readFileSync(path.join(root, "app.ts"), "utf8")).toBe("export const value = 2\n")
    expect(runGitFixture(root, ["rev-parse", checkpointLatestRefName("t")]).trim()).toBe(latestBefore)
    expect((await checkpoints.inspectStagedRevert("t")).ok).toBe(true)
    await checkpoints.dispose()
  })

  test("late revocation reports a partial revert and retains its staged recovery truth", async () => {
    const root = fixtureRepo()
    const portable = portableAuthority()
    const signals: TurnCheckpointSignal[] = []
    const checkpoints = portableService(root, portable.authority, {
      signals,
      afterGitProcess: args => {
        if (args[0] === "checkout-index") portable.revoke()
      },
    })
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.1", boundary: "turn_completed" })
    writeFileSync(path.join(root, "app.ts"), "export const value = 2\n")
    await checkpoints.capture({ threadRef: "t", turnRef: "turn.2", boundary: "turn_completed" })
    expect((await checkpoints.stageRevert("t", "turn.1")).ok).toBe(true)
    const latestBefore = runGitFixture(root, ["rev-parse", checkpointLatestRefName("t")]).trim()
    expect(await checkpoints.commitStagedRevert("t")).toEqual({
      ok: false,
      error: "operation_failed",
    })
    expect(readFileSync(path.join(root, "app.ts"), "utf8")).toBe("export const value = 1\n")
    expect(runGitFixture(root, ["rev-parse", checkpointLatestRefName("t")]).trim()).toBe(latestBefore)
    expect((await checkpoints.inspectStagedRevert("t")).ok).toBe(true)
    expect(signals.some(signal => signal.kind === "revert_committed")).toBe(false)
    await checkpoints.dispose()
  })

  test("ref components are always valid git ref pieces, even for hostile inputs", () => {
    for (const hostile of ["..", "a..b", "x.lock", ".hidden", "a/b/c", "@{upstream}", "", "ünïcode", "A B\tC\n"]) {
      const component = checkpointRefComponent(hostile)
      expect(component).toMatch(/^[a-z0-9-]+-[0-9a-f]{8}$/)
      expect(component).not.toContain("..")
    }
    // Distinct raw refs that sanitize to the same slug still get distinct refs.
    expect(checkpointRefComponent("turn 1")).not.toBe(checkpointRefComponent("turn_1"))
  })

  test("the registry enforces this contract with this suite as its oracle, and main wires both turn boundaries", () => {
    const contract = openAgentsDesktopUxContractRegistry.contracts.find(
      entry => entry.contractId === TURN_CHECKPOINT_CONTRACT,
    )
    expect(contract).toBeDefined()
    expect(contract?.state).toBe("enforced")
    expect(
      contract?.oracles.some(
        oracle => oracle.ref === "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
      ),
    ).toBe(true)

    // GUARANTEES.md documents the promise.
    const guarantees = readFileSync(path.join(appDir, "GUARANTEES.md"), "utf8")
    expect(guarantees).toContain(TURN_CHECKPOINT_CONTRACT)
    expect(guarantees).toContain("refs/openagents/checkpoints")

    // The reactor seam is wired: main gives the shared provider-lane
    // dispatcher the checkpoint service, and that one engine captures both
    // boundaries for every local lane.
    const main = readFileSync(path.join(appDir, "src", "main.ts"), "utf8")
    const providerLane = readFileSync(path.join(appDir, "src", "provider-lane.ts"), "utf8")
    expect(main).toContain("openTurnCheckpointService(")
    expect(main).toContain("captureTurnCheckpoint,")
    expect(providerLane).toContain('deps.captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_start")')
    expect(providerLane).toContain('deps.captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_completed")')
  })
})
