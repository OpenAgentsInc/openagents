/**
 * FA-H12 (#8885): the Full Auto counterpart to local-turn-restart-smoke.ts.
 *
 * Launches the REAL Electron app TWICE against one temporary user-data
 * directory and proves, across two actual OS processes, that an enabled Full
 * Auto thread survives an app quit + relaunch: the seed process writes the
 * durable registry record (enabled, workspace-bound, COMPLETED fixture turn in
 * the local-turn journal) and quits; the resume process relaunches in smoke
 * fixture mode, runs real turn recovery followed by startup Full Auto
 * reconciliation, actually DISPATCHES a fixture continuation turn, and prints
 * a public-safe JSON receipt line. A second seed/resume pair (fresh userData)
 * proves the workspace-mismatch path fails CLOSED without dispatching.
 *
 * Requires a prior build (`node --import tsx scripts/build.ts`), exactly like
 * local-turn-restart-smoke.ts and the other Electron-launching smokes.
 */
import { spawn } from "node:child_process"
import { existsSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

const appRoot = path.resolve(import.meta.dirname, "..")
const packagedBinary = path.join(appRoot, "out", "OpenAgents-darwin-arm64", "OpenAgents.app", "Contents", "MacOS", "OpenAgents")
const electronBinary = path.join(appRoot, "node_modules", ".bin", "electron")
const command = process.platform === "darwin" && existsSync(packagedBinary)
  ? [packagedBinary]
  : [electronBinary, "."]
const PHASE_TIMEOUT_MS = 180_000

type ProbePhase = "seed" | "resume" | "seed-mismatch" | "resume-mismatch"

const runPhase = (userData: string, phase: ProbePhase): Promise<string> =>
  new Promise<string>((resolve, reject) => {
    const [executable, ...args] = command
    const child = spawn(executable!, args, {
      cwd: appRoot,
      env: {
        ...process.env,
        OPENAGENTS_DESKTOP_SMOKE: "0",
        OPENAGENTS_DESKTOP_USER_DATA: userData,
        OPENAGENTS_DESKTOP_FULL_AUTO_RESTART_PROBE: phase,
      },
      stdio: ["ignore", "pipe", "inherit"],
    })
    let stdout = ""
    child.stdout.setEncoding("utf8")
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk
      process.stdout.write(chunk)
    })
    const timer = setTimeout(() => {
      child.kill("SIGKILL")
      reject(new Error(`Electron full-auto restart smoke ${phase} phase timed out after ${PHASE_TIMEOUT_MS / 1000}s`))
    }, PHASE_TIMEOUT_MS)
    child.once("error", error => { clearTimeout(timer); reject(error) })
    child.once("exit", code => {
      clearTimeout(timer)
      code === 0
        ? resolve(stdout)
        : reject(new Error(`Electron full-auto restart smoke ${phase} phase exited ${code}`))
    })
  })

/** The resume process prints one `phase-b {...}` receipt line; parse it so the
 * runner re-asserts the public-safe fields instead of trusting exit code alone. */
const parseReceipt = (phase: ProbePhase, stdout: string): Record<string, unknown> => {
  const line = stdout.split("\n").find(value => value.includes("[openagents-desktop full-auto-restart] phase-b"))
  if (line === undefined) throw new Error(`no phase-b receipt line in ${phase} output`)
  return JSON.parse(line.slice(line.indexOf("{"))) as Record<string, unknown>
}

const main = async (): Promise<void> => {
  // Happy path: seed an enabled workspace-bound record, relaunch, observe a
  // real dispatched fixture continuation and the advanced continuation count.
  const happyUserData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-full-auto-restart-"))
  // Fail-closed path: same shape, but the granted workspace deliberately does
  // not match what the resume process resolves -- no continuation may dispatch.
  const mismatchUserData = mkdtempSync(path.join(tmpdir(), "openagents-desktop-full-auto-restart-mismatch-"))
  try {
    await runPhase(happyUserData, "seed")
    const happy = parseReceipt("resume", await runPhase(happyUserData, "resume"))
    if (happy.ok !== true || happy.seeded !== true || happy.resumed !== true ||
      happy.dispatchedTurnRefPresent !== true || typeof happy.continuationCount !== "number") {
      throw new Error(`happy-path receipt failed assertions: ${JSON.stringify(happy)}`)
    }
    await runPhase(mismatchUserData, "seed-mismatch")
    const mismatch = parseReceipt("resume-mismatch", await runPhase(mismatchUserData, "resume-mismatch"))
    if (mismatch.ok !== true || mismatch.seeded !== true || mismatch.resumed !== false ||
      mismatch.dispatchedTurnRefPresent !== false || mismatch.blockedReason !== "workspace_mismatch") {
      throw new Error(`workspace-mismatch receipt failed assertions: ${JSON.stringify(mismatch)}`)
    }
    console.log(`[openagents-desktop full-auto-restart] two-process smoke OK ${JSON.stringify({
      seeded: happy.seeded,
      resumed: happy.resumed,
      dispatchedTurnRefPresent: happy.dispatchedTurnRefPresent,
      continuationCount: happy.continuationCount,
      mismatchFailedClosed: mismatch.ok,
    })}`)
  } finally {
    rmSync(happyUserData, { recursive: true, force: true })
    rmSync(mismatchUserData, { recursive: true, force: true })
  }
}

await main().catch(error => {
  console.error("[openagents-desktop full-auto-restart] two-process smoke FAILED", error instanceof Error ? error.message : error)
  process.exit(1)
})
