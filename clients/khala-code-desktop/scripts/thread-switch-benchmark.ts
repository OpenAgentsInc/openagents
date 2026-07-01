#!/usr/bin/env bun
import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page, type Route } from "playwright"

export const THREAD_SWITCH_BENCHMARK_HARNESS =
  "khala_code_thread_switch_performance_v1"

export type ThreadSwitchBenchmarkOptions = Readonly<{
  cachedResumeDelayMs?: number
  coldResumeDelayMs?: number
  outFile?: string
  prefetchDelayMs?: number
  port?: number
}>

export type ThreadSwitchBenchmarkClickResult = Readonly<{
  cacheHit: boolean
  clickToFullRenderMs: number
  clickToOptimisticRenderMs: number
  fullMessageCount: number
  optimisticMessageCount: number
  routeWallMs: number
  threadId: string
}>

export type ThreadSwitchBenchmarkResult = Readonly<{
  cached: ThreadSwitchBenchmarkClickResult
  cold: ThreadSwitchBenchmarkClickResult
  config: Required<Omit<ThreadSwitchBenchmarkOptions, "outFile">>
  harness: typeof THREAD_SWITCH_BENCHMARK_HARNESS
}>

type ThreadFixture = Readonly<{
  id: string
  name: string
  recencyAt: number
}>

const wait = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

const defaultBenchmarkConfig = (
  options: ThreadSwitchBenchmarkOptions,
): Required<Omit<ThreadSwitchBenchmarkOptions, "outFile">> => ({
  cachedResumeDelayMs: options.cachedResumeDelayMs ?? 15,
  coldResumeDelayMs: options.coldResumeDelayMs ?? 800,
  port: options.port ?? 50021,
  prefetchDelayMs: options.prefetchDelayMs ?? 25,
})

const repoRoot = (): string => resolve(import.meta.dir, "../../..")

const khalaCodeDesktopRoot = (): string =>
  join(repoRoot(), "clients/khala-code-desktop")

const threadFixtures = (): readonly ThreadFixture[] => {
  const now = Date.now()
  return [
    { id: "thread-a", name: "Alpha benchmark", recencyAt: now },
    { id: "thread-b", name: "Beta benchmark", recencyAt: now - 1_000 },
    { id: "thread-c", name: "Gamma benchmark", recencyAt: now - 2_000 },
    { id: "thread-d", name: "Delta benchmark", recencyAt: now - 3_000 },
    { id: "thread-e", name: "Cold benchmark", recencyAt: now - 4_000 },
  ]
}

const baseThread = (
  fixture: ThreadFixture,
): Record<string, unknown> => ({
  id: fixture.id,
  sessionId: fixture.id,
  name: fixture.name,
  preview: `${fixture.name} preview`,
  cwd: khalaCodeDesktopRoot(),
  modelProvider: "openai",
  source: "appServer",
  forkedFromId: null,
  parentThreadId: null,
  createdAt: fixture.recencyAt - 60_000,
  updatedAt: fixture.recencyAt,
  recencyAt: fixture.recencyAt,
  status: { type: "idle" },
  gitInfo: null,
})

const threadSummary = (
  fixture: ThreadFixture,
): Record<string, unknown> => ({
  id: fixture.id,
  sessionId: fixture.id,
  title: fixture.name,
  preview: `${fixture.name} preview`,
  cwd: khalaCodeDesktopRoot(),
  projectLabel: "khala-code-desktop",
  status: "idle",
  statusLabel: "idle",
  modelProvider: "openai",
  source: "appServer",
  forkedFromId: null,
  parentThreadId: null,
  createdAt: fixture.recencyAt - 60_000,
  updatedAt: fixture.recencyAt,
  recencyAt: fixture.recencyAt,
  badges: [],
})

const messagesForThread = (
  threadId: string,
): readonly Record<string, string>[] =>
  Array.from({ length: 120 }, (_, index) => ({
    id: `${threadId}-message-${index}`,
    role: index % 2 === 0 ? "user" : "assistant",
    body: `${threadId} benchmark message ${index}`,
  }))

const benchmarkTokenSummary = {
  auditRows: 0,
  codexStateDbPath: "",
  codexStateTokens: 0,
  leaderboardLabel: "OpenAgents Stats",
  leaderboardSyncedTokens: 0,
  localLedgerPath: "",
  localMessageAuditLedgerPath: "",
  missingUsageTurns: 0,
  ok: true,
  pendingSyncTokens: 0,
  remoteConfigured: false,
  remoteDisabled: false,
  threadId: null,
  totalTokens: 0,
  updatedAt: null,
  usageEventRows: 0,
} as const

const threadResult = (
  fixture: ThreadFixture,
): Record<string, unknown> => ({
  ok: true,
  thread: {
    ...baseThread(fixture),
    turns: [],
  },
  threadId: fixture.id,
  messages: messagesForThread(fixture.id),
})

