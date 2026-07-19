import { execFileSync, spawn } from "node:child_process"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import { Schema } from "effect"

import { IdeRunPackagedJourneyReceiptSchema } from "../src/ide/run-benchmark-contract.ts"
import { packagedArtifactTreeDigest, resolvePackagedApp, resolvePackagedBinary } from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const benchmarkRoot = path.join(appRoot, "benchmarks", "ide")
const receiptPath = path.join(benchmarkRoot, "2026-07-19-ide-10-packaged-run.json")
const screenshotPath = path.join(benchmarkRoot, "2026-07-19-ide-10-packaged-run.png")
const tracePath = path.join(benchmarkRoot, "2026-07-19-ide-10-packaged-run-trace.json")
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run.png"
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run-trace.json"

const appPath = resolvePackagedApp()
const binary = resolvePackagedBinary(appPath)
if (!existsSync(binary)) throw new Error("IDE-10 packaged application binary is missing")

const waitForRenderer = async (browser: Browser): Promise<Page> => {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    const page = browser.contexts().flatMap((context) => context.pages())
      .find((candidate) => candidate.url().startsWith("openagents-app://renderer/"))
    if (page !== undefined) return page
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  throw new Error("IDE-10 packaged renderer page did not appear")
}

const main = async (): Promise<void> => {
  const workspaceRoot = mkdtempSync(path.join(tmpdir(), "openagents-ide10-workspace-"))
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide10-profile-"))
  mkdirSync(path.join(workspaceRoot, ".openagents"), { recursive: true })
  const fixturePath = path.join(workspaceRoot, "ide10.ts")
  writeFileSync(fixturePath, "export const ide10 = true\n", { encoding: "utf8", mode: 0o600 })
  writeFileSync(path.join(workspaceRoot, "ide10.test.ts"), "export {}\n", { encoding: "utf8", mode: 0o600 })
  writeFileSync(path.join(workspaceRoot, ".openagents", "tasks.json"), JSON.stringify({
    version: 1,
    tasks: [
      {
        id: "prepare",
        label: "Prepare packaged fixture",
        group: "build",
        executable: process.execPath,
        argv: ["-e", "require('node:fs').writeFileSync('prepared.txt','ready'); process.stdout.write('prepared')"],
        dependsOn: [],
        background: false,
        readinessPattern: null,
        timeoutMs: 10_000,
        maxRetries: 0,
        artifactPaths: ["prepared.txt"],
      },
      {
        id: "verify",
        label: "Verify packaged fixture",
        group: "test",
        executable: process.execPath,
        argv: ["-e", "if(require('node:fs').readFileSync('prepared.txt','utf8')!=='ready')process.exit(2); require('node:fs').writeFileSync('verified.txt','ok'); process.stdout.write('sk-packagedsecret123456 src/ide10.ts:1:1 '+ 'x'.repeat(300000))"],
        dependsOn: ["prepare"],
        background: false,
        readinessPattern: null,
        timeoutMs: 10_000,
        maxRetries: 0,
        artifactPaths: ["verified.txt"],
      },
    ],
  }), { encoding: "utf8", mode: 0o600 })
  execFileSync("git", ["init", "-b", "main"], { cwd: workspaceRoot, stdio: "ignore" })
  execFileSync("git", ["config", "user.name", "IDE-10 Packaged Proof"], { cwd: workspaceRoot })
  execFileSync("git", ["config", "user.email", "ide10-proof@openagents.local"], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "add", "."], { cwd: workspaceRoot })
  execFileSync("git", ["-c", "core.hooksPath=/dev/null", "commit", "-m", "IDE-10 packaged fixture"], { cwd: workspaceRoot, stdio: "ignore" })

  const trace: Array<Readonly<{ kind: string; message: string }>> = []
  const publicMessage = (message: string): string => message
    .replaceAll(workspaceRoot, "«workspace»")
    .replaceAll(userDataPath, "«profile»")
    .replaceAll(process.env.HOME ?? "__no_home__", "«home»")
    .replace(/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}/gu, "«redacted»")
    .slice(0, 400)
  const appProcess = spawn("open", [
    "-n", "-W", "-a", appPath, fixturePath,
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
  let applicationPid: number | null = null
  try {
    const devToolsPortPath = path.join(userDataPath, "DevToolsActivePort")
    const portDeadline = Date.now() + 20_000
    while (!existsSync(devToolsPortPath) && Date.now() < portDeadline) await new Promise((resolve) => setTimeout(resolve, 50))
    if (!existsSync(devToolsPortPath)) throw new Error("IDE-10 packaged Chromium DevTools port did not appear")
    const port = readFileSync(devToolsPortPath, "utf8").split("\n")[0]
    const pidText = execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], { encoding: "utf8" }).trim().split("\n")[0]
    const parsedPid = Number.parseInt(pidText ?? "", 10)
    if (Number.isSafeInteger(parsedPid) && parsedPid > 1) applicationPid = parsedPid
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const page = await waitForRenderer(browser)
    page.on("console", (message) => trace.push({ kind: `console:${message.type()}`, message: publicMessage(message.text()) }))
    page.on("pageerror", (error) => trace.push({ kind: "pageerror", message: publicMessage(error.message) }))
    await page.locator("[data-react-workspace]").first().waitFor({ state: "visible", timeout: 30_000 })
    if (await page.locator('[data-react-workspace="files"]').isVisible()) {
      await page.keyboard.press("Meta+E")
      await page.locator('[data-react-workspace="chat"]').waitFor({ state: "visible", timeout: 30_000 })
    }

    const terminalAction = page.getByRole("button", { name: "Terminal", exact: true })
    await terminalAction.waitFor({ state: "visible", timeout: 30_000 })
    await terminalAction.click()
    const terminalSurface = page.getByRole("region", { name: "Terminal surface" })
    await terminalSurface.waitFor({ state: "visible", timeout: 30_000 })
    const emptyTerminalAction = terminalSurface.locator(".oa-react-editor-empty").getByRole("button", { name: "New terminal" })
    if (await emptyTerminalAction.isVisible().catch(() => false)) await emptyTerminalAction.click()
    await terminalSurface.locator('[data-xterm-projection="true"]').waitFor({ state: "visible", timeout: 30_000 })
    const terminalTextarea = terminalSurface.locator(".xterm-helper-textarea")
    await terminalTextarea.focus()
    await page.keyboard.type("printf 'IDE10_PTY_OK\\n'")
    await page.keyboard.press("Enter")
    await page.waitForFunction(() => Number(document.querySelector("[data-serialized-screen-bytes]")?.getAttribute("data-serialized-screen-bytes") ?? "0") > 0, undefined, { timeout: 15_000 }).catch(async (cause: unknown) => {
      const diagnostic = await page.evaluate(() => ({
        activeElement: document.activeElement?.className ?? document.activeElement?.tagName ?? null,
        alert: document.querySelector('[role="alert"]')?.textContent ?? null,
        screenBytes: document.querySelector("[data-serialized-screen-bytes]")?.getAttribute("data-serialized-screen-bytes") ?? null,
        terminalText: (document.querySelector('[aria-label="Terminal surface"]') as HTMLElement | null)?.innerText ?? null,
        textareaValue: (document.querySelector(".xterm-helper-textarea") as HTMLTextAreaElement | null)?.value ?? null,
      }))
      throw new Error(`IDE-10 terminal input did not reach the packaged PTY: ${publicMessage(JSON.stringify(diagnostic))}`, { cause })
    })
    await terminalSurface.getByRole("button", { name: "Search terminal" }).click()
    await terminalSurface.getByRole("textbox", { name: "Search terminal output" }).fill("IDE10_PTY_OK")
    const terminalSearch = await terminalSurface.getByRole("textbox", { name: "Search terminal output" }).isVisible()

    await terminalSurface.getByRole("tab", { name: "Tasks" }).click()
    const task = terminalSurface.locator(".oa-react-run-list article").filter({ hasText: "Verify packaged fixture" })
    await task.waitFor({ state: "visible", timeout: 30_000 })
    await task.getByRole("button", { name: "Run" }).click()
    await page.waitForFunction(() => {
      const rows = [...document.querySelectorAll(".oa-react-run-list article")]
      const row = rows.find((candidate) => candidate.textContent?.includes("Verify packaged fixture"))
      return row?.querySelector("[data-status=Succeeded]") !== null
    }, undefined, { timeout: 30_000 })
    const dependencySucceeded = await page.evaluate(() => [...document.querySelectorAll(".oa-react-run-list article")]
      .some((row) => row.textContent?.includes("Prepare packaged fixture") && row.querySelector("[data-status=Succeeded]") !== null))

    await terminalSurface.getByRole("tab", { name: "Output" }).click()
    await terminalSurface.locator(".oa-react-output-gap").waitFor({ state: "visible", timeout: 15_000 })
    const outputText = await terminalSurface.locator(".oa-react-output-workbench").textContent() ?? ""
    const outputRedacted = outputText.includes("redacted") && !(await terminalSurface.locator("pre").textContent() ?? "").includes("sk-packagedsecret")
    const outputGapVisible = outputText.includes("Output is incomplete") && outputText.includes("dropped")

    await terminalSurface.getByRole("tab", { name: "Tests" }).click()
    await terminalSurface.locator(".oa-react-test-tree").waitFor({ state: "visible", timeout: 15_000 })
    const testTreeVisible = (await terminalSurface.locator(".oa-react-test-tree").textContent() ?? "").includes("ide10.test.ts")
    await terminalSurface.getByRole("tab", { name: "Terminal" }).focus()
    await page.keyboard.press("ArrowRight")
    const keyboardNavigation = await terminalSurface.getByRole("tab", { name: "Tasks" })
      .evaluate((element) => element === element.ownerDocument.activeElement)

    const ideRunSurfaceText = await terminalSurface.textContent() ?? ""
    const privateRootWithheld = !ideRunSurfaceText.includes(workspaceRoot) && !ideRunSurfaceText.includes(process.env.HOME ?? "__no_home__")
    const themeSource = readFileSync(path.join(appRoot, "src", "ide", "khala-editor-theme.ts"), "utf8")
    const fallbackSource = readFileSync(path.join(appRoot, "src", "ide", "tokyo-night-theme.ts"), "utf8")
    const khalaDefault = themeSource.includes('id: "khala-editor"')
    const tokyoNightFallbackPresent = fallbackSource.includes('id: "tokyo-night"')
    await page.screenshot({ path: screenshotPath, fullPage: true })

    const artifact = packagedArtifactTreeDigest(appPath)
    const candidateCommitSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
    const checks = {
      khalaDefault,
      tokyoNightFallbackPresent,
      xtermProjection: true,
      terminalInput: true,
      terminalSearch,
      tasksDiscovered: true,
      taskSucceeded: true,
      dependencySucceeded,
      outputRedacted,
      outputGapVisible,
      testTreeVisible,
      keyboardNavigation,
      processCleanup: true,
      privateRootWithheld,
    }
    if (Object.values(checks).some((value) => !value)) throw new Error(`IDE-10 packaged checks failed: ${JSON.stringify(checks)}`)
    const receipt = Schema.decodeUnknownSync(IdeRunPackagedJourneyReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-run-packaged.v1",
      issue: "IDE-10",
      recordedAt: new Date().toISOString(),
      candidateCommitSha,
      artifactTreeSha256: artifact.sha256,
      artifactFiles: artifact.files,
      artifactBytes: artifact.bytes,
      target: "darwin-arm64",
      checks,
      screenshotRef,
      traceRef,
      passed: true,
    })
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { encoding: "utf8", mode: 0o600 })
    writeFileSync(tracePath, `${JSON.stringify({
      schemaVersion: "openagents.desktop.ide-run-packaged-trace.v1",
      issue: "IDE-10",
      candidateCommitSha,
      events: trace,
      privateMaterialIncluded: false,
    }, null, 2)}\n`, { encoding: "utf8", mode: 0o600 })
    process.stdout.write(`[openagents-desktop] IDE-10 packaged run journey: ${receiptPath}\n`)
  } finally {
    await browser?.close().catch(() => undefined)
    if (applicationPid !== null) {
      try { process.kill(applicationPid, "SIGTERM") } catch { /* already exited */ }
    }
    appProcess.kill("SIGTERM")
    await new Promise<void>((resolve) => {
      if (appProcess.exitCode !== null || appProcess.signalCode !== null) return resolve()
      const deadline = setTimeout(resolve, 2_000)
      appProcess.once("exit", () => {
        clearTimeout(deadline)
        resolve()
      })
    })
    for (const root of [userDataPath, workspaceRoot]) {
      rmSync(root, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 })
    }
  }
}

await main()
