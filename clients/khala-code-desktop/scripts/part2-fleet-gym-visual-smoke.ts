#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { basename, dirname, join, relative, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import {
  assertKhalaQaVisibleRect as assertVisibleRect,
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  khalaQaRectsOverlap as rectsOverlap,
  startKhalaQaViteServer as startViteServer,
  waitForKhalaQaHttp as waitForHttp,
} from "@openagentsinc/khala-qa-harness/desktop-smoke-helpers"
import {
  assertKhalaVisualBaseline,
  type KhalaVisualBaselineResult,
} from "@openagentsinc/khala-qa-harness/visual-baseline"
import {
  defaultKhalaCodeVisualBaselineOptions,
  khalaCodeVisualBaselineOptionsFromArgs,
  type KhalaCodeVisualBaselineOptions,
} from "./visual-baseline-options"
import {
  assertKhalaCodePagePublicSafe,
  assertKhalaCodePublicSafeValue,
} from "./public-safety-oracle"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

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
  visualBaseline: KhalaVisualBaselineResult
  viewport: Part2VisualViewport["name"]
}>

export const PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS =
  "khala_code_part2_fleet_gym_visual_smoke"

export const part2FleetGymVisualPlan = (): ReadonlyArray<Part2VisualViewport> => [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 390, height: 844 },
]

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

export const assertPart2VisualGeometry = (
  geometry: Part2VisualGeometry,
): void => {
  assertVisibleRect("Gym panel", geometry.gymPanel, geometry.viewport)
  assertVisibleRect("Gym graph", geometry.graph, geometry.viewport)

  if (geometry.loadedState.width < 240 || geometry.parameters.width < 240) {
    throw new Error("Part 2 proof cards are too narrow for stable reading")
  }
  if (rectsOverlap(geometry.loadedState, geometry.parameters)) {
    throw new Error("Part 2 loaded proof and active parameters overlap")
  }
  if (
    geometry.loadedState.height <= 0 ||
    geometry.graph.height <= 0 ||
    geometry.parameters.height <= 0
  ) {
    throw new Error("Part 2 proof sections must render with positive height")
  }
  if (
    geometry.loadedState.y > geometry.graph.y ||
    geometry.graph.y > geometry.parameters.y
  ) {
    throw new Error("Part 2 proof sections rendered out of order")
  }
}

async function runPart2FleetGymVisualSmoke(
  options: Readonly<{
    keepServer?: boolean
    outDir: string
    visualBaseline?: KhalaCodeVisualBaselineOptions
  }>,
): Promise<ReadonlyArray<Part2VisualCaptureResult>> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const port = await findAvailablePort(50024, khalaPreviewFallbackPorts(50024))
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
    const visualBaseline = options.visualBaseline ?? defaultKhalaCodeVisualBaselineOptions()
    for (const viewport of part2FleetGymVisualPlan()) {
      const page = await browser.newPage({
        colorScheme: "dark",
        reducedMotion: viewport.name === "mobile" ? "reduce" : "no-preference",
        viewport: { height: viewport.height, width: viewport.width },
      })
      const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
        label: `${PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS}.${viewport.name}`,
      })
      try {
        await installKhalaCodeVisualSmokeRpcMocks(page)
        const result = await capturePart2FleetGym(page, {
          baseUrl: `http://127.0.0.1:${port}`,
          outDir: options.outDir,
          visualBaseline,
          viewport,
        })
        consoleOracle.assertNoUnexpected()
        results.push(result)
      } catch (error) {
        consoleOracle.assertNoUnexpected()
        throw error
      } finally {
        await page.close()
      }
    }
    const summary = {
      harness: PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
      results,
    }
    assertKhalaCodePublicSafeValue(summary, "Part 2 visual smoke summary")
    await writeFile(join(options.outDir, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`)
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
    visualBaseline: KhalaCodeVisualBaselineOptions
    viewport: Part2VisualViewport
  }>,
): Promise<Part2VisualCaptureResult> {
  await page.goto(`${input.baseUrl}/`, { waitUntil: "domcontentloaded" })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').waitFor({
    state: "visible",
  })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').click()
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
  await page.locator("#gym-panel .khala-gym-graph").scrollIntoViewIfNeeded()
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
        graph: rectFor("#gym-panel .khala-gym-graph"),
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
  await assertKhalaCodePagePublicSafe(page, "Part 2 visual smoke")
  assertKhalaCodePublicSafeValue(capture, "Part 2 visual smoke metadata")

  const screenshot = join(
    input.outDir,
    `khala-code-part2-fleet-gym-${input.viewport.name}.png`,
  )
  await mkdir(dirname(screenshot), { recursive: true })
  await page.screenshot({
    animations: "disabled",
    caret: "hide",
    fullPage: false,
    path: screenshot,
  })
  const visualBaseline = await assertKhalaVisualBaseline({
    baselineDir: input.visualBaseline.baselineDir,
    bless: input.visualBaseline.bless,
    capture: {
      colorScheme: "dark",
      harness: PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
      id: `part2-fleet-gym.${input.viewport.name}`,
      reducedMotion: input.viewport.name === "mobile" ? "reduce" : "no-preference",
      screenshotPath: screenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  return {
    geometry: capture.geometry,
    loadedText: capture.loadedText,
    parametersText: capture.parametersText,
    screenshot: basename(screenshot),
    visualBaseline,
    viewport: input.viewport.name,
  }
}

const assertTextIncludes = (text: string, expected: string): void => {
  if (!text.includes(expected)) {
    throw new Error(`Part 2 visual smoke missing text: ${expected}`)
  }
}

if (import.meta.main) {
  const args = Bun.argv.slice(2)
  const outDir =
    argValue(args, "--out") ??
    resolve("var/khala-code-desktop/part2-fleet-gym-visual-smoke")
  try {
    const results = await runPart2FleetGymVisualSmoke({
      outDir,
      visualBaseline: khalaCodeVisualBaselineOptionsFromArgs(args),
    })
    const cliSummary = {
      harness: PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
      ok: true,
      outDir: relative(process.cwd(), outDir) || ".",
      results,
    }
    assertKhalaCodePublicSafeValue(cliSummary, "Part 2 visual smoke CLI summary")
    console.log(JSON.stringify(cliSummary, null, 2))
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
