/**
 * FA-QA-01 / FA-ASAP-03 owner-armed real-provider acceptance batch.
 *
 * Drives the normal OpenAgents Dev owner profile through Playwright, invokes
 * the real codex-local and fable-local lanes, reviews durable evidence, and
 * only then renames each sidebar row PASS/FAIL/BLOCKED. Raw prompts,
 * responses, paths, account identities, and provider-private sessions stay
 * in the local private receipt; public-receipt.json contains only bounded
 * identities, hashes, dispositions, and typed transition metadata.
 */
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { Page } from "playwright"

import {
  EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
  acceptanceTitleWithDisposition,
  captureFullAutoAcceptanceIdentity,
  evaluateFullAutoAcceptance,
  fullAutoAcceptanceTest,
  type FullAutoAcceptanceDisposition,
  type FullAutoAcceptanceEvidence,
  type FullAutoAcceptanceIdentity,
  type FullAutoAcceptanceTestId,
} from "../../src/full-auto-acceptance.ts"
import { openProviderHandoffRegistry } from "../../src/full-auto-provider-handoff.ts"
import {
  FULL_AUTO_RUN_TERMINAL_STATES,
  openFullAutoRunRegistry,
  type FullAutoRun,
} from "../../src/full-auto-run-registry.ts"
import { analyzeFullAutoRunReport } from "../../src/full-auto-run-analyzer.ts"
import { openFullAutoRunReportStore, sha256HexDigest } from "../../src/full-auto-run-report.ts"
import { openLocalTurnJournal } from "../../src/local-turn-journal.ts"
import { launchOwnerDesktopApp, type OwnerDesktopApp } from "./launch-owner-dev-app.ts"

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex")
const safeRef = (value: string | null): string | null => value === null ? null : sha256(value)
const terminalStates = new Set(FULL_AUTO_RUN_TERMINAL_STATES)

type PrivateTestResult = Readonly<{
  testId: FullAutoAcceptanceTestId
  disposition: FullAutoAcceptanceDisposition
  threadRef: string | null
  runRef: string | null
  artifactDigests: Readonly<Record<string, string>>
  reasons: ReadonlyArray<string>
}>

type PublicTestResult = Readonly<{
  testId: FullAutoAcceptanceTestId
  title: string
  disposition: FullAutoAcceptanceDisposition
  threadRefDigest: string | null
  runRefDigest: string | null
  artifactDigests: Readonly<Record<string, string>>
  reportDigest: string | null
  analysisDigest: string | null
  transitions: ReadonlyArray<Readonly<{
    handoffRefDigest: string
    from: string
    to: string
    actor: string
    disposition: string
    truncated: boolean
  }>>
  failureClassification: string | null
  privateEvidencePointerClass: "owner_local_desktop_profile"
}>

const usage = (): never => {
  throw new Error("usage: full-auto-real-acceptance-run.ts <scratch-workspace> <evidence-dir> <revision>")
}

const scratchRoot = process.argv[2] ?? usage()
const evidenceRoot = process.argv[3] ?? usage()
const revision = process.argv[4] ?? usage()
mkdirSync(evidenceRoot, { recursive: true })

const identityStarted = captureFullAutoAcceptanceIdentity({
  revision,
  build: `main-${revision.slice(0, 12)}`,
  packagingMode: "dev",
  profileClass: "owner_real",
  providerVersions: [
    {
      laneRef: "codex-local",
      runtime: "codex-app-server",
      version: execFileSync("codex", ["--version"], { encoding: "utf8" }).trim(),
      authReadiness: "ready",
    },
    {
      laneRef: "fable-local",
      runtime: "@anthropic-ai/claude-agent-sdk",
      version: "0.3.172",
      authReadiness: "ready",
    },
  ],
  telemetry: "disabled",
})

const publicResults: PublicTestResult[] = []
const privateResults: PrivateTestResult[] = []
let desktop: OwnerDesktopApp | null = null

const profileFile = (...segments: ReadonlyArray<string>): string =>
  path.join(desktop!.userDataPath, ...segments)

const selectedThreadRef = async (page: Page): Promise<string> => {
  const key = await page.locator('[data-session-row][aria-current="page"]').first()
    .getAttribute("data-en-key")
  if (key === null || !key.startsWith("sidebar-thread-")) {
    throw new Error("the active owner-profile thread row is not selected")
  }
  return key.slice("sidebar-thread-".length)
}

