#!/usr/bin/env bun
import { mkdir, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import {
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  startKhalaQaViteServer as startViteServer,
  waitForKhalaQaHttp as waitForHttp,
} from "@openagentsinc/khala-qa-harness/desktop-smoke-helpers"
import {
  assertKhalaVisualBaseline,
  type KhalaVisualBaselineResult,
} from "@openagentsinc/khala-qa-harness/visual-baseline"

import type {
  KhalaCodeDesktopFleetDelegateRunResult,
  KhalaCodeDesktopFleetStatus,
} from "../src/shared/rpc"
import {
  defaultKhalaCodeVisualBaselineOptions,
  khalaCodeVisualBaselineOptionsFromArgs,
  type KhalaCodeVisualBaselineOptions,
} from "./visual-baseline-options"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

export type Part2UiSmokeViewport = Readonly<{
  name: "desktop" | "mobile"
  width: number
  height: number
}>

type Part2UiSmokeStep = Readonly<{
  name: string
  ok: boolean
}>

export type Part2UiSmokeCapture = Readonly<{
  fleetScreenshot: string
  gymScreenshot: string
  steps: ReadonlyArray<Part2UiSmokeStep>
  visualBaselines: Readonly<{
    fleet: KhalaVisualBaselineResult
    gym: KhalaVisualBaselineResult
  }>
  viewport: Part2UiSmokeViewport["name"]
}>

export const PART2_UI_RECORDING_SMOKE_HARNESS =
  "khala_code_transcript_245_part2_ui_smoke"

export const part2UiSmokeViewports = (): ReadonlyArray<Part2UiSmokeViewport> => [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 390, height: 844 },
]

export const part2UiUnsafeTextPattern =
  /\/Users\/|\/home\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace|log|provider)|secret|sk-[a-z0-9]/i

const legacyDeadEndPattern =
  /codex_spawn_failed: No Pylon Codex assignment capacity is available right now|0\/1 available/i

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

export const assertPart2UiPublicSafeText = (text: string): void => {
  if (part2UiUnsafeTextPattern.test(text)) {
    throw new Error("Part 2 UI smoke rendered private or raw material")
  }
  if (legacyDeadEndPattern.test(text)) {
    throw new Error("Part 2 UI smoke regressed to the legacy 0/1 capacity dead-end")
  }
}

async function runPart2UiRecordingSmoke(
  options: Readonly<{
    keepServer?: boolean
    outDir: string
    visualBaseline?: KhalaCodeVisualBaselineOptions
  }>,
): Promise<ReadonlyArray<Part2UiSmokeCapture>> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = resolve(import.meta.dir, "../../..")
  const port = await findAvailablePort(50026, khalaPreviewFallbackPorts(50026))
  const server = startViteServer({
    cwd: join(repoRoot, "clients/khala-code-desktop"),
    label: "khala-code-desktop-part2-ui",
    port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${port}/`)
    browser = await chromium.launch({ headless: true })
    const captures: Part2UiSmokeCapture[] = []
    const visualBaseline = options.visualBaseline ?? defaultKhalaCodeVisualBaselineOptions()
    for (const viewport of part2UiSmokeViewports()) {
      const page = await browser.newPage({
        colorScheme: "dark",
        reducedMotion: viewport.name === "mobile" ? "reduce" : "no-preference",
        viewport: { height: viewport.height, width: viewport.width },
      })
      const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
        label: `${PART2_UI_RECORDING_SMOKE_HARNESS}.${viewport.name}`,
      })
      try {
        await installPart2RpcMocks(page)
        const capture = await capturePart2Ui(page, {
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
        harness: PART2_UI_RECORDING_SMOKE_HARNESS,
        captures,
      }, null, 2)}\n`,
    )
    return captures
  } finally {
    if (browser !== null) await browser.close()
    if (options.keepServer !== true) server.kill()
  }
}

