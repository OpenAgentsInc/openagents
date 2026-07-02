#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import {
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  khalaQaRectsOverlap as rectsOverlap,
  startKhalaQaViteServer as startViteServer,
  waitForKhalaQaHttp as waitForHttp,
  type KhalaQaRect,
} from "@openagentsinc/khala-qa-harness/desktop-smoke-helpers"
import {
  assertKhalaVisualBaseline,
  type KhalaVisualBaselineResult,
} from "@openagentsinc/khala-qa-harness/visual-baseline"

import type {
  KhalaCodeDesktopFleetRunListResult,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetStatus,
} from "../src/shared/rpc"
import {
  defaultKhalaCodeVisualBaselineOptions,
  khalaCodeVisualBaselineOptionsFromArgs,
  type KhalaCodeVisualBaselineOptions,
} from "./visual-baseline-options"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

export type CockpitVisualViewport = Readonly<{
  name: "desktop" | "mobile"
  width: number
  height: number
}>

type CockpitVisualGeometry = Readonly<{
  accountCards: readonly KhalaQaRect[]
  fleetPanel: KhalaQaRect
  gauges: readonly KhalaQaRect[]
  runHeader: KhalaQaRect
  viewport: KhalaQaRect
  workerCards: readonly KhalaQaRect[]
}>

export type CockpitVisualCapture = Readonly<{
  accountCardCount: number
  geometry: CockpitVisualGeometry
  screenshot: string
  visualBaseline: KhalaVisualBaselineResult
  viewport: CockpitVisualViewport["name"]
  workerCardCount: number
}>

export const COCKPIT_VISUAL_SMOKE_HARNESS =
  "khala_code_t5_6_cockpit_visual_smoke"

export const cockpitVisualSmokeViewports = (): readonly CockpitVisualViewport[] => [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 390, height: 844 },
]

export const assertCockpitVisualGeometry = (
  geometry: CockpitVisualGeometry,
): void => {
  assertRect("Fleet panel", geometry.fleetPanel)
  assertRect("Active FleetRun header", geometry.runHeader)
  assertCount("worker cards", geometry.workerCards, 18)
  assertCount("account cards", geometry.accountCards, 3)
  assertCount("throughput gauges", geometry.gauges, 3)

  for (const [index, rect] of geometry.workerCards.entries()) {
    assertRect(`worker card ${index + 1}`, rect)
    assertNoHorizontalClipping(`worker card ${index + 1}`, rect, geometry.viewport)
  }
  for (const [index, rect] of geometry.accountCards.entries()) {
    assertRect(`account card ${index + 1}`, rect)
    assertNoHorizontalClipping(`account card ${index + 1}`, rect, geometry.viewport)
  }
  for (const [index, rect] of geometry.gauges.entries()) {
    assertRect(`throughput gauge ${index + 1}`, rect)
    assertNoHorizontalClipping(`throughput gauge ${index + 1}`, rect, geometry.viewport)
  }
  assertNoListOverlap("worker cards", geometry.workerCards)
  assertNoListOverlap("account cards", geometry.accountCards)
  assertNoListOverlap("throughput gauges", geometry.gauges)
}

export async function runCockpitVisualSmoke(
  options: Readonly<{
    keepServer?: boolean
    outDir: string
    visualBaseline?: KhalaCodeVisualBaselineOptions
  }>,
): Promise<readonly CockpitVisualCapture[]> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const port = await findAvailablePort(50027, khalaPreviewFallbackPorts(50027))
  const server = startViteServer({
    cwd: join(repoRoot, "clients/khala-code-desktop"),
    label: "khala-code-desktop-cockpit",
    port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${port}/`)
    browser = await chromium.launch({ headless: true })
    const captures: CockpitVisualCapture[] = []
    const visualBaseline = options.visualBaseline ?? defaultKhalaCodeVisualBaselineOptions()
    for (const viewport of cockpitVisualSmokeViewports()) {
      const page = await browser.newPage({
        colorScheme: "dark",
        reducedMotion: viewport.name === "mobile" ? "reduce" : "no-preference",
        viewport: { height: viewport.height, width: viewport.width },
      })
      const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
        label: `${COCKPIT_VISUAL_SMOKE_HARNESS}.${viewport.name}`,
      })
      try {
        await installCockpitRpcMocks(page)
        const capture = await captureCockpit(page, {
          baseUrl: `http://127.0.0.1:${port}`,
          outDir: options.outDir,
          visualBaseline,
          viewport,
        })
        consoleOracle.assertNoUnexpected()
        captures.push(capture)
      } catch (error) {
        consoleOracle.assertNoUnexpected()
        throw error
      } finally {
        await page.close()
      }
    }
    await writeFile(
      join(options.outDir, "summary.json"),
      `${JSON.stringify({
        captures,
        fixture: {
          accounts: 3,
          clock: cockpitClockIso,
          workers: 18,
        },
        harness: COCKPIT_VISUAL_SMOKE_HARNESS,
      }, null, 2)}\n`,
    )
    return captures
  } finally {
    if (browser !== null) await browser.close()
    if (options.keepServer !== true) server.kill()
  }
}