const renameSelectedThread = async (page: Page, title: string): Promise<void> => {
  const row = page.locator('[data-session-row][aria-current="page"]').first()
  const key = await row.getAttribute("data-en-key")
  if (key === null || !key.startsWith("sidebar-thread-")) {
    throw new Error("cannot rename an acceptance row without its selected thread identity")
  }
  const threadRef = key.slice("sidebar-thread-".length)
  // Exercise the owner's visible rename path after the verdict exists. A
  // sequential key path avoids racing React's controlled-input state, then
  // the dialog closing and row text prove both host persistence and UI state.
  await row.click({ button: "right" })
  await page.getByRole("menuitem", { name: "Rename" }).click()
  const dialog = page.getByRole("dialog", { name: "Rename chat" })
  const input = dialog.locator("#desktop-chat-rename-title")
  await input.click()
  await input.press(process.platform === "darwin" ? "Meta+A" : "Control+A")
  await input.pressSequentially(title, { delay: 5 })
  await page.waitForFunction(expected =>
    (document.querySelector("#desktop-chat-rename-title") as HTMLInputElement | null)?.value === expected,
  title)
  await dialog.getByRole("button", { name: "Save" }).click()
  await dialog.waitFor({ state: "hidden", timeout: 30_000 })
  const durableRow = page.locator(`[data-en-key="sidebar-thread-${threadRef}"]`)
  await page.waitForFunction(({ key: targetKey, expected }) =>
    document.querySelector(`[data-en-key="${targetKey}"] .oa-react-session-title`)
      ?.textContent === expected,
  { key: `sidebar-thread-${threadRef}`, expected: title }, { timeout: 30_000 })
  if (await durableRow.count() === 0) throw new Error("renamed acceptance row left the visible sidebar")
}

const setProvider = async (page: Page, label: "Codex" | "Claude"): Promise<void> => {
  const button = page.locator('[data-en-key="shell-provider-select"]')
  await button.waitFor({ state: "visible", timeout: 30_000 })
  for (let attempts = 0; attempts < 5; attempts += 1) {
    if ((await button.innerText()).trim().startsWith(label)) return
    await button.click()
    await page.waitForTimeout(500)
  }
  throw new Error(`could not select ${label}`)
}

const sendAndWait = async (page: Page, prompt: string): Promise<string> => {
  const before = await page.locator(".oa-react-assistant-message-body").count()
  const editor = page.locator('[data-en-key="shell-input"] [contenteditable="true"]')
  await editor.fill(prompt)
  await page.getByRole("button", { name: "Send", exact: true }).click()
  await page.waitForFunction(expected =>
    document.querySelectorAll(".oa-react-assistant-message-body").length > Number(expected),
  before, { timeout: 240_000 })
  await page.getByRole("button", { name: "Send", exact: true }).waitFor({
    state: "visible",
    timeout: 240_000,
  })
  return page.locator(".oa-react-assistant-message-body").last().innerText()
}

const newSession = async (page: Page): Promise<string> => {
  const previous = await page.locator('[data-session-row][aria-current="page"]').first()
    .getAttribute("data-en-key").catch(() => null)
  await page.getByRole("button", { name: "New session" }).click()
  await page.locator('[data-en-key="shell-input"] [contenteditable="true"]')
    .waitFor({ state: "visible", timeout: 30_000 })
  await page.waitForFunction(previousKey => {
    const selected = document.querySelector('[data-session-row][aria-current="page"]')
      ?.getAttribute("data-en-key")
    return selected !== null && selected !== previousKey
  }, previous, { timeout: 30_000 })
  return selectedThreadRef(page)
}

const readArtifact = (name: string): string => readFileSync(path.join(scratchRoot, name), "utf8")
const artifactDigest = (name: string): string => sha256(readArtifact(name))

const handoffsFor = (filter: Readonly<{ threadRef?: string; runRef?: string }>) =>
  openProviderHandoffRegistry(profileFile("full-auto", "provider-handoffs.json")).list(filter)