async function requestArgs(route: Route): Promise<readonly unknown[]> {
  const postData = route.request().postData()
  if (postData === null || postData.trim() === "") return []
  const parsed = JSON.parse(postData) as { args?: readonly unknown[] }
  return parsed.args ?? []
}

async function installBenchmarkRpcMocks(
  page: Page,
  config: Required<Omit<ThreadSwitchBenchmarkOptions, "outFile">>,
): Promise<void> {
  const fixtures = threadFixtures()
  const fixtureById = new Map(fixtures.map(fixture => [fixture.id, fixture]))
  const prefetchedThreadIds = new Set<string>()

  await page.route("**/rpc/*", async route => {
    const method = decodeURIComponent(
      new URL(route.request().url()).pathname.split("/").at(-1) ?? "",
    )
    const args = await requestArgs(route)
    switch (method) {
      case "appInfo":
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({
            app: "Khala Code Desktop",
            observedAt: new Date().toISOString(),
            ok: true,
          }),
        })
        return
      case "codexThreadList":
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({
            ok: true,
            data: fixtures.map(baseThread),
            groups: [{
              key: khalaCodeDesktopRoot(),
              label: "khala-code-desktop",
              threadIds: fixtures.map(fixture => fixture.id),
            }],
            threads: fixtures.map(threadSummary),
          }),
        })
        return
      case "codexThreadRead": {
        const request = args[0] as { threadId?: string } | undefined
        const threadId = request?.threadId ?? "thread-a"
        const fixture = fixtureById.get(threadId)
        if (fixture === undefined) throw new Error(`unknown thread ${threadId}`)
        await wait(config.prefetchDelayMs)
        prefetchedThreadIds.add(threadId)
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify(threadResult(fixture)),
        })
        return
      }
      case "codexThreadResume": {
        const request = args[0] as { threadId?: string } | undefined
        const threadId = request?.threadId ?? "thread-a"
        const fixture = fixtureById.get(threadId)
        if (fixture === undefined) throw new Error(`unknown thread ${threadId}`)
        await wait(
          prefetchedThreadIds.has(threadId)
            ? config.cachedResumeDelayMs
            : config.coldResumeDelayMs,
        )
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify(threadResult(fixture)),
        })
        return
      }
      case "slashCommandList":
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({ commands: [] }),
        })
        return
      case "threadTokenSummary":
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify(benchmarkTokenSummary),
        })
        return
      default:
        await route.fulfill({
          contentType: "application/json",
          status: 500,
          body: JSON.stringify({ error: `unexpected benchmark RPC: ${method}` }),
        })
    }
  })
}

async function clickThreadAndMeasure(
  page: Page,
  threadId: string,
): Promise<ThreadSwitchBenchmarkClickResult> {
  await page.evaluate(() => {
    const api = (globalThis as typeof globalThis & {
      khalaCodeDesktop?: { resetThreadSwitchPerformance?: () => void }
    }).khalaCodeDesktop
    api?.resetThreadSwitchPerformance?.()
  })
  const startedAt = await page.evaluate(() => performance.now())
  await page.locator(`[data-thread-id="${threadId}"] .khala-thread-sidebar-item-row`).click()
  await page.waitForFunction(
    id => {
      const api = (globalThis as typeof globalThis & {
        khalaCodeDesktop?: { threadSwitchPerformance?: () => { latest: unknown } }
      }).khalaCodeDesktop
      const latest = api?.threadSwitchPerformance?.().latest as
        | {
            fullRenderMs?: number
            optimisticRenderMs?: number
            threadId?: string
          }
        | null
        | undefined
      return (
        latest?.threadId === id &&
        typeof latest.optimisticRenderMs === "number" &&
        typeof latest.fullRenderMs === "number"
      )
    },
    threadId,
  )
  await page.locator("#message-list")
    .getByText(`${threadId} benchmark message 119`)
    .waitFor({ timeout: 5_000 })
  const endedAt = await page.evaluate(() => performance.now())
  const latest = await page.evaluate(() => {
    const api = (globalThis as typeof globalThis & {
      khalaCodeDesktop?: {
        threadSwitchPerformance?: () => {
          latest: {
            cacheHit: boolean
            fullMessageCount?: number
            fullRenderMs?: number
            optimisticMessageCount: number
            optimisticRenderMs?: number
            threadId: string
          } | null
        }
      }
    }).khalaCodeDesktop
    return api?.threadSwitchPerformance?.().latest ?? null
  })
  if (latest === null || latest.threadId !== threadId) {
    throw new Error(`No thread switch sample recorded for ${threadId}`)
  }
  return {
    cacheHit: latest.cacheHit,
    clickToFullRenderMs: Number((latest.fullRenderMs ?? 0).toFixed(1)),
    clickToOptimisticRenderMs: Number((latest.optimisticRenderMs ?? 0).toFixed(1)),
    fullMessageCount: latest.fullMessageCount ?? 0,
    optimisticMessageCount: latest.optimisticMessageCount,
    routeWallMs: Number((endedAt - startedAt).toFixed(1)),
    threadId,
  }
}

