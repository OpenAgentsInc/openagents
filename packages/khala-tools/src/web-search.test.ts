import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  createWebSearchTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  KhalaToolRuntimeError,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaPermissionRequest,
  type KhalaPermissionService,
  type KhalaWebSearchInput,
  type KhalaWebSearchResult,
  type KhalaWebSearchService,
} from "./index.js"

type SearchUi = Readonly<{
  domains: ReadonlyArray<string>
  provider: string
  query: string
  recencyDays: number | null
  resultCount: number
  results: ReadonlyArray<Readonly<{ title: string; url: string; snippet: string }>>
}>

function searchService(handler: (input: KhalaWebSearchInput) => KhalaWebSearchResult): KhalaWebSearchService {
  return {
    marker: "khala.web_search_service",
    search: input => Effect.sync(() => handler(input)),
  }
}

function failingSearchService(code: string, reason: string): KhalaWebSearchService {
  return {
    marker: "khala.web_search_service",
    search: () => Effect.fail(new KhalaToolRuntimeError({ code, reason })),
  }
}

function runSearch(
  args: Readonly<Record<string, unknown>>,
  search?: KhalaWebSearchService,
  permission: KhalaPermissionService = allowAllKhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createWebSearchTool()]),
      { arguments: args, id: "call_1", name: "web_search", sessionId: "s1" },
      makeKhalaToolServices({
        permission,
        ...(search === undefined ? {} : { search }),
      }),
    ),
  )
}

function uiOf(result: Awaited<ReturnType<typeof runSearch>>): SearchUi {
  return result.ui as SearchUi
}

describe("web_search tool", () => {
  test("is denied when network permission is disabled", async () => {
    let called = false
    const service = searchService(() => {
      called = true
      return { provider: "mock", results: [], searchedAtMs: 1 }
    })

    const result = await runSearch({ query: "openagents" }, service, denyAllKhalaPermissionService)

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("permission_denied")
    expect(called).toBe(false)
  })

  test("fails typed-unconfigured when no provider is configured", async () => {
    const result = await runSearch({ query: "openagents" })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("web_search_unconfigured")
  })

  test("returns bounded source-attributed results with query filters", async () => {
    const calls: KhalaWebSearchInput[] = []
    const service = searchService(input => {
      calls.push(input)
      return {
        provider: "mock-search",
        results: [
          {
            publishedAt: "2026-06-01T00:00:00.000Z",
            snippet: "A first result.",
            title: "First",
            url: "https://example.com/first",
          },
          {
            snippet: "A second result.",
            title: "Second",
            url: "https://docs.example.com/second",
          },
        ],
        searchedAtMs: Date.parse("2026-06-29T12:00:00.000Z"),
      }
    })

    const result = await runSearch({
      domains: ["example.com"],
      query: "Khala tools",
      recency_days: 30,
    }, service)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("1. First")
    expect(result.modelOutput.text).toContain("URL: https://example.com/first")
    expect(result.modelOutput.text).toContain("Published: 2026-06-01T00:00:00.000Z")
    expect(result.publicSummary).toBe("Web search returned 2 results from mock-search at 2026-06-29T12:00:00.000Z.")
    expect(calls[0]).toEqual({
      domains: ["example.com"],
      limit: 5,
      query: "Khala tools",
      recencyDays: 30,
    })
    expect(ui).toMatchObject({
      domains: ["example.com"],
      provider: "mock-search",
      query: "Khala tools",
      recencyDays: 30,
      resultCount: 2,
    })
  })

  test("honors result limits and no-result responses", async () => {
    const many = searchService(() => ({
      provider: "mock-search",
      results: [
        { snippet: "one", title: "One", url: "https://example.com/1" },
        { snippet: "two", title: "Two", url: "https://example.com/2" },
        { snippet: "three", title: "Three", url: "https://example.com/3" },
      ],
      searchedAtMs: 1,
    }))
    const empty = searchService(() => ({ provider: "mock-search", results: [], searchedAtMs: 1 }))

    const limited = await runSearch({ limit: 2, query: "limit" }, many)
    const none = await runSearch({ query: "nothing" }, empty)

    expect(uiOf(limited).results.map(result => result.title)).toEqual(["One", "Two"])
    expect(limited.modelOutput.text).not.toContain("Three")
    expect(none.status).toBe("ok")
    expect(none.modelOutput.text).toContain("No web results")
    expect(uiOf(none).resultCount).toBe(0)
  })

  test("surfaces provider errors with redaction", async () => {
    const result = await runSearch(
      { query: "provider error" },
      failingSearchService("search_provider_error", "provider failed with Bearer abcdefghijklmnopqrstuvwxyz"),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("search_provider_error")
    expect(result.publicSummary).toContain("Bearer [REDACTED_TOKEN]")
  })

  test("redacts private snippets and rejects provider credential args", async () => {
    const service = searchService(() => ({
      provider: "mock-search",
      results: [
        {
          snippet: "Bearer abcdefghijklmnopqrstuvwxyz",
          title: "Secret result",
          url: "https://example.com/secret",
        },
      ],
      searchedAtMs: 1,
    }))

    const fetched = await runSearch({ query: "secret result" }, service)
    const smuggled = await runSearch({
      provider_key: "sk-or-secretsecret",
      query: "secret result",
    }, service)

    expect(fetched.modelOutput.text).toContain("Bearer [REDACTED_TOKEN]")
    expect(fetched.publicSummary).not.toContain("abcdefghijklmnopqrstuvwxyz")
    expect(smuggled.status).toBe("failed")
    expect(smuggled.publicSummary).toContain("provider_key")
    expect(smuggled.publicSummary).not.toContain("sk-or-secretsecret")
  })

  test("uses network authority rather than shell or filesystem permission", async () => {
    const requests: KhalaPermissionRequest[] = []
    const permission: KhalaPermissionService = {
      decide: request => Effect.sync(() => {
        requests.push(request)
        return "allow" as const
      }),
    }
    const service = searchService(() => ({ provider: "mock-search", results: [], searchedAtMs: 1 }))

    await runSearch({ query: "authority boundary" }, service, permission)

    expect(requests[0]).toMatchObject({
      action: "network",
      resources: ["authority boundary"],
      toolName: "web_search",
    })
  })

  test("is only in the optional network preset by default", () => {
    const registry = makeKhalaToolRegistry([createWebSearchTool()])

    expect(registry.materialize("coding").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("inspect").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("network").map(tool => tool.name)).toEqual(["web_search"])
  })
})
