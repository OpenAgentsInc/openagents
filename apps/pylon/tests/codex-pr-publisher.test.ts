import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  assignmentBranchName,
  deriveConventionalTitle,
  findOpenPullRequestForIssue,
  issueBranchName,
  issueNumberFromSummary,
  issueRefFromSummary,
  assignmentPullRequestWritebackRuntimeEvent,
  publishAssignmentPullRequest,
  type AssignmentPrCommandResult,
} from "../src/codex-pr-publisher"

const validGithubBroker = {
  schema: "openagents.pylon.scm_auth_broker.v1" as const,
  kind: "github_user_oauth" as const,
  brokerUrl: "https://openagents.com/api/pylon/github/git-credentials",
  authRefs: ["github-identity:token:user_123"],
  repositoryRef: "repo.github/OpenAgentsInc/openagents",
  allowed: {
    protocol: "https" as const,
    host: "github.com",
    pathPrefix: "/OpenAgentsInc/openagents.git",
  },
  fallback: "fail_closed" as const,
}

async function git(args: string[], cwd: string): Promise<void> {
  const proc = Bun.spawn(["git", ...args], { cwd, stderr: "pipe", stdout: "pipe" })
  const code = await proc.exited
  if (code !== 0) throw new Error(`git ${args.join(" ")} failed (${code})`)
}

async function headSha(cwd: string): Promise<string> {
  const proc = Bun.spawn(["git", "rev-parse", "HEAD"], { cwd, stderr: "pipe", stdout: "pipe" })
  const [out] = await Promise.all([new Response(proc.stdout).text(), proc.exited])
  return out.trim()
}

async function withGitWorkspace<T>(
  fn: (ctx: { cacheRoot: string; workingDirectory: string; baseSha: string }) => Promise<T>,
): Promise<T> {
  const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-pr-pub-"))
  const workingDirectory = join(cacheRoot, "ws")
  try {
    await git(["init", "-q", workingDirectory], cacheRoot)
    await git(["-C", workingDirectory, "config", "user.email", "t@example.invalid"], cacheRoot)
    await git(["-C", workingDirectory, "config", "user.name", "Test"], cacheRoot)
    await writeFile(join(workingDirectory, "base.txt"), "base\n")
    await git(["add", "-A"], workingDirectory)
    await git(["commit", "-q", "-m", "base"], workingDirectory)
    const baseSha = await headSha(workingDirectory)
    return await fn({ cacheRoot, workingDirectory, baseSha })
  } finally {
    await rm(cacheRoot, { recursive: true, force: true })
  }
}

describe("assignmentBranchName", () => {
  test("is deterministic and prefixed", () => {
    const a = assignmentBranchName("assignment.public.codex.x")
    const b = assignmentBranchName("assignment.public.codex.x")
    expect(a).toBe(b)
    expect(a.startsWith("pylon/assignment-")).toBe(true)
    expect(assignmentBranchName("assignment.public.codex.y")).not.toBe(a)
  })
})

describe("issueRefFromSummary", () => {
  test("extracts the first issue ref", () => {
    expect(issueRefFromSummary("Implement public issue #6439 and verify.")).toBe("#6439")
    expect(issueRefFromSummary("no issue here")).toBeNull()
    expect(issueRefFromSummary(undefined)).toBeNull()
  })
})