const cockpitClockIso = "2026-07-01T18:15:00.000Z"

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

async function installCockpitRpcMocks(page: Page): Promise<void> {
  await page.addInitScript((iso: string) => {
    const RealDate = Date
    class FixtureDate extends RealDate {
      constructor(value?: string | number | Date) {
        if (arguments.length === 0) {
          super(iso)
        } else {
          super(value as string)
        }
      }
      static override now(): number {
        return new RealDate(iso).getTime()
      }
    }
    Object.defineProperty(FixtureDate, "parse", { value: RealDate.parse })
    Object.defineProperty(FixtureDate, "UTC", { value: RealDate.UTC })
    Object.defineProperty(window, "Date", { configurable: true, value: FixtureDate })
  }, cockpitClockIso)

  await installKhalaCodeVisualSmokeRpcMocks(page, {
    observedAt: cockpitClockIso,
    overrides: {
      appInfo: () => ({
        app: "Khala Code Desktop",
        observedAt: cockpitClockIso,
        ok: true,
      }),
      codexFleetStatus: () => cockpitFleetStatusFixture(),
      fleetRunList: () => cockpitFleetRunListFixture(),
      openExternalUrl: () => true,
    },
  })
}

async function captureCockpit(
  page: Page,
  input: Readonly<{
    baseUrl: string
    outDir: string
    visualBaseline: KhalaCodeVisualBaselineOptions
    viewport: CockpitVisualViewport
  }>,
): Promise<CockpitVisualCapture> {
  await page.goto(`${input.baseUrl}/`, { waitUntil: "domcontentloaded" })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').waitFor({
    state: "visible",
  })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').click()
  await page.locator("#fleet-panel").waitFor({ state: "visible" })
  await expectText(page, "#fleet-panel", "Active FleetRun")
  await expectText(page, "#fleet-panel", "Worker Codex accounts")
  await expectText(page, "#fleet-panel", "18 active")
  await expectText(page, "#fleet-panel", "3 ready")
  await expectText(page, "#fleet-panel", "12/30 Codex slots free")
  await expectText(page, "#fleet-panel", "resets in 45m")
  await expectText(page, "#fleet-panel", "resets in 2d 23h")
  await assertPagePublicSafe(page)

  const geometry = await collectCockpitGeometry(page)
  assertCockpitVisualGeometry(geometry)

  const screenshot = join(
    input.outDir,
    `khala-code-cockpit-${input.viewport.name}.png`,
  )
  await mkdir(dirname(screenshot), { recursive: true })
  await page.screenshot({
    animations: "disabled",
    caret: "hide",
    fullPage: true,
    path: screenshot,
  })
  const visualBaseline = await assertKhalaVisualBaseline({
    baselineDir: input.visualBaseline.baselineDir,
    bless: input.visualBaseline.bless,
    capture: {
      colorScheme: "dark",
      harness: COCKPIT_VISUAL_SMOKE_HARNESS,
      id: `cockpit.${input.viewport.name}`,
      reducedMotion: input.viewport.name === "mobile" ? "reduce" : "no-preference",
      screenshotPath: screenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  return {
    accountCardCount: geometry.accountCards.length,
    geometry,
    screenshot,
    visualBaseline,
    viewport: input.viewport.name,
    workerCardCount: geometry.workerCards.length,
  }
}

const collectCockpitGeometry = async (page: Page): Promise<CockpitVisualGeometry> =>
  page.evaluate(() => {
    const rectFor = (selector: string): KhalaQaRect => {
      const element = document.querySelector(selector)
      if (!(element instanceof HTMLElement) && !(element instanceof SVGElement)) {
        throw new Error(`missing cockpit visual selector: ${selector}`)
      }
      return toRect(element.getBoundingClientRect())
    }
    const rectsFor = (selector: string): KhalaQaRect[] =>
      [...document.querySelectorAll(selector)]
        .filter((element): element is HTMLElement | SVGElement =>
          element instanceof HTMLElement || element instanceof SVGElement,
        )
        .map(element => toRect(element.getBoundingClientRect()))
    const toRect = (rect: DOMRect): KhalaQaRect => ({
      height: rect.height,
      width: rect.width,
      x: rect.x,
      y: rect.y + window.scrollY,
    })
    return {
      accountCards: rectsFor(".khala-fleet-account"),
      fleetPanel: rectFor("#fleet-panel"),
      gauges: rectsFor(".khala-fleet-throughput-gauge"),
      runHeader: rectFor(".khala-fleet-run-header"),
      viewport: {
        height: document.documentElement.scrollHeight,
        width: window.innerWidth,
        x: 0,
        y: 0,
      },
      workerCards: rectsFor(".khala-fleet-worker-card"),
    }
  })

const assertRect = (label: string, rect: KhalaQaRect): void => {
  if (rect.width < 1 || rect.height < 1) {
    throw new Error(`${label} is not visible`)
  }
}

const assertNoHorizontalClipping = (
  label: string,
  rect: KhalaQaRect,
  viewport: KhalaQaRect,
): void => {
  if (rect.x < -1 || rect.x + rect.width > viewport.width + 1) {
    throw new Error(`${label} is horizontally clipped: ${JSON.stringify({ rect, viewport })}`)
  }
}

const assertCount = (
  label: string,
  rects: readonly KhalaQaRect[],
  expected: number,
): void => {
  if (rects.length !== expected) {
    throw new Error(`expected ${expected} ${label}, rendered ${rects.length}`)
  }
}

const assertNoListOverlap = (
  label: string,
  rects: readonly KhalaQaRect[],
): void => {
  for (let left = 0; left < rects.length; left += 1) {
    for (let right = left + 1; right < rects.length; right += 1) {
      if (rectsOverlap(rects[left]!, rects[right]!)) {
        throw new Error(`${label} overlap: ${left + 1} and ${right + 1}`)
      }
    }
  }
}

const expectText = async (
  page: Page,
  selector: string,
  expected: string,
): Promise<void> => {
  await page.waitForFunction(
    ({ selector: targetSelector, expected: targetExpected }) =>
      document.querySelector(targetSelector)?.textContent?.includes(targetExpected) === true,
    { expected, selector },
  )
}

const unsafeTextPattern =
  /\/Users\/|\/home\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace|log|provider)|secret|sk-[a-z0-9]/i

const assertPagePublicSafe = async (page: Page): Promise<void> => {
  const text = await page.locator("body").textContent()
  if (unsafeTextPattern.test(text ?? "")) {
    throw new Error("Cockpit visual smoke rendered private or raw material")
  }
}

const cockpitFleetRunListFixture = (): KhalaCodeDesktopFleetRunListResult => ({
  ok: true,
  runs: [cockpitFleetRunFixture()],
})

const cockpitFleetRunFixture = (): KhalaCodeDesktopFleetRunProjection => ({
  counters: {
    activeAssignments: 18,
    blockedAssignments: 2,
    completedAssignments: 42,
    failedAssignments: 0,
    workUnitsTotal: 80,
  },
  createdAt: "2026-07-01T18:00:00.000Z",
  dispatchKind: "supervised_dispatch",
  objectiveProjected: false,
  pylonRef: "pylon.local.cockpit",
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 10,
    stopCondition: "backlog_empty",
  },
  runRef: "fleet.run.public.t5_6_cockpit_fixture",
  startedAt: "2026-07-01T18:00:00.000Z",
  state: "running",
  targetConcurrency: 18,
  updatedAt: cockpitClockIso,
  workerKind: "codex",
  workSource: { kind: "fixture" },
})

