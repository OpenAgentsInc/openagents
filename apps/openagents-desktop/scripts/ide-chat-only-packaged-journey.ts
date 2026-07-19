import { execFileSync, spawn } from "node:child_process"
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, type Browser } from "playwright"
import { Schema } from "effect"

import { IdeBasicIdeChatOnlyReceiptSchema } from "../src/ide/basic-ide-acceptance-contract.ts"
import {
  packagedArtifactTreeDigest,
  resolvePackagedApp,
} from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const outputPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-07-chat-only.json")
const packagedAppPath = resolvePackagedApp()
const candidateCommitSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim()
const artifactTreeSha256 = packagedArtifactTreeDigest(packagedAppPath).sha256
const acceptanceLaunchRoot = "/var/empty"
const repetitions = Math.min(20, Math.max(3, Number.parseInt(
  process.env.OPENAGENTS_DESKTOP_IDE07_REPETITIONS ?? "7",
  10,
)))

const percentile = (values: ReadonlyArray<number>, fraction: number): number => {
  const sorted = [...values].sort((left, right) => left - right)
  const rank = fraction * (sorted.length - 1)
  const low = Math.floor(rank)
  const high = Math.ceil(rank)
  return (sorted[low] ?? 0) + ((sorted[high] ?? sorted[low] ?? 0) - (sorted[low] ?? 0)) * (rank - low)
}

const round3 = (value: number): number => Math.round(value * 1_000) / 1_000

type Sample = Readonly<{
  shellReadyMs: number
  editorAssetsRequested: number
  rendererWorkers: number
  monacoHosts: number
  pierreTrees: number
  languagePlacements: number
  projectIndexSurfaces: number
  homeRootVisible: boolean
  repositoryRootVisible: boolean
  rootWithheld: boolean
  stopped: boolean
}>

const runSample = async (): Promise<Sample> => {
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide07-chat-only-"))
  const startedAt = performance.now()
  let launchedApplicationPid: number | null = null
  const appProcess = spawn("open", [
    "-n",
    "-W",
    "-a",
    packagedAppPath,
    "--args",
    "--remote-debugging-port=0",
  ], {
    cwd: acceptanceLaunchRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_IDE07_CHAT_ONLY_PROOF: "1",
      OPENAGENTS_DESKTOP_SMOKE_REACT: "1",
      OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: acceptanceLaunchRoot,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
    stdio: "ignore",
  })
  let browser: Browser | null = null
  let sample: Omit<Sample, "stopped"> | null = null
  try {
    const devToolsPortPath = path.join(userDataPath, "DevToolsActivePort")
    const deadline = Date.now() + 30_000
    while (!existsSync(devToolsPortPath) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 25))
    if (!existsSync(devToolsPortPath)) throw new Error("packaged chat-only DevTools port did not appear")
    const port = readFileSync(devToolsPortPath, "utf8").split("\n")[0]
    const pidOutput = execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], { encoding: "utf8" }).trim()
    launchedApplicationPid = Number.parseInt(pidOutput.split("\n")[0] ?? "", 10)
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const pageDeadline = Date.now() + 30_000
    let page = browser.contexts().flatMap(context => context.pages())
      .find(candidate => candidate.url().startsWith("openagents-app://renderer/"))
    while (page === undefined && Date.now() < pageDeadline) {
      await new Promise(resolve => setTimeout(resolve, 25))
      page = browser.contexts().flatMap(context => context.pages())
        .find(candidate => candidate.url().startsWith("openagents-app://renderer/"))
    }
    if (page === undefined) throw new Error("packaged chat-only renderer did not appear")
    await page.locator('[data-react-workspace="chat"]').waitFor({ state: "visible", timeout: 30_000 })
    const shellReadyMs = performance.now() - startedAt
    const projection = await page.evaluate(() => ({
      editorAssetsRequested: performance.getEntriesByType("resource")
        .filter(entry => entry.name.includes("/ide-editor/")).length,
      monacoHosts: document.querySelectorAll(".oa-react-monaco-pane, .monaco-editor").length,
      pierreTrees: document.querySelectorAll('[data-oa-pierre-tree="true"]').length,
      languagePlacements: document.querySelectorAll("[data-language-tier], [data-language-service]").length,
      projectIndexSurfaces: document.querySelectorAll("[data-path-index-generation], [data-react-workspace=files]").length,
      body: document.body.innerText,
      shellMountedAt: (globalThis as { __oaStartupMarks?: Record<string, number> })
        .__oaStartupMarks?.shellMounted ?? null,
      rendererTimeOrigin: performance.timeOrigin,
    }))
    sample = {
      shellReadyMs: typeof projection.shellMountedAt === "number"
        ? projection.shellMountedAt - projection.rendererTimeOrigin
        : shellReadyMs,
      ...projection,
      rendererWorkers: page.workers().length + page.context().serviceWorkers().length,
      homeRootVisible: projection.body.includes(process.env.HOME ?? "__no_home__"),
      repositoryRootVisible: projection.body.includes(repositoryRoot),
      rootWithheld: !projection.body.includes(process.env.HOME ?? "__no_home__")
        && !projection.body.includes(repositoryRoot),
    }
  } finally {
    await browser?.close()
    if (launchedApplicationPid !== null && Number.isSafeInteger(launchedApplicationPid)) {
      try { process.kill(launchedApplicationPid, "SIGTERM") } catch { /* already stopped */ }
      const stopDeadline = Date.now() + 5_000
      while (Date.now() < stopDeadline) {
        try {
          process.kill(launchedApplicationPid, 0)
          await new Promise(resolve => setTimeout(resolve, 50))
        } catch {
          break
        }
      }
    }
    appProcess.kill("SIGTERM")
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ])
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL")
    if (existsSync(userDataPath)) rmSync(userDataPath, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 })
  }
  if (sample === null) throw new Error("packaged chat-only sample did not complete")
  let stopped = true
  if (launchedApplicationPid !== null && Number.isSafeInteger(launchedApplicationPid)) {
    try {
      process.kill(launchedApplicationPid, 0)
      stopped = false
    } catch {
      // The exact isolated application process has exited.
    }
  }
  return { ...sample, stopped }
}

