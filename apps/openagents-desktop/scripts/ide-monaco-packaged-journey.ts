import { execFileSync, spawn } from "node:child_process"
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import { Schema } from "effect"

import { IdeMonacoPackagedJourneyReceiptSchema } from "../src/ide/monaco-benchmark-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const packagedOutputRoot = path.join(appRoot, "out")
const packagedDirectory = readdirSync(packagedOutputRoot, { withFileTypes: true })
  .find(entry => entry.isDirectory() && entry.name.endsWith("-darwin-arm64"))
const packagedApp = packagedDirectory === undefined
  ? undefined
  : readdirSync(path.join(packagedOutputRoot, packagedDirectory.name), { withFileTypes: true })
      .find(entry => entry.isDirectory() && entry.name.endsWith(".app"))
const packagedAppPath = packagedDirectory === undefined || packagedApp === undefined
  ? null
  : path.join(packagedOutputRoot, packagedDirectory.name, packagedApp.name)
const packagedMacOsDirectory = packagedDirectory === undefined || packagedApp === undefined
  ? undefined
  : path.join(packagedOutputRoot, packagedDirectory.name, packagedApp.name, "Contents", "MacOS")
const packagedExecutable = packagedMacOsDirectory === undefined
  ? undefined
  : readdirSync(packagedMacOsDirectory, { withFileTypes: true }).find(entry => entry.isFile())
const packagedBinary = packagedMacOsDirectory === undefined || packagedExecutable === undefined
  ? null
  : path.join(packagedMacOsDirectory, packagedExecutable.name)
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-03-packaged-editor.png"
const screenshotPath = path.join(repositoryRoot, screenshotRef)
const receiptPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-03-packaged-journey.json")
const workbenchReceiptPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-04-packaged-workbench.json")
const marker = "// IDE-03 packaged Monaco edit"
const pathRef = "ide03.ts"

const workspaceRoot = process.argv.slice(2).find(argument => path.isAbsolute(argument))
if (workspaceRoot === undefined || !existsSync(workspaceRoot) || !statSync(workspaceRoot).isDirectory()) {
  throw new Error("usage: pnpm run ide:monaco-packaged-journey -- <absolute disposable workspace beneath the OS temp directory>")
}
const relativeToTemp = path.relative(path.resolve(tmpdir()), path.resolve(workspaceRoot))
if (relativeToTemp === "" || relativeToTemp === ".." || relativeToTemp.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToTemp)) {
  throw new Error("IDE-03 packaged journey accepts only a disposable workspace beneath the OS temporary directory")
}
if (packagedBinary === null || packagedAppPath === null || !existsSync(packagedBinary)) {
  throw new Error("packaged Desktop application missing; run package:mac first")
}
const finderOpenPath = path.join(workspaceRoot, "src", "ide03.ts")
if (!existsSync(finderOpenPath) || !statSync(finderOpenPath).isFile()) {
  throw new Error("IDE-03 packaged journey fixture must contain src/ide03.ts")
}

const enterFiles = async (page: Page): Promise<void> => {
  await page.keyboard.press(process.platform === "darwin" ? "Meta+E" : "Control+E")
  await page.waitForTimeout(300)
  if (await page.locator('[data-react-workspace="files"]').count() === 0) {
    const files = page.getByRole("button", { name: "Files", exact: true })
    if (await files.count() > 0) await files.first().click()
  }
  await page.locator('[data-react-workspace="files"]').waitFor({ state: "visible", timeout: 30_000 })
  await page.locator('[data-oa-pierre-tree="true"]').waitFor({ state: "visible", timeout: 30_000 })
}

const recoveryContainsMarker = async (page: Page): Promise<boolean> => page.evaluate((expected) => {
  for (const key of Object.keys(localStorage)) {
    if (!key.startsWith("openagents.desktop.workspace-editor.v2.")) continue
    try {
      const value = JSON.parse(localStorage.getItem(key) ?? "null")
      if (value?.version === 4 && value?.tabs?.some((tab: { draft?: unknown }) =>
        typeof tab.draft === "string" && tab.draft.includes(expected as string))) return true
    } catch { /* malformed unrelated key */ }
  }
  return false
}, marker)

