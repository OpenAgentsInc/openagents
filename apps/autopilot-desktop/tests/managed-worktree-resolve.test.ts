// #5471 (EPIC #5461): node-side managed-worktree resolver tests.
//
// `resolveManagedWorktreeRepoRef` turns a {repo, baseRef, branch} request into
// the concrete `repoRef` Pylon's `repositoryRefFrom` accepts on session.spawn,
// resolving the 40-char commit SHA with `git ls-remote`. These tests inject a
// fake git runner so the whole resolution (parse → ls-remote → repoRef) is
// covered without touching the network or a local clone.

import { describe, expect, test } from "bun:test"

import { resolveManagedWorktreeRepoRef } from "../src/bun/pylon-control"

const COMMIT = "abc1234abc1234abc1234abc1234abc1234abc12"

describe("resolveManagedWorktreeRepoRef (#5471)", () => {
  test("resolves the first ls-remote SHA into a github repoRef", async () => {
    let seenArgs: string[] = []
    const result = await resolveManagedWorktreeRepoRef({
      fullName: "OpenAgentsInc/openagents",
      baseRef: "origin/main",
      branch: "main",
      gitRunner: async (args) => {
        seenArgs = args
        return { ok: true, stdout: `${COMMIT}\trefs/heads/main\n` }
      },
    })
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected ok")
    expect(result.repoRef.provider).toBe("github")
    expect(result.repoRef.visibility).toBe("public")
    expect(result.repoRef.fullName).toBe("OpenAgentsInc/openagents")
    expect(result.repoRef.branch).toBe("main")
    expect(result.repoRef.commitSha).toBe(COMMIT)
    // ls-remote is called against the public remote with the origin/-stripped ref.
    expect(seenArgs).toEqual([
      "ls-remote",
      "https://github.com/OpenAgentsInc/openagents.git",
      "main",
    ])
  })

  test("rejects a non-GitHub owner/name", async () => {
    const result = await resolveManagedWorktreeRepoRef({
      fullName: "nope",
      baseRef: "main",
      branch: "main",
      gitRunner: async () => ({ ok: true, stdout: `${COMMIT}\trefs/heads/main` }),
    })
    expect(result.ok).toBe(false)
  })

  test("rejects a dangerous base ref before running git", async () => {
    let called = false
    const result = await resolveManagedWorktreeRepoRef({
      fullName: "a/b",
      baseRef: "../escape",
      branch: "escape",
      gitRunner: async () => {
        called = true
        return { ok: true, stdout: `${COMMIT}\tx` }
      },
    })
    expect(result.ok).toBe(false)
    expect(called).toBe(false)
  })

  test("fails when ls-remote returns no matching ref", async () => {
    const result = await resolveManagedWorktreeRepoRef({
      fullName: "a/b",
      baseRef: "origin/nope",
      branch: "nope",
      gitRunner: async () => ({ ok: true, stdout: "" }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected failure")
    expect(result.error).toContain("did not resolve")
  })

  test("propagates a git failure", async () => {
    const result = await resolveManagedWorktreeRepoRef({
      fullName: "a/b",
      baseRef: "main",
      branch: "main",
      gitRunner: async () => ({ ok: false, stdout: "", error: "git failed: no network" }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected failure")
    expect(result.error).toContain("git failed")
  })
})
