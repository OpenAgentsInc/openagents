import { execFileSync, spawn } from "node:child_process"
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import { Schema } from "effect"

import { IdeAgentCodePackagedJourneyReceiptSchema } from "../src/ide/agent-code-benchmark-contract.ts"
import { packagedArtifactTreeDigest } from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const receiptPath = path.join(benchmarkRoot, "2026-07-19-ide-08-packaged-agent-code.json")
const screenshotPath = path.join(benchmarkRoot, "2026-07-19-ide-08-packaged-agent-code.png")
const tracePath = path.join(benchmarkRoot, "2026-07-19-ide-08-packaged-agent-code-trace.json")
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code-trace.json"
const packagedOutputRoot = path.join(appRoot, "out")
const packagedDirectory = readdirSync(packagedOutputRoot, { withFileTypes: true })
  .find(entry => entry.isDirectory() && entry.name.endsWith("-darwin-arm64"))
const packagedApp = packagedDirectory === undefined ? undefined
  : readdirSync(path.join(packagedOutputRoot, packagedDirectory.name), { withFileTypes: true })
      .find(entry => entry.isDirectory() && entry.name.endsWith(".app"))
const packagedAppPath = packagedDirectory === undefined || packagedApp === undefined ? null
  : path.join(packagedOutputRoot, packagedDirectory.name, packagedApp.name)
const packagedMacOsDirectory = packagedAppPath === null ? null : path.join(packagedAppPath, "Contents", "MacOS")
const packagedExecutable = packagedMacOsDirectory === null ? undefined
  : readdirSync(packagedMacOsDirectory, { withFileTypes: true }).find(entry => entry.isFile())
const packagedBinary = packagedMacOsDirectory === null || packagedExecutable === undefined ? null
  : path.join(packagedMacOsDirectory, packagedExecutable.name)

if (packagedAppPath === null || packagedBinary === null || !existsSync(packagedBinary)) {
  throw new Error("IDE-08 packaged journey requires a current darwin-arm64 package; run package:mac first")
}

const enterFiles = async (page: Page): Promise<void> => {
  await page.keyboard.press("Meta+E")
  await page.locator('[data-react-workspace="files"]').waitFor({ state: "visible", timeout: 30_000 })
}

