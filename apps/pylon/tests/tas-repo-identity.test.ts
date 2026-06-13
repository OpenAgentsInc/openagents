import { describe, expect, test } from "bun:test"

import {
  buildRepoIdentity,
  scopeKey,
  type RepoIdentityInput,
} from "../src/tas/repo-identity"

const sha = "0123456789abcdef0123456789abcdef01234567"

const identity = (overrides: Partial<RepoIdentityInput> = {}): RepoIdentityInput => ({
  repoFullName: "OpenAgentsInc/openagents",
  commitSha: sha,
  branch: "main",
  detached: false,
  worktreeRef: "worktree.fixture.pc1",
  dirty: false,
  ...overrides,
})

describe("tas repo identity core", () => {
  test("builds a valid repository identity snapshot", () => {
    expect(
      buildRepoIdentity(
        identity({
          commitSha: sha.toUpperCase(),
        }),
      ),
    ).toEqual({
      repoFullName: "OpenAgentsInc/openagents",
      commitSha: sha,
      branch: "main",
      detached: false,
      worktreeRef: "worktree.fixture.pc1",
      dirty: false,
    })
  })

  test("rejects bad commitSha values", () => {
    expect(() =>
      buildRepoIdentity(
        identity({
          commitSha: "not-a-commit",
        }),
      ),
    ).toThrow("40-hex")

    expect(() =>
      buildRepoIdentity(
        identity({
          commitSha: `${sha}0`,
        }),
      ),
    ).toThrow("40-hex")
  })

  test("enforces detached versus branch invariant", () => {
    expect(
      buildRepoIdentity(
        identity({
          branch: null,
          detached: true,
        }),
      ),
    ).toMatchObject({
      branch: null,
      detached: true,
    })

    expect(() =>
      buildRepoIdentity(
        identity({
          branch: null,
          detached: false,
        }),
      ),
    ).toThrow("detached")

    expect(() =>
      buildRepoIdentity(
        identity({
          branch: "main",
          detached: true,
        }),
      ),
    ).toThrow("detached")
  })

  test("produces a deterministic scope key for scoped repo work", () => {
    const snapshot = buildRepoIdentity(identity())

    expect(scopeKey(snapshot)).toBe(scopeKey(snapshot))
    expect(scopeKey(snapshot)).toBe(
      "repo-scope:OpenAgentsInc%2Fopenagents:worktree.fixture.pc1:0123456789abcdef0123456789abcdef01234567:branch%3Amain:clean",
    )
    expect(
      scopeKey(
        buildRepoIdentity(
          identity({
            worktreeRef: "worktree.fixture.pc2",
          }),
        ),
      ),
    ).not.toBe(scopeKey(snapshot))
  })
})