const cockpitFleetStatusFixture = (): KhalaCodeDesktopFleetStatus => ({
  activeAssignments: Array.from({ length: 18 }, (_, index) => {
    const worker = index + 1
    const blocked = worker === 7 || worker === 14
    return {
      assignmentRef: `assignment.public.t5_6_cockpit.${worker.toString().padStart(2, "0")}`,
      blockerRefs: blocked ? [`blocker.public.fixture.${worker}`] : [],
      closeoutStatus: null,
      elapsedMs: 60_000 + worker * 7_000,
      issueRef: `github.issue.openagents.${7800 + worker}`,
      runRef: "fleet.run.public.t5_6_cockpit_fixture",
      tokenRate: {
        source: worker % 3 === 0 ? "token_usage_events" : "fleet.activeAssignments.tokensSoFar",
        status: worker % 3 === 0 ? "exact" : "pending",
        tokenCountKind: worker % 3 === 0 ? "total" : null,
        tokens: worker % 3 === 0 ? 1_200 + worker : null,
        tokensPerMinute: worker % 3 === 0 ? 240 + worker : null,
      },
      updatedAt: cockpitClockIso,
      workerSession: {
        approvalState: blocked ? "blocked" : "none",
        blockerRefs: blocked ? [`blocker.public.fixture.${worker}`] : [],
        closeoutStatus: null,
        executionRuntime: "codex_harness",
        homeRole: "pylon_isolated_worker_codex_home",
        queuePolicy: {
          admission: "pylon_capacity_gate",
          cooldown: worker % 6 === 0 ? "cooling_down" : "ready",
          refill: "pylon_presence_heartbeat",
          queued: worker % 4 === 0 ? 1 : 0,
        },
        reviewState: blocked ? "blocked" : "active",
        role: "swarm_worker_codex_session",
        transcriptRef: `transcript.public.t5_6_cockpit.${worker}`,
      },
    }
  }),
  accounts: [
    accountFixture("codex", 4, 6, 0, 0, 25, 75, 1),
    accountFixture("codex-2", 5, 6, 1, 0, 42, 58, 0),
    accountFixture("codex-3", 3, 6, 1, 2, 63, 37, 2),
  ],
  availableCodexAssignments: 12,
  maxCodexAssignments: 30,
  observedAt: cockpitClockIso,
  ok: true,
  processes: [],
  pylon: {
    message: "Fixture Pylon ready with 18 active Codex workers",
    pylonRef: "pylon.local.cockpit",
    status: "online",
  },
  sessionLayers: {
    main: {
      homeRole: "main_user_codex_home_display_only",
      label: "Primary user Codex session",
      mutationPolicy: "codex_app_server_owned",
      role: "main_local_codex_session",
      runtime: "codex_harness",
      transcriptSurface: "chat",
    },
    workers: {
      homeRole: "pylon_isolated_worker_codex_home",
      label: "Khala swarm worker Codex sessions",
      mutationPolicy: "pylon_isolated_home_only",
      role: "swarm_worker_codex_session",
      runtime: "codex_harness",
      transcriptSurface: "fleet",
    },
  },
  tokenRate: {
    activeAdjustedTokensPerMinute: 12_400,
    completedStatus: "exact",
    completedTokenRows: 42,
    completedTokensPerMinute: 8_640,
    inFlightTokens: 28_000,
    inFlightTokensPerMinute: 3_760,
    source: "pylon_khala_apm",
    tokensWindow: 86_400,
    unavailableReason: null,
  },
})

