import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, readFile, rm, utimes, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { Deferred, Effect, Exit } from "effect"
import {
  captureWorkspaceChanges,
  cleanupExpiredWorkspaces,
  commitWorkspaceChanges,
  createGitWorktreeCheckoutRunner,
  detectInFlightVirtualBranchConflicts,
  detectWorkspaceChangeConflicts,
  enforcePreparedWorktreeCacheBudget,
  gitCredentialHelperRuntimePathsFor,
  materializeGitCheckoutWorkspaceWithLease,
  preparedWorktreeCacheKeyFor,
  prebuiltBaselineCacheKeyFor,
  pruneWorkspaceCacheDirectories,
  publicWorkspaceChangeCaptureProjection,
  publicWorkspaceLeaseProjection,
  releaseWorkspace,
  repositoryCacheProcessLockOwnerIsLive,
  repositoryCacheKeyFor,
  scopedMaterializedGitCheckoutWorkspace,
  stageWorkspacePaths,
  withWorkspaceMaterializerCapability,
  virtualBranchChangeFileRef,
  workspaceChangeFileRef,
  workspaceLeaseRecordFor,
  WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA,
  WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA,
  WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF,
  WORKSPACE_LEASE_SCHEMA,
  WORKSPACE_MATERIALIZER_CAPABILITY_REF,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
  type WorkspaceLeaseRecord,
} from "../src/workspace-materializer"
import { CLAUDE_AGENT_CAPABILITY_REF } from "../src/claude-agent"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { assertPublicProjectionSafe } from "../src/state"

// Behavior contract oracle: background_agents.credentials.no_long_lived_tokens_in_workspaces.v1
// Behavior contract oracle: background_agents.warm_dispatch.prepared_worktree_cache.v1
// Behavior contract oracle: background_agents.warm_dispatch.prebuilt_baseline_cache.v1

async function run(args: string[], cwd: string): Promise<string> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new Error(`command failed: ${args.join(" ")}\n${stderr}`)
  return stdout
}

async function createOriginRepo(root: string, branch = "main"): Promise<{ url: string; commitSha: string }> {
  await mkdir(root, { recursive: true })
  await run(["git", "init"], root)
  await run(["git", "config", "user.email", "fixture@test.local"], root)
  await run(["git", "config", "user.name", "Fixture"], root)
  await writeFile(
    join(root, "package.json"),
    `${JSON.stringify({ private: true, scripts: { test: "bun test sum.test.ts" }, type: "module" }, null, 2)}\n`,
  )
  await writeFile(join(root, ".gitignore"), "node_modules/\n.pylon-prebuilt/\n")
  await writeFile(join(root, "sum.ts"), "export const sum = (left: number, right: number) => left - right\n")
  await run(["git", "add", "."], root)
  await run(["git", "commit", "-m", "fixture"], root)
  await run(["git", "branch", "-M", branch], root)
  const commitSha = (await run(["git", "rev-parse", "HEAD"], root)).trim()
  return { url: `file://${root}`, commitSha }
}

async function commitOriginChange(root: string, fileName: string, content: string): Promise<string> {
  await writeFile(join(root, fileName), content)
  await run(["git", "add", fileName], root)
  await run(["git", "commit", "-m", `fixture ${fileName}`], root)
  return (await run(["git", "rev-parse", "HEAD"], root)).trim()
}

function checkoutFor(commitSha: string, fullName = "OpenAgentsInc/worktree-fixture"): GitCheckoutWorkspace {
  return {
    kind: "git_checkout",
    repository: {
      branch: "main",
      commitSha,
      fullName,
      provider: "github",
      visibility: "public",
    },
    verificationCommand: {
      args: ["bun", "test", "sum.test.ts"],
      commandRef: "command.public.autopilot_coder.bun_test_sum",
    },
  }
}

const validBroker = {
  schema: "openagents.pylon.scm_auth_broker.v1",
  kind: "forge_git_access",
  brokerUrl: "https://openagents.com/api/pylon/forge/git-credentials",
  authRefs: ["forge_git_token.background_agent.run_001.receive_pack"],
  repositoryRef: "repo.openagents.openagents",
  allowed: {
    protocol: "https",
    host: "openagents.com",
    pathPrefix: "/git/tenant.openagents.background_agents/repo.openagents.openagents.git",
  },
  cacheTtlSeconds: 60,
  fallback: "fail_closed",
} satisfies NonNullable<GitCheckoutWorkspace["scmAuthBroker"]>

const stubRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
  await mkdir(workingDirectory, { recursive: true })
  await writeFile(join(workingDirectory, "checked-out"), "ok\n")
}

function leaseInput(root: string, overrides: Record<string, unknown> = {}) {
  return {
    cacheRoot: join(root, "adapter-tasks"),
    checkout: checkoutFor("3333333333333333333333333333333333333333"),
    checkoutRunner: stubRunner,
    leaseRef: "lease.public.worktree.test",
    refPrefix: "workspace.pylon.claude_agent_task",
    repositoryCacheRoot: join(root, "git-cache"),
    workspaceStateRoot: join(root, "workspace-leases"),
    ...overrides,
  }
}

describe("repositoryCacheKeyFor", () => {
  test("is stable for the same repository and distinct across repositories", () => {
    expect(repositoryCacheKeyFor("owner/repo")).toBe(repositoryCacheKeyFor("owner/repo"))
    expect(repositoryCacheKeyFor("owner/repo")).not.toBe(repositoryCacheKeyFor("owner/other"))
    expect(repositoryCacheKeyFor("owner/repo")).toMatch(/^[a-f0-9]{24}$/)
  })
})

describe("preparedWorktreeCacheKeyFor", () => {
  test("is stable for one repo+baseline and changes across repos or baselines", () => {
    const base = {
      baselineCommitSha: "1111111111111111111111111111111111111111",
      repositoryFullName: "OpenAgentsInc/openagents",
    }
    expect(preparedWorktreeCacheKeyFor(base)).toBe(preparedWorktreeCacheKeyFor(base))
    expect(preparedWorktreeCacheKeyFor(base)).not.toBe(
      preparedWorktreeCacheKeyFor({
        ...base,
        baselineCommitSha: "2222222222222222222222222222222222222222",
      }),
    )
    expect(preparedWorktreeCacheKeyFor(base)).not.toBe(
      preparedWorktreeCacheKeyFor({
        ...base,
        repositoryFullName: "OpenAgentsInc/other",
      }),
    )
    expect(preparedWorktreeCacheKeyFor(base)).toMatch(/^[a-f0-9]{32}$/)
  })
})

