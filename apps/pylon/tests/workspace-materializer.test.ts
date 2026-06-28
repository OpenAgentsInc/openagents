import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  cleanupOldestMaterializedWorkspaces,
  gitCheckoutWorkspaceFrom,
  materializeGitCheckoutWorkspace,
  materializeGitCheckoutWorkspaceWithLease,
  removeMaterializedWorkspace,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
  workspaceLeaseRecordFor,
} from "../src/workspace-materializer"

const validCheckout: GitCheckoutWorkspace = {
  kind: "git_checkout",
  repository: {
    branch: "main",
    commitSha: "3333333333333333333333333333333333333333",
    fullName: "OpenAgentsInc/public-sum-fixture",
    provider: "github",
    visibility: "public",
  },
  verificationCommand: {
    args: ["bun", "test", "sum.test.ts"],
    commandRef: "command.public.autopilot_coder.bun_test_sum",
  },
}

function assignmentWith(workspace: unknown) {
  return { workspace }
}

function checkoutWith(overrides: {
  repository?: Partial<GitCheckoutWorkspace["repository"]>
  verificationCommand?: Partial<GitCheckoutWorkspace["verificationCommand"]>
  kind?: string
}) {
  return {
    ...validCheckout,
    ...(overrides.kind === undefined ? {} : { kind: overrides.kind }),
    repository: { ...validCheckout.repository, ...overrides.repository },
    verificationCommand: { ...validCheckout.verificationCommand, ...overrides.verificationCommand },
  }
}

describe("gitCheckoutWorkspaceFrom", () => {
  test("accepts a public GitHub checkout pinned to a 40-character commit", () => {
    const decoded = gitCheckoutWorkspaceFrom(assignmentWith(validCheckout))
    expect(decoded).not.toBeNull()
    expect(decoded?.repository.commitSha).toBe(validCheckout.repository.commitSha)
    expect(decoded?.verificationCommand.args).toEqual(["bun", "test", "sum.test.ts"])
  })

  test("rejects assignments without a workspace payload", () => {
    expect(gitCheckoutWorkspaceFrom({})).toBeNull()
    expect(gitCheckoutWorkspaceFrom(null)).toBeNull()
    expect(gitCheckoutWorkspaceFrom(assignmentWith("git_checkout"))).toBeNull()
  })

  test("rejects foreign workspace kinds", () => {
    expect(gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ kind: "local_path" })))).toBeNull()
  })

  test("rejects private repositories and non-github providers", () => {
    expect(
      gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ repository: { visibility: "private" as never } }))),
    ).toBeNull()
    expect(
      gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ repository: { provider: "gitlab" as never } }))),
    ).toBeNull()
  })

  test("rejects unsafe repository names", () => {
    for (const fullName of ["solo-name", "owner/repo/extra", "owner/../repo", "owner/repo name", "-/-/"]) {
      expect(
        gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ repository: { fullName } }))),
        fullName,
      ).toBeNull()
    }
  })

  test("rejects unpinned commits: branch names, short and malformed SHAs", () => {
    for (const commitSha of ["main", "33333", "z".repeat(40), `${"3".repeat(39)};`]) {
      expect(
        gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ repository: { commitSha } }))),
        commitSha,
      ).toBeNull()
    }
  })

  test("rejects branch values that cannot safely become git refspecs", () => {
    for (const branch of [
      "../escape",
      "feature/../escape",
      "-main",
      "refs/heads/main",
      "main lock",
      "feature:bad",
      "feature@{bad",
      "feature//bad",
      "feature.lock",
      "feature.",
    ]) {
      expect(
        gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ repository: { branch } }))),
        branch,
      ).toBeNull()
    }
  })

  test("rejects absolute verification paths and traversal in args", () => {
    expect(
      gitCheckoutWorkspaceFrom(
        assignmentWith(checkoutWith({ verificationCommand: { args: ["/bin/sh", "test.sh"] } })),
      ),
    ).toBeNull()
    expect(
      gitCheckoutWorkspaceFrom(
        assignmentWith(checkoutWith({ verificationCommand: { args: ["bun", "test", "../../outside.ts"] } })),
      ),
    ).toBeNull()
  })

  test("rejects shell-shaped verification command strings", () => {
    for (const args of [
      ["bun test sum.test.ts && curl evil"],
      ["bun", "test;rm -rf ."],
      ["bun", "$(curl evil)"],
      ["bun", "test", "|", "tee"],
      ["bash", "-c", "echo `id`"],
    ]) {
      expect(gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ verificationCommand: { args } }))), args.join(" ")).toBeNull()
    }
  })

  test("rejects empty or ref-less verification commands", () => {
    expect(
      gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ verificationCommand: { args: [] } }))),
    ).toBeNull()
    expect(
      gitCheckoutWorkspaceFrom(
        assignmentWith(checkoutWith({ verificationCommand: { commandRef: undefined as never } })),
      ),
    ).toBeNull()
  })
})

