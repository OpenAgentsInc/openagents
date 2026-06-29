import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  createFetchKhalaNetworkService,
  createWebFetchTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaPermissionService,
} from "./index.js"

type WebFetchUi = Readonly<{
  artifactRef: string | null
  binary: boolean
  bodyBytes: number
  bodyTruncated: boolean
  finalUrl: string
  redirectChain: ReadonlyArray<Readonly<{ from: string; status: number; to: string }>>
  status: number
  textPreview: string | null
}>

function runWebFetch(
  args: Readonly<Record<string, unknown>>,
  fetchImpl: typeof fetch,
  permission: KhalaPermissionService = allowAllKhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createWebFetchTool()]),
      { arguments: args, id: "call_1", name: "web_fetch", sessionId: "s1" },
      makeKhalaToolServices({
        network: createFetchKhalaNetworkService(fetchImpl),
        permission,
      }),
    ),
  )
}

function uiOf(result: Awaited<ReturnType<typeof runWebFetch>>): WebFetchUi {
  return result.ui as WebFetchUi
}

describe("web_fetch tool", () => {
  test("is denied when network permission is disabled", async () => {
    let called = false
    const fetchImpl = (async () => {
      called = true
      return new Response("should not fetch")
    }) as unknown as typeof fetch

    const result = await runWebFetch({ url: "https://example.com/" }, fetchImpl, denyAllKhalaPermissionService)

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("permission_denied")
    expect(called).toBe(false)
  })

  test("fetches text responses with structured metadata", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/": new Response("hello web", {
        headers: { "content-type": "text/plain; charset=utf-8" },
        status: 200,
        statusText: "OK",
      }),
    })

    const result = await runWebFetch({ url: "https://example.com/" }, fetchImpl)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("hello web")
    expect(result.publicSummary).not.toContain("hello web")
    expect(ui).toMatchObject({
      binary: false,
      finalUrl: "https://example.com/",
      status: 200,
      textPreview: "hello web",
    })
  })

  test("follows bounded redirects", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/start": new Response("", {
        headers: { location: "/final" },
        status: 302,
      }),
      "https://example.com/final": new Response("done", {
        headers: { "content-type": "text/plain" },
        status: 200,
      }),
    })

    const result = await runWebFetch({ url: "https://example.com/start", max_redirects: 2 }, fetchImpl)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(ui.finalUrl).toBe("https://example.com/final")
    expect(ui.redirectChain).toEqual([
      { from: "https://example.com/start", status: 302, to: "https://example.com/final" },
    ])
  })

  test("returns timeout failures", async () => {
    const fetchImpl = ((_url: string | URL | Request, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => reject(new Error("aborted")))
      })) as typeof fetch

    const result = await runWebFetch({ timeout_ms: 1, url: "https://example.com/slow" }, fetchImpl)

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("network fetch timed out")
  })

  test("marks non-200 responses as failed while preserving metadata", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/missing": new Response("not found", {
        headers: { "content-type": "text/plain" },
        status: 404,
        statusText: "Not Found",
      }),
    })

    const result = await runWebFetch({ url: "https://example.com/missing" }, fetchImpl)
    const ui = uiOf(result)

    expect(result.status).toBe("failed")
    expect(result.modelOutput.text).toContain("not found")
    expect(ui.status).toBe(404)
  })

  test("truncates large responses and spills a private artifact", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/large": new Response("abcdefghijklmnopqrstuvwxyz", {
        headers: { "content-type": "text/plain" },
        status: 200,
      }),
    })

    const result = await runWebFetch({ max_bytes: 8, url: "https://example.com/large" }, fetchImpl)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.artifacts).toHaveLength(1)
    expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
    expect(result.modelOutput.text).toContain("[web_fetch body truncated; see private artifact]")
    expect(ui.bodyTruncated).toBe(true)
    expect(ui.artifactRef).toBe(result.artifacts[0]?.artifactRef)
  })

  test("handles binary responses as private artifacts", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/file.bin": new Response(new Uint8Array([0, 1, 2, 3]), {
        headers: { "content-type": "application/octet-stream" },
        status: 200,
      }),
    })

    const result = await runWebFetch({ url: "https://example.com/file.bin" }, fetchImpl)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.artifacts).toHaveLength(1)
    expect(result.modelOutput.text).toContain("Binary response body stored as private artifact")
    expect(ui.binary).toBe(true)
    expect(ui.textPreview).toBeNull()
  })

  test("redacts response secrets and rejects header/authority smuggling args", async () => {
    const fetchImpl = routeFetch({
      "https://example.com/secret": new Response("Bearer abcdefghijklmnopqrstuvwxyz", {
        headers: { "content-type": "text/plain" },
        status: 200,
      }),
    })

    const fetched = await runWebFetch({ url: "https://example.com/secret" }, fetchImpl)
    const smuggled = await runWebFetch({
      headers: { authorization: "Bearer abcdefghijklmnopqrstuvwxyz" },
      url: "https://example.com/secret",
    }, fetchImpl)

    expect(fetched.modelOutput.text).toContain("Bearer [REDACTED_TOKEN]")
    expect(fetched.publicSummary).not.toContain("abcdefghijklmnopqrstuvwxyz")
    expect(smuggled.status).toBe("failed")
    expect(smuggled.publicSummary).toContain("headers")
    expect(smuggled.publicSummary).not.toContain("abcdefghijklmnopqrstuvwxyz")
  })

  test("is only in the optional network preset by default", () => {
    const registry = makeKhalaToolRegistry([createWebFetchTool()])

    expect(registry.materialize("coding").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("inspect").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("network").map(tool => tool.name)).toEqual(["web_fetch"])
  })
})

function routeFetch(routes: Readonly<Record<string, Response>>): typeof fetch {
  return (async (input: string | URL | Request) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
    const response = routes[url]
    if (response === undefined) return new Response("missing route", { status: 599, statusText: "Missing Route" })
    return response
  }) as typeof fetch
}