async function installPart2RpcMocks(page: Page): Promise<void> {
  await installKhalaCodeVisualSmokeRpcMocks(page, {
    overrides: {
      codexFleetDelegateRun: ({ args }) => {
        assertDelegateRequestSafe(args)
        return delegateRunResultFixture()
      },
      codexFleetStatus: () => fleetStatusFixture(),
      fleetRunList: () => ({ ok: true, runs: [] }),
      openExternalUrl: () => true,
    },
  })
}

async function capturePart2Ui(
  page: Page,
  input: Readonly<{
    baseUrl: string
    outDir: string
    visualBaseline: KhalaCodeVisualBaselineOptions
    viewport: Part2UiSmokeViewport
  }>,
): Promise<Part2UiSmokeCapture> {
  const steps: Part2UiSmokeStep[] = []
  const mark = (name: string): void => {
    steps.push({ name, ok: true })
  }

  await page.goto(`${input.baseUrl}/`, { waitUntil: "domcontentloaded" })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').waitFor({
    state: "visible",
  })
  await page.locator('[data-khala-code-hotbar-value="fleet"]').click()
  await page.locator("#fleet-panel").waitFor({ state: "visible" })
  await expectText(page, "#fleet-panel", "Delegation optimization")
  await expectText(page, "#fleet-panel", "Worker Codex accounts")
  await expectText(page, "#fleet-panel", "4/5 Codex slots free")
  await assertPagePublicSafe(page)
  mark("fleet hotbar and panel rendered")

  await page.getByRole("button", { name: /Run delegate/i }).click()
  await expectText(page, "#fleet-panel", "khala.fleet.delegate")
  await expectText(page, "#fleet-panel", "1/1 accepted")
  for (const module of [
    "ensure_pylon",
    "advertise_capacity",
    "select_account",
    "prepare_work",
    "dispatch",
    "verify_closeout",
  ]) {
    await expectText(page, "#fleet-panel", module)
  }
  await expectText(page, "#fleet-panel", "Codex capacity advertisement recovered.")
  await expectText(page, "#fleet-panel", "assignment.public.codex_agent_task.part2_demo")
  await assertPagePublicSafe(page)
  mark("deterministic delegate recovery rendered")

  const fleetScreenshot = join(
    input.outDir,
    `part2-ui-fleet-${input.viewport.name}.png`,
  )
  await mkdir(dirname(fleetScreenshot), { recursive: true })
  await page.screenshot({
    animations: "disabled",
    caret: "hide",
    fullPage: false,
    path: fleetScreenshot,
  })
  const fleetVisualBaseline = await assertKhalaVisualBaseline({
    baselineDir: input.visualBaseline.baselineDir,
    bless: input.visualBaseline.bless,
    capture: {
      colorScheme: "dark",
      harness: PART2_UI_RECORDING_SMOKE_HARNESS,
      id: `part2-ui.fleet.${input.viewport.name}`,
      reducedMotion: input.viewport.name === "mobile" ? "reduce" : "no-preference",
      screenshotPath: fleetScreenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  await page
    .locator(".khala-fleet-optimization button")
    .filter({ hasText: "Optimize delegation policy" })
    .click()
  await page.locator("#gym-panel").waitFor({ state: "visible" })
  await page.locator(".khala-gym-state[data-state=\"loaded\"]").waitFor({
    state: "visible",
  })
  await expectText(page, "#gym-panel", "metricValueBps")
  await expectText(page, "#gym-panel", "10000 bps")
  await expectText(page, "#gym-panel", "gated_proposal_ready")
  await expectText(page, "#gym-panel", "decisionGrade")
  await expectText(page, "#gym-panel", "false")
  await expectText(page, "#gym-panel", "candidate manifest")
  await expectText(page, "#gym-panel", "Gym ingest")
  await expectText(page, "#gym-panel", "admission")
  await expectText(page, "#gym-panel", "manifest.khala_fleet_delegation.part2_fixture.v1")
  await expectText(page, "#gym-panel", "candidate.khala_fleet_delegation.part2_fixture.v1")
  await expectText(page, "#gym-panel", "action_submission.proposal.khala_delegation.part2_fixture.v1")
  await expectText(page, "#gym-panel", "Active delegation parameters")
  await expectText(page, "#gym-panel", "parameters.khala_fleet_delegation.default.v1")
  await assertPagePublicSafe(page)
  mark("Gym candidate, ingest, and admission proof rendered")

  const gymScreenshot = join(
    input.outDir,
    `part2-ui-gym-${input.viewport.name}.png`,
  )
  await mkdir(dirname(gymScreenshot), { recursive: true })
  await page.screenshot({
    animations: "disabled",
    caret: "hide",
    fullPage: false,
    path: gymScreenshot,
  })
  const gymVisualBaseline = await assertKhalaVisualBaseline({
    baselineDir: input.visualBaseline.baselineDir,
    bless: input.visualBaseline.bless,
    capture: {
      colorScheme: "dark",
      harness: PART2_UI_RECORDING_SMOKE_HARNESS,
      id: `part2-ui.gym.${input.viewport.name}`,
      reducedMotion: input.viewport.name === "mobile" ? "reduce" : "no-preference",
      screenshotPath: gymScreenshot,
      viewport: input.viewport.name,
    },
    requireBaseline: input.visualBaseline.requireBaseline,
  })

  return {
    fleetScreenshot,
    gymScreenshot,
    steps,
    visualBaselines: {
      fleet: fleetVisualBaseline,
      gym: gymVisualBaseline,
    },
    viewport: input.viewport.name,
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

const assertPagePublicSafe = async (page: Page): Promise<void> => {
  const text = await page.locator("body").textContent()
  assertPart2UiPublicSafeText(text ?? "")
}

const assertDelegateRequestSafe = (args: readonly unknown[]): void => {
  const serialized = JSON.stringify(args)
  if (!serialized.includes("\"mode\":\"fixture\"")) {
    throw new Error("Part 2 UI smoke expected fixture delegation mode")
  }
  assertPart2UiPublicSafeText(serialized)
}

const fleetStatusFixture = (): KhalaCodeDesktopFleetStatus => ({
  activeAssignments: [{
    assignmentRef: "assignment.public.codex_agent_task.part2_demo",
    blockerRefs: [],
    closeoutStatus: "accepted",
    elapsedMs: 12_000,
    issueRef: "github.issue.openagents.7801",
    tokenRate: {
      source: "token_usage_events",
      status: "exact",
      tokenCountKind: "total",
      tokens: 100,
      tokensPerMinute: 500,
    },
    updatedAt: "2026-07-01T00:00:12.000Z",
    workerSession: {
      approvalState: "none",
      blockerRefs: [],
      closeoutStatus: "accepted",
      executionRuntime: "codex_harness",
      homeRole: "pylon_isolated_worker_codex_home",
      queuePolicy: {
        admission: "pylon_capacity_gate",
        cooldown: "ready",
        refill: "pylon_presence_heartbeat",
        queued: 0,
      },
      reviewState: "ready_for_review",
      role: "swarm_worker_codex_session",
      transcriptRef: "transcript.public.part2_demo",
    },
  }],
  accounts: [{
    accountKey: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
    accountRef: "codex-2",
    capacity: {
      available: 4,
      busy: 1,
      queued: 0,
      ready: 5,
    },
    email: null,
    homeRole: "pylon_isolated_worker_codex_home",
    provider: "codex",
    queuePolicy: {
      admission: "pylon_capacity_gate",
      cooldown: "ready",
      refill: "pylon_presence_heartbeat",
      queued: 0,
    },
    quotaState: "available",
    readiness: "ready",
    sessionRole: "swarm_worker_codex_session",
  }],
  availableCodexAssignments: 4,
  maxCodexAssignments: 5,
  observedAt: "2026-07-01T00:00:12.000Z",
  ok: true,
  processes: [],
  pylon: {
    message: "Pylon ready with advertised Codex capacity",
    pylonRef: "pylon.local.part2",
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
    activeAdjustedTokensPerMinute: 500,
    completedStatus: "exact",
    completedTokenRows: 1,
    completedTokensPerMinute: 500,
    inFlightTokens: 100,
    inFlightTokensPerMinute: 500,
    source: "pylon_khala_apm",
    unavailableReason: null,
  },
})

const delegateRunResultFixture = (): KhalaCodeDesktopFleetDelegateRunResult => ({
  acceptedCount: 1,
  delegateSignature: "khala.fleet.delegate",
  delegateStatus: "completed",
  mode: "fixture",
  ok: true,
  projection: {
    localPathsProjected: false,
    objectiveProjected: false,
    providerPayloadProjected: false,
    rawTraceMessagesProjected: false,
  },
  pylonRef: "pylon.local.part2",
  requestedCount: 1,
  results: [{
    accountRef: "codex-2",
    assignmentRef: "assignment.public.codex_agent_task.part2_demo",
    blockerRefs: [],
    closeoutStatus: "accepted",
    slot: 1,
    status: "accepted",
    tokensVerified: 100,
    transcriptRef: "transcript.public.part2_demo",
  }],
  trace: [
    {
      blockerCode: null,
      fallbackModule: null,
      module: "ensure_pylon",
      precondition: "pylon.online",
      refs: ["pylon.local.part2"],
      status: "satisfied",
      summary: "Pylon online gate satisfied.",
    },
    {
      blockerCode: null,
      fallbackModule: "presence_heartbeat",
      module: "advertise_capacity",
      precondition: "pylon.codex_capacity.available",
      refs: ["heartbeat.pylon.local.part2.capacity_advertised"],
      status: "recovered",
      summary: "Codex capacity advertisement recovered.",
    },
    {
      blockerCode: null,
      fallbackModule: null,
      module: "select_account",
      precondition: "codex_account.ready",
      refs: ["account.pylon.codex.4db4cc18ebc55f39fb4da894"],
      status: "satisfied",
      summary: "Worker account selection satisfied.",
    },
    {
      blockerCode: null,
      fallbackModule: null,
      module: "prepare_work",
      precondition: "work.fixture_public_safe",
      refs: ["fixture.khala.fleet.delegate.part2_demo"],
      status: "satisfied",
      summary: "Fixture work preparation satisfied.",
    },
    {
      blockerCode: null,
      fallbackModule: null,
      module: "dispatch",
      precondition: "pylon.dispatch.accepts_assignment",
      refs: ["assignment.public.codex_agent_task.part2_demo"],
      status: "satisfied",
      summary: "Codex spawn dispatch satisfied.",
    },
    {
      blockerCode: null,
      fallbackModule: null,
      module: "verify_closeout",
      precondition: "assignment.closeout.public_safe",
      refs: ["transcript.public.part2_demo"],
      status: "satisfied",
      summary: "Closeout verification satisfied.",
    },
  ],
  validation: {
    fixture: true,
    repoPinsComplete: true,
  },
  workerRuntime: {
    assignmentTool: "codex_spawn",
    homeRole: "pylon_isolated_worker_codex_home",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
  },
})

if (import.meta.main) {
  const args = Bun.argv.slice(2)
  const outDir =
    argValue(args, "--out") ??
    resolve("var/khala-code-desktop/part2-ui-recording-smoke")
  try {
    const captures = await runPart2UiRecordingSmoke({
      outDir,
      visualBaseline: khalaCodeVisualBaselineOptionsFromArgs(args),
    })
    console.log("Part 2 UI recording smoke: PASS")
    for (const capture of captures) {
      console.log(`- ${capture.viewport}: ${capture.steps.map(step => step.name).join(" -> ")}`)
    }
    console.log(JSON.stringify({
      harness: PART2_UI_RECORDING_SMOKE_HARNESS,
      ok: true,
      outDir,
      captures,
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