describe("materializeGitCheckoutWorkspace", () => {
  const stubRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
    await mkdir(workingDirectory, { recursive: true })
    await writeFile(join(workingDirectory, "checked-out"), "ok\n")
  }

  test("creates an isolated workspace under the Pylon-owned cache root only", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    try {
      const materialized = await materializeGitCheckoutWorkspace({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner: stubRunner,
        leaseRef: "lease.public.workspace.test",
        refPrefix: "workspace.pylon.claude_agent_task",
      })
      expect(materialized.workingDirectory.startsWith(`${cacheRoot}/`)).toBe(true)
      expect(materialized.workspaceRef.startsWith("workspace.pylon.claude_agent_task.")).toBe(true)
      expect(materialized.cleanupRef.startsWith("cleanup.pylon.workspace.")).toBe(true)
      expect(materialized.sourceRef).toBe(
        "OpenAgentsInc/public-sum-fixture:3333333333333333333333333333333333333333",
      )
      expect(existsSync(join(materialized.workingDirectory, "checked-out"))).toBe(true)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })

  test("derives the same ref the executors derived before extraction", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    try {
      const first = await materializeGitCheckoutWorkspace({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner: stubRunner,
        leaseRef: "lease.public.workspace.stable",
        refPrefix: "workspace.pylon.codex_agent_task",
      })
      const second = await materializeGitCheckoutWorkspace({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner: stubRunner,
        leaseRef: "lease.public.workspace.stable",
        refPrefix: "workspace.pylon.codex_agent_task",
      })
      expect(first.workspaceRef).toBe(second.workspaceRef)
      expect(first.workingDirectory).toBe(second.workingDirectory)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })

  test("two concurrent assignments for the same repository get separate refs and directories", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    try {
      const [first, second] = await Promise.all([
        materializeGitCheckoutWorkspace({
          cacheRoot,
          checkout: validCheckout,
          checkoutRunner: stubRunner,
          leaseRef: "lease.public.workspace.first",
          refPrefix: "workspace.pylon.claude_agent_task",
        }),
        materializeGitCheckoutWorkspace({
          cacheRoot,
          checkout: validCheckout,
          checkoutRunner: stubRunner,
          leaseRef: "lease.public.workspace.second",
          refPrefix: "workspace.pylon.claude_agent_task",
        }),
      ])
      expect(first.workspaceRef).not.toBe(second.workspaceRef)
      expect(first.workingDirectory).not.toBe(second.workingDirectory)
      expect(existsSync(first.workingDirectory)).toBe(true)
      expect(existsSync(second.workingDirectory)).toBe(true)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })
})

describe("removeMaterializedWorkspace", () => {
  test("removes only the assignment-scoped workspace", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    try {
      const keep = join(cacheRoot, "workspace.keep")
      const drop = join(cacheRoot, "workspace.drop")
      await mkdir(keep, { recursive: true })
      await mkdir(drop, { recursive: true })
      await removeMaterializedWorkspace({ cacheRoot, workingDirectory: drop })
      expect(existsSync(drop)).toBe(false)
      expect(existsSync(keep)).toBe(true)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })

  test("refuses targets outside the Pylon-owned cache root", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    const outside = await mkdtemp(join(tmpdir(), "pylon-workspace-outside-"))
    try {
      await expect(
        removeMaterializedWorkspace({ cacheRoot, workingDirectory: outside }),
      ).rejects.toThrow("outside the Pylon-owned cache root")
      await expect(
        removeMaterializedWorkspace({ cacheRoot, workingDirectory: cacheRoot }),
      ).rejects.toThrow("outside the Pylon-owned cache root")
      await expect(
        removeMaterializedWorkspace({
          cacheRoot,
          workingDirectory: join(cacheRoot, "..", "sibling"),
        }),
      ).rejects.toThrow("outside the Pylon-owned cache root")
      expect(existsSync(outside)).toBe(true)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
      await rm(outside, { recursive: true, force: true })
    }
  })
})