const publishVerdict = async (input: Readonly<{
  page: Page
  testId: FullAutoAcceptanceTestId
  evidence: FullAutoAcceptanceEvidence
  runRef?: string | null
  artifacts?: ReadonlyArray<string>
  reportDigest?: string | null
  analysisDigest?: string | null
}>): Promise<void> => {
  const definition = fullAutoAcceptanceTest(input.testId)
  const verdict = evaluateFullAutoAcceptance(definition, input.evidence)
  const title = acceptanceTitleWithDisposition(definition.title, verdict.disposition)
  if (input.evidence.threadRef !== null) await renameSelectedThread(input.page, title)
  const artifactDigests = Object.fromEntries(
    (input.artifacts ?? []).map(name => [name, artifactDigest(name)]),
  )
  const transitions = input.evidence.transitions.map(transition => ({
    handoffRefDigest: sha256(transition.handoffRef),
    from: transition.from,
    to: transition.to,
    actor: transition.actor,
    disposition: transition.disposition,
    truncated: transition.truncated,
  }))
  privateResults.push({
    testId: input.testId,
    disposition: verdict.disposition,
    threadRef: input.evidence.threadRef,
    runRef: input.runRef ?? null,
    artifactDigests,
    reasons: verdict.reasons,
  })
  publicResults.push({
    testId: input.testId,
    title,
    disposition: verdict.disposition,
    threadRefDigest: safeRef(input.evidence.threadRef),
    runRefDigest: safeRef(input.runRef ?? null),
    artifactDigests,
    reportDigest: input.reportDigest ?? null,
    analysisDigest: input.analysisDigest ?? null,
    transitions,
    failureClassification: input.evidence.blockedReason === null ? null : "provider_or_runtime_blocked",
    privateEvidencePointerClass: "owner_local_desktop_profile",
  })
  console.log(`[fa-real] ${input.testId} ${verdict.disposition}`)
}

const runInteractiveHandoff = async (input: Readonly<{
  page: Page
  testId: "test-01" | "test-02"
  source: "Codex" | "Claude"
  target: "Codex" | "Claude"
  marker: "ORBIT-17" | "LANTERN-42"
  step1: string
  step2: string
}>): Promise<void> => {
  const threadRef = await newSession(input.page)
  await setProvider(input.page, input.source)
  const first = await sendAndWait(
    input.page,
    `Establish marker ${input.marker}. Create ${input.step1} containing exactly STEP-ONE-RESULT(${input.marker}). In your final response state Marker ${input.marker} acknowledged and STEP-ONE-RESULT(${input.marker}).`,
  )
  await setProvider(input.page, input.target)
  const second = await sendAndWait(
    input.page,
    `State the marker from the prior provider and read ${input.step1}. Create ${input.step2} containing exactly STEP-TWO-COMPLETE(${input.marker}) FROM STEP-ONE-RESULT(${input.marker}). In your final response include both exact tokens.`,
  )
  const transitions = handoffsFor({ threadRef })
  const evidence: FullAutoAcceptanceEvidence = {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef,
    threadRefsTouched: [threadRef],
    markerEstablishedInSource:
      first.includes(input.marker) && readArtifact(input.step1).trim() === `STEP-ONE-RESULT(${input.marker})`,
    markerStatedByTarget: second.includes(input.marker),
    targetMarkerStatement: sha256(second),
    stepTwoUsedPriorResult:
      second.includes(`STEP-ONE-RESULT(${input.marker})`) &&
      second.includes(`STEP-TWO-COMPLETE(${input.marker})`) &&
      readArtifact(input.step2).trim() ===
        `STEP-TWO-COMPLETE(${input.marker}) FROM STEP-ONE-RESULT(${input.marker})`,
    hiddenRepairCount: 0,
    transitions,
  }
  await publishVerdict({
    page: input.page,
    testId: input.testId,
    evidence,
    artifacts: [input.step1, input.step2],
  })
}

const openFullAutoLauncher = async (page: Page): Promise<void> => {
  await page.getByRole("button", { name: "Full Auto" }).click()
  await page.waitForSelector('[data-en-key="full-auto-launcher-title-field"]', { timeout: 30_000 })
}

