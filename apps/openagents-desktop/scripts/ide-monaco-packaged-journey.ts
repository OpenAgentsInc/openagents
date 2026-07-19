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
const marker = "// IDE-03 packaged Monaco edit"

const workspaceRoot = process.argv.slice(2).find(argument => path.isAbsolute(argument))
if (workspaceRoot === undefined || !existsSync(workspaceRoot) || !statSync(workspaceRoot).isDirectory()) {
  throw new Error("usage: pnpm run ide:monaco-packaged-journey -- <absolute disposable workspace beneath the OS temp directory>")
}
const relativeToTemp = path.relative(path.resolve(tmpdir()), path.resolve(workspaceRoot))
if (relativeToTemp === "" || relativeToTemp === ".." || relativeToTemp.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToTemp)) {
  throw new Error("IDE-03 packaged journey accepts only a disposable workspace beneath the OS temporary directory")
}
if (packagedBinary === null || !existsSync(packagedBinary)) throw new Error("packaged Desktop binary missing; run package:mac first")

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
      if (value?.version === 3 && value?.tabs?.some((tab: { draft?: unknown }) =>
        typeof tab.draft === "string" && tab.draft.includes(expected as string))) return true
    } catch { /* malformed unrelated key */ }
  }
  return false
}, marker)

const main = async (): Promise<void> => {
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide03-packaged-"))
  const appProcess = spawn(packagedBinary!, ["--remote-debugging-port=0"], {
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
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const page = browser.contexts().flatMap(context => context.pages())[0]
    if (page === undefined) throw new Error("packaged renderer page did not appear")
    await page.locator('aside[aria-label="Sessions"]').waitFor({ state: "attached", timeout: 60_000 })
    await enterFiles(page)

    const tree = page.locator('[data-oa-pierre-tree="true"]')
    const target = tree.locator('[data-item-path="src/ide03.ts"]')
    await target.waitFor({ state: "visible", timeout: 30_000 })
    await target.click()
    const primary = page.locator('.oa-react-monaco-pane[data-monaco-view="primary"]')
    await primary.locator('[data-monaco-phase="ready"]').waitFor({ state: "visible", timeout: 30_000 })
    const pathRef = "src/ide03.ts"
    const editorReady = await primary.getAttribute("aria-label") === `Editor for ${pathRef}`
    const legacyTextareaAbsent = await page.locator('.oa-react-editor-textarea, .oa-react-file-editor > textarea').count() === 0
    const rootWithheld = !(await page.locator('body').innerText()).includes(workspaceRoot)

    const input = primary.locator('.monaco-editor textarea').first()
    await input.focus()
    await page.keyboard.press(process.platform === "darwin" ? "Meta+End" : "Control+End")
    await page.keyboard.insertText(`\n${marker}`)
    await page.waitForFunction((expected) => {
      for (const key of Object.keys(localStorage)) {
        if (!key.startsWith("openagents.desktop.workspace-editor.v2.")) continue
        try {
          const value = JSON.parse(localStorage.getItem(key) ?? "null")
          if (value?.version === 3 && value?.tabs?.some((tab: { draft?: unknown }) =>
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

    const offlinePrivateScheme = await page.evaluate(() => {
      const urls = performance.getEntriesByType("resource").map(entry => entry.name)
        .filter(url => url.includes("/ide-editor/"))
      return urls.length >= 2 && urls.every(url => url.startsWith("openagents-app://renderer/ide-editor/"))
    })

    await page.reload({ waitUntil: "domcontentloaded" })
    await page.locator('aside[aria-label="Sessions"]').waitFor({ state: "attached", timeout: 60_000 })
    await enterFiles(page)
    await page.locator('.oa-react-monaco-pane[data-monaco-view="primary"] [data-monaco-phase="ready"]')
      .waitFor({ state: "visible", timeout: 30_000 })
    const recoveryReloaded = await recoveryContainsMarker(page)

    const close = page.getByRole("button", { name: `Close ${pathRef}`, exact: true })
    await close.click()
    if (await page.getByRole("button", { name: `Close ${pathRef}`, exact: true }).count() > 0) {
      await page.getByRole("button", { name: `Close ${pathRef}`, exact: true }).click()
    }
    await page.waitForTimeout(100)
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
    process.stdout.write(`[openagents-desktop] IDE-03 packaged Monaco journey: ${receiptPath}\n`)
  } finally {
    await browser?.close()
    appProcess.kill("SIGTERM")
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ])
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL")
    if (existsSync(userDataPath)) rmSync(userDataPath, { recursive: true, force: true })
  }
}

await main()
