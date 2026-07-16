/**
 * FA-H13 (#8886): live proof for the Full Auto local control surface.
 *
 * Launches the REAL Electron app once (windowless control probe mode, fixture
 * lanes, isolated OS-temp userData) with the control server enabled
 * (OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1). The probe seeds one thread with an
 * enabled, workspace-bound Full Auto record and keeps the process alive; this
 * runner then exercises the ACTUAL CLI (scripts/full-auto-cli.ts) as a second
 * process against the running app -- discovery via the mode-0600
 * full-auto/control.json file, bearer auth, `list`, then `status` -- proving
 * the end-to-end wire works against real Electron main, not just the unit
 * harness. The probe is ended by writing full-auto/control-probe-stop.
 *
 * Requires a prior build (`node --import tsx scripts/build.ts`), exactly like
 * full-auto-restart-smoke.ts.
 */
import { spawn } from "node:child_process"
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

const appRoot = path.resolve(import.meta.dirname, "..")
const packagedBinary = path.join(appRoot, "out", "OpenAgents-darwin-arm64", "OpenAgents.app", "Contents", "MacOS", "OpenAgents")
const electronBinary = path.join(appRoot, "node_modules", ".bin", "electron")
const command = process.platform === "darwin" && existsSync(packagedBinary)
  ? [packagedBinary]
  : [electronBinary, "."]
const READY_TIMEOUT_MS = 120_000

const runCli = (userData: string, args: ReadonlyArray<string>): Promise<string> =>
  new Promise<string>((resolve, reject) => {
    const child = spawn(
      process.execPath,
      ["--import", "tsx", path.join(appRoot, "scripts", "full-auto-cli.ts"), ...args, "--user-data", userData],
      { cwd: appRoot, stdio: ["ignore", "pipe", "inherit"] },
    )
    let stdout = ""
    child.stdout.setEncoding("utf8")
    child.stdout.on("data", (chunk: string) => { stdout += chunk })
    child.once("error", reject)
    child.once("exit", code => {
      code === 0 ? resolve(stdout) : reject(new Error(`full-auto-cli ${args.join(" ")} exited ${code}\n${stdout}`))
    })
  })

const main = async (): Promise<void> => {
  const userData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-full-auto-control-"))
  const [executable, ...args] = command
  const app = spawn(executable!, args, {
    cwd: appRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_SMOKE: "0",
      OPENAGENTS_DESKTOP_USER_DATA: userData,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: "1",
      OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL_PROBE: "1",
    },
    stdio: ["ignore", "pipe", "inherit"],
  })
  let appStdout = ""
  app.stdout.setEncoding("utf8")
  app.stdout.on("data", (chunk: string) => {
    appStdout += chunk
    process.stdout.write(chunk)
  })
  const appExit = new Promise<number | null>(resolve => app.once("exit", code => resolve(code)))
  const stopFile = path.join(userData, "full-auto", "control-probe-stop")
  try {
    // Wait for the probe ready line (carrying the seeded threadRef) AND the
    // control connection file the CLI discovers the server from.
    const deadline = Date.now() + READY_TIMEOUT_MS
    const readyLine = (): string | undefined =>
      appStdout.split("\n").find(line => line.includes("[openagents-desktop full-auto-control] probe ready"))
    const controlFile = path.join(userData, "full-auto", "control.json")
    while ((readyLine() === undefined || !existsSync(controlFile)) && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 200))
    }
    const ready = readyLine()
    if (ready === undefined || !existsSync(controlFile)) {
      throw new Error("the control probe never became ready (no ready line / control.json)")
    }
    const { threadRef } = JSON.parse(ready.slice(ready.indexOf("{"))) as { threadRef: string }

    // Second process: the real CLI, end to end (discovery file -> bearer -> HTTP).
    const listOutput = await runCli(userData, ["list"])
    console.log(`[openagents-desktop full-auto-control] CLI list output:\n${listOutput}`)
    const list = JSON.parse(listOutput) as { records: Array<{ threadRef: string; enabled: boolean }> }
    const seeded = list.records.find(record => record.threadRef === threadRef)
    if (seeded === undefined || seeded.enabled !== true) {
      throw new Error(`CLI list did not show the seeded enabled record for ${threadRef}`)
    }
    const statusOutput = await runCli(userData, ["status", threadRef])
    console.log(`[openagents-desktop full-auto-control] CLI status output:\n${statusOutput}`)
    const status = JSON.parse(statusOutput) as { record: { threadRef: string; enabled: boolean; workspaceRef: string | null; live: { state: string } } }
    if (status.record.threadRef !== threadRef || status.record.enabled !== true ||
      status.record.workspaceRef === null || status.record.live.state !== "idle") {
      throw new Error(`CLI status assertions failed: ${statusOutput}`)
    }
    console.log(`[openagents-desktop full-auto-control] live smoke OK ${JSON.stringify({
      threadRef,
      listedRecords: list.records.length,
      enabled: status.record.enabled,
      workspaceBound: status.record.workspaceRef !== null,
      liveState: status.record.live.state,
    })}`)
  } finally {
    try { writeFileSync(stopFile, "stop\n") } catch { /* the probe also times out on its own */ }
    const exitCode = await Promise.race([
      appExit,
      new Promise<number | null>(resolve => setTimeout(() => resolve(null), 15_000)),
    ])
    if (exitCode === null) app.kill("SIGKILL")
    rmSync(userData, { recursive: true, force: true })
  }
}

await main().catch(error => {
  console.error(
    "[openagents-desktop full-auto-control] live smoke FAILED",
    error instanceof Error ? error.message : error,
  )
  process.exit(1)
})