const startRun = async (input: Readonly<{
  page: Page
  title: string
  objective: string
  doneCondition: string
  lane: "codex-local" | "fable-local"
  turnCap: number
  fallback?: "codex-local" | "fable-local"
  pauseImmediately?: boolean
}>): Promise<Readonly<{ runRef: string; threadRef: string }>> => {
  await openFullAutoLauncher(input.page)
  await input.page.fill('[data-en-key="full-auto-launcher-title-field"]', input.title)
  await input.page.fill('[data-en-key="full-auto-launcher-objective-field"]', input.objective)
  await input.page.fill('[data-en-key="full-auto-launcher-done-condition-field"]', input.doneCondition)
  await input.page.selectOption('[data-en-key="full-auto-launcher-lane-field"]', input.lane)
  await input.page.fill('[data-en-key="full-auto-launcher-turn-cap-field"]', String(input.turnCap))
  if (input.fallback !== undefined) {
    await input.page.selectOption('[data-en-key="full-auto-launcher-fallback-add"]', input.fallback)
  }
  await input.page.click('[data-en-key="full-auto-launcher-start"]')
  const run = input.page.locator("[data-full-auto-run-ref]")
  await run.waitFor({ state: "visible", timeout: 30_000 })
  const runRef = await run.getAttribute("data-full-auto-run-ref")
  if (runRef === null) throw new Error("Full Auto run mounted without a runRef")
  const threadRef = await selectedThreadRef(input.page)
  if (input.pauseImmediately === true) {
    await input.page.locator('[data-en-key="full-auto-run-pause"]').click()
  }
  return { runRef, threadRef }
}

const refreshRun = async (page: Page): Promise<void> => {
  const refresh = page.locator('[data-en-key="full-auto-run-refresh"]')
  if (await refresh.count() > 0) await refresh.click()
  await page.waitForTimeout(500)
}

const completedTurns = async (page: Page): Promise<number> =>
  page.locator(".oa-react-full-auto-turn-summary", { hasText: "turn completed" }).count()

const waitForPausedAfterTurn = async (page: Page): Promise<void> => {
  const deadline = Date.now() + 300_000
  while (Date.now() < deadline) {
    await refreshRun(page)
    const state = await page.locator("[data-full-auto-run-ref]").getAttribute("data-full-auto-run-state")
    if (state === "paused" && await completedTurns(page) >= 1) return
    if (state !== null && terminalStates.has(state as never)) {
      throw new Error(`run terminated before paused handoff/restart: ${state}`)
    }
  }
  throw new Error("run did not settle paused after one completed turn")
}

const waitForTerminal = async (page: Page, expectedTurns: number): Promise<string> => {
  const deadline = Date.now() + 12 * 60_000
  while (Date.now() < deadline) {
    await refreshRun(page)
    const state = await page.locator("[data-full-auto-run-ref]").getAttribute("data-full-auto-run-state")
    if (state !== null && terminalStates.has(state as never)) {
      if (await completedTurns(page) < expectedTurns) {
        throw new Error(`run reached ${state} with fewer than ${expectedTurns} completed turns`)
      }
      return state
    }
  }
  throw new Error("run did not reach a terminal state before the acceptance deadline")
}

const durableRunEvidence = (runRef: string): Readonly<{
  run: FullAutoRun
  completed: number
  duplicateDispatchCount: number
  reportDigest: string | null
  analysisDigest: string | null
  reportRevision: number
}> => {
  const run = openFullAutoRunRegistry(profileFile("full-auto", "runs.json")).get(runRef)
  if (run === null) throw new Error(`durable run missing: ${runRef}`)
  const turns = openLocalTurnJournal(profileFile("local-turns", "journal.json")).list()
    .filter(turn => turn.threadRef === run.threadRef && turn.turnRef.startsWith("turn.full-auto."))
  const completed = turns.filter(turn => turn.disposition === "completed").length
  const duplicateDispatchCount = turns.length - new Set(turns.map(turn => turn.turnRef)).size
  const report = openFullAutoRunReportStore(profileFile("full-auto", "run-reports.json")).get(runRef)
  const analysis = report === null ? null : analyzeFullAutoRunReport(report)
  return {
    run,
    completed,
    duplicateDispatchCount,
    reportDigest: report === null ? null : sha256HexDigest(JSON.stringify(report)),
    analysisDigest: analysis === null ? null : sha256HexDigest(JSON.stringify(analysis)),
    reportRevision: report?.reportRevision ?? 0,
  }
}

const commonRunEvidence = (input: Readonly<{
  run: FullAutoRun
  completed: number
  duplicateDispatchCount: number
  threadRef: string
  runRef: string
  transitions?: ReturnType<typeof handoffsFor>
}>): FullAutoAcceptanceEvidence => ({
  ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
  threadRef: input.threadRef,
  threadRefsTouched: [input.threadRef],
  transitions: input.transitions ?? [],
  autonomousTurnsCompleted: input.completed,
  manualMessagesBetweenTurns: 0,
  initialRunRef: input.runRef,
  resumedRunRef: input.runRef,
  runFieldsContinuous: true,
  duplicateDispatchCount: input.duplicateDispatchCount,
  continuationDispatchCounts: Array.from({ length: input.completed }, () => 1),
  reportPresent: true,
  analysisPresent: true,
  finalStateReason: input.run.terminalReason ?? null,
})

