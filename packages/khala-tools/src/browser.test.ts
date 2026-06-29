import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  createBrowserTools,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaBrowserActionInput,
  type KhalaBrowserNavigateInput,
  type KhalaBrowserPageSnapshot,
  type KhalaBrowserReadDomResult,
  type KhalaBrowserReadInput,
  type KhalaBrowserReadTextResult,
  type KhalaBrowserScreenshotInput,
  type KhalaBrowserScreenshotResult,
  type KhalaBrowserService,
  type KhalaBrowserTypeInput,
  type KhalaBrowserWaitInput,
  type KhalaBrowserWaitResult,
  type KhalaPermissionRequest,
  type KhalaPermissionService,
} from "./index.js"

type BrowserUi = Readonly<{
  action: string
  artifactRef?: string | null
  htmlPreview?: string
  kind: string
  met?: boolean
  selector?: string | null
  textPreview?: string
  truncated?: boolean
  url?: string
  waitKind?: string
}>

type FakeBrowserService = KhalaBrowserService & Readonly<{
  calls: ReadonlyArray<Readonly<{ action: string; input: unknown }>>
}>

function fakeBrowserService(input: {
  readonly dom?: string
  readonly text?: string
} = {}): FakeBrowserService {
  const calls: Array<Readonly<{ action: string; input: unknown }>> = []
  const snapshot = (url = "https://app.example/chat"): KhalaBrowserPageSnapshot => ({
    timestampMs: Date.parse("2026-06-29T12:00:00.000Z"),
    title: "Khala Chat",
    url,
  })
  return {
    calls,
    click: args => Effect.sync(() => {
      calls.push({ action: "click", input: args })
      return snapshot()
    }),
    marker: "khala.browser_service",
    navigate: args => Effect.sync(() => {
      calls.push({ action: "navigate", input: args })
      return snapshot(args.url.startsWith("/") ? `https://app.example${args.url}` : args.url)
    }),
    readDom: args => Effect.sync((): KhalaBrowserReadDomResult => {
      calls.push({ action: "readDom", input: args })
      return {
        ...snapshot(),
        html: input.dom ?? "<main><h1>Khala</h1></main>",
      }
    }),
    readText: args => Effect.sync((): KhalaBrowserReadTextResult => {
      calls.push({ action: "readText", input: args })
      return {
        ...snapshot(),
        text: input.text ?? "Khala ready",
      }
    }),
    screenshot: args => Effect.sync((): KhalaBrowserScreenshotResult => {
      calls.push({ action: "screenshot", input: args })
      return {
        ...snapshot(),
        bytes: new Uint8Array([137, 80, 78, 71]),
        height: 480,
        mediaType: "image/png",
        width: 640,
      }
    }),
    typeText: args => Effect.sync(() => {
      calls.push({ action: "typeText", input: args })
      return snapshot()
    }),
    waitFor: args => Effect.sync((): KhalaBrowserWaitResult => {
      calls.push({ action: "waitFor", input: args })
      return {
        ...snapshot(),
        met: true,
      }
    }),
  }
}

function runBrowserTool(
  name: string,
  args: Readonly<Record<string, unknown>>,
  browser?: KhalaBrowserService,
  permission: KhalaPermissionService = allowAllKhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry(createBrowserTools()),
      { arguments: args, id: "call_1", name, sessionId: "s1" },
      makeKhalaToolServices({
        permission,
        ...(browser === undefined ? {} : { browser }),
      }),
    ),
  )
}

function uiOf(result: Awaited<ReturnType<typeof runBrowserTool>>): BrowserUi {
  return result.ui as BrowserUi
}