const main = async (): Promise<void> => {
  const workspaceRoot = mkdtempSync(path.join(tmpdir(), "openagents-ide08-workspace-"))
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide08-profile-"))
  const fixturePath = path.join(workspaceRoot, "agent.ts")
  const baseContent = "export const answer: number = 41\n"
  const targetContent = "export const answer: number = 42\n"
  writeFileSync(fixturePath, baseContent, { encoding: "utf8", mode: 0o600 })
  execFileSync("git", ["init", "-b", "main"], { cwd: workspaceRoot, stdio: "ignore" })
  execFileSync("git", ["config", "user.name", "IDE-08 Packaged Proof"], { cwd: workspaceRoot })
  execFileSync("git", ["config", "user.email", "ide08-proof@openagents.local"], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "add", "agent.ts"], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "commit", "-m", "IDE-08 packaged fixture"], { cwd: workspaceRoot, stdio: "ignore" })

  let launchedApplicationPid: number | null = null
  const appProcess = spawn("open", [
    "-n", "-W", "-a", packagedAppPath!, fixturePath,
    "--args", "--remote-debugging-port=0",
  ], {
    cwd: workspaceRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspaceRoot,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
    stdio: "ignore",
  })
  let browser: Browser | null = null
  try {
    const devToolsPortPath = path.join(userDataPath, "DevToolsActivePort")
    const portDeadline = Date.now() + 20_000
    while (!existsSync(devToolsPortPath) && Date.now() < portDeadline) await new Promise(resolve => setTimeout(resolve, 50))
    if (!existsSync(devToolsPortPath)) throw new Error("IDE-08 packaged Chromium DevTools port did not appear")
    const port = readFileSync(devToolsPortPath, "utf8").split("\n")[0]
    const pidOutput = execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], { encoding: "utf8" }).trim()
    const parsedPid = Number.parseInt(pidOutput.split("\n")[0] ?? "", 10)
    if (Number.isSafeInteger(parsedPid) && parsedPid > 1) launchedApplicationPid = parsedPid
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const pageDeadline = Date.now() + 30_000
    let page: Page | undefined
    while (page === undefined && Date.now() < pageDeadline) {
      page = browser.contexts().flatMap(context => context.pages())
        .find(candidate => candidate.url().startsWith("openagents-app://renderer/"))
      if (page === undefined) await new Promise(resolve => setTimeout(resolve, 50))
    }
    if (page === undefined) throw new Error("IDE-08 packaged renderer page did not appear")
    page.on("console", message => process.stderr.write(`[IDE-08 renderer ${message.type()}] ${message.text()}\n`))
    page.on("pageerror", error => process.stderr.write(`[IDE-08 renderer error] ${error.message}\n`))

    await enterFiles(page)
    const treeTarget = page.locator('[data-oa-pierre-tree="true"] [data-item-path="agent.ts"]')
    await treeTarget.waitFor({ state: "visible", timeout: 30_000 })
    await treeTarget.click()
    const primary = page.locator('.oa-react-monaco-pane[data-monaco-view="primary"]')
    await primary.locator('[data-monaco-phase="ready"]').waitFor({ state: "visible", timeout: 30_000 })
    const documentTier = primary.locator('[data-language-tier="document-local"]')
    await documentTier.waitFor({ state: "visible", timeout: 30_000 })
    await page.waitForFunction(() => {
      const tier = document.querySelector('[data-language-tier="document-local"]')
      return tier?.textContent?.includes("worker ready") === true || tier?.getAttribute("data-language-state") === "Failed"
    }, undefined, { timeout: 30_000 })
    if (await documentTier.getAttribute("data-language-state") === "Failed") {
      throw new Error(await documentTier.getAttribute("data-language-message") ?? "IDE-08 document-local worker failed")
    }
    await page.locator('.oa-react-language-status[data-language-service="ready"]').waitFor({ state: "visible", timeout: 30_000 })
    await page.waitForFunction(() => {
      const text = document.querySelector('[data-language-tier="project-local"]')?.textContent ?? ""
      return !text.includes("no current evidence")
    }, undefined, { timeout: 30_000 })
    await page.waitForFunction(() => {
      const text = document.querySelector('.oa-react-language-panel > header > span')?.textContent ?? ""
      return !text.includes("No current diagnostic receipt")
    }, undefined, { timeout: 30_000 })
    const diagnosticObserved = !((await page.locator('.oa-react-language-panel > header > span').textContent() ?? "")
      .includes("No current diagnostic receipt"))
    // Finder-open catalog publication may replace the equivalent workspace
    // service once during its delayed startup handoff. Exercise proposals only
    // after that admitted service generation has settled.
    await page.waitForTimeout(2_000)
    const addContextControl = page.getByRole("button", { name: "Add context", exact: true })
    await addContextControl.waitFor({ state: "visible", timeout: 30_000 })
    await addContextControl.click()

    type PreparedInjection = Readonly<{
      proposalRef: string
      manifestRef: string
      included: number
      omitted: number
      prepared: string
    }>
    let injection: PreparedInjection | null = null
    const injectionDeadline = Date.now() + 30_000
    while (injection === null && Date.now() < injectionDeadline) {
      const attempt = await page.evaluate(async ({ target }) => {
      const bridge = (globalThis as unknown as { openagentsDesktop: {
        openWorkspaceDocument: (value: unknown) => Promise<unknown>
        ideAgentCode: { snapshot: () => Promise<unknown>; command: (value: unknown) => Promise<unknown> }
      } }).openagentsDesktop
      const snapshot = await bridge.ideAgentCode.snapshot() as any
      const attachment = snapshot.attachment
      const manifest = snapshot.manifests.at(-1)
      if (attachment == null || manifest == null) return false
      const opened = await bridge.openWorkspaceDocument({ grantRef: attachment.grantRef, pathRef: "agent.ts" }) as any
      if (opened.state !== "available") throw new Error("agent fixture document unavailable")
      const openedDocument = opened.document
      const pathBytes = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(openedDocument.pathRef)))
      const revisionBytes = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(openedDocument.revisionRef)))
      const contentBytes = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(openedDocument.content)))
      const targetBytes = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(target)))
      const pathSuffix = [...pathBytes].map(byte => byte.toString(16).padStart(2, "0")).join("").slice(0, 32)
      const revisionSuffix = [...revisionBytes].map(byte => byte.toString(16).padStart(2, "0")).join("").slice(0, 32)
      const contentDigest = `sha256:${[...contentBytes].map(byte => byte.toString(16).padStart(2, "0")).join("")}`
      const targetDigest = `sha256:${[...targetBytes].map(byte => byte.toString(16).padStart(2, "0")).join("")}`
      const proposal = {
        schemaVersion: "openagents.desktop.ide-agent-code.v1",
        proposalRef: "ide.proposal.packaged.ide08",
        parentProposalRef: null,
        attachment,
        manifestRef: manifest.manifestRef,
        sessionRef: attachment.sessionRef,
        turnRef: manifest.turnRef,
        conversationThreadRef: manifest.conversationThreadRef,
        createdAt: new Date().toISOString(),
        operations: [{
          _tag: "Edit",
          operationRef: "ide.agent-operation.packaged.ide08.edit",
          fileRef: `ide.file.workspace.${pathSuffix}`,
          pathRef: openedDocument.pathRef,
          base: {
            existed: true,
            content: openedDocument.content,
            diskRevisionRef: `ide.disk-revision.workspace.${revisionSuffix}`,
            documentRef: `ide.document.workspace.${pathSuffix}`,
            documentGeneration: 1,
            gitSnapshotRef: null,
            gitSnapshotGeneration: null,
            checkpointRef: null,
            contentDigest,
            encoding: openedDocument.encoding,
            lineEnding: openedDocument.lineEnding,
            mode: "regular",
          },
          policy: { encoding: "preserve", lineEnding: "preserve", mode: "preserve", symlink: "refuse" },
          documentRef: `ide.document.workspace.${pathSuffix}`,
          targetContent: target,
          targetContentDigest: targetDigest,
        }],
        lifecycle: { _tag: "Pending" },
        lineage: null,
      }
        return { prepared: JSON.stringify({ attachment, manifest, proposal }), proposalRef: proposal.proposalRef, manifestRef: manifest.manifestRef, included: manifest.items.filter((item: any) => item.disposition._tag === "Included").length, omitted: manifest.omittedCount }
      }, { target: targetContent })
      if (attempt !== false) injection = attempt
      else await new Promise(resolve => setTimeout(resolve, 25))
    }
    if (injection === null) {
      const disclosure = await page.getByRole("region", { name: "Agent context disclosure" }).textContent().catch(() => "unavailable")
      const visibleText = await page.locator("body").innerText().catch(() => "unavailable")
      throw new Error(`IDE-08 agent context manifest did not attach within 30 seconds: url=${page.url()} disclosure=${disclosure} view=${visibleText.slice(0, 800)}`)
    }

    const closeFiles = page.getByRole("button", { name: "Close Files", exact: true })
    if (await closeFiles.isVisible()) await closeFiles.click()
    await page.locator('[data-react-workspace="chat"]').waitFor({ state: "visible", timeout: 10_000 })
    await page.evaluate(async ({ prepared }) => {
      const bridge = (globalThis as any).openagentsDesktop.ideAgentCode
      const { attachment, manifest } = JSON.parse(prepared)
      const attached = await bridge.command({ _tag: "Attach", attachment })
      if (attached._tag !== "Succeeded") throw new Error(`stable host attachment refused: ${attached.reason}`)
      const manifested = await bridge.command({
        _tag: "AssembleManifest",
        input: { manifest, expectedAttachmentGeneration: (attachment as any).attachmentGeneration },
      })
      if (manifested._tag !== "Succeeded") throw new Error(`stable host manifest refused: ${manifested.reason}`)
    }, { prepared: injection.prepared })
    await page.getByRole("button", { name: "Refresh agent context state", exact: true }).click()
    const tray = page.getByRole("region", { name: "Agent context disclosure" })
    await tray.waitFor({ state: "visible", timeout: 10_000 })
    const trayToggle = tray.getByRole("button", { name: /Context \d+ included/ })
    if (await trayToggle.getAttribute("aria-expanded") !== "true") await trayToggle.click()
    const trayText = await tray.textContent() ?? ""
    const contextManifestDisclosed = trayText.includes("Effective agent runtime") || trayText.includes("Harness")
    const omittedContextDisclosed = trayText.includes("Omitted") && trayText.includes("retrieval disabled")
    await page.waitForFunction(async () => {
      const snapshot = await (globalThis as any).openagentsDesktop.ideAgentCode.snapshot()
      return snapshot.attachment != null && snapshot.manifests.length > 0
    }, undefined, { timeout: 30_000 })
    await page.evaluate(async ({ prepared }) => {
      const bridge = (globalThis as any).openagentsDesktop.ideAgentCode
      const { proposal } = JSON.parse(prepared)
      const attachmentGeneration = (proposal as any).attachment.attachmentGeneration
      const submitted = await bridge.command({
        _tag: "SubmitProposal",
        input: { proposal, expectedAttachmentGeneration: attachmentGeneration },
      })
      if (submitted._tag !== "Succeeded") throw new Error(`proposal submit refused: ${submitted.reason}`)
      const reviewed = await bridge.command({
        _tag: "BeginReview",
        input: { proposalRef: (proposal as any).proposalRef, reviewRef: "ide.agent-review.packaged.ide08", expectedAttachmentGeneration: attachmentGeneration },
      })
      if (reviewed._tag !== "Succeeded") throw new Error(`proposal review refused: ${reviewed.reason}`)
      const accepted = await bridge.command({
        _tag: "Decide",
        decision: {
          decisionRef: "ide.agent-decision.packaged.ide08",
          proposalRef: (proposal as any).proposalRef,
          decidedAt: new Date().toISOString(),
          disposition: "accept",
          operationRefs: (proposal as any).operations.map((operation: any) => operation.operationRef),
          reason: null,
        },
        expectedAttachmentGeneration: attachmentGeneration,
      })
      if (accepted._tag !== "Succeeded") throw new Error(`proposal decision refused: ${accepted.reason}`)
    }, { prepared: injection.prepared })

    await page.getByLabel("Project actions").getByRole("button", { name: "Review", exact: true }).click()
    const review = page.getByLabel("Review surface")
    await review.waitFor({ state: "visible", timeout: 20_000 })
    const proposalButton = review.getByRole("button", { name: /1 operation.*Accepted/ }).first()
    await proposalButton.click()
    await review.locator('[data-oa-pierre-review]').waitFor({ state: "visible", timeout: 20_000 })
    const applyButton = review.getByRole("button", { name: "Apply exact accepted proposal", exact: true })
    await applyButton.focus()
    await page.keyboard.press("Enter")
    await review.getByText("Applied", { exact: true }).first().waitFor({ state: "visible", timeout: 30_000 })
    const evidenceText = await review.getByRole("region", { name: "Post-apply evidence" }).textContent() ?? ""
    const evidenceSeparatedFromHarness = evidenceText.includes("Separate from harness completion") &&
      evidenceText.includes("test") && evidenceText.includes("delivery") && evidenceText.includes("unavailable")
    const appliedDocument = await page.evaluate(async () => {
      const bridge = (globalThis as any).openagentsDesktop
      const snapshot = await bridge.ideAgentCode.snapshot()
      const opened = await bridge.openWorkspaceDocument({ grantRef: snapshot.attachment.grantRef, pathRef: "agent.ts" })
      return { content: opened.document?.content ?? null, evidence: snapshot.evidence, backlinks: snapshot.backlinks }
    })
    const canonicalApplyObserved = appliedDocument.content === targetContent
    const exactProposalAdmitted = injection.proposalRef === "ide.proposal.packaged.ide08"
    const pierreReviewRendered = await review.locator('[data-oa-pierre-review]').count() === 1
    const nonColorStateCues = (await review.textContent() ?? "").includes("Applied") && evidenceText.includes("unavailable")
    await page.screenshot({ path: screenshotPath, fullPage: true })

    const backlink = review.getByRole("button", { name: /agent\.ts · current generation/ }).first()
    await backlink.focus()
    await page.keyboard.press("Enter")
    await page.locator('[data-react-workspace="files"]').waitFor({ state: "visible", timeout: 20_000 })
    const backlinkRoundTrip = await page.getByRole("tab", { name: /agent\.ts/ }).count() > 0 ||
      await page.getByRole("button", { name: /agent\.ts/ }).count() > 0

    await page.getByRole("button", { name: "Close Files", exact: true }).click()
    await page.getByLabel("Project actions").getByRole("button", { name: "Review", exact: true }).click()
    await page.getByLabel("Review surface").getByRole("button", { name: "Undo to checkpoint", exact: true }).focus()
    await page.keyboard.press("Enter")
    await page.waitForTimeout(300)
    const restored = await page.evaluate(async () => {
      const bridge = (globalThis as any).openagentsDesktop
      const snapshot = await bridge.ideAgentCode.snapshot()
      const opened = await bridge.openWorkspaceDocument({ grantRef: snapshot.attachment.grantRef, pathRef: "agent.ts" })
      return opened.document?.content ?? null
    })
    const undoRestoredPreimage = restored === baseContent
    // The empty-conversation shell intentionally displays the owner's chosen
    // working directory. IDE-08's privacy boundary is narrower: the agent
    // context and review projections must expose only grant-scoped refs and
    // root-relative paths, never the host root used by main.
    const agentSurfaceText = `${await review.textContent() ?? ""}\n${await page.getByLabel("Agent context disclosure").textContent() ?? ""}`
    const rootWithheld = !agentSurfaceText.includes(workspaceRoot)
    const keyboardOperable = canonicalApplyObserved && backlinkRoundTrip && undoRestoredPreimage

    const journey = {
      diagnosticObserved,
      contextManifestDisclosed,
      omittedContextDisclosed,
      exactProposalAdmitted,
      pierreReviewRendered,
      canonicalApplyObserved,
      evidenceSeparatedFromHarness,
      backlinkRoundTrip,
      undoRestoredPreimage,
      rootWithheld,
      keyboardOperable,
      nonColorStateCues,
    }
    const passed = Object.values(journey).every(Boolean)
    const candidateCommitSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
    const receipt = Schema.decodeUnknownSync(IdeAgentCodePackagedJourneyReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-agent-code-packaged-journey.v1",
      issue: "IDE-08",
      capturedAt: new Date().toISOString(),
      candidateCommitSha,
      artifactTreeSha256: `sha256:${packagedArtifactTreeDigest(packagedAppPath!).sha256}`,
      target: "darwin-arm64",
      journey,
      screenshotRef,
      traceRef,
      passed,
    })
    writeFileSync(tracePath, `${JSON.stringify({
      schemaVersion: "openagents.desktop.ide-agent-code-packaged-trace.v1",
      candidateCommitSha,
      steps: ["diagnostics", "context_manifest", "proposal", "pierre_review", "canonical_apply", "host_evidence", "backlink", "undo"],
      proposalRef: injection.proposalRef,
      manifestRef: injection.manifestRef,
      includedCount: injection.included,
      omittedCount: injection.omitted,
      lifecycle: ["Pending", "Reviewing", "Accepted", "Applied", "Undone"],
    }, null, 2)}\n`, { mode: 0o600 })
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
    if (!passed) throw new Error(`IDE-08 packaged agent-code journey failed: ${JSON.stringify(journey)}`)
    process.stdout.write(`[openagents-desktop] IDE-08 packaged agent-code journey: ${receiptPath}\n`)
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
