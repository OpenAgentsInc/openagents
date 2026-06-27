import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  assignmentBranchName,
  issueRefFromSummary,
  publishAssignmentPullRequest,
  type AssignmentPrCommandResult,
} from "../src/codex-pr-publisher"

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
      const runner = async (input: { args: string[] }): Promise<AssignmentPrCommandResult> => {
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
        expect(result.branch).toBe(assignmentBranchName("assignment.public.codex.opened"))
        expect(result.changedCount).toBe(1)
        expect(result.reused).toBe(false)
      }
      // exactly one PR-create call, and never a push to the base branch
      const creates = commands.filter((c) => c[0] === "gh" && c[2] === "create")
      expect(creates.length).toBe(1)
      const pushes = commands.filter((c) => c[0] === "git" && c[1] === "push")
      expect(pushes.length).toBe(1)
      expect(pushes[0].some((arg) => arg.includes(":refs/heads/main"))).toBe(false)
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
})
