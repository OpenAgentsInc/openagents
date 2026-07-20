import { execFileSync, spawn } from "node:child_process"
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { extractFile } from "@electron/asar"
import { Schema } from "effect"
import { chromium, type Browser, type Page } from "playwright"

import { IdeCursorPackagedJourneyReceiptSchema } from "../src/ide/cursor-benchmark-contract.ts"
import { packagedArtifactTreeDigest } from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const receiptPath = path.join(benchmarkRoot, "2026-07-19-ide-09-packaged-cursor.json")
const screenshotPath = path.join(benchmarkRoot, "2026-07-19-ide-09-packaged-cursor.png")
const tracePath = path.join(benchmarkRoot, "2026-07-19-ide-09-packaged-cursor-trace.json")
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-packaged-cursor.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-09-packaged-cursor-trace.json"

const packagedOutputRoot = path.join(appRoot, "out")
const packagedDirectory = readdirSync(packagedOutputRoot, { withFileTypes: true })
  .find(entry => entry.isDirectory() && entry.name.endsWith("-darwin-arm64"))
const packagedApp = packagedDirectory === undefined ? undefined
  : readdirSync(path.join(packagedOutputRoot, packagedDirectory.name), { withFileTypes: true })
      .find(entry => entry.isDirectory() && entry.name.endsWith(".app"))
const packagedAppPath = packagedDirectory === undefined || packagedApp === undefined ? null
  : path.join(packagedOutputRoot, packagedDirectory.name, packagedApp.name)

if (packagedAppPath === null || !existsSync(packagedAppPath)) {
  throw new Error("IDE-09 packaged journey requires a current darwin-arm64 package; run package:mac first")
}

const waitForRenderer = async (browser: Browser): Promise<Page> => {
  const deadline = Date.now() + 30_000
  let page: Page | undefined
  while (page === undefined && Date.now() < deadline) {
    page = browser.contexts().flatMap(context => context.pages())
      .find(candidate => candidate.url().startsWith("openagents-app://renderer/"))
    if (page === undefined) await new Promise(resolve => setTimeout(resolve, 50))
  }
  if (page === undefined) throw new Error("IDE-09 packaged renderer page did not appear")
  return page
}

const cursorSnapshot = (page: Page) => page.evaluate(async () =>
  (globalThis as any).openagentsDesktop.ideCursor.snapshot() as any)