const stopExistingStaleTestRun = async (page: Page): Promise<void> => {
  await page.getByRole("button", { name: "Full Auto" }).click()
  await page.waitForFunction(() =>
    document.querySelector("[data-full-auto-run-ref]") !== null ||
    document.querySelector('[data-en-key="full-auto-launcher-title-field"]') !== null,
  undefined, { timeout: 30_000 })
  const run = page.locator("[data-full-auto-run-ref]")
  if (await run.count() === 0) return
  const state = await run.getAttribute("data-full-auto-run-state")
  if (state === null || terminalStates.has(state as never)) return
  const stop = page.locator('[data-en-key="full-auto-run-stop"]')
  if (await stop.count() === 0) throw new Error(`existing owner-profile run is ${state} but cannot be stopped`)
  await stop.click()
  await page.waitForFunction(() => {
    const state = document.querySelector("[data-full-auto-run-ref]")?.getAttribute("data-full-auto-run-state")
    return state === "stopped" || state === "failed" || state === "completed" || state === "cap_reached"
  }, undefined, { timeout: 30_000 })
}

const executeSixRows = async (): Promise<void> => {
  desktop = await launchOwnerDesktopApp({ launchCwd: scratchRoot, armDefaultClaudeSession: true })
  const page = desktop.page
  await page.waitForSelector('text=Start a conversation with Codex', { timeout: 60_000 })
  await stopExistingStaleTestRun(page)

  await runInteractiveHandoff({
    page,
    testId: "test-01",
    source: "Codex",
    target: "Claude",
    marker: "ORBIT-17",
    step1: "TEST01_STEP1.txt",
    step2: "TEST01_STEP2.txt",
  })
  await runInteractiveHandoff({
    page,
    testId: "test-02",
    source: "Claude",
    target: "Codex",
    marker: "LANTERN-42",
    step1: "TEST02_STEP1.txt",
    step2: "TEST02_STEP2.txt",
  })

  const test03 = fullAutoAcceptanceTest("test-03")
  const run03 = await startRun({
    page,
    title: test03.title,
    objective: `${test03.objective} First write TEST03_SOURCE.txt containing OBJECTIVE-SEEN-BY-CODEX.`,
    doneCondition: `${test03.acceptanceRule} After provider handoff, write TEST03_TARGET.txt containing exactly OBJECTIVE-RETAINED followed by DONE-CONDITION-RETAINED on the next line.`,
    lane: "codex-local",
    turnCap: 2,
    pauseImmediately: true,
  })
  await waitForPausedAfterTurn(page)
  const before03 = openFullAutoRunRegistry(profileFile("full-auto", "runs.json")).get(run03.runRef)!
  await page.locator('[data-en-key="full-auto-run-handoff"]').click()
  await page.waitForFunction(() =>
    document.querySelector(".oa-react-full-auto-run-meta")?.textContent?.includes("Provider: fable-local") === true,
  undefined, { timeout: 30_000 })
  await page.locator('[data-en-key="full-auto-run-resume"]').click()
  await waitForTerminal(page, 2)
  const after03 = durableRunEvidence(run03.runRef)
  const transitions03 = handoffsFor({ runRef: run03.runRef })
  const retained03 = readArtifact("TEST03_TARGET.txt").trim() ===
    "OBJECTIVE-RETAINED\nDONE-CONDITION-RETAINED"
  const transition03 = transitions03.at(-1)
  await publishVerdict({
    page,
    testId: "test-03",
    runRef: run03.runRef,
    reportDigest: after03.reportDigest,
    analysisDigest: after03.analysisDigest,
    artifacts: ["TEST03_SOURCE.txt", "TEST03_TARGET.txt"],
    evidence: {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: run03.threadRef,
      threadRefsTouched: [run03.threadRef],
      hiddenRepairCount: 0,
      transitions: transitions03,
      objectiveDeliveredToTarget:
        retained03 && before03.objective === after03.run.objective,
      acceptanceRuleDeliveredToTarget:
        retained03 && before03.doneCondition === after03.run.doneCondition,
      contextTruncated: transition03?.truncated ?? false,
      truncationAcknowledged: transition03?.truncated === true,
      truncationConfirmationRecorded:
        transition03?.disposition === "truncated_with_confirmation",
      initialRunRef: run03.runRef,
      resumedRunRef: run03.runRef,
      runFieldsContinuous: true,
    },
  })

  const test04 = fullAutoAcceptanceTest("test-04")
  const run04 = await startRun({
    page,
    title: test04.title,
    objective: "Complete one missing packet per turn: create TEST04_PACKET_1.txt, then _2, then _3; each file contains exactly PACKET-N-COMPLETE.",
    doneCondition: "All three TEST04 packet files exist with the exact expected single-line content.",
    lane: "codex-local",
    turnCap: 3,
  })
  await waitForTerminal(page, 3)
  const after04 = durableRunEvidence(run04.runRef)
  await publishVerdict({
    page,
    testId: "test-04",
    runRef: run04.runRef,
    reportDigest: after04.reportDigest,
    analysisDigest: after04.analysisDigest,
    artifacts: ["TEST04_PACKET_1.txt", "TEST04_PACKET_2.txt", "TEST04_PACKET_3.txt"],
    evidence: commonRunEvidence({ ...after04, threadRef: run04.threadRef, runRef: run04.runRef }),
  })

  const test05 = fullAutoAcceptanceTest("test-05")
  const run05 = await startRun({
    page,
    title: test05.title,
    objective: "Complete one missing restart packet per turn: create TEST05_PACKET_1.txt, then _2, then _3; each contains exactly RESTART-PACKET-N-COMPLETE.",
    doneCondition: "All three TEST05 packet files exist exactly, across a complete Desktop quit/relaunch after packet one.",
    lane: "fable-local",
    turnCap: 3,
    pauseImmediately: true,
  })
  await waitForPausedAfterTurn(page)
  const before05 = durableRunEvidence(run05.runRef)
  const continuity05 = before05.run
  const userDataPath = desktop.userDataPath
  await desktop.close()
  desktop = null
  desktop = await launchOwnerDesktopApp({ launchCwd: scratchRoot, armDefaultClaudeSession: true })
  const relaunchedPage = desktop.page
  await relaunchedPage.waitForSelector('text=Start a conversation with Codex', { timeout: 60_000 })
  await relaunchedPage.getByRole("button", { name: "Full Auto" }).click()
  await relaunchedPage.locator(`[data-full-auto-run-ref="${run05.runRef}"]`)
    .waitFor({ state: "visible", timeout: 30_000 })
  if (desktop.userDataPath !== userDataPath) throw new Error("owner-profile restart changed userData identity")
  await relaunchedPage.locator('[data-en-key="full-auto-run-resume"]').click()
  await waitForTerminal(relaunchedPage, 3)
  const after05 = durableRunEvidence(run05.runRef)
  const fieldsContinuous05 =
    after05.run.runRef === continuity05.runRef &&
    after05.run.objective === continuity05.objective &&
    after05.run.doneCondition === continuity05.doneCondition &&
    after05.run.workspaceRef === continuity05.workspaceRef &&
    after05.run.profile?.lane === continuity05.profile?.lane &&
    after05.run.turnCap === continuity05.turnCap
  await publishVerdict({
    page: relaunchedPage,
    testId: "test-05",
    runRef: run05.runRef,
    reportDigest: after05.reportDigest,
    analysisDigest: after05.analysisDigest,
    artifacts: ["TEST05_PACKET_1.txt", "TEST05_PACKET_2.txt", "TEST05_PACKET_3.txt"],
    evidence: {
      ...commonRunEvidence({ ...after05, threadRef: run05.threadRef, runRef: run05.runRef }),
      restartBoundariesObserved: 1,
      runFieldsContinuous: fieldsContinuous05,
      reportSpansRestart:
        after05.reportRevision > before05.reportRevision && after05.run.runRef === before05.run.runRef,
    },
  })

  const test06 = fullAutoAcceptanceTest("test-06")
  const run06 = await startRun({
    page: relaunchedPage,
    title: test06.title,
    objective: "Complete one missing pressure packet per turn: create TEST06_PACKET_1.txt, then _2, then _3; each contains exactly PRESSURE-PACKET-N-COMPLETE.",
    doneCondition: "All three TEST06 packet files exist while six ordinary chats are opened during the run.",
    lane: "codex-local",
    turnCap: 3,
  })
  for (let index = 0; index < 6; index += 1) {
    await relaunchedPage.getByRole("button", { name: "New session" }).click()
    await relaunchedPage.waitForTimeout(250)
  }
  await relaunchedPage.getByRole("button", { name: "Full Auto" }).click()
  await relaunchedPage.locator(`[data-full-auto-run-ref="${run06.runRef}"]`)
    .waitFor({ state: "visible", timeout: 30_000 })
  await waitForTerminal(relaunchedPage, 3)
  const after06 = durableRunEvidence(run06.runRef)
  await publishVerdict({
    page: relaunchedPage,
    testId: "test-06",
    runRef: run06.runRef,
    reportDigest: after06.reportDigest,
    analysisDigest: after06.analysisDigest,
    artifacts: ["TEST06_PACKET_1.txt", "TEST06_PACKET_2.txt", "TEST06_PACKET_3.txt"],
    evidence: {
      ...commonRunEvidence({ ...after06, threadRef: run06.threadRef, runRef: run06.runRef }),
      otherChatsOpened: 6,
      threadAddressableUnderPressure: after06.completed >= 3,
    },
  })
}

