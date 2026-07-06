import { describe, expect, test } from "bun:test"

import {
  fetchKhalaMobileRepositories,
  fetchKhalaMobileRepository,
  type KhalaMobileReposFetchLike,
} from "../src/sync/khala-mobile-repos-api"

const sampleRepo = {
  defaultBranch: "main",
  description: "An OpenAgents repo",
  fullName: "OpenAgentsInc/openagents",
  htmlUrl: "https://github.com/OpenAgentsInc/openagents",
  id: "gh-repo-1",
  name: "openagents",
  owner: "OpenAgentsInc",
  private: false,
  provider: "github" as const,
}

const fakeFetch = (response: { body: unknown; ok: boolean; status?: number }): KhalaMobileReposFetchLike =>
  (async () => ({ json: async () => response.body, ok: response.ok, status: response.status })) as KhalaMobileReposFetchLike

describe("fetchKhalaMobileRepositories", () => {
  test("parses a successful paginated response", async () => {
    const result = await fetchKhalaMobileRepositories(
      "https://openagents.com",
      "tok",
      { page: 1, perPage: 50 },
      fakeFetch({ body: { hasNextPage: true, page: 1, perPage: 50, repositories: [sampleRepo] }, ok: true }),
    )
    expect(result).toEqual({
      ok: true,
      value: { hasNextPage: true, page: 1, perPage: 50, repositories: [sampleRepo] },
    })
  })

  test("maps github_token_missing to a friendly re-auth message", async () => {
    const result = await fetchKhalaMobileRepositories(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: { error: "github_token_missing" }, ok: false, status: 409 }),
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.kind).toBe("github_token_missing")
      expect(result.messageSafe.length).toBeGreaterThan(0)
    }
  })

  test("maps github_token_expired to a re-auth message", async () => {
    const result = await fetchKhalaMobileRepositories(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: { error: "github_token_expired" }, ok: false, status: 401 }),
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.kind).toBe("github_token_expired")
  })

  test("maps a bare 401 with no error code to unauthorized", async () => {
    const result = await fetchKhalaMobileRepositories(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: {}, ok: false, status: 401 }),
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.kind).toBe("unauthorized")
  })

  test("reports unknown for a malformed 200 body", async () => {
    const result = await fetchKhalaMobileRepositories(
      "https://openagents.com",
      "tok",
      {},
      fakeFetch({ body: { repositories: "not an array" }, ok: true }),
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.kind).toBe("unknown")
  })

  test("builds the request URL with page/perPage query params", async () => {
    let capturedUrl = ""
    const fetchImpl: KhalaMobileReposFetchLike = (async (url: string) => {
      capturedUrl = url
      return { json: async () => ({ hasNextPage: false, page: 2, perPage: 10, repositories: [] }), ok: true }
    }) as KhalaMobileReposFetchLike
    await fetchKhalaMobileRepositories("https://openagents.com", "tok", { page: 2, perPage: 10 }, fetchImpl)
    expect(capturedUrl).toBe("https://openagents.com/api/mobile/repos?page=2&perPage=10")
  })
})

describe("fetchKhalaMobileRepository", () => {
  test("parses a successful single-repo response", async () => {
    const result = await fetchKhalaMobileRepository(
      "https://openagents.com",
      "tok",
      "OpenAgentsInc",
      "openagents",
      fakeFetch({ body: { repository: sampleRepo }, ok: true }),
    )
    expect(result).toEqual({ ok: true, value: sampleRepo })
  })

  test("maps repository_not_found to the not_found kind", async () => {
    const result = await fetchKhalaMobileRepository(
      "https://openagents.com",
      "tok",
      "OpenAgentsInc",
      "missing-repo",
      fakeFetch({ body: { error: "repository_not_found", repositoryId: "OpenAgentsInc/missing-repo" }, ok: false, status: 404 }),
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.kind).toBe("not_found")
  })

  test("URL-encodes owner/name path segments", async () => {
    let capturedUrl = ""
    const fetchImpl: KhalaMobileReposFetchLike = (async (url: string) => {
      capturedUrl = url
      return { json: async () => ({ repository: sampleRepo }), ok: true }
    }) as KhalaMobileReposFetchLike
    await fetchKhalaMobileRepository("https://openagents.com", "tok", "some owner", "some/name", fetchImpl)
    expect(capturedUrl).toBe("https://openagents.com/api/mobile/repos/some%20owner/some%2Fname")
  })
})
