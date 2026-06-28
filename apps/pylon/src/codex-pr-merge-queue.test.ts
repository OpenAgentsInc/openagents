import { describe, expect, test } from "bun:test"

import {
  fastForwardMergePullRequest,
  type SupervisorPrMergeCommandRunner,
} from "./codex-pr-merge-queue.js"

const ok = (stdout = "") => ({ exitCode: 0, stdout, stderr: "", timedOut: false })

describe("supervisor PR fast-forward merge queue", () => {
  test("projects the PR onto base, verifies it, then merges the exact head through GitHub", async () => {
    const calls: string[][] = []
    const runner: SupervisorPrMergeCommandRunner = async (input) => {
      calls.push(input.args)
      if (input.args[0] === "gh" && input.args[1] === "pr" && input.args[2] === "view") {
        return ok(
          JSON.stringify({
            number: 6695,
            state: "OPEN",
            isDraft: false,
            url: "https://github.com/OpenAgentsInc/openagents/pull/7001",
            baseRefName: "main",
            headRefOid: "a".repeat(40),
          }),
        )
      }
      return ok()
    }

    const result = await fastForwardMergePullRequest({
      repository: { fullName: "OpenAgentsInc/openagents", baseBranch: "main" },
      prNumber: 6695,
      workingDirectory: "/tmp/pylon-merge-queue-test",
      verifyCommand: ["bun", "test", "apps/pylon/src/codex-pr-merge-queue.test.ts"],
      runner,
    })

    expect(result).toEqual({
      state: "merged",
      prNumber: 6695,
      prUrl: "https://github.com/OpenAgentsInc/openagents/pull/7001",
      headSha: "a".repeat(40),
      verifyExitCode: 0,
    })
    expect(calls).toEqual([
      [
        "gh",
        "pr",
        "view",
        "6695",
        "--repo",
        "OpenAgentsInc/openagents",
        "--json",
        "number,state,isDraft,url,baseRefName,headRefOid",
      ],
      [
        "git",
        "fetch",
        "--no-tags",
        "https://github.com/OpenAgentsInc/openagents.git",
        "refs/heads/main:refs/remotes/pylon-merge/main",
        "refs/pull/6695/head:refs/remotes/pylon-merge/pr-6695",
      ],
      ["git", "checkout", "-B", "pylon/virtual-merge/pr-6695", "refs/remotes/pylon-merge/main"],
      ["git", "merge", "--ff-only", "refs/remotes/pylon-merge/pr-6695"],
      ["bun", "test", "apps/pylon/src/codex-pr-merge-queue.test.ts"],
      [
        "gh",
        "pr",
        "merge",
        "6695",
        "--repo",
        "OpenAgentsInc/openagents",
        "--rebase",
        "--delete-branch",
        "--match-head-commit",
        "a".repeat(40),
      ],
    ])
  })

  test("does not ask GitHub to merge when the local fast-forward projection fails", async () => {
    const calls: string[][] = []
    const runner: SupervisorPrMergeCommandRunner = async (input) => {
      calls.push(input.args)
      if (input.args[0] === "gh" && input.args[1] === "pr" && input.args[2] === "view") {
        return ok(
          JSON.stringify({
            number: 6695,
            state: "OPEN",
            isDraft: false,
            url: "https://github.com/OpenAgentsInc/openagents/pull/7001",
            baseRefName: "main",
            headRefOid: "b".repeat(40),
          }),
        )
      }
      if (input.args[0] === "git" && input.args[1] === "merge") {
        return { exitCode: 1, stdout: "", stderr: "Not possible to fast-forward", timedOut: false }
      }
      return ok()
    }

    const result = await fastForwardMergePullRequest({
      repository: { fullName: "OpenAgentsInc/openagents", baseBranch: "main" },
      prNumber: 6695,
      workingDirectory: "/tmp/pylon-merge-queue-test",
      runner,
    })

    expect(result).toEqual({
      state: "skipped",
      reasonRef: "merge_queue.skipped_not_fast_forward",
      prNumber: 6695,
      prUrl: "https://github.com/OpenAgentsInc/openagents/pull/7001",
    })
    expect(calls.some((args) => args[0] === "gh" && args[1] === "pr" && args[2] === "merge")).toBe(false)
  })

  test("guards stale GitHub merges by matching the viewed head commit", async () => {
    const mergeCalls: string[][] = []
    const runner: SupervisorPrMergeCommandRunner = async (input) => {
      if (input.args[0] === "gh" && input.args[1] === "pr" && input.args[2] === "view") {
        return ok(
          JSON.stringify({
            number: 42,
            state: "OPEN",
            isDraft: false,
            url: "https://github.com/OpenAgentsInc/openagents/pull/42",
            baseRefName: "main",
            headRefOid: "c".repeat(40),
          }),
        )
      }
      if (input.args[0] === "gh" && input.args[1] === "pr" && input.args[2] === "merge") {
        mergeCalls.push(input.args)
      }
      return ok()
    }

    await fastForwardMergePullRequest({
      repository: { fullName: "OpenAgentsInc/openagents", baseBranch: "main" },
      prNumber: 42,
      workingDirectory: "/tmp/pylon-merge-queue-test",
      runner,
    })

    expect(mergeCalls).toHaveLength(1)
    expect(mergeCalls[0]).toContain("--match-head-commit")
    expect(mergeCalls[0]).toContain("c".repeat(40))
  })
})