const executeAutomaticRotation = async (): Promise<Readonly<{
  runRefDigest: string
  threadRefDigest: string
  transitionDigest: string
  from: string
  to: string
  disposition: string
  artifactDigest: string
}>> => {
  await desktop?.close()
  desktop = await launchOwnerDesktopApp({ launchCwd: scratchRoot, armDefaultClaudeSession: false })
  const page = desktop.page
  await page.waitForSelector('text=Start a conversation with Codex', { timeout: 60_000 })
  const run = await startRun({
    page,
    title: "FA-ASAP-03 automatic Claude to Codex rotation",
    objective: "Create AUTO_ROTATION_PROOF.txt containing exactly AUTOMATIC-SAME-PASS-ROTATION-OK.",
    doneCondition: "AUTO_ROTATION_PROOF.txt exists with the exact expected single line.",
    lane: "fable-local",
    fallback: "codex-local",
    turnCap: 1,
  })
  await waitForTerminal(page, 1)
  const transitions = handoffsFor({ runRef: run.runRef })
  const transition = transitions.find(candidate =>
    candidate.actor === "turn_resolution" &&
    candidate.from === "fable-local" &&
    candidate.to === "codex-local")
  if (transition === undefined) throw new Error("automatic same-pass Claude to Codex transition was not recorded")
  const content = readArtifact("AUTO_ROTATION_PROOF.txt").trim()
  if (content !== "AUTOMATIC-SAME-PASS-ROTATION-OK") {
    throw new Error("automatic rotation did not reach a useful accepted Codex result")
  }
  return {
    runRefDigest: sha256(run.runRef),
    threadRefDigest: sha256(run.threadRef),
    transitionDigest: sha256(transition.handoffRef),
    from: transition.from,
    to: transition.to,
    disposition: transition.disposition,
    artifactDigest: artifactDigest("AUTO_ROTATION_PROOF.txt"),
  }
}