const accountFixture = (
  accountRef: string,
  available: number,
  ready: number,
  busy: number,
  queued: number,
  usedPercent: number,
  remainingPercent: number,
  resetCredits: number,
): KhalaCodeDesktopFleetStatus["accounts"][number] => ({
  accountKey: `account.public.${accountRef}`,
  accountRef,
  capacity: {
    available,
    busy,
    queued,
    ready,
  },
  email: null,
  homeRole: "pylon_isolated_worker_codex_home",
  paused: false,
  provider: "codex",
  queuePolicy: {
    admission: "pylon_capacity_gate",
    cooldown: queued > 0 ? "cooling_down" : "ready",
    refill: "pylon_presence_heartbeat",
    queued,
  },
  quotaState: queued > 0 ? "cooling_down" : "available",
  rateLimits: {
    error: null,
    provider: "codex",
    rateLimitResetCredits: {
      availableCount: resetCredits,
      nextExpiresAtIso: "2026-07-02T18:00:00.000Z",
      totalEarnedCount: resetCredits,
    },
    session: {
      remainingPercent,
      resetDescription: null,
      resetsAtIso: "2026-07-01T19:00:00.000Z",
      usedPercent,
      windowMinutes: 300,
    },
    status: "ok",
    updatedAtIso: cockpitClockIso,
    weekly: {
      remainingPercent: Math.max(0, remainingPercent - 12),
      resetDescription: null,
      resetsAtIso: "2026-07-04T17:15:00.000Z",
      usedPercent: Math.min(100, usedPercent + 12),
      windowMinutes: 10_080,
    },
  },
  readiness: "ready",
  sessionRole: "swarm_worker_codex_session",
})

if (import.meta.main) {
  const args = Bun.argv.slice(2)
  const outDir =
    argValue(args, "--out") ??
    resolve("var/khala-code-desktop/cockpit-visual-smoke")
  try {
    const captures = await runCockpitVisualSmoke({
      outDir,
      visualBaseline: khalaCodeVisualBaselineOptionsFromArgs(args),
    })
    console.log(JSON.stringify({
      captures,
      harness: COCKPIT_VISUAL_SMOKE_HARNESS,
      ok: true,
      outDir,
    }, null, 2))
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : error)
    process.exit(1)
  }
}

function argValue(args: readonly string[], name: string): string | undefined {
  const index = args.indexOf(name)
  if (index === -1) return undefined
  return args[index + 1]
}