describe("publishAssignmentPullRequest", () => {
  test("skips a non-git working directory", async () => {
    const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-pr-pub-skip-"))
    try {
      const workingDirectory = join(cacheRoot, "ws")
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.test",
        sourceRef: "OpenAgentsInc/openagents:" + "a".repeat(40),
        repository: { branch: "main", commitSha: "a".repeat(40), fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.skip",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner: async () => ({ exitCode: 0, stdout: "", stderr: "", timedOut: false }),
      })
      expect(result.state).toBe("skipped")
    } finally {
      await rm(cacheRoot, { recursive: true, force: true })
    }
  })

  test("returns no_change for a clean git worktree", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.clean",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.clean",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner: async () => ({ exitCode: 0, stdout: "", stderr: "", timedOut: false }),
      })
      expect(result.state).toBe("no_change")
    })
  })

  test("opens exactly one PR for a verified non-empty diff", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      const commands: string[][] = []
      const runner = async (input: { args: string[]; env?: Record<string, string | undefined> }): Promise<AssignmentPrCommandResult> => {
        commands.push(input.args)
        const [bin, sub] = input.args
        if (bin === "git") {
          // run the local-only git ops for real so branch/commit reflect state,
          // but never let a push reach the network.
          if (sub === "push") return { exitCode: 0, stdout: "", stderr: "", timedOut: false }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        if (bin === "gh" && sub === "pr" && input.args[2] === "list") {
          return { exitCode: 0, stdout: "[]", stderr: "", timedOut: false }
        }
        if (bin === "gh" && sub === "pr" && input.args[2] === "create") {
          return {
            exitCode: 0,
            stdout: "https://github.com/OpenAgentsInc/openagents/pull/12345\n",
            stderr: "",
            timedOut: false,
          }
        }
        return { exitCode: 1, stdout: "", stderr: "unexpected", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.opened",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.opened",
        objectiveSummary: "Implement public issue #6439 and run the verification.",
        verification: { args: ["bun", "test", "sum.test.ts"], exitCode: 0, passed: true },
        runner,
      })
      expect(result.state).toBe("opened")
      if (result.state === "opened") {
        expect(result.prUrl).toBe("https://github.com/OpenAgentsInc/openagents/pull/12345")
        expect(result.prNumber).toBe(12345)
        // Branch is now keyed on the ISSUE number, not the per-run assignment ref.
        expect(result.branch).toBe(issueBranchName(6439))
        expect(result.changedCount).toBe(1)
        expect(result.reused).toBe(false)
      }
      // exactly one PR-create call, and never a push to the base branch
      const creates = commands.filter((c) => c[0] === "gh" && c[2] === "create")
      expect(creates.length).toBe(1)
      const pushes = commands.filter((c) => c[0] === "git" && c[1] === "push")
      expect(pushes.length).toBe(1)
      expect(pushes[0].some((arg) => arg.includes(":refs/heads/main"))).toBe(false)
      expect(pushes[0].some((arg) => arg.startsWith("+"))).toBe(false)
    })
  })

  test("branch-only writeback pushes the branch and opens no PR (#8477)", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      const commands: string[][] = []
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
        commands.push(input.args)
        const [bin, sub] = input.args
        if (bin === "git") {
          if (sub === "push") return { exitCode: 0, stdout: "", stderr: "", timedOut: false }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        // No gh call should ever be made in branch-only mode.
        return { exitCode: 1, stdout: "", stderr: "unexpected gh call in branch-only mode", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.branch_only",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.branch_only",
        objectiveSummary: "Implement public issue #8477 and run the verification.",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        openPullRequest: false,
        runner,
      })
      expect(result.state).toBe("branch_pushed")
      if (result.state === "branch_pushed") {
        expect(result.branch).toBe(issueBranchName(8477))
        expect(result.branchUrl).toBe(
          "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
        )
        expect(result.changedCount).toBe(1)
      }
      // Never opens or lists a PR, and pushes exactly once without a force refspec.
      expect(commands.some((c) => c[0] === "gh")).toBe(false)
      const pushes = commands.filter((c) => c[0] === "git" && c[1] === "push")
      expect(pushes).toHaveLength(1)
      expect(pushes[0].some((arg) => arg.includes(":refs/heads/main"))).toBe(false)
      expect(pushes[0].some((arg) => arg.startsWith("+"))).toBe(false)
    })
  })

  test("branch-only writeback surfaces a typed permission failure from the push (#8477)", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
        const [bin, sub] = input.args
        if (bin === "git") {
          if (sub === "push") {
            return {
              exitCode: 1,
              stdout: "",
              stderr: "remote: Permission to OpenAgentsInc/openagents.git denied to user.",
              timedOut: false,
            }
          }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        return { exitCode: 1, stdout: "", stderr: "unexpected", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.branch_only_denied",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.branch_only_denied",
        objectiveSummary: "Implement public issue #8477 and run the verification.",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        openPullRequest: false,
        runner,
      })
      expect(result.state).toBe("failed")
      if (result.state === "failed") {
        expect(result.reasonRef).toBe("pull_request.permission_denied")
      }
    })
  })

  test("uses the brokered GitHub credential for gh API calls without force-pushing (#8477)", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      const ghTokens: Array<string | undefined> = []
      const commands: string[][] = []
      const runner = async (input: {
        args: string[]
        env?: Record<string, string | undefined>
        stdin?: string
      }): Promise<AssignmentPrCommandResult> => {
        commands.push(input.args)
        const [bin, sub] = input.args
        if (bin === "git" && sub === "credential") {
          expect(input.stdin).toContain("path=OpenAgentsInc/openagents.git")
          return {
            exitCode: 0,
          stdout: "username=x-access-token\npassword=unit_test_user_oauth_token\n\n",
            stderr: "",
            timedOut: false,
          }
        }
        if (bin === "git") {
          if (sub === "push") return { exitCode: 0, stdout: "", stderr: "", timedOut: false }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        if (bin === "gh") {
          ghTokens.push(input.env?.GH_TOKEN)
          if (input.args[2] === "list") return { exitCode: 0, stdout: "[]", stderr: "", timedOut: false }
          if (input.args[2] === "view") return { exitCode: 0, stdout: "Brokered writeback\n", stderr: "", timedOut: false }
          if (input.args[2] === "create") {
            return {
              exitCode: 0,
              stdout: "https://github.com/OpenAgentsInc/openagents/pull/8477\n",
              stderr: "",
              timedOut: false,
            }
          }
        }
        return { exitCode: 1, stdout: "", stderr: "unexpected", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.brokered",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        scmAuthBroker: validGithubBroker,
        assignmentRef: "assignment.public.codex.brokered",
        objectiveSummary: "Implement public issue #8477 and run the verification.",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner,
      })
      expect(result.state).toBe("opened")
      if (result.state === "opened") {
        expect(result.prUrl).toBe("https://github.com/OpenAgentsInc/openagents/pull/8477")
        expect(result.branchUrl).toBe("https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477")
      }
      expect(ghTokens.length).toBeGreaterThan(0)
      expect(ghTokens.every((token) => token === "unit_test_user_oauth_token")).toBe(true)
      const pushes = commands.filter((c) => c[0] === "git" && c[1] === "push")
      expect(pushes).toHaveLength(1)
      expect(pushes[0].some((arg) => arg.startsWith("+"))).toBe(false)
    })
  })

  test("returns a typed permission failure when branch push is denied (#8477)", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      let createCalls = 0
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
        const [bin, sub] = input.args
        if (bin === "git") {
          if (sub === "push") {
            return {
              exitCode: 1,
              stdout: "",
              stderr: "remote: Permission to OpenAgentsInc/openagents.git denied to user.",
              timedOut: false,
            }
          }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        if (bin === "gh" && input.args[2] === "list") {
          return { exitCode: 0, stdout: "[]", stderr: "", timedOut: false }
        }
        if (bin === "gh" && input.args[2] === "create") {
          createCalls += 1
        }
        return { exitCode: 1, stdout: "", stderr: "unexpected", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.permission",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.permission",
        objectiveSummary: "Implement public issue #8477 and run the verification.",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner,
      })
      expect(result).toMatchObject({
        state: "failed",
        reasonRef: "pull_request.permission_denied",
        branch: issueBranchName(8477),
      })
      expect(createCalls).toBe(0)
    })
  })

  test("reuses an existing open PR for the same assignment branch", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      let createCalls = 0
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
        const [bin, sub] = input.args
        if (bin === "git") {
          if (sub === "push") return { exitCode: 0, stdout: "", stderr: "", timedOut: false }
          const proc = Bun.spawn(["git", ...input.args.slice(1)], {
            cwd: workingDirectory,
            stderr: "pipe",
            stdout: "pipe",
          })
          const [out, err, code] = await Promise.all([
            new Response(proc.stdout).text(),
            new Response(proc.stderr).text(),
            proc.exited,
          ])
          return { exitCode: code, stdout: out, stderr: err, timedOut: false }
        }
        if (bin === "gh" && input.args[2] === "list") {
          return {
            exitCode: 0,
            stdout: JSON.stringify([{ url: "https://github.com/OpenAgentsInc/openagents/pull/777", number: 777 }]),
            stderr: "",
            timedOut: false,
          }
        }
        if (bin === "gh" && input.args[2] === "create") {
          createCalls += 1
          return { exitCode: 0, stdout: "https://github.com/x/y/pull/1\n", stderr: "", timedOut: false }
        }
        return { exitCode: 1, stdout: "", stderr: "", timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.reuse",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        assignmentRef: "assignment.public.codex.reuse",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner,
      })
      expect(result.state).toBe("opened")
      if (result.state === "opened") {
        expect(result.reused).toBe(true)
        expect(result.prNumber).toBe(777)
      }
      expect(createCalls).toBe(0)
    })
  })

  test("dedups by ISSUE: reuses an existing open PR for the same issue without pushing or creating", async () => {
    await withGitWorkspace(async ({ cacheRoot, workingDirectory, baseSha }) => {
      await writeFile(join(workingDirectory, "fix.txt"), "the codex change\n")
      const ghCalls: string[][] = []
      let pushCalls = 0
      let createCalls = 0
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
        const [bin, sub] = input.args
        if (bin === "git" && sub === "push") {
          pushCalls += 1
          return { exitCode: 0, stdout: "", stderr: "", timedOut: false }
        }
        if (bin === "gh") {
          ghCalls.push(input.args)
          // Issue-level lookup carries --search; head-branch lookup carries --head.
          if (input.args.includes("--search")) {
            return {
              exitCode: 0,
              stdout: JSON.stringify([
                {
                  number: 4242,
                  url: "https://github.com/OpenAgentsInc/openagents/pull/4242",
                  headRefName: "pylon/assignment-issue-6439",
                  title: "feat(pylon): earlier fix",
                  body: "Addresses #6439.",
                },
              ]),
              stderr: "",
              timedOut: false,
            }
          }
          if (input.args[2] === "create") createCalls += 1
          return { exitCode: 0, stdout: "https://github.com/x/y/pull/1\n", stderr: "", timedOut: false }
        }
        // local git ops run for real (capture only needs them; no branch work expected)
        const proc = Bun.spawn(["git", ...input.args.slice(1)], {
          cwd: workingDirectory,
          stderr: "pipe",
          stdout: "pipe",
        })
        const [out, err, code] = await Promise.all([
          new Response(proc.stdout).text(),
          new Response(proc.stderr).text(),
          proc.exited,
        ])
        return { exitCode: code, stdout: out, stderr: err, timedOut: false }
      }
      const result = await publishAssignmentPullRequest({
        cacheRoot,
        workingDirectory,
        workspaceRef: "workspace.pylon.codex_agent_task.issuededup",
        sourceRef: `OpenAgentsInc/openagents:${baseSha}`,
        repository: { branch: "main", commitSha: baseSha, fullName: "OpenAgentsInc/openagents" },
        // A FRESH assignment ref every run — the old branch-by-assignmentRef key
        // would have minted a new branch + PR here. Dedup must key on the issue.
        assignmentRef: "assignment.public.codex.run-" + Math.random().toString(36).slice(2),
        objectiveSummary: "Implement public issue #6439 and run the named verification.",
        verification: { args: ["bun", "test"], exitCode: 0, passed: true },
        runner,
      })
      expect(result.state).toBe("opened")
      if (result.state === "opened") {
        expect(result.reused).toBe(true)
        expect(result.prNumber).toBe(4242)
        expect(result.branch).toBe("pylon/assignment-issue-6439")
      }
      // No duplicate PR opened, and nothing pushed.
      expect(createCalls).toBe(0)
      expect(pushCalls).toBe(0)
      // Exactly one gh call (the issue-level search lookup).
      expect(ghCalls.length).toBe(1)
      expect(ghCalls[0].includes("--search")).toBe(true)
    })
  })
})