describe("prebuiltBaselineCacheKeyFor", () => {
  test("is stable for one repo+branch and changes across repos or branches", () => {
    const base = {
      branch: "main",
      repositoryFullName: "OpenAgentsInc/openagents",
    }
    expect(prebuiltBaselineCacheKeyFor(base)).toBe(prebuiltBaselineCacheKeyFor(base))
    expect(prebuiltBaselineCacheKeyFor(base)).not.toBe(
      prebuiltBaselineCacheKeyFor({
        ...base,
        branch: "release",
      }),
    )
    expect(prebuiltBaselineCacheKeyFor(base)).not.toBe(
      prebuiltBaselineCacheKeyFor({
        ...base,
        repositoryFullName: "OpenAgentsInc/other",
      }),
    )
    expect(prebuiltBaselineCacheKeyFor(base)).toMatch(/^[a-f0-9]{32}$/)
  })
})

describe("createGitWorktreeCheckoutRunner", () => {
  test("materializes a detached worktree from a shared bare cache keyed by repo-name hash", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const repositoryCacheRoot = join(root, "git-cache")
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => origin.url,
      })
      const checkout = checkoutFor(origin.commitSha)
      const workingDirectory = join(root, "worktrees", "workspace.test.one")
      await runner(workingDirectory, checkout)

      const bareDirectory = join(
        repositoryCacheRoot,
        `${repositoryCacheKeyFor(checkout.repository.fullName)}.git`,
      )
      expect(existsSync(join(bareDirectory, "HEAD"))).toBe(true)
      expect(existsSync(join(workingDirectory, "sum.ts"))).toBe(true)
      // a linked worktree has a .git file pointer, not a .git directory
      expect((await Bun.file(join(workingDirectory, ".git")).text()).startsWith("gitdir:")).toBe(true)
      const head = (await run(["git", "rev-parse", "HEAD"], workingDirectory)).trim()
      expect(head).toBe(origin.commitSha)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("installs brokered helpers in linked worktrees without enabling worktree config", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = {
        ...checkoutFor(origin.commitSha),
        scmAuthBroker: validBroker,
      } satisfies GitCheckoutWorkspace
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: runner,
          leaseRef: "lease.public.worktree.brokered_git",
        }) as never,
      )

      expect(await run(["git", "status", "--porcelain"], materialized.workingDirectory)).toBe("")
      const helper = await run(["git", "config", "--get", "credential.helper"], materialized.workingDirectory)
      const useHttpPath = await run(["git", "config", "--get", "credential.useHttpPath"], materialized.workingDirectory)
      const interactive = await run(["git", "config", "--get", "credential.interactive"], materialized.workingDirectory)
      const paths = await gitCredentialHelperRuntimePathsFor(materialized.workingDirectory)
      const helperConfig = await readFile(paths.gitConfigPath, "utf8")
      const brokerConfig = await readFile(paths.configPath, "utf8")
      const extension = Bun.spawn(["git", "config", "--get", "extensions.worktreeConfig"], {
        cwd: materialized.workingDirectory,
        stderr: "pipe",
        stdout: "pipe",
      })
      const [extensionStdout, extensionExitCode] = await Promise.all([
        new Response(extension.stdout).text(),
        extension.exited,
      ])

      expect(helper.trim()).toContain("pylon-git-credential-helper.mjs")
      expect(useHttpPath.trim()).toBe("true")
      expect(interactive.trim()).toBe("never")
      expect(helperConfig).toContain("\thelper =\n")
      expect(helperConfig).toContain("pylon-git-credential-helper.mjs")
      expect(brokerConfig).toContain("forge_git_token.background_agent.run_001.receive_pack")
      expect(extensionExitCode).not.toBe(0)
      expect(extensionStdout.trim()).toBe("")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("reuses the cached commit without contacting the remote again", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const repositoryCacheRoot = join(root, "git-cache")
      const checkout = checkoutFor(origin.commitSha)
      const warm = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => origin.url,
      })
      await warm(join(root, "worktrees", "first"), checkout)

      const offline = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => `file://${join(root, "no-such-remote")}`,
      })
      const second = join(root, "worktrees", "second")
      await offline(second, checkout)
      expect(existsSync(join(second, "sum.ts"))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("fetches a freshly advanced main commit into an existing bare cache", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const originRoot = join(root, "origin")
      const origin = await createOriginRepo(originRoot)
      const repositoryCacheRoot = join(root, "git-cache")
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => origin.url,
      })

      await runner(join(root, "worktrees", "first"), checkoutFor(origin.commitSha))
      const secondCommit = await commitOriginChange(
        originRoot,
        "fresh-main.ts",
        "export const freshMain = true\n",
      )
      const second = join(root, "worktrees", "second")
      await runner(second, checkoutFor(secondCommit))

      expect(existsSync(join(second, "fresh-main.ts"))).toBe(true)
      expect((await run(["git", "rev-parse", "HEAD"], second)).trim()).toBe(secondCommit)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("materializes from the virtual merge queue base when one is supplied", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const originRoot = join(root, "origin")
      const origin = await createOriginRepo(originRoot)
      const virtualBase = await commitOriginChange(
        originRoot,
        "virtual-head.ts",
        "export const projectedHead = true\n",
      )
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const checkout = {
        ...checkoutFor(origin.commitSha),
        virtualBranch: {
          kind: "pylon_virtual_merge_queue" as const,
          baseCommitSha: virtualBase,
          branchName: "pylon/virtual-issue-6690",
          queueRef: "virtual_merge_queue.openagents.main",
        },
      }
      const workingDirectory = join(root, "worktrees", "virtual-base")

      await runner(workingDirectory, checkout)

      expect(existsSync(join(workingDirectory, "virtual-head.ts"))).toBe(true)
      expect((await run(["git", "rev-parse", "HEAD"], workingDirectory)).trim()).toBe(virtualBase)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("materializes a pinned commit when the requested branch is absent", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"), "master")
      const repositoryCacheRoot = join(root, "git-cache")
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => origin.url,
      })
      const checkout = checkoutFor(origin.commitSha)
      const workingDirectory = join(root, "worktrees", "master-default")

      await runner(workingDirectory, checkout)

      expect(existsSync(join(workingDirectory, "sum.ts"))).toBe(true)
      expect((await run(["git", "rev-parse", "HEAD"], workingDirectory)).trim()).toBe(origin.commitSha)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("rejects a commit object the remote cannot provide", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const missing = checkoutFor("4444444444444444444444444444444444444444")
      await expect(runner(join(root, "worktrees", "missing"), missing)).rejects.toThrow(
        "reason.workspace_checkout.commit_missing_after_fetch",
      )
      expect(existsSync(join(root, "worktrees", "missing", "sum.ts"))).toBe(false)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("two concurrent assignments for the same repository get separate worktrees", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = checkoutFor(origin.commitSha)
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const first = join(root, "worktrees", "concurrent-first")
      const second = join(root, "worktrees", "concurrent-second")
      await Promise.all([runner(first, checkout), runner(second, checkout)])
      expect(existsSync(join(first, "sum.ts"))).toBe(true)
      expect(existsSync(join(second, "sum.ts"))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("waits for a live process lock before mutating the shared bare cache", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const repositoryCacheRoot = join(root, "git-cache")
      const checkout = checkoutFor(origin.commitSha)
      const bareDirectory = join(
        repositoryCacheRoot,
        `${repositoryCacheKeyFor(checkout.repository.fullName)}.git`,
      )
      const lockDirectory = `${bareDirectory}.pylon-lock`
      await mkdir(lockDirectory, { recursive: true })
      const release = setTimeout(() => {
        void rm(lockDirectory, { recursive: true, force: true })
      }, 150)

      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot,
        remoteUrlFor: () => origin.url,
      })
      const startedAt = Date.now()
      try {
        await runner(join(root, "worktrees", "external-lock"), checkout)
      } finally {
        clearTimeout(release)
        await rm(lockDirectory, { recursive: true, force: true })
      }

      expect(Date.now() - startedAt).toBeGreaterThanOrEqual(100)
      expect(existsSync(join(root, "worktrees", "external-lock", "sum.ts"))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("lane change capture, conflict detection, and commits stay scoped to each worktree", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const originRoot = join(root, "origin")
      const origin = await createOriginRepo(originRoot)
      const checkout = checkoutFor(origin.commitSha)
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const cacheRoot = join(root, "adapter-tasks")
      const [first, second] = await Promise.all([
        materializeGitCheckoutWorkspaceWithLease(
          leaseInput(root, {
            cacheRoot,
            checkout,
            checkoutRunner: runner,
            leaseRef: "lease.public.worktree.capture.first",
          }) as never,
        ),
        materializeGitCheckoutWorkspaceWithLease(
          leaseInput(root, {
            cacheRoot,
            checkout,
            checkoutRunner: runner,
            leaseRef: "lease.public.worktree.capture.second",
          }) as never,
        ),
      ])

      await writeFile(join(first.workingDirectory, "sum.ts"), "export const sum = () => 11\n")
      await writeFile(join(first.workingDirectory, "first-only.ts"), "export const firstOnly = true\n")
      await writeFile(join(second.workingDirectory, "sum.ts"), "export const sum = () => 22\n")
      await writeFile(join(second.workingDirectory, "second-only.ts"), "export const secondOnly = true\n")

      const firstStatus = await run(["git", "status", "--porcelain"], first.workingDirectory)
      const secondStatus = await run(["git", "status", "--porcelain"], second.workingDirectory)
      expect(firstStatus).toContain("sum.ts")
      expect(firstStatus).toContain("first-only.ts")
      expect(firstStatus).not.toContain("second-only.ts")
      expect(secondStatus).toContain("sum.ts")
      expect(secondStatus).toContain("second-only.ts")
      expect(secondStatus).not.toContain("first-only.ts")
      expect(await run(["git", "status", "--porcelain"], originRoot)).toBe("")

      const firstCapture = await captureWorkspaceChanges({
        cacheRoot,
        sourceRef: first.sourceRef,
        workingDirectory: first.workingDirectory,
        workspaceRef: first.workspaceRef,
      })
      const secondCapture = await captureWorkspaceChanges({
        cacheRoot,
        sourceRef: second.sourceRef,
        workingDirectory: second.workingDirectory,
        workspaceRef: second.workspaceRef,
      })
      expect(firstCapture.local.changedPaths).toEqual(["first-only.ts", "sum.ts"])
      expect(secondCapture.local.changedPaths).toEqual(["second-only.ts", "sum.ts"])
      expect(firstCapture.fileRefs).toContain(workspaceChangeFileRef(first.sourceRef, "sum.ts"))
      expect(firstCapture.fileRefs).toContain(workspaceChangeFileRef(first.sourceRef, "first-only.ts"))
      expect(secondCapture.fileRefs).toContain(workspaceChangeFileRef(second.sourceRef, "sum.ts"))
      expect(secondCapture.fileRefs).toContain(workspaceChangeFileRef(second.sourceRef, "second-only.ts"))
      const publicCapture = publicWorkspaceChangeCaptureProjection(firstCapture)
      expect("local" in publicCapture).toBe(false)
      assertPublicProjectionSafe(publicCapture)

      await expect(
        stageWorkspacePaths({
          cacheRoot,
          relativePaths: ["../second-only.ts"],
          workingDirectory: first.workingDirectory,
        }),
      ).rejects.toThrow("traversal")
      await expect(
        stageWorkspacePaths({
          cacheRoot,
          relativePaths: ["second-only.ts"],
          workingDirectory: first.workingDirectory,
        }),
      ).rejects.toThrow("not in the lane change set")

      const conflicts = detectWorkspaceChangeConflicts([firstCapture, secondCapture])
      expect(conflicts.state).toBe("conflicted")
      expect(conflicts.conflicts).toEqual([
        {
          conflictRef: conflicts.conflictRefs[0],
          fileRef: workspaceChangeFileRef(first.sourceRef, "sum.ts"),
          sourceRef: first.sourceRef,
          workspaceRefs: [first.workspaceRef, second.workspaceRef].sort(),
        },
      ])

      const virtualBranchConflicts = detectInFlightVirtualBranchConflicts([
        {
          virtualBranchRef: "virtual_branch.pylon.issue_1",
          target: { repositoryFullName: "OpenAgentsInc/worktree-fixture", branch: "main" },
          capture: firstCapture,
        },
        {
          virtualBranchRef: "virtual_branch.pylon.issue_2",
          target: { repositoryFullName: "OpenAgentsInc/worktree-fixture", branch: "main" },
          capture: {
            ...secondCapture,
            sourceRef: `${checkout.repository.fullName}:4444444444444444444444444444444444444444`,
            fileRefs: secondCapture.local.changedPaths.map((path) =>
              workspaceChangeFileRef(
                `${checkout.repository.fullName}:4444444444444444444444444444444444444444`,
                path,
              ),
            ),
          },
        },
      ])
      expect(virtualBranchConflicts.state).toBe("conflicted")
      expect(virtualBranchConflicts.conflicts).toEqual([
        {
          conflictRef: virtualBranchConflicts.conflictRefs[0],
          fileRef: virtualBranchChangeFileRef({
            repositoryFullName: "OpenAgentsInc/worktree-fixture",
            targetBranch: "main",
            relativePath: "sum.ts",
          }),
          repositoryFullName: "OpenAgentsInc/worktree-fixture",
          targetBranch: "main",
          sourceRefs: [
            `${checkout.repository.fullName}:4444444444444444444444444444444444444444`,
            first.sourceRef,
          ].sort(),
          virtualBranchRefs: ["virtual_branch.pylon.issue_1", "virtual_branch.pylon.issue_2"],
          workspaceRefs: [first.workspaceRef, second.workspaceRef].sort(),
        },
      ])
      assertPublicProjectionSafe(virtualBranchConflicts)

      const independentVirtualBranches = detectInFlightVirtualBranchConflicts([
        {
          virtualBranchRef: "virtual_branch.pylon.issue_1",
          target: { repositoryFullName: "OpenAgentsInc/worktree-fixture", branch: "main" },
          capture: firstCapture,
        },
        {
          virtualBranchRef: "virtual_branch.pylon.issue_3",
          target: { repositoryFullName: "OpenAgentsInc/worktree-fixture", branch: "release" },
          capture: secondCapture,
        },
      ])
      expect(independentVirtualBranches).toEqual({ state: "clear", conflictRefs: [], conflicts: [] })

      const firstCommit = await commitWorkspaceChanges({
        cacheRoot,
        message: "lane first scoped commit",
        sourceRef: first.sourceRef,
        workingDirectory: first.workingDirectory,
        workspaceRef: first.workspaceRef,
      })
      const secondCommit = await commitWorkspaceChanges({
        cacheRoot,
        message: "lane second scoped commit",
        sourceRef: second.sourceRef,
        workingDirectory: second.workingDirectory,
        workspaceRef: second.workspaceRef,
      })
      expect(firstCommit.state).toBe("committed")
      expect(secondCommit.state).toBe("committed")
      if (firstCommit.state !== "committed" || secondCommit.state !== "committed") {
        throw new Error("expected scoped commits")
      }
      const firstCommitFiles = await run(["git", "show", "--name-only", "--format=", firstCommit.commitSha], first.workingDirectory)
      const secondCommitFiles = await run(["git", "show", "--name-only", "--format=", secondCommit.commitSha], second.workingDirectory)
      expect(firstCommitFiles).toContain("sum.ts")
      expect(firstCommitFiles).toContain("first-only.ts")
      expect(firstCommitFiles).not.toContain("second-only.ts")
      expect(secondCommitFiles).toContain("sum.ts")
      expect(secondCommitFiles).toContain("second-only.ts")
      expect(secondCommitFiles).not.toContain("first-only.ts")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})

describe("materializeGitCheckoutWorkspaceWithLease", () => {
  test("prunes oldest workspace cache directories while preserving protected refs", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-prune-"))
    try {
      const cacheRoot = join(root, "adapter-tasks")
      const oldRef = "workspace.pylon.codex_agent_task.old"
      const protectedRef = "workspace.pylon.codex_agent_task.protected"
      const freshRef = "workspace.pylon.codex_agent_task.fresh"
      await mkdir(join(cacheRoot, oldRef), { recursive: true })
      await mkdir(join(cacheRoot, protectedRef), { recursive: true })
      await mkdir(join(cacheRoot, freshRef), { recursive: true })
      await utimes(join(cacheRoot, oldRef), new Date("2026-06-11T00:00:00.000Z"), new Date("2026-06-11T00:00:00.000Z"))
      await utimes(join(cacheRoot, protectedRef), new Date("2026-06-11T01:00:00.000Z"), new Date("2026-06-11T01:00:00.000Z"))
      await utimes(join(cacheRoot, freshRef), new Date("2026-06-11T02:00:00.000Z"), new Date("2026-06-11T02:00:00.000Z"))

      const pruned = await pruneWorkspaceCacheDirectories({
        cacheRoot,
        maxEntries: 1,
        protectedWorkspaceRefs: [protectedRef],
      })

      expect(pruned.removedWorkspaceRefs).toEqual([oldRef])
      expect(existsSync(join(cacheRoot, oldRef))).toBe(false)
      expect(existsSync(join(cacheRoot, protectedRef))).toBe(true)
      expect(existsSync(join(cacheRoot, freshRef))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("records a workspace lease with state, TTL, retention, and refs", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const now = new Date("2026-06-11T12:00:00.000Z")
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, { now, ttlSeconds: 600 }) as never,
      )
      const record = await workspaceLeaseRecordFor({
        workspaceStateRoot: join(root, "workspace-leases"),
        workspaceRef: materialized.workspaceRef,
      })
      expect(record).not.toBeNull()
      expect(record?.schema).toBe(WORKSPACE_LEASE_SCHEMA)
      expect(record?.state).toBe("materialized")
      expect(record?.strategy).toBe("injected")
      expect(record?.ttlSeconds).toBe(600)
      expect(record?.retentionPolicy).toBe("retain_until_ttl")
      expect(record?.materializedAt).toBe(now.toISOString())
      expect(record?.generatedAt).toBe(now.toISOString())
      expect(record?.cleanupRef).toBe(materialized.cleanupRef)
      expect(record?.local.workingDirectory).toBe(materialized.workingDirectory)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("scoped materialization releases the workspace when interrupted", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-scoped-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const materialized = await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const acquired = yield* Deferred.make<never, Awaited<ReturnType<typeof materializeGitCheckoutWorkspaceWithLease>>>()
            yield* scopedMaterializedGitCheckoutWorkspace(
              leaseInput(root, {
                leaseRef: "lease.public.worktree.scoped.interrupt",
                retentionPolicy: "remove_on_closeout",
              }) as never,
            ).pipe(
              Effect.tap((workspace) => Deferred.succeed(acquired, workspace)),
              Effect.andThen(Effect.never),
              Effect.forkScoped,
            )
            return yield* Deferred.await(acquired)
          }),
        ),
      )

      expect(existsSync(materialized.workingDirectory)).toBe(false)
      const record = await workspaceLeaseRecordFor({
        workspaceStateRoot,
        workspaceRef: materialized.workspaceRef,
      })
      expect(record?.state).toBe("cleaned")
      expect(record?.cleanupReceiptRef?.startsWith("receipt.pylon.workspace_cleanup.")).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("scoped materialization reports acquisition failures through Exit", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-scoped-"))
    try {
      const exit = await Effect.runPromiseExit(
        Effect.scoped(
          scopedMaterializedGitCheckoutWorkspace(
            leaseInput(root, {
              checkoutRunner: async () => {
                throw new Error("fixture checkout failed")
              },
            }) as never,
          ),
        ),
      )
      expect(Exit.isFailure(exit)).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("uses the git_worktree strategy by default and records the cache directory", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: checkoutFor(origin.commitSha),
          checkoutRunner: undefined,
          remoteUrlFor: () => origin.url,
        }) as never,
      )
      expect(existsSync(join(materialized.workingDirectory, "sum.ts"))).toBe(true)
      const record = await workspaceLeaseRecordFor({
        workspaceStateRoot: join(root, "workspace-leases"),
        workspaceRef: materialized.workspaceRef,
      })
      expect(record?.strategy).toBe("git_worktree")
      expect(record?.local.repositoryCacheDirectory).toBe(
        join(root, "git-cache", `${repositoryCacheKeyFor("OpenAgentsInc/worktree-fixture")}.git`),
      )
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("snapshots clean closeouts and restores matching repo+baseline from the prepared cache", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-prepared-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = checkoutFor(origin.commitSha)
      const preparedWorktreeCacheRoot = join(root, "prepared-cache")
      const workspaceStateRoot = join(root, "workspace-leases")
      const first = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.first",
          now: new Date("2026-07-03T13:00:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )

      await releaseWorkspace({
        now: new Date("2026-07-03T13:01:00.000Z"),
        workspaceRef: first.workspaceRef,
        workspaceStateRoot,
      })

      const cacheKey = preparedWorktreeCacheKeyFor({
        baselineCommitSha: origin.commitSha,
        repositoryFullName: checkout.repository.fullName,
      })
      const preparedDirectory = join(preparedWorktreeCacheRoot, `prepared.${cacheKey}`)
      const snapshotRecord = JSON.parse(await readFile(`${preparedDirectory}.json`, "utf8"))
      expect(snapshotRecord.schema).toBe(WORKSPACE_PREPARED_WORKTREE_CACHE_SCHEMA)
      expect(snapshotRecord.cacheKey).toBe(cacheKey)
      expect(snapshotRecord.reuseReason).toBe("post_completion_snapshot")
      expect(snapshotRecord.sourceRef).toBe(`${checkout.repository.fullName}:${origin.commitSha}`)
      expect(existsSync(first.workingDirectory)).toBe(false)

      await rm(join(root, "origin"), { recursive: true, force: true })
      const restored = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.second",
          now: new Date("2026-07-03T13:02:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )
      const restoredHead = (await run(["git", "rev-parse", "HEAD"], restored.workingDirectory)).trim()
      const restoredStatus = await run(["git", "status", "--porcelain"], restored.workingDirectory)
      const restoredRecord = await workspaceLeaseRecordFor({
        workspaceRef: restored.workspaceRef,
        workspaceStateRoot,
      })
      const updatedCacheRecord = JSON.parse(await readFile(`${preparedDirectory}.json`, "utf8"))

      expect(restored.preparedWorktreeCache).toMatchObject({
        cacheKey,
        reuseReason: "restore_quick_sync_reset",
        state: "hit",
      })
      expect(restoredHead).toBe(origin.commitSha)
      expect(restoredStatus).toBe("")
      expect(restoredRecord?.local.repositoryCacheDirectory).toBeUndefined()
      expect(restoredRecord?.local.preparedWorktreeCache?.restore?.reuseReason).toBe("restore_quick_sync_reset")
      expect(updatedCacheRecord.reuseReason).toBe("restore_quick_sync_reset")
      expect(updatedCacheRecord.useCount).toBe(1)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("evicts an invalid prepared cache entry and falls back to normal materialization", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-prepared-invalid-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = checkoutFor(origin.commitSha)
      const preparedWorktreeCacheRoot = join(root, "prepared-cache")
      const workspaceStateRoot = join(root, "workspace-leases")
      const first = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.invalid.first",
          now: new Date("2026-07-03T13:10:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )
      await releaseWorkspace({
        now: new Date("2026-07-03T13:11:00.000Z"),
        workspaceRef: first.workspaceRef,
        workspaceStateRoot,
      })

      const cacheKey = preparedWorktreeCacheKeyFor({
        baselineCommitSha: origin.commitSha,
        repositoryFullName: checkout.repository.fullName,
      })
      const preparedDirectory = join(preparedWorktreeCacheRoot, `prepared.${cacheKey}`)
      await writeFile(join(preparedDirectory, "dirty.ts"), "export const dirty = true\n")

      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.invalid.second",
          now: new Date("2026-07-03T13:12:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
        }) as never,
      )

      expect(materialized.preparedWorktreeCache).toBeUndefined()
      expect(existsSync(join(materialized.workingDirectory, "sum.ts"))).toBe(true)
      expect(existsSync(preparedDirectory)).toBe(false)
      expect(existsSync(`${preparedDirectory}.json`)).toBe(false)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("enforces a byte budget by evicting the oldest prepared worktree entry", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-prepared-budget-"))
    try {
      const originRoot = join(root, "origin")
      const origin = await createOriginRepo(originRoot)
      const preparedWorktreeCacheRoot = join(root, "prepared-cache")
      const workspaceStateRoot = join(root, "workspace-leases")
      const firstCheckout = checkoutFor(origin.commitSha)
      const first = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: firstCheckout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.budget.first",
          now: new Date("2026-07-03T13:20:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )
      await releaseWorkspace({
        now: new Date("2026-07-03T13:21:00.000Z"),
        workspaceRef: first.workspaceRef,
        workspaceStateRoot,
      })

      const secondCommitSha = await commitOriginChange(
        originRoot,
        "sum.ts",
        "export const sum = (left: number, right: number) => left + right\n",
      )
      const secondCheckout = checkoutFor(secondCommitSha)
      const second = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: secondCheckout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prepared.budget.second",
          now: new Date("2026-07-03T13:22:00.000Z"),
          preparedWorktreeCacheRoot,
          remoteUrlFor: () => origin.url,
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )
      await releaseWorkspace({
        now: new Date("2026-07-03T13:23:00.000Z"),
        workspaceRef: second.workspaceRef,
        workspaceStateRoot,
      })

      const firstCacheKey = preparedWorktreeCacheKeyFor({
        baselineCommitSha: origin.commitSha,
        repositoryFullName: firstCheckout.repository.fullName,
      })
      const secondCacheKey = preparedWorktreeCacheKeyFor({
        baselineCommitSha: secondCommitSha,
        repositoryFullName: secondCheckout.repository.fullName,
      })
      const firstPreparedDirectory = join(preparedWorktreeCacheRoot, `prepared.${firstCacheKey}`)
      const secondPreparedDirectory = join(preparedWorktreeCacheRoot, `prepared.${secondCacheKey}`)
      const secondRecord = JSON.parse(await readFile(`${secondPreparedDirectory}.json`, "utf8"))
      const evicted = await enforcePreparedWorktreeCacheBudget({
        diskBudgetBytes: secondRecord.sizeBytes,
        preparedWorktreeCacheRoot,
      })

      expect(evicted.removedCacheKeys).toEqual([firstCacheKey])
      expect(existsSync(firstPreparedDirectory)).toBe(false)
      expect(existsSync(secondPreparedDirectory)).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("prebuilds the latest upstream baseline with setup artifacts and reuses it within the refresh cadence", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-prebuilt-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = checkoutFor(origin.commitSha)
      const prebuiltBaselineCacheRoot = join(root, "prebuilt-cache")
      const setupRunner = async (input: { checkout: GitCheckoutWorkspace; workingDirectory: string }) => {
        await mkdir(join(input.workingDirectory, "node_modules"), { recursive: true })
        await writeFile(join(input.workingDirectory, "node_modules", ".prebuilt-ready"), `${input.checkout.repository.commitSha}\n`)
        return {
          state: "completed" as const,
          setupRef: "setup.public.fixture.prebuilt_baseline",
          commandRef: "command.public.fixture.prebuilt_baseline_setup",
        }
      }

      const first = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prebuilt.first",
          now: new Date("2026-07-03T14:00:00.000Z"),
          prebuiltBaselineCacheRoot,
          prebuiltBaselineSetupRunner: setupRunner,
          remoteUrlFor: () => origin.url,
        }) as never,
      )

      const cacheKey = prebuiltBaselineCacheKeyFor({
        branch: checkout.repository.branch,
        repositoryFullName: checkout.repository.fullName,
      })
      const prebuiltDirectory = join(prebuiltBaselineCacheRoot, `prebuilt.${cacheKey}`)
      const registry = JSON.parse(await readFile(`${prebuiltDirectory}.json`, "utf8"))

      expect(first.prebuiltBaselineCache).toMatchObject({
        baselineCommitSha: origin.commitSha,
        cacheKey,
        hitCount: 1,
        missCount: 0,
        reasonRef: "reason.workspace_prebuilt_baseline.hit",
        state: "hit",
      })
      expect(registry.schema).toBe(WORKSPACE_PREBUILT_BASELINE_CACHE_SCHEMA)
      expect(registry.baselineCommitSha).toBe(origin.commitSha)
      expect(registry.hitCount).toBe(1)
      expect(registry.missCount).toBe(0)
      expect(registry.setup.state).toBe("completed")
      expect(await readFile(join(first.workingDirectory, "node_modules", ".prebuilt-ready"), "utf8")).toBe(`${origin.commitSha}\n`)

      await rm(join(root, "origin"), { recursive: true, force: true })
      const second = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prebuilt.second",
          now: new Date("2026-07-03T14:05:00.000Z"),
          prebuiltBaselineCacheRoot,
          prebuiltBaselineSetupRunner: setupRunner,
          remoteUrlFor: () => origin.url,
        }) as never,
      )

      expect(second.prebuiltBaselineCache).toMatchObject({
        baselineCommitSha: origin.commitSha,
        hitCount: 2,
        missCount: 0,
        state: "hit",
      })
      expect(await readFile(join(second.workingDirectory, "node_modules", ".prebuilt-ready"), "utf8")).toBe(`${origin.commitSha}\n`)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("records a prebuilt miss before cadence refresh, then refreshes to the newest upstream baseline", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-prebuilt-refresh-"))
    try {
      const originRoot = join(root, "origin")
      const origin = await createOriginRepo(originRoot)
      const prebuiltBaselineCacheRoot = join(root, "prebuilt-cache")
      const setupRunner = async (input: { checkout: GitCheckoutWorkspace; workingDirectory: string }) => {
        await mkdir(join(input.workingDirectory, "node_modules"), { recursive: true })
        await writeFile(join(input.workingDirectory, "node_modules", ".prebuilt-ready"), `${input.checkout.repository.commitSha}\n`)
        return {
          state: "completed" as const,
          setupRef: "setup.public.fixture.prebuilt_baseline",
          commandRef: "command.public.fixture.prebuilt_baseline_setup",
        }
      }

      await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: checkoutFor(origin.commitSha),
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prebuilt.refresh.first",
          now: new Date("2026-07-03T15:00:00.000Z"),
          prebuiltBaselineCacheRoot,
          prebuiltBaselineRefreshCadenceSeconds: 3600,
          prebuiltBaselineSetupRunner: setupRunner,
          remoteUrlFor: () => origin.url,
        }) as never,
      )

      const secondCommitSha = await commitOriginChange(
        originRoot,
        "fresh-main.ts",
        "export const freshMain = true\n",
      )
      const secondCheckout = checkoutFor(secondCommitSha)
      const beforeCadence = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: secondCheckout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prebuilt.refresh.before_cadence",
          now: new Date("2026-07-03T15:05:00.000Z"),
          prebuiltBaselineCacheRoot,
          prebuiltBaselineRefreshCadenceSeconds: 3600,
          prebuiltBaselineSetupRunner: setupRunner,
          remoteUrlFor: () => origin.url,
        }) as never,
      )

      const cacheKey = prebuiltBaselineCacheKeyFor({
        branch: secondCheckout.repository.branch,
        repositoryFullName: secondCheckout.repository.fullName,
      })
      const prebuiltDirectory = join(prebuiltBaselineCacheRoot, `prebuilt.${cacheKey}`)
      const missRegistry = JSON.parse(await readFile(`${prebuiltDirectory}.json`, "utf8"))
      expect(beforeCadence.prebuiltBaselineCache).toMatchObject({
        baselineCommitSha: origin.commitSha,
        missCount: 1,
        reasonRef: "reason.workspace_prebuilt_baseline.requested_commit_not_prebuilt",
        state: "miss",
      })
      expect((await run(["git", "rev-parse", "HEAD"], beforeCadence.workingDirectory)).trim()).toBe(secondCommitSha)
      expect(missRegistry.baselineCommitSha).toBe(origin.commitSha)
      expect(missRegistry.missCount).toBe(1)

      const afterCadence = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: secondCheckout,
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.prebuilt.refresh.after_cadence",
          now: new Date("2026-07-03T16:10:00.000Z"),
          prebuiltBaselineCacheRoot,
          prebuiltBaselineRefreshCadenceSeconds: 3600,
          prebuiltBaselineSetupRunner: setupRunner,
          remoteUrlFor: () => origin.url,
        }) as never,
      )
      const refreshedRegistry = JSON.parse(await readFile(`${prebuiltDirectory}.json`, "utf8"))

      expect(afterCadence.prebuiltBaselineCache).toMatchObject({
        baselineCommitSha: secondCommitSha,
        hitCount: 2,
        missCount: 1,
        state: "hit",
      })
      expect(refreshedRegistry.baselineCommitSha).toBe(secondCommitSha)
      expect(refreshedRegistry.missCount).toBe(1)
      expect(await readFile(join(afterCadence.workingDirectory, "node_modules", ".prebuilt-ready"), "utf8")).toBe(`${secondCommitSha}\n`)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("serializes parallel bare-cache materialization for the same repository", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const [first, second] = await Promise.all([
        materializeGitCheckoutWorkspaceWithLease(
          leaseInput(root, {
            checkout: checkoutFor(origin.commitSha),
            checkoutRunner: undefined,
            leaseRef: "lease.public.worktree.parallel.first",
            remoteUrlFor: () => origin.url,
          }) as never,
        ),
        materializeGitCheckoutWorkspaceWithLease(
          leaseInput(root, {
            checkout: checkoutFor(origin.commitSha),
            checkoutRunner: undefined,
            leaseRef: "lease.public.worktree.parallel.second",
            remoteUrlFor: () => origin.url,
          }) as never,
        ),
      ])

      expect(first.workspaceRef).not.toBe(second.workspaceRef)
      expect(first.workingDirectory).not.toBe(second.workingDirectory)
      expect(existsSync(join(first.workingDirectory, "sum.ts"))).toBe(true)
      expect(existsSync(join(second.workingDirectory, "sum.ts"))).toBe(true)
      const bareDirectory = join(
        root,
        "git-cache",
        `${repositoryCacheKeyFor("OpenAgentsInc/worktree-fixture")}.git`,
      )
      expect(existsSync(join(bareDirectory, "HEAD"))).toBe(true)
      expect(
        await workspaceLeaseRecordFor({
          workspaceStateRoot: join(root, "workspace-leases"),
          workspaceRef: first.workspaceRef,
        }),
      ).toMatchObject({ strategy: "git_worktree", state: "materialized" })
      expect(
        await workspaceLeaseRecordFor({
          workspaceStateRoot: join(root, "workspace-leases"),
          workspaceRef: second.workspaceRef,
        }),
      ).toMatchObject({ strategy: "git_worktree", state: "materialized" })
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("many concurrent same-repo materializations all succeed with isolated worktrees (regression #6434)", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-concurrent-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const checkout = checkoutFor(origin.commitSha)
      const cacheRoot = join(root, "adapter-tasks")
      const workspaceStateRoot = join(root, "workspace-leases")
      const repositoryCacheRoot = join(root, "git-cache")
      const concurrency = 12

      const settled = await Promise.allSettled(
        Array.from({ length: concurrency }, (_value, index) =>
          materializeGitCheckoutWorkspaceWithLease({
            cacheRoot,
            checkout,
            // undefined runner exercises the real git_worktree shared-bare-cache strategy
            leaseRef: `lease.public.worktree.concurrent.${index}`,
            refPrefix: "workspace.pylon.codex_agent_task",
            repositoryCacheRoot,
            workspaceStateRoot,
            remoteUrlFor: () => origin.url,
          }),
        ),
      )

      const failures = settled.filter((result) => result.status === "rejected")
      // the whole point of #6434: zero workspace_checkout_failed under concurrency
      expect(failures).toEqual([])

      const dirs = new Set<string>()
      for (const result of settled) {
        if (result.status !== "fulfilled") continue
        const materialized = result.value
        dirs.add(materialized.workingDirectory)
        expect(existsSync(join(materialized.workingDirectory, "sum.ts"))).toBe(true)
        expect((await run(["git", "rev-parse", "HEAD"], materialized.workingDirectory)).trim()).toBe(
          origin.commitSha,
        )
      }
      // every assignment got its own isolated working tree
      expect(dirs.size).toBe(concurrency)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("does not treat a stale lock mtime as abandoned while the owner process is live", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-live-lock-"))
    try {
      const lockDirectory = join(root, "repo.git.pylon-lock")
      await mkdir(lockDirectory, { recursive: true })
      await writeFile(
        join(lockDirectory, "owner.json"),
        `${JSON.stringify({ pid: process.pid, acquiredAt: new Date(0).toISOString() }, null, 2)}\n`,
      )
      const old = new Date(Date.now() - 5 * 60 * 1000)
      await utimes(lockDirectory, old, old)

      await expect(repositoryCacheProcessLockOwnerIsLive(lockDirectory)).resolves.toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("TTL expiry produces a cleanup receipt and removes only the expired workspace", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const t0 = new Date("2026-06-11T12:00:00.000Z")
      const expired = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, { now: t0, ttlSeconds: 60, leaseRef: "lease.public.worktree.expired" }) as never,
      )
      const fresh = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, { now: t0, ttlSeconds: 3600, leaseRef: "lease.public.worktree.fresh" }) as never,
      )

      const early = await cleanupExpiredWorkspaces({
        workspaceStateRoot,
        now: new Date(t0.getTime() + 30_000),
      })
      expect(early.cleanupReceiptRefs).toEqual([])
      expect(existsSync(expired.workingDirectory)).toBe(true)

      const due = await cleanupExpiredWorkspaces({
        workspaceStateRoot,
        now: new Date(t0.getTime() + 61_000),
      })
      expect(due.cleanupReceiptRefs.length).toBe(1)
      expect(due.cleanupReceiptRefs[0]?.startsWith("receipt.pylon.workspace_cleanup.")).toBe(true)
      expect(existsSync(expired.workingDirectory)).toBe(false)
      expect(existsSync(fresh.workingDirectory)).toBe(true)

      const cleanedRecord = await workspaceLeaseRecordFor({
        workspaceStateRoot,
        workspaceRef: expired.workspaceRef,
      })
      expect(cleanedRecord?.state).toBe("cleaned")
      expect(cleanedRecord?.cleanupReceiptRef).toBe(due.cleanupReceiptRefs[0])

      const again = await cleanupExpiredWorkspaces({
        workspaceStateRoot,
        now: new Date(t0.getTime() + 120_000),
      })
      expect(again.cleanupReceiptRefs).toEqual([])
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("cleanup retains dirty workspaces while deleting expired clean lanes", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const workspaceStateRoot = join(root, "workspace-leases")
      const t0 = new Date("2026-06-11T12:00:00.000Z")
      const dirty = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: checkoutFor(origin.commitSha),
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.dirty",
          now: t0,
          remoteUrlFor: () => origin.url,
          ttlSeconds: 60,
        }) as never,
      )
      const clean = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          checkout: checkoutFor(origin.commitSha),
          checkoutRunner: undefined,
          leaseRef: "lease.public.worktree.clean",
          now: t0,
          remoteUrlFor: () => origin.url,
          ttlSeconds: 60,
        }) as never,
      )
      await writeFile(join(dirty.workingDirectory, "dirty-only.ts"), "export const dirtyOnly = true\n")

      const swept = await cleanupExpiredWorkspaces({
        workspaceStateRoot,
        now: new Date(t0.getTime() + 61_000),
      })
      expect(swept.retainedWorkspaceRefs).toEqual([dirty.workspaceRef])
      expect(swept.cleanupReceiptRefs.length).toBe(1)
      expect(existsSync(dirty.workingDirectory)).toBe(true)
      expect(existsSync(clean.workingDirectory)).toBe(false)

      const dirtyRecord = await workspaceLeaseRecordFor({ workspaceStateRoot, workspaceRef: dirty.workspaceRef })
      expect(dirtyRecord?.state).toBe("materialized")
      expect(dirtyRecord?.retentionReasonRef).toBe("retention.workspace.dirty")
      const projection = publicWorkspaceLeaseProjection(dirtyRecord as WorkspaceLeaseRecord)
      expect(projection.retentionReasonRef).toBe("retention.workspace.dirty")
      assertPublicProjectionSafe(projection)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("cleanup removes dirty workspaces that contain long-lived SCM credentials", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          leaseRef: "lease.public.worktree.scm_cleanup",
          now: new Date("2026-07-03T12:00:00.000Z"),
          retentionPolicy: "remove_on_closeout",
        }) as never,
      )
      await writeFile(join(materialized.workingDirectory, "dirty.ts"), "export const dirty = true\n")
      await writeFile(
        join(materialized.workingDirectory, ".git-credentials"),
        "https://x-access-token:github_pat_abcdefghijklmnopqrstuvwxyz1234567890@github.com/OpenAgentsInc/openagents.git\n",
      )

      const released = await releaseWorkspace({
        now: new Date("2026-07-03T12:05:00.000Z"),
        workspaceRef: materialized.workspaceRef,
        workspaceStateRoot,
      })
      const record = await workspaceLeaseRecordFor({
        workspaceRef: materialized.workspaceRef,
        workspaceStateRoot,
      })

      expect(released?.cleanupReceiptRef).toStartWith("receipt.pylon.workspace_cleanup.")
      expect(record?.state).toBe("cleaned")
      expect(existsSync(materialized.workingDirectory)).toBe(false)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("each materialization sweeps expired workspaces opportunistically", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const t0 = new Date("2026-06-11T12:00:00.000Z")
      const old = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, { now: t0, ttlSeconds: 60, leaseRef: "lease.public.worktree.old" }) as never,
      )
      await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, {
          now: new Date(t0.getTime() + 7_200_000),
          leaseRef: "lease.public.worktree.new",
        }) as never,
      )
      expect(existsSync(old.workingDirectory)).toBe(false)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("releaseWorkspace removes one workspace on closeout and mints its receipt", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root, { retentionPolicy: "remove_on_closeout" }) as never,
      )
      const released = await releaseWorkspace({
        workspaceStateRoot,
        workspaceRef: materialized.workspaceRef,
      })
      expect(released?.cleanupReceiptRef.startsWith("receipt.pylon.workspace_cleanup.")).toBe(true)
      expect(existsSync(materialized.workingDirectory)).toBe(false)
      expect(
        await releaseWorkspace({ workspaceStateRoot, workspaceRef: materialized.workspaceRef }),
      ).toBeNull()
      expect(
        await releaseWorkspace({ workspaceStateRoot, workspaceRef: "workspace.unknown" }),
      ).toBeNull()
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("cleanup never acts on a lease record pointing outside the cache root", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    const outside = await mkdtemp(join(tmpdir(), "pylon-worktree-outside-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      await mkdir(workspaceStateRoot, { recursive: true })
      const tampered: WorkspaceLeaseRecord = {
        schema: WORKSPACE_LEASE_SCHEMA,
        workspaceRef: "workspace.pylon.claude_agent_task.tampered",
        cleanupRef: "cleanup.pylon.workspace.tampered",
        sourceRef: "OpenAgentsInc/worktree-fixture:3333333333333333333333333333333333333333",
        strategy: "injected",
        state: "materialized",
        materializedAt: "2026-06-11T00:00:00.000Z",
        ttlSeconds: 1,
        retentionPolicy: "retain_until_ttl",
        generatedAt: "2026-06-11T00:00:00.000Z",
        local: { cacheRoot: join(root, "adapter-tasks"), workingDirectory: outside },
      }
      await writeFile(
        join(workspaceStateRoot, `${tampered.workspaceRef}.json`),
        JSON.stringify(tampered),
      )
      const swept = await cleanupExpiredWorkspaces({
        workspaceStateRoot,
        now: new Date("2026-06-12T00:00:00.000Z"),
      })
      expect(swept.cleanupReceiptRefs).toEqual([])
      expect(existsSync(outside)).toBe(true)
      const record = await workspaceLeaseRecordFor({
        workspaceStateRoot,
        workspaceRef: tampered.workspaceRef,
      })
      expect(record?.state).toBe("materialized")
    } finally {
      await rm(root, { recursive: true, force: true })
      await rm(outside, { recursive: true, force: true })
    }
  })

  test("the public lease projection carries refs and freshness but no local paths", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-lease-"))
    try {
      const materialized = await materializeGitCheckoutWorkspaceWithLease(
        leaseInput(root) as never,
      )
      const record = await workspaceLeaseRecordFor({
        workspaceStateRoot: join(root, "workspace-leases"),
        workspaceRef: materialized.workspaceRef,
      })
      const projection = publicWorkspaceLeaseProjection(record as WorkspaceLeaseRecord)
      expect("local" in projection).toBe(false)
      expect(JSON.stringify(projection).includes(root)).toBe(false)
      expect(projection.generatedAt).toBe(record?.generatedAt as string)
      assertPublicProjectionSafe(projection)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})