describe("browser tools", () => {
  test("return unavailable when no browser surface is configured", async () => {
    const result = await runBrowserTool("browser_navigate", { url: "https://example.com/" })

    expect(result.status).toBe("unavailable")
    expect(result.publicSummary).toContain("browser_unavailable")
  })

  test("use browser authority and do not run when permission is denied", async () => {
    const browser = fakeBrowserService()
    const requests: KhalaPermissionRequest[] = []
    const permission: KhalaPermissionService = {
      decide: request => Effect.sync(() => {
        requests.push(request)
        return "deny" as const
      }),
    }

    const result = await runBrowserTool("browser_navigate", { url: "https://example.com/" }, browser, permission)

    expect(result.status).toBe("denied")
    expect(browser.calls).toHaveLength(0)
    expect(requests[0]).toMatchObject({
      action: "browser",
      resources: ["https://example.com/"],
      toolName: "browser_navigate",
    })
  })

  test("navigates with structured private browser metadata", async () => {
    const browser = fakeBrowserService()

    const result = await runBrowserTool("browser_navigate", { timeout_ms: 5_000, url: "/chat" }, browser)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.publicSummary).toBe("Browser navigate completed at 2026-06-29T12:00:00.000Z.")
    expect(ui).toMatchObject({
      action: "navigate",
      kind: "browser",
      url: "https://app.example/chat",
    })
    expect(browser.calls[0]?.input as KhalaBrowserNavigateInput).toEqual({
      timeoutMs: 5_000,
      url: "/chat",
    })
  })

  test("clicks and types without echoing typed text in public summaries", async () => {
    const browser = fakeBrowserService()

    const clicked = await runBrowserTool("browser_click", { label: "Send", selector: "[data-send]" }, browser)
    const typed = await runBrowserTool("browser_type", {
      label: "Composer",
      selector: "textarea",
      text: "Bearer abcdefghijklmnopqrstuvwxyz",
    }, browser)

    expect(clicked.status).toBe("ok")
    expect(typed.status).toBe("ok")
    expect(typed.publicSummary).not.toContain("abcdefghijklmnopqrstuvwxyz")
    expect(typed.modelOutput.text).toContain("Typed 33 characters")
    expect(browser.calls[0]?.input as KhalaBrowserActionInput).toMatchObject({ selector: "[data-send]" })
    expect(browser.calls[1]?.input as KhalaBrowserTypeInput).toMatchObject({ selector: "textarea" })
  })

  test("waits on explicit browser conditions", async () => {
    const browser = fakeBrowserService()

    const result = await runBrowserTool("browser_wait_for", {
      kind: "selector-visible",
      selector: "[data-ready]",
      timeout_ms: 2_000,
    }, browser)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(ui).toMatchObject({
      action: "wait_for",
      kind: "browser_wait",
      met: true,
      selector: "[data-ready]",
      waitKind: "selector-visible",
    })
    expect(browser.calls[0]?.input as KhalaBrowserWaitInput).toEqual({
      kind: "selector-visible",
      selector: "[data-ready]",
      timeoutMs: 2_000,
    })
  })

  test("bounds visible text output and spills truncated text to a private artifact", async () => {
    const browser = fakeBrowserService({ text: "x".repeat(300) })

    const result = await runBrowserTool("browser_read_text", {
      max_output_tokens: 1,
      selector: "main",
    }, browser)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("[browser text truncated; see private artifact")
    expect(result.modelOutput.text).not.toContain("x".repeat(300))
    expect(result.artifacts).toHaveLength(1)
    expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
    expect(ui).toMatchObject({
      action: "read_text",
      artifactRef: result.artifacts[0]?.artifactRef,
      selector: "main",
      truncated: true,
    })
    expect(browser.calls[0]?.input as KhalaBrowserReadInput).toEqual({ selector: "main" })
  })

  test("stores raw DOM as a private artifact while returning a bounded preview", async () => {
    const browser = fakeBrowserService({ dom: `<main>${"d".repeat(300)}</main>` })

    const result = await runBrowserTool("browser_read_dom", {
      max_output_tokens: 1,
    }, browser)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.artifacts).toHaveLength(1)
    expect(result.artifacts[0]?.mediaType).toBe("text/html; charset=utf-8")
    expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
    expect(result.modelOutput.text).toContain("[browser DOM truncated; see private artifact")
    expect(ui).toMatchObject({
      action: "read_dom",
      artifactRef: result.artifacts[0]?.artifactRef,
      kind: "browser_dom",
      truncated: true,
    })
  })

  test("captures screenshots as private image artifacts", async () => {
    const browser = fakeBrowserService()

    const result = await runBrowserTool("browser_screenshot", { label: "chat screen" }, browser)
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.artifacts).toHaveLength(1)
    expect(result.artifacts[0]).toMatchObject({
      mediaType: "image/png",
      private: true,
    })
    expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
    expect(ui).toMatchObject({
      action: "screenshot",
      artifactRef: result.artifacts[0]?.artifactRef,
      kind: "browser_screenshot",
    })
    expect(browser.calls[0]?.input as KhalaBrowserScreenshotInput).toEqual({ label: "chat screen" })
  })

  test("rejects browser credential and authority smuggling args", async () => {
    const result = await runBrowserTool("browser_click", {
      selector: "button",
      network_permission: true,
    }, fakeBrowserService())

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("network_permission")
  })

  test("is only in the optional browser preset by default", () => {
    const registry = makeKhalaToolRegistry(createBrowserTools())

    expect(registry.materialize("coding").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("inspect").map(tool => tool.name)).toEqual([])
    expect(registry.materialize("browser").map(tool => tool.name)).toEqual([
      "browser_navigate",
      "browser_click",
      "browser_type",
      "browser_read_text",
      "browser_read_dom",
      "browser_wait_for",
      "browser_screenshot",
    ])
  })

  test("honors the stock deny-all permission service", async () => {
    const browser = fakeBrowserService()

    const result = await runBrowserTool("browser_screenshot", {}, browser, denyAllKhalaPermissionService)

    expect(result.status).toBe("denied")
    expect(browser.calls).toHaveLength(0)
  })
})