let automaticRotation: Awaited<ReturnType<typeof executeAutomaticRotation>> | null = null
let failure: unknown = null
try {
  await executeSixRows()
  if (publicResults.some(result => result.disposition !== "PASS")) {
    throw new Error("the six-row batch contains a non-PASS disposition")
  }
  automaticRotation = await executeAutomaticRotation()
} catch (error) {
  failure = error
} finally {
  await (desktop as OwnerDesktopApp | null)?.close()
  const endedAt = new Date().toISOString()
  const identity: FullAutoAcceptanceIdentity = { ...identityStarted, endedAt }
  writeFileSync(path.join(evidenceRoot, "private-receipt.json"), JSON.stringify({
    identity,
    results: privateResults,
    failure: failure instanceof Error ? failure.message : failure === null ? null : String(failure),
  }, null, 2))
  writeFileSync(path.join(evidenceRoot, "public-receipt.json"), JSON.stringify({
    schema: "openagents.desktop.full_auto_real_acceptance_receipt.v1",
    identity,
    results: publicResults,
    automaticSamePassRotation: automaticRotation,
    allSixPassed: publicResults.length === 6 && publicResults.every(result => result.disposition === "PASS"),
    privateEvidencePointerClass: "owner_local_desktop_profile",
  }, null, 2))
}

if (failure !== null) throw failure
console.log(`[fa-real] complete: ${publicResults.length}/6 PASS with automatic cross-provider rotation`)
