import { describe, expect, test } from "vite-plus/test"

import {
  MOBILE_REPOSITORY_GIT_MUTATE_ENDPOINT,
  MOBILE_REPOSITORY_GIT_STATUS_ENDPOINT,
  MOBILE_REPOSITORY_DIFF_ENDPOINT,
  MOBILE_REPOSITORY_READ_ENDPOINT,
  MOBILE_REPOSITORY_REVIEW_ENDPOINT,
  MOBILE_REPOSITORY_SEARCH_ENDPOINT,
  MOBILE_REPOSITORY_STATUS_ENDPOINT,
  MOBILE_REPOSITORY_TERMINAL_COMMAND_ENDPOINT,
  MOBILE_REPOSITORY_TERMINAL_CREATE_ENDPOINT,
  MOBILE_REPOSITORY_TERMINAL_REPLAY_ENDPOINT,
  MOBILE_REPOSITORY_TERMINAL_SNAPSHOT_ENDPOINT,
  MOBILE_REPOSITORY_TREE_ENDPOINT,
  createAuthenticatedMobileRepositoryEnvironment,
} from "../src/coding/mobile-repository-environment-client"

const token = `mobile_${"a".repeat(32)}`
const scope = {
  sessionRef: "session.mobile.files",
  repositoryRef: "repository.mobile.files",
  worktreeRef: "worktree.mobile.files",
}

describe("T3M-D1.2 authenticated repository environment client", () => {
  test("sends exact scoped operations with bearer custody and no ambient credentials", async () => {
    const requests: Request[] = []
    const client = createAuthenticatedMobileRepositoryEnvironment({
      baseUrl: "https://openagents.com",
      accessToken: token,
      fetch: async (input, init) => {
        const request = new Request(input, init)
        requests.push(request)
        return new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { "content-type": "application/json" },
        })
      },
    })
    await client.tree({ ...scope, directoryRef: "", cursor: null, limit: 100 })
    await client.read({ ...scope, pathRef: "README.md", expectedRevisionRef: "revision.readme" })
    await client.search({ repositoryRef: scope.repositoryRef, worktreeRef: scope.worktreeRef, query: "readme", limit: 20 })
    await client.status(scope)
    await client.diff({
      ...scope,
      statusRef: "status.mobile.1",
      pathRef: "README.md",
      source: "unstaged",
      expectedRevisionRef: "revision.readme",
    })
    await client.submitReview({
      ...scope,
      statusRef: "status.mobile.1",
      pathRef: "README.md",
      rowRef: "row.mobile.1",
      expectedRevisionRef: "revision.readme",
      comment: "Keep this exact.",
      idempotencyRef: "review.mobile.1",
    })
    await client.gitStatus(scope)
    await client.gitMutate({
      ...scope,
      op: "push",
      statusRef: "status.mobile.1",
      expectedHeadRef: "commit.mobile.1",
      branchName: "feature/mobile",
      idempotencyRef: "git.mobile.push.1",
      confirmationRef: "confirmation.mobile.push.1",
    })
    await client.terminalSnapshot(scope)
    await client.terminalCreate({ ...scope, cols: 80, rows: 24, idempotencyRef: "terminal.mobile.create.1" })
    await client.terminalReplay({ ...scope, terminalRef: "terminal.mobile.1", sessionVersionRef: "terminal.version.1", afterSeq: 0, limit: 500 })
    await client.terminalCommand({ ...scope, terminalRef: "terminal.mobile.1", sessionVersionRef: "terminal.version.1",
      op: "input", data: "pwd\r", idempotencyRef: "terminal.mobile.input.1" })

    expect(requests.map(request => new URL(request.url).pathname)).toEqual([
      MOBILE_REPOSITORY_TREE_ENDPOINT,
      MOBILE_REPOSITORY_READ_ENDPOINT,
      MOBILE_REPOSITORY_SEARCH_ENDPOINT,
      MOBILE_REPOSITORY_STATUS_ENDPOINT,
      MOBILE_REPOSITORY_DIFF_ENDPOINT,
      MOBILE_REPOSITORY_REVIEW_ENDPOINT,
      MOBILE_REPOSITORY_GIT_STATUS_ENDPOINT,
      MOBILE_REPOSITORY_GIT_MUTATE_ENDPOINT,
      MOBILE_REPOSITORY_TERMINAL_SNAPSHOT_ENDPOINT,
      MOBILE_REPOSITORY_TERMINAL_CREATE_ENDPOINT,
      MOBILE_REPOSITORY_TERMINAL_REPLAY_ENDPOINT,
      MOBILE_REPOSITORY_TERMINAL_COMMAND_ENDPOINT,
    ])
    for (const request of requests) {
      expect(request.method).toBe("POST")
      expect(request.headers.get("authorization")).toBe(`Bearer ${token}`)
      expect(request.credentials).toBe("omit")
      expect(request.url).not.toContain(token)
    }
    expect(await requests[0]!.json()).toEqual({ ...scope, directoryRef: "", cursor: null, limit: 100 })
  })

  test("rejects unsafe origins, malformed custody, redirects, non-JSON, failures, and oversized bodies", async () => {
    for (const baseUrl of ["http://openagents.com", "https://user:pass@openagents.com", "https://openagents.com/api?token=x"]) {
      expect(() => createAuthenticatedMobileRepositoryEnvironment({ baseUrl, accessToken: token })).toThrow()
    }
    expect(() => createAuthenticatedMobileRepositoryEnvironment({
      baseUrl: "https://openagents.com",
      accessToken: "short",
    })).toThrow()

    const request = { ...scope, directoryRef: "", cursor: null, limit: 100 }
    const port = (response: Response) => createAuthenticatedMobileRepositoryEnvironment({
      baseUrl: "https://openagents.com",
      accessToken: token,
      fetch: async () => response,
    })
    await expect(port(new Response("not found", { status: 404 })).tree(request)).rejects.toThrow()
    await expect(port(new Response("{}", { status: 200, headers: { "content-type": "text/plain" } })).tree(request)).rejects.toThrow()
    await expect(port(new Response("{}", {
      status: 200,
      headers: { "content-type": "application/json", "content-length": "999999" },
    })).tree(request)).rejects.toThrow()
  })
})
