#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"

export type Part2VisualViewport = Readonly<{
  name: "desktop" | "mobile"
  width: number
  height: number
}>

type Rect = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

export type Part2VisualGeometry = Readonly<{
  graph: Rect
  gymPanel: Rect
  loadedState: Rect
  parameters: Rect
  viewport: Rect
}>

export type Part2VisualCaptureResult = Readonly<{
  geometry: Part2VisualGeometry
  loadedText: string
  parametersText: string
  screenshot: string
  viewport: Part2VisualViewport["name"]
}>

export const PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS =
  "khala_code_part2_fleet_gym_visual_smoke"

export const part2FleetGymVisualPlan = (): ReadonlyArray<Part2VisualViewport> => [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 390, height: 844 },
]

export const assertPart2VisualGeometry = (
  geometry: Part2VisualGeometry,
): void => {
  assertVisibleRect("Gym panel", geometry.gymPanel, geometry.viewport)
  assertVisibleRect("loaded proof state", geometry.loadedState, geometry.viewport)
  assertVisibleRect("Gym graph", geometry.graph, geometry.viewport)
  assertVisibleRect("active parameters", geometry.parameters, geometry.viewport)

  if (geometry.loadedState.width < 240 || geometry.parameters.width < 240) {
    throw new Error("Part 2 proof cards are too narrow for stable reading")
  }
  if (rectsOverlap(geometry.loadedState, geometry.parameters)) {
    throw new Error("Part 2 loaded proof and active parameters overlap")
  }
}

async function runPart2FleetGymVisualSmoke(
  options: Readonly<{
    keepServer?: boolean
    outDir: string
  }>,
): Promise<ReadonlyArray<Part2VisualCaptureResult>> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const port = 50024
  const server = startViteServer({
    cwd: join(repoRoot, "clients/khala-code-desktop"),
    label: "khala-code-desktop-part2",
    port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${port}/`)
    browser = await chromium.launch({ headless: true })
    const results: Part2VisualCaptureResult[] = []
    for (const viewport of part2FleetGymVisualPlan()) {
      const page = await browser.newPage({
        colorScheme: "dark",
        reducedMotion: viewport.name === "mobile" ? "reduce" : "no-preference",
        viewport: { height: viewport.height, width: viewport.width },
      })
      try {
        results.push(
          await capturePart2FleetGym(page, {
            baseUrl: `http://127.0.0.1:${port}`,
            outDir: options.outDir,
            viewport,
          }),
        )
      } finally {
        await page.close()
      }
    }
    await writeFile(
      join(options.outDir, "summary.json"),
      `${JSON.stringify({
        harness: PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
        results,
      }, null, 2)}\n`,
    )
    return results
  } finally {
    if (browser !== null) await browser.close()
    if (options.keepServer !== true) server.kill()
  }
}

async function capturePart2FleetGym(
  page: Page,
  input: Readonly<{
    baseUrl: string
    outDir: string
    viewport: Part2VisualViewport
  }>,
): Promise<Part2VisualCaptureResult> {
  await page.goto(`${input.baseUrl}/`, { waitUntil: "domcontentloaded" })
  await page.locator('[data-hotbar-action="action_bar.slot_3"]').click()
  await page.locator("#fleet-panel").waitFor({ state: "visible" })
  await page
    .locator(".khala-fleet-optimization button")
    .filter({ hasText: "Optimize delegation policy" })
    .click()
  await page.locator("#gym-panel").waitFor({ state: "visible" })
  await page.locator(".khala-gym-state[data-state=\"loaded\"]").waitFor({
    state: "visible",
  })
  await page.locator(".khala-gym-parameters").waitFor({ state: "visible" })
  await page.waitForTimeout(input.viewport.name === "mobile" ? 150 : 250)

  const capture = await page.evaluate(() => {
    const rectFor = (selector: string): Rect => {
      const element = document.querySelector(selector)
      if (!(element instanceof HTMLElement) && !(element instanceof SVGElement)) {
        throw new Error(`missing Part 2 visual selector: ${selector}`)
      }
      const rect = element.getBoundingClientRect()
      return {
        height: rect.height,
        width: rect.width,
        x: rect.x,
        y: rect.y,
      }
    }
    const loaded = document.querySelector(".khala-gym-state[data-state=\"loaded\"]")
    const parameters = document.querySelector(".khala-gym-parameters")
    return {
      geometry: {
        graph: rectFor(".khala-gym-graph"),
        gymPanel: rectFor("#gym-panel"),
        loadedState: rectFor(".khala-gym-state[data-state=\"loaded\"]"),
        parameters: rectFor(".khala-gym-parameters"),
        viewport: {
          height: window.innerHeight,
          width: window.innerWidth,
          x: 0,
          y: 0,
        },
      },
      loadedText: loaded?.textContent ?? "",
      parametersText: parameters?.textContent ?? "",
    }
  })
  assertPart2VisualGeometry(capture.geometry)
  assertTextIncludes(capture.loadedText, "gated_proposal_ready")
  assertTextIncludes(capture.loadedText, "10000 bps")
  assertTextIncludes(capture.parametersText, "parameters.khala_fleet_delegation.default.v1")
  assertTextIncludes(capture.parametersText, "source")
  assertTextIncludes(capture.parametersText, "default")

  const screenshot = join(
    input.outDir,
    `khala-code-part2-fleet-gym-${input.viewport.name}.png`,
  )
  await mkdir(dirname(screenshot), { recursive: true })
  await page.screenshot({ fullPage: false, path: screenshot })

  return {
    geometry: capture.geometry,
    loadedText: capture.loadedText,
    parametersText: capture.parametersText,
    screenshot,
    viewport: input.viewport.name,
  }
}

const assertVisibleRect = (
  label: string,
  rect: Rect,
  viewport: Rect,
): void => {
  if (rect.width < 1 || rect.height < 1) {
    throw new Error(`${label} is not visible`)
  }
  if (rect.x < -1 || rect.y < -1) {
    throw new Error(`${label} is clipped outside the viewport`)
  }
  if (rect.x + rect.width > viewport.width + 1) {
    throw new Error(`${label} overflows the viewport width: ${JSON.stringify({ rect, viewport })}`)
  }
}

const rectsOverlap = (left: Rect, right: Rect): boolean =>
  left.x < right.x + right.width - 1 &&
  right.x < left.x + left.width - 1 &&
  left.y < right.y + right.height - 1 &&
  right.y < left.y + left.height - 1

const assertTextIncludes = (text: string, expected: string): void => {
  if (!text.includes(expected)) {
    throw new Error(`Part 2 visual smoke missing text: ${expected}`)
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
    await Bun.sleep(250)
  }
  throw new Error(`Timed out waiting for ${url}: ${String(lastError)}`)
}

if (import.meta.main) {
  const outDir =
    argValue(Bun.argv.slice(2), "--out") ??
    resolve("var/khala-code-desktop/part2-fleet-gym-visual-smoke")
  try {
    const results = await runPart2FleetGymVisualSmoke({ outDir })
    console.log(JSON.stringify({
      harness: PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
      ok: true,
      outDir,
      results,
    }, null, 2))
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : error)
    process.exit(1)
  }
}

function argValue(args: ReadonlyArray<string>, name: string): string | undefined {
  const index = args.indexOf(name)
  if (index === -1) return undefined
  return args[index + 1]
}