describe("withWorkspaceMaterializerCapability", () => {
  test("declares both workspace capabilities when a local coding lane is present", () => {
    const declared = withWorkspaceMaterializerCapability([CLAUDE_AGENT_CAPABILITY_REF])
    expect(declared).toContain(WORKSPACE_MATERIALIZER_CAPABILITY_REF)
    expect(declared).toContain(WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF)
    const codexOnly = withWorkspaceMaterializerCapability([CODEX_AGENT_CAPABILITY_REF])
    expect(codexOnly).toContain(WORKSPACE_MATERIALIZER_CAPABILITY_REF)
  })

  test("strips stale workspace capabilities when no coding lane remains", () => {
    const stripped = withWorkspaceMaterializerCapability([
      WORKSPACE_MATERIALIZER_CAPABILITY_REF,
      WORKSPACE_CLEANUP_RECEIPTS_CAPABILITY_REF,
      "capability.pylon.nip90_provider",
    ])
    expect(stripped).toEqual(["capability.pylon.nip90_provider"])
  })

  test("is idempotent", () => {
    const once = withWorkspaceMaterializerCapability([CLAUDE_AGENT_CAPABILITY_REF])
    expect(withWorkspaceMaterializerCapability(once).sort()).toEqual([...once].sort())
  })
})