const waitForCursor = async (
  page: Page,
  predicate: (snapshot: any) => boolean,
  label: string,
): Promise<any> => {
  const deadline = Date.now() + 30_000
  let snapshot: any = null
  while (Date.now() < deadline) {
    snapshot = await cursorSnapshot(page)
    if (predicate(snapshot)) return snapshot
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  throw new Error(`IDE-09 packaged cursor timed out waiting for ${label}: ${JSON.stringify(snapshot)}`)
}

const main = async (): Promise<void> => {
  const workspaceRoot = mkdtempSync(path.join(tmpdir(), "openagents-ide09-workspace-"))
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide09-profile-"))
  const fixturePath = path.join(workspaceRoot, "cursor.ts")
  const baseContent = "export const cursorValue: number = 41\n"
  writeFileSync(fixturePath, baseContent, { encoding: "utf8", mode: 0o600 })
  execFileSync("git", ["init", "-b", "main"], { cwd: workspaceRoot, stdio: "ignore" })
  execFileSync("git", ["config", "user.name", "IDE-09 Packaged Proof"], { cwd: workspaceRoot })
  execFileSync("git", ["config", "user.email", "ide09-proof@openagents.local"], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "add", "cursor.ts"], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "commit", "-m", "IDE-09 packaged fixture"], {
    cwd: workspaceRoot,
    stdio: "ignore",
  })

  let launchedApplicationPid: number | null = null
  const appProcess = spawn("open", [
    "-n", "-W", "-a", packagedAppPath, fixturePath,
    "--args", "--remote-debugging-port=0",
  ], {
    cwd: workspaceRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_IDE07_CHAT_ONLY_PROOF: "1",
      OPENAGENTS_DESKTOP_SMOKE_REACT: "1",
      OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspaceRoot,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
    stdio: "ignore",
  })
  let browser: Browser | null = null
  try {
    const devToolsPortPath = path.join(userDataPath, "DevToolsActivePort")
    const portDeadline = Date.now() + 30_000
    while (!existsSync(devToolsPortPath) && Date.now() < portDeadline) {
      await new Promise(resolve => setTimeout(resolve, 50))
    }
    if (!existsSync(devToolsPortPath)) throw new Error("IDE-09 packaged Chromium DevTools port did not appear")
    const port = readFileSync(devToolsPortPath, "utf8").split("\n")[0]
    const pidOutput = execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], { encoding: "utf8" }).trim()
    const parsedPid = Number.parseInt(pidOutput.split("\n")[0] ?? "", 10)
    if (Number.isSafeInteger(parsedPid) && parsedPid > 1) launchedApplicationPid = parsedPid
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const page = await waitForRenderer(browser)
    page.on("console", message => process.stderr.write(`[IDE-09 renderer ${message.type()}] ${message.text()}\n`))
    page.on("pageerror", error => process.stderr.write(`[IDE-09 renderer error] ${error.message}\n`))

    await page.locator('[data-react-workspace]').first().waitFor({ state: "visible", timeout: 30_000 })
    const initialFiles = page.locator('[data-react-workspace="files"]')
    if (await initialFiles.isVisible()) {
      await page.getByRole("button", { name: "Close Files", exact: true }).click()
    }
    await page.locator('[data-react-workspace="chat"]').waitFor({ state: "visible", timeout: 30_000 })
    const provider = page.locator('[data-en-key="shell-provider-select"]')
    await provider.waitFor({ state: "visible", timeout: 30_000 })
    if ((await provider.textContent() ?? "").trim() !== "Claude") await provider.click()
    await page.waitForFunction(() =>
      document.querySelector('[data-en-key="shell-provider-select"]')?.textContent?.trim() === "Claude")

    // A real shell send establishes the exact active-thread provider target.
    // The subsequent IDE-08 manifest therefore carries claude-local, the
    // currently selected Claude model, and claude-pylon-3 instead of an
    // invented account.
    const composer = page.locator('[data-en-key="shell-input"] [contenteditable="true"]')
    await composer.fill("Establish the packaged IDE-09 Claude fixture turn.")
    await page.getByRole("button", { name: "Send", exact: true }).click()
    try {
      // The fixture deliberately streams a markdown marker across deltas. The
      // renderer may compact the earlier progressive fragments before the
      // final "proof." fragment lands, so settlement is keyed to that final
      // fixture fragment rather than a transient concatenated DOM string.
      await page.waitForFunction(() => document.body.innerText.includes("proof."), undefined, { timeout: 30_000 })
    } catch (error) {
      const diagnostic = await page.evaluate(() => ({
        body: document.body.innerText.slice(0, 4_000),
        provider: document.querySelector('[data-en-key="shell-provider-select"]')?.textContent ?? null,
        sendDisabled: (document.querySelector('[aria-label="Send"]') as HTMLButtonElement | null)?.disabled ?? null,
      }))
      throw new Error(`IDE-09 Claude fixture turn did not settle: ${JSON.stringify(diagnostic)}`, { cause: error })
    }

    await page.getByLabel("Project actions").getByRole("button", { name: "Files", exact: true }).click()
    await page.locator('[data-react-workspace="files"]').waitFor({ state: "visible", timeout: 30_000 })
    const treeTarget = page.locator('[data-oa-pierre-tree="true"] [data-item-path="cursor.ts"]')
    await treeTarget.waitFor({ state: "visible", timeout: 30_000 })
    await treeTarget.click()
    const primary = page.locator('.oa-react-monaco-pane[data-monaco-view="primary"]')
    await primary.locator('[data-monaco-phase="ready"]').waitFor({ state: "visible", timeout: 30_000 })
    const vimControl = page.getByRole("button", { name: /Vim (on|off)/ }).first()
    await vimControl.waitFor({ state: "visible", timeout: 30_000 })
    await page.getByRole("button", { name: "Add context", exact: true }).click()
    await page.waitForFunction(async () => {
      const snapshot = await (globalThis as any).openagentsDesktop.ideAgentCode.snapshot()
      const manifest = snapshot.manifests.at(-1)
      return manifest?.effectiveRuntime?.providerRef === "claude-local" &&
        manifest?.effectiveRuntime?.accountRef === "claude-pylon-3"
    }, undefined, { timeout: 30_000 })
    // Add context intentionally returns to the conversation where the tray is
    // visible. Re-enter the persisted editor tab to drive inline AI controls.
    if (!await page.locator('[data-react-workspace="files"]').isVisible()) {
      await page.getByLabel("Project actions").getByRole("button", { name: "Files", exact: true }).click()
      await page.locator('[data-react-workspace="files"]').waitFor({ state: "visible", timeout: 30_000 })
    }

    const ai = page.locator('[aria-label="AI editing"]')
    try {
      await ai.waitFor({ state: "visible", timeout: 30_000 })
    } catch (error) {
      const diagnostic = await page.evaluate(() => ({
        count: document.querySelectorAll('[aria-label="AI editing"]').length,
        html: document.querySelector('[aria-label="AI editing"]')?.outerHTML.slice(0, 1_000) ?? null,
        bodyTail: document.body.innerText.slice(-4_000),
      }))
      throw new Error(`IDE-09 AI-editing surface did not become visible: ${JSON.stringify(diagnostic)}`, { cause: error })
    }
    await ai.getByRole("button", { name: "Complete", exact: true }).click()
    let completion: any
    try {
      completion = await waitForCursor(page,
        snapshot => snapshot.candidates.some((candidate: any) => candidate._tag === "Completion"),
        "completion candidate")
    } catch (error) {
      const diagnostic = await page.evaluate(async () => ({
        ai: document.querySelector('[aria-label="AI editing"]')?.textContent ?? null,
        agent: await (globalThis as any).openagentsDesktop.ideAgentCode.snapshot(),
        cursor: await (globalThis as any).openagentsDesktop.ideCursor.snapshot(),
      }))
      throw new Error(`IDE-09 completion request did not admit: ${JSON.stringify(diagnostic)}`, { cause: error })
    }
    await ai.getByRole("button", { name: "Accept word", exact: true }).click()
    const accepted = await waitForCursor(page,
      snapshot => snapshot.receipts.some((receipt: any) => receipt.decision?._tag === "Accept" && receipt.applied),
      "partial accept receipt")
    const partialContent = await page.evaluate(async () => {
      const bridge = (globalThis as any).openagentsDesktop
      const agent = await bridge.ideAgentCode.snapshot()
      const opened = await bridge.openWorkspaceDocument({ grantRef: agent.attachment.grantRef, pathRef: "cursor.ts" })
      return opened.document?.content ?? null
    })
    await ai.getByRole("button", { name: "Undo", exact: true }).click()
    const undone = await waitForCursor(page,
      snapshot => snapshot.receipts.some((receipt: any) => receipt.decision?._tag === "Undo" && receipt.applied),
      "undo receipt")
    const restoredContent = await page.evaluate(async () => {
      const bridge = (globalThis as any).openagentsDesktop
      const agent = await bridge.ideAgentCode.snapshot()
      const opened = await bridge.openWorkspaceDocument({ grantRef: agent.attachment.grantRef, pathRef: "cursor.ts" })
      return opened.document?.content ?? null
    })

    await ai.getByRole("button", { name: "Next edit", exact: true }).click()
    const nextEdit = await waitForCursor(page,
      snapshot => snapshot.candidates.some((candidate: any) => candidate._tag === "NextEdit"),
      "next-edit candidate")
    await ai.getByRole("button", { name: "Compare", exact: true }).click()
    const compared = await waitForCursor(page,
      snapshot => snapshot.receipts.some((receipt: any) => receipt.decision?._tag === "Compare"),
      "compare receipt")
    const priorSequence = compared.latestSequence
    await ai.getByRole("button", { name: "Retry", exact: true }).click()
    const retried = await waitForCursor(page,
      snapshot => snapshot.latestSequence > priorSequence &&
        snapshot.receipts.some((receipt: any) => receipt.decision?._tag === "Retry"),
      "retry receipt and next sequence")

    const prompt = ai.getByLabel("Ask or change code")
    await prompt.fill("What does the current file establish?")
    await ai.getByRole("button", { name: "Ask", exact: true }).click()
    const answer = await waitForCursor(page,
      snapshot => snapshot.candidates.some((candidate: any) => candidate._tag === "Answer"),
      "ask answer")
    const beforeEscapeReceipts = answer.receipts.length
    await prompt.focus()
    await page.keyboard.press("Escape")
    const cancelled = await waitForCursor(page,
      snapshot => snapshot.receipts.length > beforeEscapeReceipts &&
        snapshot.receipts.some((receipt: any) => receipt.decision?._tag === "Cancel"),
      "Escape cancellation")

    await prompt.fill("Append a version-bound fixture comment.")
    await ai.getByRole("button", { name: "Change", exact: true }).click()
    const proposal = await waitForCursor(page,
      snapshot => snapshot.candidates.some((candidate: any) => candidate._tag === "Proposal"),
      "proposal candidate")
    await ai.getByRole("button", { name: "Review proposal", exact: true }).click()
    const submitted = await waitForCursor(page,
      snapshot => snapshot.receipts.some((receipt: any) => receipt.proposalSubmitted === true),
      "IDE-08 proposal submission")
    const ide08 = await page.evaluate(async () => (globalThis as any).openagentsDesktop.ideAgentCode.snapshot())

    const disclosure = ai.locator("details")
    await disclosure.locator("summary").click()
    const disclosureText = await disclosure.textContent() ?? ""
    const identityDisclosed = disclosureText.includes("claude") &&
      disclosureText.includes("claude-pylon-3") &&
      disclosureText.includes("ide.placement.desktop-local")
    const noRemoteIndexDependencyDisclosed = disclosureText.includes("No remote index dependency")
    // Chromium intentionally exposes no Resource Timing entries for the
    // privileged custom scheme. Inspect the exact packaged ASAR bytes instead
    // of weakening the assertion or relying on the mutable source checkout.
    const editorBundle = extractFile(
      path.join(packagedAppPath, "Contents", "Resources", "app.asar"),
      "dist/renderer/ide-editor/editor.js",
    ).toString("utf8")
    const vimAndTokyoNightPresent = await vimControl.isVisible() &&
      editorBundle.includes("openagents-tokyo-night")
    const keyboardOperable = await page.evaluate(() => document.activeElement !== null) &&
      accepted.receipts.length > 0 && undone.receipts.length > accepted.receipts.length
    const focusAndEscape = cancelled.receipts.some((receipt: any) => receipt.decision?._tag === "Cancel")
    const proposalCandidate = proposal.candidates.find((candidate: any) => candidate._tag === "Proposal")
    const proposalSubmittedToIde08 = submitted.receipts.some((receipt: any) =>
      receipt.proposalSubmitted === true && receipt.applied === false) &&
      ide08.proposals.some((item: any) => item.proposalRef === proposalCandidate?.proposalRef)

    const journey = {
      completionRendered: completion.candidates.some((candidate: any) => candidate._tag === "Completion"),
      partialAcceptApplied: partialContent !== baseContent && accepted.receipts.some((receipt: any) => receipt.applied),
      undoRestored: restoredContent === baseContent,
      nextEditRendered: nextEdit.candidates.some((candidate: any) => candidate._tag === "NextEdit"),
      askRendered: answer.candidates.some((candidate: any) => candidate._tag === "Answer"),
      proposalSubmittedToIde08,
      compareAndRetryReceipted: retried.receipts.some((receipt: any) => receipt.decision?._tag === "Compare") &&
        retried.receipts.some((receipt: any) => receipt.decision?._tag === "Retry"),
      identityDisclosed,
      noRemoteIndexDependencyDisclosed,
      keyboardOperable,
      focusAndEscape,
      vimAndTokyoNightPresent,
    }
    const passed = Object.values(journey).every(Boolean)
    await page.screenshot({ path: screenshotPath, fullPage: true })
    const candidateCommitSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
    const receipt = Schema.decodeUnknownSync(IdeCursorPackagedJourneyReceiptSchema)({
      schemaVersion: "openagents.ide-cursor-packaged-journey.v1",
      issue: "IDE-09",
      capturedAt: new Date().toISOString(),
      candidateCommitSha,
      artifactTreeSha256: `sha256:${packagedArtifactTreeDigest(packagedAppPath).sha256}`,
      target: "darwin-arm64",
      journey,
      screenshotRef,
      traceRef,
      passed,
    })
    writeFileSync(tracePath, `${JSON.stringify({
      schemaVersion: "openagents.ide-cursor-packaged-trace.v1",
      candidateCommitSha,
      fixtureCohort: "deterministic_claude_query",
      steps: ["claude_turn", "manifest", "completion", "partial_accept", "undo", "next_edit", "compare", "retry", "ask", "escape", "proposal", "disclosure"],
      sequences: {
        completion: completion.latestSequence,
        nextEdit: nextEdit.latestSequence,
        retry: retried.latestSequence,
        ask: answer.latestSequence,
        proposal: proposal.latestSequence,
      },
      provider: "claude-local",
      account: "claude-pylon-3",
      proposalRef: proposalCandidate?.proposalRef ?? null,
      finalReceiptCount: submitted.receipts.length,
    }, null, 2)}\n`, { mode: 0o600 })
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
    if (!passed) throw new Error(`IDE-09 packaged cursor journey failed: ${JSON.stringify(journey)}`)
    process.stdout.write(`[openagents-desktop] IDE-09 packaged cursor journey: ${receiptPath}\n`)
  } finally {
    await browser?.close()
    if (launchedApplicationPid !== null) {
      try { process.kill(launchedApplicationPid, "SIGTERM") } catch { /* already stopped */ }
      const stopDeadline = Date.now() + 5_000
      while (Date.now() < stopDeadline) {
        try { process.kill(launchedApplicationPid, 0); await new Promise(resolve => setTimeout(resolve, 100)) } catch { break }
      }
    }
    appProcess.kill("SIGTERM")
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ])
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL")
    for (const root of [userDataPath, workspaceRoot]) {
      if (!existsSync(root)) continue
      try { rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }) } catch { /* best effort */ }
    }
  }
}

await main()