describe("issueNumberFromSummary", () => {
  test("extracts the numeric issue", () => {
    expect(issueNumberFromSummary("Implement public issue #6439 and verify.")).toBe(6439)
    expect(issueNumberFromSummary("none")).toBeNull()
    expect(issueNumberFromSummary(undefined)).toBeNull()
  })
})

describe("issueBranchName", () => {
  test("is deterministic per issue and prefixed", () => {
    expect(issueBranchName(6439)).toBe("pylon/assignment-issue-6439")
    expect(issueBranchName(6439)).toBe(issueBranchName(6439))
    expect(issueBranchName(6439).startsWith("pylon/assignment-")).toBe(true)
    expect(issueBranchName(6440)).not.toBe(issueBranchName(6439))
  })
})

describe("assignmentPullRequestWritebackRuntimeEvent", () => {
  test("builds a thread-scoped runtime event with branch and PR links (#8477)", () => {
    const event = assignmentPullRequestWritebackRuntimeEvent({
      result: {
        state: "opened",
        prUrl: "https://github.com/OpenAgentsInc/openagents/pull/8477",
        prNumber: 8477,
        branch: "pylon/assignment-issue-8477",
        branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
        changedCount: 3,
        reused: false,
      },
      repositoryFullName: "OpenAgentsInc/openagents",
      threadId: "thread.private.khala.8477",
      turnId: "turn.private.khala.8477",
      sequence: 8,
      observedAt: "2026-07-06T12:00:00.000Z",
      source: { adapterKind: "codex", lane: "codex_app_server", surface: "server" },
    })
    expect(event).toMatchObject({
      kind: "writeback.recorded",
      visibility: "private",
      redactionClass: "private_ref",
      repositoryFullName: "OpenAgentsInc/openagents",
      branch: "pylon/assignment-issue-8477",
      branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
      pullRequestUrl: "https://github.com/OpenAgentsInc/openagents/pull/8477",
      pullRequestNumber: 8477,
      changedFileCount: 3,
      status: "pull_request_opened",
    })
    expect(JSON.stringify(event)).not.toContain("gho_")
  })

  test("builds a branch-only writeback event with no PR fields (#8477)", () => {
    const event = assignmentPullRequestWritebackRuntimeEvent({
      result: {
        state: "branch_pushed",
        branch: "pylon/assignment-issue-8477",
        branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
        changedCount: 2,
      },
      repositoryFullName: "OpenAgentsInc/openagents",
      threadId: "thread.private.khala.8477",
      turnId: "turn.private.khala.8477",
      sequence: 9,
      observedAt: "2026-07-06T12:00:00.000Z",
      source: { adapterKind: "codex", lane: "codex_app_server", surface: "server" },
    })
    expect(event).toMatchObject({
      kind: "writeback.recorded",
      visibility: "private",
      repositoryFullName: "OpenAgentsInc/openagents",
      branch: "pylon/assignment-issue-8477",
      branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/pylon/assignment-issue-8477",
      changedFileCount: 2,
      status: "branch_pushed",
    })
    expect(event).not.toHaveProperty("pullRequestUrl")
    expect(event).not.toHaveProperty("pullRequestNumber")
  })
})