const samples: Sample[] = []
for (let index = 0; index < repetitions; index += 1) samples.push(await runSample())
if (samples.some(sample => !sample.rootWithheld)) {
  throw new Error(`chat-only root withholding failed: ${JSON.stringify(samples.map(sample => ({
    homeRootVisible: sample.homeRootVisible,
    repositoryRootVisible: sample.repositoryRootVisible,
  })))}`)
}
const shell = samples.map(sample => sample.shellReadyMs)
const receipt = Schema.decodeUnknownSync(IdeBasicIdeChatOnlyReceiptSchema)({
  schemaVersion: "openagents.desktop.ide-chat-only-packaged.v1",
  capturedAt: new Date().toISOString(),
  candidateCommitSha,
  artifactTreeSha256,
  repetitions,
  shellReadyMs: {
    p50: round3(percentile(shell, 0.5)),
    p95: round3(percentile(shell, 0.95)),
    p99: round3(percentile(shell, 0.99)),
  },
  editorAssetsRequested: Math.max(...samples.map(sample => sample.editorAssetsRequested)),
  rendererWorkers: Math.max(...samples.map(sample => sample.rendererWorkers)),
  monacoHosts: Math.max(...samples.map(sample => sample.monacoHosts)),
  pierreTrees: Math.max(...samples.map(sample => sample.pierreTrees)),
  languagePlacements: Math.max(...samples.map(sample => sample.languagePlacements)),
  projectIndexSurfaces: Math.max(...samples.map(sample => sample.projectIndexSurfaces)),
  rootWithheld: samples.every(sample => sample.rootWithheld),
  appProcessesAfter: samples.filter(sample => !sample.stopped).length,
})

await import("node:fs/promises").then(fs => fs.writeFile(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 }))
process.stdout.write(`[openagents-desktop] IDE-07 packaged chat-only journey: ${outputPath}\n`)