const main = async (): Promise<void> => {
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide03-packaged-"))
  let launchedApplicationPid: number | null = null
  // LaunchServices is the Finder-equivalent path that causes Electron's
  // pre-ready `open-file` event. Executing Contents/MacOS directly with a file
  // argument would only test argv parsing, which is not the production route.
  const appProcess = spawn("open", [
    "-n",
    "-W",
    "-a",
    packagedAppPath!,
    finderOpenPath,
    "--args",
    "--remote-debugging-port=0",
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
    const deadline = Date.now() + 20_000
    while (!existsSync(devToolsPortPath) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 50))
    if (!existsSync(devToolsPortPath)) throw new Error("packaged Chromium DevTools port did not appear")
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
    if (page === undefined) throw new Error("packaged renderer page did not appear")
    await enterFiles(page)

    const tree = page.locator('[data-oa-pierre-tree="true"]')
    const primary = page.locator('.oa-react-monaco-pane[data-monaco-view="primary"]')
    if (await primary.locator('[data-monaco-phase="ready"]').count() === 0) {
      const target = tree.locator(`[data-item-path="${pathRef}"]`)
      await target.waitFor({ state: "visible", timeout: 30_000 })
      await target.click()
    }
    await primary.locator('[data-monaco-phase="ready"]').waitFor({ state: "visible", timeout: 30_000 })
    const editorReady = await primary.getAttribute("aria-label") === `Editor for ${pathRef}`
    const legacyTextareaAbsent = await page.locator('.oa-react-editor-textarea, .oa-react-file-editor > textarea').count() === 0
    const rootWithheld = !(await page.locator('body').innerText()).includes(workspaceRoot)

    await page.keyboard.press(process.platform === "darwin" ? "Meta+P" : "Control+P")
    const quickOpen = page.getByRole("dialog", { name: "Quick Open" })
    await quickOpen.waitFor({ state: "visible", timeout: 10_000 })
    await quickOpen.getByRole("textbox", { name: "Search files by path" }).fill("README")
    const readmeResult = quickOpen.getByRole("button", { name: /README\.md/ }).first()
    await readmeResult.waitFor({ state: "visible", timeout: 10_000 })
    await readmeResult.click()
    await page.locator('.oa-react-file-tabs button[data-tab-mode="preview"]').waitFor({ state: "visible", timeout: 10_000 })
    const previewOpened = await page.locator('.oa-react-file-tabs button[data-tab-mode="preview"]').count() === 1
    await page.locator('.oa-react-file-tabs button[data-tab-mode="preview"]').dblclick()
    const previewPinned = await page.locator('.oa-react-file-tabs button[data-tab-mode="preview"]').count() === 0
    await page.getByRole("button", { name: /ide03\.ts/ }).first().click()
    await primary.locator('[data-monaco-phase="ready"]').waitFor({ state: "visible", timeout: 10_000 })

    // Current Monaco uses Chromium's native EditContext (`role=textbox`) and
    // retains a read-only IME textarea only as an implementation detail.
    const input = primary.locator('.monaco-editor [role="textbox"]').first()
    await input.focus()
    await page.keyboard.press(process.platform === "darwin" ? "Meta+End" : "Control+End")
    await page.keyboard.press("Enter")
    await page.keyboard.type(marker)
    await page.waitForFunction((expected) => {
      for (const key of Object.keys(localStorage)) {
        if (!key.startsWith("openagents.desktop.workspace-editor.v2.")) continue
        try {
          const value = JSON.parse(localStorage.getItem(key) ?? "null")
          if (value?.version === 4 && value?.tabs?.some((tab: { draft?: unknown }) =>
            typeof tab.draft === "string" && tab.draft.includes(expected as string))) return true
        } catch {}
      }
      return false
    }, marker, { timeout: 10_000 })
    const edited = await recoveryContainsMarker(page)

    await page.getByRole("button", { name: "Vim off", exact: true }).click()
    await page.getByRole("button", { name: "Vim on", exact: true }).waitFor({ state: "visible" })
    const vimToggled = await primary.getAttribute("data-vim-enabled") === "true"
    await page.getByRole("button", { name: "Split", exact: true }).click()
    await page.locator('.oa-react-monaco-pane [data-monaco-phase="ready"]').nth(1).waitFor({ state: "visible", timeout: 10_000 })
    const splitViews = await page.locator('.oa-react-monaco-pane').count()
    await page.screenshot({ path: screenshotPath, fullPage: true })

    const offlinePrivateScheme = await page.evaluate(async () => {
      const editorRuntimeUrl = "openagents-app://renderer/ide-editor/editor.js"
      const editorCssUrl = "openagents-app://renderer/ide-editor/editor.css"
      const module = await import(editorRuntimeUrl)
      const resources = module.runtime.resources()
      return location.protocol === "openagents-app:"
        && [...document.styleSheets].some(sheet => sheet.href === editorCssUrl)
        && resources.state === "ready"
        && resources.workerCount >= 1
    })

    await page.reload({ waitUntil: "domcontentloaded" })
    const recoveryReloaded = await recoveryContainsMarker(page)

    // The previous page's `pagehide` finalizer must tear down its complete
    // Monaco scope. Importing the fresh island after reload lets the proof
    // inspect and stop the replacement singleton without reopening a file.
    const resourcesAfterClose = await page.evaluate(async () => {
      const editorRuntimeUrl = "openagents-app://renderer/ide-editor/editor.js"
      const module = await import(editorRuntimeUrl)
      module.runtime.dispose()
      const resources = module.runtime.resources()
      return {
        models: resources.modelCount,
        views: resources.viewCount,
        workers: resources.workerCount,
        listeners: resources.listenerCount + resources.vimHandlerCount,
      }
    })

    const receipt = Schema.decodeUnknownSync(IdeMonacoPackagedJourneyReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-monaco-packaged-journey.v1",
      capturedAt: new Date().toISOString(),
      commitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
      platform: process.platform,
      architecture: process.arch,
      packaged: true,
      pathRef,
      editorReady,
      edited,
      vimToggled,
      splitViews,
      recoveryReloaded,
      offlinePrivateScheme,
      rootWithheld,
      legacyTextareaAbsent,
      resourcesAfterClose,
      screenshotRef,
    })
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
    const workbenchReceipt = {
      schemaVersion: "openagents.desktop.ide-workbench-packaged-journey.v1",
      capturedAt: new Date().toISOString(),
      commitSha: receipt.commitSha,
      platform: process.platform,
      architecture: process.arch,
      packaged: true,
      quickOpenReady: true,
      previewOpened,
      previewPinned,
      recoveryVersion: 4,
      splitViews,
      rootWithheld,
      screenshotRef,
    }
    if (!previewOpened || !previewPinned || !recoveryReloaded || splitViews !== 2 || !rootWithheld) {
      throw new Error(`IDE-04 packaged workbench journey failed: ${JSON.stringify(workbenchReceipt)}`)
    }
    writeFileSync(workbenchReceiptPath, `${JSON.stringify(workbenchReceipt, null, 2)}\n`, { mode: 0o600 })
    process.stdout.write(`[openagents-desktop] IDE-03 packaged Monaco journey: ${receiptPath}\n`)
    process.stdout.write(`[openagents-desktop] IDE-04 packaged workbench journey: ${workbenchReceiptPath}\n`)
  } finally {
    await browser?.close()
    if (launchedApplicationPid !== null) {
      try { process.kill(launchedApplicationPid, "SIGTERM") } catch { /* already stopped */ }
      const stopDeadline = Date.now() + 5_000
      while (Date.now() < stopDeadline) {
        try {
          process.kill(launchedApplicationPid, 0)
          await new Promise(resolve => setTimeout(resolve, 100))
        } catch {
          break
        }
      }
    }
    // `open -W` remains attached to the exact LaunchServices instance. Closing
    // CDP should terminate it; the signal is a bounded fallback for the waiter.
    appProcess.kill("SIGTERM")
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ])
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL")
    if (existsSync(userDataPath)) {
      try { rmSync(userDataPath, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }) } catch { /* OS cleanup is best-effort */ }
    }
  }
}

await main()