describe("deriveConventionalTitle", () => {
  test("prefers a real issue title and adds a diff-derived type/scope", () => {
    const title = deriveConventionalTitle({
      issueNumber: 6439,
      issueTitle: "Stop the publisher from opening duplicate PRs",
      changedPaths: ["apps/pylon/src/codex-pr-publisher.ts"],
    })
    expect(title).toBe("feat(pylon): stop the publisher from opening duplicate PRs")
  })

  test("keeps an already-conventional issue title verbatim", () => {
    const title = deriveConventionalTitle({
      issueNumber: 1,
      issueTitle: "fix(api): correct labor earnings rounding",
      changedPaths: ["apps/openagents.com/workers/api/src/x.ts"],
    })
    expect(title).toBe("fix(api): correct labor earnings rounding")
  })

  test("falls back to the issue number when the summary is generic boilerplate", () => {
    const title = deriveConventionalTitle({
      issueNumber: 6439,
      objectiveSummary: "Implement public issue #6439 and run the named verification.",
      changedPaths: ["docs/notes.md"],
    })
    expect(title).toBe("docs: resolve issue #6439")
  })
})

describe("findOpenPullRequestForIssue", () => {
  test("matches a fleet PR that references the issue and ignores unrelated PRs", async () => {
    const runner = async (): Promise<AssignmentPrCommandResult> => ({
      exitCode: 0,
      stdout: JSON.stringify([
        { number: 11, url: "u11", headRefName: "feature/human-pr", title: "mentions #6439", body: "" },
        { number: 12, url: "u12", headRefName: "pylon/assignment-issue-6439", title: "feat: x", body: "Addresses #6439." },
      ]),
      stderr: "",
      timedOut: false,
    })
    const found = await findOpenPullRequestForIssue({
      runner,
      cwd: "/tmp",
      fullName: "OpenAgentsInc/openagents",
      issueNumber: 6439,
    })
    expect(found?.number).toBe(12)
    expect(found?.headRefName).toBe("pylon/assignment-issue-6439")
  })

  test("does not match a near-miss number", async () => {
    const runner = async (): Promise<AssignmentPrCommandResult> => ({
      exitCode: 0,
      stdout: JSON.stringify([
        { number: 9, url: "u9", headRefName: "pylon/assignment-issue-64390", title: "x", body: "Addresses #64390." },
      ]),
      stderr: "",
      timedOut: false,
    })
    const found = await findOpenPullRequestForIssue({
      runner,
      cwd: "/tmp",
      fullName: "OpenAgentsInc/openagents",
      issueNumber: 6439,
    })
    expect(found).toBeNull()
  })
})