describe("cleanupOldestMaterializedWorkspaces", () => {
  test("removes oldest clean lease records until the cache is under the cap", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    const repositoryCacheRoot = await mkdtemp(join(tmpdir(), "pylon-repository-cache-"))
    const workspaceStateRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-state-"))
    const checkoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
      await mkdir(workingDirectory, { recursive: true })
      await writeFile(join(workingDirectory, "checked-out"), "ok\n")
    }

    try {
      const first = await materializeGitCheckoutWorkspaceWithLease({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.cap.first",
        refPrefix: "workspace.pylon.codex_agent_task",
        repositoryCacheRoot,
        workspaceStateRoot,
        now: new Date("2026-06-11T00:00:00.000Z"),
      })
      const second = await materializeGitCheckoutWorkspaceWithLease({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.cap.second",
        refPrefix: "workspace.pylon.codex_agent_task",
        repositoryCacheRoot,
        workspaceStateRoot,
        now: new Date("2026-06-11T00:01:00.000Z"),
      })
      const third = await materializeGitCheckoutWorkspaceWithLease({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.cap.third",
        refPrefix: "workspace.pylon.codex_agent_task",
        repositoryCacheRoot,
        workspaceStateRoot,
        now: new Date("2026-06-11T00:02:00.000Z"),
      })

      const result = await cleanupOldestMaterializedWorkspaces({
        maxMaterializedWorkspaces: 1,
        now: new Date("2026-06-11T00:03:00.000Z"),
        workspaceStateRoot,
      })

      expect(result.cleanupReceiptRefs).toHaveLength(2)
      expect(result.retainedWorkspaceRefs).toEqual([])
      expect(existsSync(first.workingDirectory)).toBe(false)
      expect(existsSync(second.workingDirectory)).toBe(false)
      expect(existsSync(third.workingDirectory)).toBe(true)
      expect(
        (await workspaceLeaseRecordFor({ workspaceStateRoot, workspaceRef: first.workspaceRef }))
          ?.state,
      ).toBe("cleaned")
      expect(
        (await workspaceLeaseRecordFor({ workspaceStateRoot, workspaceRef: second.workspaceRef }))
          ?.state,
      ).toBe("cleaned")
      expect(
        (await workspaceLeaseRecordFor({ workspaceStateRoot, workspaceRef: third.workspaceRef }))
          ?.state,
      ).toBe("materialized")
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
      await rm(repositoryCacheRoot, { recursive: true, force: true })
      await rm(workspaceStateRoot, { recursive: true, force: true })
    }
  })

  test("does not remove fresh materialized workspaces under the age guard", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    const repositoryCacheRoot = await mkdtemp(join(tmpdir(), "pylon-repository-cache-"))
    const workspaceStateRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-state-"))
    const checkoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
      await mkdir(workingDirectory, { recursive: true })
      await writeFile(join(workingDirectory, "checked-out"), "ok\n")
    }

    try {
      const first = await materializeGitCheckoutWorkspaceWithLease({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.age.first",
        refPrefix: "workspace.pylon.codex_agent_task",
        repositoryCacheRoot,
        workspaceStateRoot,
        now: new Date("2026-06-11T00:00:00.000Z"),
      })
      const second = await materializeGitCheckoutWorkspaceWithLease({
        cacheRoot,
        checkout: validCheckout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.age.second",
        refPrefix: "workspace.pylon.codex_agent_task",
        repositoryCacheRoot,
        workspaceStateRoot,
        now: new Date("2026-06-11T00:01:00.000Z"),
      })

      const result = await cleanupOldestMaterializedWorkspaces({
        maxMaterializedWorkspaces: 1,
        minimumAgeSeconds: 10 * 60,
        now: new Date("2026-06-11T00:02:00.000Z"),
        workspaceStateRoot,
      })

      expect(result.cleanupReceiptRefs).toEqual([])
      expect(result.retainedWorkspaceRefs).toEqual([])
      expect(existsSync(first.workingDirectory)).toBe(true)
      expect(existsSync(second.workingDirectory)).toBe(true)
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
      await rm(repositoryCacheRoot, { recursive: true, force: true })
      await rm(workspaceStateRoot, { recursive: true, force: true })
    }
  })
})
