import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  captureWorkspaceChanges,
  cleanupExpiredWorkspaces,
  commitWorkspaceChanges,
  createGitWorktreeCheckoutRunner,
  detectWorkspaceChangeConflicts,
  materializeGitCheckoutWorkspaceWithLease,
  publicWorkspaceChangeCaptureProjection,
  publicWorkspaceLeaseProjection,
  releaseWorkspace,
  repositoryCacheKeyFor,
  stageWorkspacePaths,
  withWorkspaceMaterializerCapability,
  workspaceChangeFileRef,
  workspaceLeaseRecordFor,
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

async function createOriginRepo(root: string): Promise<{ url: string; commitSha: string }> {
  await mkdir(root, { recursive: true })
  await run(["git", "init"], root)
  await run(["git", "config", "user.email", "fixture@test.local"], root)
  await run(["git", "config", "user.name", "Fixture"], root)
  // local transports refuse unadvertised-object fetches by default; the
  // production path fetches by SHA from GitHub, which allows them
  await run(["git", "config", "uploadpack.allowAnySHA1InWant", "true"], root)
  await writeFile(
    join(root, "package.json"),
    `${JSON.stringify({ private: true, scripts: { test: "bun test sum.test.ts" }, type: "module" }, null, 2)}\n`,
  )
  await writeFile(join(root, "sum.ts"), "export const sum = (left: number, right: number) => left - right\n")
  await run(["git", "add", "."], root)
  await run(["git", "commit", "-m", "fixture"], root)
  const commitSha = (await run(["git", "rev-parse", "HEAD"], root)).trim()
  return { url: `file://${root}`, commitSha }
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

  test("rejects a commit object the remote cannot provide", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-worktree-"))
    try {
      const origin = await createOriginRepo(join(root, "origin"))
      const runner = createGitWorktreeCheckoutRunner({
        repositoryCacheRoot: join(root, "git-cache"),
        remoteUrlFor: () => origin.url,
      })
      const missing = checkoutFor("4444444444444444444444444444444444444444")
      await expect(runner(join(root, "worktrees", "missing"), missing)).rejects.toThrow()
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
