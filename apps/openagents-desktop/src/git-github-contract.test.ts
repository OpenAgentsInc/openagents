/**
 * Typed Git/GitHub contract tests (EP250 E2–E5, #8712): both-sides decode.
 * Proves the schema accepts the closed operation set, rejects malformed input
 * and unknown ops, and round-trips every result variant plus the typed error.
 */
import { describe, expect, test } from "vite-plus/test"

import {
  decodeGitGithubRequest,
  decodeGitGithubResult,
  gitGithubError,
  gitGithubOps,
  gitGithubErrorCodes,
  type GitGithubResult,
} from "./git-github-contract.ts"

describe("request decoding", () => {
  test("accepts each op with its typed params", () => {
    const valid: ReadonlyArray<unknown> = [
      { op: "status" },
      { op: "diff", repositoryRef: "workspace.repository.1", statusRef: "workspace.git-status.1", path: "a.txt", source: "unstaged", causalItemRef: null },
      { op: "discard", repositoryRef: "workspace.repository.1", statusRef: "workspace.git-status.1", path: "a.txt" },
      { op: "stage", paths: ["a.txt", "dir/b.ts"] },
      { op: "unstage", paths: ["a.txt"] },
      { op: "commit", message: "feat: x" },
      { op: "push" },
      { op: "branchList" },
      { op: "branchCreate", name: "feature/x", checkout: true },
      { op: "checkout", name: "main" },
      { op: "issueList" },
      { op: "issueList", limit: 5 },
      { op: "issueView", number: 8712 },
      { op: "issueCreate", title: "t", body: "b" },
      { op: "prList", limit: 10 },
      { op: "prView", number: 42 },
      { op: "prCreate", title: "t", body: "b" },
      { op: "prCreate", title: "t", body: "b", base: "main", head: "feat" },
    ]
    for (const request of valid) {
      expect(decodeGitGithubRequest(request)).not.toBeNull()
    }
  })

  test("rejects unknown ops and malformed params", () => {
    const invalid: ReadonlyArray<unknown> = [
      null,
      {},
      { op: "rm-rf" },
      { op: "commit" }, // missing message
      { op: "stage" }, // missing paths
      { op: "stage", paths: "a.txt" }, // paths not an array
      { op: "diff", repositoryRef: "r", statusRef: "s", path: "a.txt", source: "working", causalItemRef: null },
      { op: "branchCreate", name: "x" }, // missing checkout
      { op: "issueView", number: "8712" }, // number not a number
    ]
    for (const request of invalid) {
      expect(decodeGitGithubRequest(request)).toBeNull()
    }
  })

  test("excess keys are stripped to the clean typed request (never passed through)", () => {
    expect(decodeGitGithubRequest({ op: "status", rm: { rf: true } })).toEqual({ op: "status" })
    expect(decodeGitGithubRequest({ op: "commit", message: "x", extra: 1 })).toEqual({ op: "commit", message: "x" })
  })
})

describe("result decoding", () => {
  const results: ReadonlyArray<GitGithubResult> = [
    {
      ok: true,
      op: "status",
      branch: "main",
      upstream: "origin/main",
      detached: false,
      ahead: 1,
      behind: 0,
      staged: [{ path: "a.txt", status: "modified" }],
      unstaged: [],
      untracked: [{ path: "b.txt", status: "untracked" }],
      truncated: false,
      repositoryRef: "workspace.repository.1",
      statusRef: "workspace.git-status.1",
      headRef: "a".repeat(40),
    },
    {
      ok: true,
      op: "diff",
      repositoryRef: "workspace.repository.1",
      statusRef: "workspace.git-status.1",
      path: "a.txt",
      source: "unstaged",
      causalItemRef: "timeline.item.file-change.1",
      content: "@@ -1 +1 @@\n-old\n+new\n",
      hunks: [{ header: "@@ -1 +1 @@", oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, content: "@@ -1 +1 @@\n-old\n+new\n" }],
      truncated: false,
    },
    { ok: true, op: "discard", repositoryRef: "workspace.repository.1", path: "a.txt", statusRef: "workspace.git-status.2" },
    { ok: true, op: "stage", paths: ["a.txt"] },
    { ok: true, op: "commit", sha: "0".repeat(40), shortSha: "0000000", summary: "feat: x" },
    { ok: true, op: "push", ref: "main", remote: "origin", sha: "a".repeat(40) },
    { ok: true, op: "branchList", current: "main", branches: [{ name: "main", current: true, upstream: null }], truncated: false },
    { ok: true, op: "branchCreate", name: "feat", checkedOut: true },
    { ok: true, op: "checkout", name: "main" },
    { ok: true, op: "issueList", issues: [{ number: 1, title: "t", url: "u", state: "OPEN" }] },
    { ok: true, op: "issueView", issue: { number: 1, title: "t", url: "u", state: "OPEN", body: "b" } },
    { ok: true, op: "issueCreate", number: 8712, url: "https://x/8712" },
    { ok: true, op: "prCreate", number: 42, url: "https://x/42" },
    { ok: true, op: "prList", prs: [{ number: 5, title: "t", url: "u", state: "OPEN", headRefName: "f", baseRefName: "main" }] },
    { ok: true, op: "prView", pr: { number: 5, title: "t", url: "u", state: "OPEN", headRefName: "f", baseRefName: "main", body: "b" } },
    gitGithubError("push", "non_fast_forward", "diverged"),
  ]

  test("round-trips every result variant", () => {
    for (const result of results) {
      expect(decodeGitGithubResult(result)).toEqual(result)
    }
  })

  test("rejects malformed results", () => {
    expect(decodeGitGithubResult({ ok: true, op: "commit" })).toBeNull() // missing sha
    expect(decodeGitGithubResult({ ok: false, op: "push", error: "made_up", message: "x" })).toBeNull()
    expect(decodeGitGithubResult(null)).toBeNull()
  })
})

describe("closed enums", () => {
  test("op and error-code sets are stable and complete", () => {
    expect(gitGithubOps).toContain("commit")
    expect(gitGithubOps).toContain("prCreate")
    expect(gitGithubOps).toContain("discard")
    expect(gitGithubErrorCodes).toContain("no_upstream")
    expect(gitGithubErrorCodes).toContain("gh_unauthenticated")
    expect(gitGithubErrorCodes).toContain("stale_status")
  })

  test("gitGithubError produces a decodable typed error", () => {
    const error = gitGithubError("commit", "empty_message", "needs a message")
    expect(decodeGitGithubResult(error)).toEqual(error)
  })
})