export async function runThreadSwitchBenchmark(
  options: ThreadSwitchBenchmarkOptions = {},
): Promise<ThreadSwitchBenchmarkResult> {
  const config = defaultBenchmarkConfig(options)
  const server = startViteServer({
    cwd: khalaCodeDesktopRoot(),
    label: "khala-code-thread-switch-benchmark",
    port: config.port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${config.port}/`)
    browser = await chromium.launch({ headless: true })
    const page = await browser.newPage({
      colorScheme: "dark",
      viewport: { height: 800, width: 1280 },
    })
    await installBenchmarkRpcMocks(page, config)
    await page.goto(`http://127.0.0.1:${config.port}/`, {
      waitUntil: "domcontentloaded",
    })
    await page.locator('[data-thread-id="thread-a"] .khala-thread-sidebar-item-row')
      .waitFor({ state: "visible" })
    await page.waitForFunction(() => {
      const api = (globalThis as typeof globalThis & {
        khalaCodeDesktop?: {
          threadSwitchPerformance?: () => { cachedThreadIds: readonly string[] }
        }
      }).khalaCodeDesktop
      return api?.threadSwitchPerformance?.().cachedThreadIds.includes("thread-a") === true
    })

    const cold = await clickThreadAndMeasure(page, "thread-e")
    const cached = await clickThreadAndMeasure(page, "thread-a")
    const result: ThreadSwitchBenchmarkResult = {
      cached,
      cold,
      config,
      harness: THREAD_SWITCH_BENCHMARK_HARNESS,
    }
    if (options.outFile !== undefined) {
      await mkdir(dirname(options.outFile), { recursive: true })
      await writeFile(options.outFile, `${JSON.stringify(result, null, 2)}\n`)
    }
    return result
  } finally {
    if (browser !== null) await browser.close()
    server.kill()
  }
}

type ViteServer = Readonly<{
  kill: () => void
}>

function startViteServer(
  input: Readonly<{ cwd: string; label: string; port: number }>,
): ViteServer {
  const proc = Bun.spawn(
    [
      "bunx",
      "vite",
      "--host",
      "127.0.0.1",
      "--port",
      String(input.port),
      "--strictPort",
    ],
    {
      cwd: input.cwd,
      stderr: "pipe",
      stdout: "pipe",
    },
  )
  void streamServerOutput(input.label, proc.stdout)
  void streamServerOutput(input.label, proc.stderr)
  return {
    kill: () => {
      proc.kill()
    },
  }
}

async function streamServerOutput(
  label: string,
  stream: ReadableStream<Uint8Array>,
): Promise<void> {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  try {
    for (;;) {
      const chunk = await reader.read()
      if (chunk.done) return
      const text = decoder.decode(chunk.value, { stream: true })
      for (const line of text.split("\n")) {
        if (line.trim().length > 0) console.error(`[${label}] ${line}`)
      }
    }
  } catch {
    // Server output is diagnostic only.
  }
}

async function waitForHttp(url: string): Promise<void> {
  const deadline = Date.now() + 30_000
  let lastError: unknown = null
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url)
      if (response.ok) return
      lastError = new Error(`${url} returned ${response.status}`)
    } catch (error) {
      lastError = error
    }
    await wait(250)
  }
  throw new Error(`Timed out waiting for ${url}: ${String(lastError)}`)
}

const argValue = (
  args: readonly string[],
  name: string,
): string | undefined => {
  const index = args.indexOf(name)
  if (index === -1) return undefined
  return args[index + 1]
}

const numberArg = (
  args: readonly string[],
  name: string,
): number | undefined => {
  const value = argValue(args, name)
  if (value === undefined) return undefined
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : undefined
}

if (import.meta.main) {
  const args = Bun.argv.slice(2)
  try {
    const cachedResumeDelayMs = numberArg(args, "--cached-resume-delay-ms")
    const coldResumeDelayMs = numberArg(args, "--cold-resume-delay-ms")
    const outFile = argValue(args, "--out")
    const port = numberArg(args, "--port")
    const prefetchDelayMs = numberArg(args, "--prefetch-delay-ms")
    const options: ThreadSwitchBenchmarkOptions = {
      ...(cachedResumeDelayMs === undefined ? {} : { cachedResumeDelayMs }),
      ...(coldResumeDelayMs === undefined ? {} : { coldResumeDelayMs }),
      ...(outFile === undefined ? {} : { outFile }),
      ...(port === undefined ? {} : { port }),
      ...(prefetchDelayMs === undefined ? {} : { prefetchDelayMs }),
    }
    const result = await runThreadSwitchBenchmark(options)
    console.log(JSON.stringify(result, null, 2))
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : error)
    process.exit(1)
  }
}
